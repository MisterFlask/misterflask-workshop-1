// The city screen (design §5.1 "Cities", §7 P6): the plots the city works (click to work a plot
// or stop working it), the specialists with their +/- buttons, food, production, commerce,
// happiness and health with their parts, culture, great people, buildings, religions, the
// garrison; hurry, draft, emphasis, automated citizens and production; the production queue
// and the chooser (units, buildings, projects (FfH rituals) and processes with cost and turns):
// click replaces the current production, Shift-click appends, Ctrl-click inserts at the front
// (C++ `CvNetPushOrder`'s flags), ✕ removes an order while the queue holds several.
//
// Every button sends what the Civ4 city screen sends (`do_task`, `push_order`, `pop_order`), and
// the server says which are possible (the `city` query: `plus`/`minus` per specialist,
// `can_work` per plot, `enabled` per hurry and buildable item).

import * as api from "./api.js";
import { S, dname, orderName, info, turns, me, emit, tidx, ORDER_KINDS } from "./state.js";
import { h, signed, setText } from "./dom.js";
import { openModal } from "./modals.js";
import { run } from "./commands.js";
import { render as markup, plain } from "./markup.js";
import { YIELD_COLORS } from "./tiles.js";
import { look, uiIcon, typeIcon, barEl, pix } from "./look.js";

const GROUPS = [
  [0, "Units"],
  [1, "Buildings"],
  [2, "Projects and rituals"],
  [3, "Processes"],
];

/** The human's cities in the game's order (C++ city loop: creation order, here by id). */
export function ownCities() {
  return [...S.cities.values()].filter((c) => c.owner === me()?.player).sort((a, b) => a.id - b.id);
}

/** Whether the human owns city `id`. */
export function ownCity(id) {
  return S.cities.get(id)?.owner === me()?.player;
}

/** The open city screen's previous / next (`go`), for Home / End (C++ CONTROL_NEXTCITY /
 *  PREVCITY with the city screen up: `cycleCities`, the screen moves to that city); null while
 *  no city screen is open, or the screen is a production popup (it has no previous / next). */
let screenGo = null;

/** Home / End in the city screen: moves it to the next (`step` 1) or previous (-1) city. Whether
 *  a city screen took the key (it is the top dialog). */
export function stepCityScreen(step, topTag) {
  if (!screenGo || topTag !== "city") return false;
  screenGo(step);
  return true;
}

/** Opens city `id`'s screen. With `popup` (an idle city's chooser at a control point, design
 *  §2.3) the dialog offers "Later" and closes once production is chosen. Resolves with "chosen",
 *  "later" or null (closed). */
export async function openCity(id, { popup = false } = {}) {
  let cityId = id;
  let result = null;
  const title = () => {
    const city = S.cities.get(cityId);
    return popup ? `What should ${city?.name ?? "the city"} build?` : (city?.name ?? `City ${cityId}`);
  };
  const m = openModal({ title: title(), body: [h("p.dim", "…")], tag: "city", wide: true, className: "city-screen", buttons: [] });
  // A refused task changes nothing: the screen is read again either way (a toggle shows the
  // city's state, not the click).
  const task = async (t, data1 = -1, data2 = -1, option = false) => {
    await run({ c: "do_task", city: cityId, task: tidx("TaskType", t), data1, data2, option, alt: false, shift: false, ctrl: false });
    load();
  };
  const choose = async (order, type, ev) => {
    const reply = await run({ c: "push_order", city: cityId, order, data: type, alt: false, shift: !!ev.shiftKey, ctrl: !!(ev.ctrlKey || ev.metaKey) });
    if (!reply) return;
    if (popup && !ev.shiftKey) {
      result = "chosen";
      m.close("chosen");
    } else load();
  };
  const remove = async (index) => {
    if (await run({ c: "pop_order", city: cityId, index })) load();
  };
  const go = (step) => {
    const list = ownCities();
    const i = list.findIndex((c) => c.id === cityId);
    if (list.length < 2 || i < 0) return;
    cityId = list[(i + step + list.length) % list.length].id;
    setText(m.el.querySelector(".modal-head h2"), title());
    emit("center", S.cities.get(cityId).x, S.cities.get(cityId).y);
    load();
  };
  let loading = 0;
  const load = async () => {
    const n = ++loading;
    let c;
    try {
      c = await api.query("city", { city: cityId });
    } catch (e) {
      m.setBody(h("p.warn", e.detail ?? e.message));
      return;
    }
    if (m.closed || n !== loading) return;
    m.el.dataset.city = String(cityId);
    m.setBody(screen(c, { task, choose, remove, go, popup }));
  };
  m.setButtons(
    popup
      ? [{ label: "Later", value: "later", title: "Leave the city idle for now (the end of turn reminds you)" }]
      : [
          { label: "‹ Previous city", onClick: () => go(-1), title: "The previous city" },
          { label: "Next city ›", onClick: () => go(1), title: "The next city" },
          { label: "Close", value: null },
        ],
  );
  load();
  if (!popup) screenGo = go;
  const v = await m.done;
  if (screenGo === go) screenGo = null;
  return result ?? v;
}

// ---------------------------------------------------------------------------------------------

function yieldNumbers(y) {
  if (!y) return h("span.dim", "?");
  // With the art: each yield's 8 px icon and its number (docs/ART.md 11.5).
  if (look.art) return h("span.yields", y.map((v, j) => (v > 0 ? h("span.yield-n", { title: dname("YieldType", j) }, uiIcon("yield", j, 8), pix(String(v))) : null)));
  return h("span.yields", y.map((v, j) => (v > 0 ? h("span", { style: { color: YIELD_COLORS[j] } }, String(v)) : null)));
}

/** The city's plots as a grid north up, centred on the city. */
function plotGrid(c, task) {
  const w = S.map?.w ?? 0;
  const wrap = !!S.map?.wrap_x;
  const rel = (p) => {
    let dx = p.x - c.x;
    if (wrap && w) {
      if (dx > w / 2) dx -= w;
      if (dx < -w / 2) dx += w;
    }
    return { dx, dy: p.y - c.y };
  };
  let r = 1;
  for (const p of c.plots) {
    const { dx, dy } = rel(p);
    r = Math.max(r, Math.abs(dx), Math.abs(dy));
  }
  const cells = new Map(c.plots.map((p) => [`${rel(p).dx},${rel(p).dy}`, p]));
  const mine = me()?.player;
  const rows = [];
  for (let dy = r; dy >= -r; dy--) {
    for (let dx = -r; dx <= r; dx++) {
      const p = cells.get(`${dx},${dy}`);
      if (!p) {
        rows.push(h("div.cplot.none"));
        continue;
      }
      const centre = p.i === 0;
      const ours = p.owner === mine;
      const cls = [centre ? "centre" : "", p.worked ? "worked" : "", p.worked_by !== null && p.worked_by !== undefined ? "other" : "", !p.revealed ? "hidden" : "", ours ? "" : "foreign"].filter(Boolean).join(" ");
      const tip = centre
        ? "The city's own plot (click: automate the citizens)"
        : !p.revealed
          ? "Unexplored"
          : p.worked
            ? "Worked: click to stop working it (the citizen becomes a specialist)"
            : p.worked_by !== null && p.worked_by !== undefined
              ? `Worked by ${S.cities.get(p.worked_by)?.name ?? "another city"}${ours ? ": click to take it" : ""}`
              : ours
                ? p.can_work
                  ? "Click: work this plot"
                  : "Not workable now"
                : "Not your plot";
      rows.push(
        h(
          "button.cplot",
          {
            class: cls,
            title: tip,
            "data-i": p.i,
            disabled: !(centre || ours) || !p.revealed,
            onclick: () => task("TASK_CHANGE_WORKING_PLOT", p.i),
          },
          centre ? h("span.ccity", "★") : null,
          p.revealed ? yieldNumbers(p.yields) : null,
        ),
      );
    }
  }
  return h("div.cgrid", { style: { gridTemplateColumns: `repeat(${2 * r + 1}, 1fr)` } }, rows);
}

function specialistsView(c, task) {
  const rows = c.specialists
    .filter((s) => s.count > 0 || s.plus || s.minus || s.free > 0)
    .map((s) =>
      h(
        "div.spec",
        { "data-specialist": s.specialist },
        typeIcon("specialist", s.specialist, 16),
        h("span.spec-name", dname("SpecialistType", s.specialist)),
        h("span.dim", `${s.count}${s.forced ? ` (${s.forced} set)` : ""}${s.free ? `, ${s.free} free` : ""}${s.max ? ` / ${s.max}` : ""}`),
        h("button.small.minus", { disabled: !s.minus, title: "One less", onclick: () => task("TASK_CHANGE_SPECIALIST", s.specialist, -1) }, "−"),
        h("button.small.plus", { disabled: !s.plus, title: "One more", onclick: () => task("TASK_CHANGE_SPECIALIST", s.specialist, 1) }, "+"),
      ),
    );
  return h(
    "div.section",
    h("h4", "Citizens"),
    h(
      "label.toggle",
      h("input", { type: "checkbox", checked: c.citizens_automated, id: "city-auto-citizens", onchange: (ev) => task("TASK_SET_AUTOMATED_CITIZENS", -1, -1, ev.target.checked) }),
      " Automate citizens (the governor picks plots and specialists)",
    ),
    c.extra_population > 0 ? h("div.dim", `${c.extra_population} citizen${c.extra_population > 1 ? "s" : ""} without work`) : null,
    rows.length ? h("div.specs", rows) : h("p.dim", "No specialists."),
  );
}

function kv(rows) {
  return h("table.kv", rows.filter(([, v]) => v !== null && v !== undefined && v !== false).map(([k, v]) => h("tr", h("th", k), h("td", v instanceof Node ? v : String(v)))));
}

function parts(obj, labels) {
  return labels
    .map(([k, label]) => [label, obj[k]])
    .filter(([, v]) => v)
    .map(([label, v]) => `${label} ${signed(v)}`)
    .join(", ");
}

function statsView(c) {
  const y = (i) => c.yields.find((v) => v.yield === i) ?? { rate: 0 };
  const commerce = c.commerces.filter((k) => k.rate).map((k) => `${dname("CommerceType", k.commerce)} ${k.rate}`).join(", ");
  const f = c.food;
  const hp = c.happiness;
  const he = c.health;
  return h(
    "div.section",
    h("h4", "The city"),
    kv([
      ["Size", `${c.size}${c.capital ? " (capital)" : ""}${c.disorder ? " — in disorder" : ""}${c.occupation ? ` — occupied ${c.occupation}` : ""}`],
      ["Food", `${f.stored}/${f.threshold} (${signed(f.difference)}; eats ${f.consumption}) ${f.difference > 0 ? `grows in ${turns(f.turns)}` : f.difference < 0 ? "starving" : "stagnant"}`],
      ["Production", `${y(1).rate} per turn${y(1).modifier ? ` (+${y(1).modifier}%)` : ""}`],
      ["Commerce", `${y(2).rate}: ${commerce || "-"}`],
      ["Happy", `${hp.happy} happy, ${hp.unhappy} unhappy${hp.angry ? `, ${hp.angry} angry` : ""}`],
      ["", h("span.dim", parts(hp, [["buildings", "buildings"], ["religions", "religions"], ["bonuses", "resources"], ["features", "features"], ["military", "military"], ["largest_city", "largest city"], ["commerce", "luxury"], ["extra", "other"]]) + (hp.buildings_bad || hp.religions_bad || hp.bonuses_bad ? `; against: ${parts(hp, [["buildings_bad", "buildings"], ["religions_bad", "religions"], ["bonuses_bad", "resources"], ["features_bad", "features"]])}` : "") + (hp.war_weariness_percent_anger ? `; war weariness ${hp.war_weariness_percent_anger}` : "") + (hp.hurry_anger_timer ? `; hurry anger ${hp.hurry_anger_timer}` : "") + (hp.conscript_anger_timer ? `; draft anger ${hp.conscript_anger_timer}` : ""))],
      ["Health", `${he.good} healthy, ${he.bad} unhealthy${he.unhealthy_population ? ` (${he.unhealthy_population} sick)` : ""}`],
      ["", h("span.dim", parts(he, [["fresh_water", "fresh water"], ["features", "features"], ["buildings", "buildings"], ["bonuses", "resources"], ["power", "power"], ["extra", "other"]]) + (he.buildings_bad || he.features_bad || he.bonuses_bad ? `; against: ${parts(he, [["buildings_bad", "buildings"], ["features_bad", "features"], ["bonuses_bad", "resources"], ["power_bad", "power"], ["fresh_water_bad", "water"]])}` : ""))],
      ["Culture", `${c.culture}/${c.culture_threshold}${c.culture_level !== null ? ` (${dname("CultureLevelType", c.culture_level)})` : ""}`],
      ["Great people", `${c.great_people.progress} (+${c.great_people.rate} per turn)`],
      ["Defence", `${c.defense}%${c.defense_damage ? ` (damaged ${c.defense_damage})` : ""}`],
      ["Maintenance", `${c.maintenance} gold`],
      ["Crime", c.crime || null],
      ["Trade routes", c.trade_routes || null],
      ["Founded", `turn ${c.founded}`],
    ]),
  );
}

function buildingsView(c) {
  const b = c.buildings.map((x) => h("span.chip", { title: plain(info("BuildingType", x.building)?.help ?? "") }, typeIcon("building", x.building, 8), dname("BuildingType", x.building), x.free && !x.real ? h("span.dim", " (free)") : null));
  const r = c.religions.map((x) => h("span.chip", typeIcon("religion", x.religion, 8), dname("ReligionType", x.religion), x.holy ? h("span.good", " ★ holy city") : null));
  const units = c.garrison.map((u) => h("span.chip", `${dname("UnitType", u.type)}${u.owner !== me()?.player ? ` (${u.owner})` : ""}`));
  return [
    h("div.section", h("h4", `Buildings (${b.length})`), b.length ? h("div.chips", b) : h("p.dim", "None.")),
    r.length ? h("div.section", h("h4", "Religions"), h("div.chips", r)) : null,
    h("div.section", h("h4", `Garrison (${units.length})`), units.length ? h("div.chips", units) : h("p.dim", "Nobody.")),
  ];
}

function manageView(c, task) {
  const hurries = c.hurries.map((x) =>
    h(
      "button.small",
      { disabled: !x.enabled, "data-hurry": x.hurry, title: dname("HurryType", x.hurry), onclick: () => task("TASK_HURRY", x.hurry) },
      `${dname("HurryType", x.hurry)}${x.gold ? ` (${x.gold} gold)` : ""}${x.population ? ` (${x.population} pop.)` : ""}`,
    ),
  );
  const emphasize = c.emphasize.map((on, e) =>
    h("label.toggle", { title: plain(info("EmphasizeType", e)?.help ?? "") }, h("input", { type: "checkbox", checked: on, "data-emphasize": e, onchange: (ev) => task("TASK_SET_EMPHASIZE", e, -1, ev.target.checked) }), ` ${dname("EmphasizeType", e)}`),
  );
  return h(
    "div.section",
    h("h4", "Orders"),
    h(
      "div.row",
      hurries,
      c.can_conscript ? h("button.small", { onclick: () => task("TASK_CONSCRIPT"), title: "Draft a unit (costs population and anger)" }, "Draft") : null,
    ),
    h("label.toggle", h("input", { type: "checkbox", checked: c.production.automated, id: "city-auto-production", onchange: (ev) => task("TASK_SET_AUTOMATED_PRODUCTION", -1, -1, ev.target.checked) }), " Automate production (the governor chooses)"),
    emphasize.length ? h("details", h("summary", "Emphasis"), h("div.checks", emphasize)) : null,
  );
}

function productionView(c, choose, remove) {
  const prod = c.production;
  const queue = h(
    "ol.queue",
    c.queue.length
      ? c.queue.map((o, i) =>
          h(
            "li",
            h("span", orderName(o.order, o.type)),
            h("span.dim", o.turns !== null ? ` ${turns(o.turns)}` : ""),
            // The exe pops an order only while the queue holds more than one (CvGameInterface
            // `getOrderQueueLength() > 1`); the only order is replaced by choosing another.
            c.queue.length > 1 ? h("button.small", { title: "Remove from the queue", "aria-label": "Remove", "data-index": i, onclick: () => remove(i) }, "✕") : null,
          ),
        )
      : h("li.dim", "Nothing: choose below."),
  );
  const lists = GROUPS.map(([order, label]) => {
    const items = c.buildable.filter((b) => b.order === order);
    if (!items.length) return null;
    return h(
      "div.section",
      h("h4", label),
      h(
        "div.build-grid",
        items.map((b) => {
          const kind = ORDER_KINDS[order];
          const e = info(kind, b.type);
          const help = e?.help ? markup(e.help).textContent : e?.strategy ? markup(e.strategy).textContent : "";
          return h(
            "button.build",
            {
              disabled: !b.enabled,
              "data-order": order,
              "data-type": b.type,
              title: `${help}${help ? "\n" : ""}Click: build now. Shift: add to the queue. Ctrl: put first.`,
              onclick: (ev) => choose(order, b.type, ev),
            },
            h("span", dname(kind, b.type)),
            h("span.dim", b.turns !== null ? `${turns(b.turns)} · ${b.cost}` : ""),
          );
        }),
      ),
    );
  });
  return [
    h(
      "div.section",
      h("h4", "Production"),
      h(
        "div.prod-line",
        prod.current && look.art ? uiIcon("yield", 1, 16) : null,
        prod.current ? `${prod.stored}/${prod.needed ?? "-"} (+${prod.rate} per turn)` : h("span.warn", "Nothing is being built."),
        prod.current && prod.needed ? barEl(prod.stored / prod.needed, "production", { w: 50, next: (prod.stored + prod.rate) / prod.needed }) : null,
      ),
      queue,
    ),
    ...lists,
  ];
}

function screen(c, { task, choose, remove, popup }) {
  const f = c.food;
  const head = h(
    "div.city-summary",
    h("b", `Size ${c.size}`),
    c.capital ? " · capital" : "",
    c.disorder ? h("span.warn", " · in disorder") : "",
    c.occupation ? h("span.warn", ` · occupied ${c.occupation}`) : "",
    ` · food ${signed(c.food.difference)} · production ${c.production.rate}`,
    look.art && f.threshold > 0 ? h("span.growth", uiIcon("yield", 0, 8), barEl(f.stored / f.threshold, "food", { w: 40, next: (f.stored + f.difference) / f.threshold, title: `${f.stored}/${f.threshold}` })) : null,
  );
  const left = h("div.city-col", plotGrid(c, task), specialistsView(c, task), statsView(c), buildingsView(c));
  const right = h("div.city-col", manageView(c, task), productionView(c, choose, remove));
  return [head, popup ? h("div.city-cols.popup", right, left) : h("div.city-cols", left, right)];
}
