// Diplomacy (design §5.1 "Diplomacy minimum", §7 P6; Civ4's foreign advisor, F4): the players you
// met (leader, civilization, war or peace, open borders, pacts, vassals, attitude, score), your
// deals with them, declaring war (`declare_war`), and the trade screen: what each side can give
// (the `trade` query, C++ `buildTradeTable`, refusals greyed), a deal put together from both
// tables and offered (`propose_deal`), which the AI accepts or declines at once.

import * as api from "./api.js";
import { S, dname, tname, teamName, tradeItemLabel, tradeRefusal, DUAL_ITEMS } from "./state.js";
import { h, toast } from "./dom.js";
import { openModal, ask, closeTag } from "./modals.js";
import { run } from "./commands.js";

/** The players screen. */
export async function openDiplomacy() {
  const m = openModal({ title: "Foreign affairs", body: [h("p.dim", "…")], tag: "diplomacy", wide: true, buttons: [{ label: "Close", value: null }] });
  const load = async () => {
    let r;
    try {
      r = await api.query("players");
    } catch (e) {
      return m.setBody(h("p.warn", e.detail ?? e.message));
    }
    if (m.closed) return;
    const rows = r.players
      .filter((p) => !p.barbarian)
      .map((p) => {
        const relation = [p.war ? h("span.warn", "war") : "peace", p.open_borders ? "open borders" : null, p.defensive_pact ? "defensive pact" : null, p.vassal_of_us ? "your vassal" : null, p.our_master ? "your master" : null].filter(Boolean);
        const deals = p.deals.map((d) => `${d.we_give.map(tradeItemLabel).join(", ") || "nothing"} ⇄ ${d.they_give.map(tradeItemLabel).join(", ") || "nothing"} (since turn ${d.turn})`);
        return h(
          "tr",
          { class: p.alive ? "" : "dead", "data-player": p.id },
          h("td", h("b", p.name), h("div.dim", `${dname("CivilizationType", p.civ)} · team ${p.team}`)),
          h("td", relation.flatMap((x, i) => (i ? [", ", x] : [x]))),
          h("td", p.attitude !== null ? dname("AttitudeType", p.attitude) : "-"),
          h("td.num", String(p.score)),
          h(
            "td.actions",
            p.alive && p.contact ? h("button.small", { "data-trade": p.id, onclick: () => openTrade(p.id, p.name).then(load) }, "Trade…") : h("span.dim", p.alive ? "no contact" : "dead"),
            p.can_declare_war && !p.war ? h("button.small.danger", { "data-war": p.team, onclick: () => declareWar(p).then(load) }, "Declare war") : null,
          ),
          deals.length ? h("td.dim", deals.map((d) => h("div", d))) : h("td"),
        );
      });
    m.setBody(
      rows.length
        ? h("table.scores.diplo", h("tr", h("th", "Leader"), h("th", "Relations"), h("th", "Attitude"), h("th", "Score"), h("th", ""), h("th", "Deals")), rows)
        : h("p.dim", "You have not met anyone yet."),
    );
  };
  load();
  return m.done;
}

async function declareWar(p) {
  const v = await ask("Declare war?", `Declare war on ${teamName(p.team)}? Their allies may join them.`, [
    { label: "Declare war", value: true, primary: true },
    { label: "Cancel", value: null },
  ]);
  if (v) await run({ c: "declare_war", team: p.team });
}

/** The trade screen with player `with`: both trade tables; clicking an item puts it into the
 *  deal (an item that goes on both sides, such as a peace treaty, goes on both, and is greyed on
 *  both when either side refuses it), gold as an amount; Propose offers it. */
export async function openTrade(withPlayer, name = null) {
  const m = openModal({ title: `Trade with ${name ?? S.players.get(withPlayer)?.name ?? `player ${withPlayer}`}`, body: [h("p.dim", "…")], tag: "trade", wide: true, buttons: [{ label: "Close", value: null }] });
  let t;
  try {
    t = await api.query("trade", { with: withPlayer });
  } catch (e) {
    m.setBody(h("p.warn", e.detail ?? e.message));
    return m.done;
  }
  if (!t.contact) {
    m.setBody(h("p.warn", t.why_not ?? "No contact."));
    return m.done;
  }
  // The deal: per side, the chosen items by key (gold: the amount typed).
  const deal = { give: new Map(), get: new Map() };
  const keyOf = (it) => `${it.item}:${it.data}`;
  const isDual = (it) => DUAL_ITEMS.includes(tname("TradeableItem", it.item));
  const isGold = (it) => ["TRADE_GOLD", "TRADE_GOLD_PER_TURN"].includes(tname("TradeableItem", it.item));
  const status = h("p.dim");
  const draw = () => {
    const side = (list, mapKey, other, title) =>
      h(
        "div.trade-side",
        h("h4", title),
        list.length
          ? list.map((it) => {
              const k = keyOf(it);
              const on = deal[mapKey].has(k);
              // Refused by its side, or (an item on both sides) by the other side.
              const refusal = tradeRefusal(it, other);
              const denied = !!refusal;
              const why = refusal ? `${refusal.twin ? "the other side refuses: " : ""}${dname("DenialType", refusal.denial)}` : "";
              const label = isGold(it) ? `${tradeItemLabel({ ...it, data: 0 })} (up to ${it.max})` : tradeItemLabel(it);
              const row = h(
                "div.trade-item",
                { class: on ? "on" : "", "data-item": tname("TradeableItem", it.item), "data-data": it.data },
                h(
                  "button.small",
                  {
                    disabled: denied || (isGold(it) && !it.max),
                    title: denied ? `Refused (${why})` : "Add to the deal / take it out",
                    onclick: () => {
                      const toggle = (map, item) => (map.has(keyOf(item)) ? map.delete(keyOf(item)) : map.set(keyOf(item), { item: item.item, data: isGold(item) ? Math.min(item.max, 1) : item.data }));
                      toggle(deal[mapKey], it);
                      if (isDual(it)) {
                        const twin = other.find((o) => o.item === it.item);
                        const otherKey = mapKey === "give" ? "get" : "give";
                        if (twin && deal[mapKey].has(k) !== deal[otherKey].has(keyOf(twin))) toggle(deal[otherKey], twin);
                      }
                      draw();
                    },
                  },
                  on ? "✓" : "+",
                ),
                h("span", label),
                on && isGold(it)
                  ? h("input.amount", {
                      type: "number",
                      min: 1,
                      max: it.max,
                      value: deal[mapKey].get(k).data,
                      oninput: (ev) => (deal[mapKey].get(k).data = Math.max(1, Math.min(it.max, Number(ev.target.value) || 1))),
                    })
                  : null,
                denied ? h("span.dim", ` (${why})`) : null,
              );
              return row;
            })
          : h("p.dim", "Nothing."),
      );
    m.setBody(
      t.war ? h("p.warn", "You are at war: a deal makes peace, so it must hold the peace treaty (or a capitulation).") : null,
      h("div.trade", side(t.we_give, "give", t.they_give, "You give"), side(t.they_give, "get", t.we_give, "They give")),
      status,
    );
  };
  m.setButtons([
    {
      label: "Propose",
      primary: true,
      onClick: async () => {
        const give = [...deal.give.values()];
        const get = [...deal.get.values()];
        if (!give.length && !get.length) {
          status.textContent = "Put something into the deal first.";
          return;
        }
        const reply = await run({ c: "propose_deal", to: withPlayer, give, get });
        if (!reply) return;
        if (reply.r?.accepted) {
          toast("They accepted the deal.");
          closeTag("trade");
        } else {
          status.textContent = "They declined.";
          status.className = "warn";
        }
      },
    },
    { label: "Close", value: null },
  ]);
  draw();
  return m.done;
}
