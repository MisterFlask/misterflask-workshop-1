// Cities (design §5.1 "Cities"; the full city screen is P6): a city's summary, its production
// queue and the chooser (units, buildings, projects (FfH rituals) and processes with cost and
// turns): click replaces the current production, Shift-click appends, Ctrl-click inserts at
// the front (C++ `CvNetPushOrder`'s flags), and the queue's entries can be removed.

import * as api from "./api.js";
import { S, dname, orderName, info, turns, me } from "./state.js";
import { h, signed } from "./dom.js";
import { openModal } from "./modals.js";
import { run } from "./commands.js";
import { render as markup } from "./markup.js";

const GROUPS = [
  [0, "Units"],
  [1, "Buildings"],
  [2, "Projects and rituals"],
  [3, "Processes"],
];

/** Opens city `id` (its production). With `popup` (an idle city's chooser at a control
 *  point, design §2.3) the dialog offers "Later" and closes once production is chosen.
 *  Resolves with "chosen", "later" or null (closed). */
export async function openCity(id, { popup = false } = {}) {
  const city = S.cities.get(id);
  const title = popup ? `What should ${city?.name ?? "the city"} build?` : (city?.name ?? `City ${id}`);
  const m = openModal({ title, body: [h("p.dim", "…")], tag: "city", wide: true, buttons: buttons(popup, null) });
  let result = null;
  const load = async () => {
    let c;
    try {
      c = await api.query("city", { city: id });
    } catch (e) {
      m.setBody(h("p.warn", e.detail ?? e.message));
      return;
    }
    if (m.closed) return;
    m.setBody(body(c, async (order, type, ev) => {
      const reply = await run({ c: "push_order", city: id, order, data: type, alt: false, shift: !!ev.shiftKey, ctrl: !!(ev.ctrlKey || ev.metaKey) });
      if (!reply) return;
      if (popup) {
        result = "chosen";
        m.close("chosen");
      } else load();
    }, async (index) => {
      if (await run({ c: "pop_order", city: id, index })) load();
    }));
  };
  m.setButtons(buttons(popup, m));
  load();
  const v = await m.done;
  return result ?? v;
}

function buttons(popup, m) {
  if (popup) return [{ label: "Later", value: "later", title: "Leave the city idle for now (the end of turn reminds you)" }];
  return [{ label: "Close", value: null, onClick: () => m?.close(null) }];
}

function body(c, choose, remove) {
  const food = c.food;
  const prod = c.production;
  const y = (i) => c.yields.find((v) => v.yield === i)?.rate ?? 0;
  const summary = h(
    "div.city-summary",
    h("div", h("b", `Size ${c.size}`), c.capital ? " · capital" : "", c.disorder ? h("span.warn", " · in disorder") : "", c.occupation ? h("span.warn", ` · occupied ${c.occupation}`) : ""),
    h("div", `Food ${food.stored}/${food.threshold} (${signed(food.difference)}), `, food.difference > 0 ? `grows in ${turns(food.turns)}` : food.difference < 0 ? h("span.warn", "starving") : "stagnant"),
    h("div", `Production ${y(1)}/turn${prod.current ? ` · ${prod.stored}/${prod.needed ?? "-"}` : ""}${prod.automated ? " · automated" : ""}`),
    h("div", `Commerce ${y(2)} · happy ${c.happiness.happy} / unhappy ${c.happiness.unhappy} · health ${c.health.good} / ${c.health.bad}`),
  );
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
          const kind = ["UnitType", "BuildingType", "ProjectType", "ProcessType"][order];
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
  return [summary, h("div.section", h("h4", "Queue"), queue), ...lists];
}

/** Whether the human owns city `id`. */
export function ownCity(id) {
  return S.cities.get(id)?.owner === me()?.player;
}
