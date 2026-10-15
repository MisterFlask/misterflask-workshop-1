//! Trade (design §4.5 `propose_deal`, §4.6 `trade`): what the diplomacy screen offers, both ways,
//! and the checks a proposed deal must pass.
//!
//! The screen lists `CvPlayer::buildTradeTable` (ported here as [`trade_table`]: gold, gold per
//! turn, maps, vassal, open borders, defensive pact, permanent alliance, at war a peace treaty
//! and capitulation, then techs, resources, cities, and for an AI side peace, war and embargo
//! with third teams, civics and religions), each item greyed when `getTradeDenial` refuses it.
//! Gold and gold per turn are amounts up to the side's maximum (`AI_maxGoldTrade`,
//! `AI_maxGoldPerTurnTrade`: for a human its gold and its gold rate; for an AI the C++ AI's own
//! limits are not ported, so the same bounds apply). The C++ AI's denials are not ported either
//! (ffh-core reads them as "no denial" except the rules-based ones); whether an AI accepts is its
//! controller's `consider_offer`.
//!
//! Whether an AI talks at all is [`ai_willing_to_talk`] (`CvPlayerAI::AI_isWillingToTalk`, which
//! gates the exe's diplomacy screen: "refuses to talk"): at war, not before its war counter
//! reaches its leader's refuse-to-talk threshold. A deal between teams at war always makes peace
//! (`CvDeal::addTrades` calls `makePeace` for any deal at war), so at war a deal must say so: a
//! peace treaty on both sides, or a capitulation, as the C++ AI's own war offers and counter
//! proposals have it (`AI_doDiplo`, `AI_counterPropose`). A gift alone would end the war past the
//! AI's peace judgement.

use ffh_core::deal::Deal;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::TradeData;
use ffh_data::*;

use crate::json::Json;
use crate::query::{invalid, QueryErr};
use crate::view::Viewer;

fn item(item_type: TradeableItem, data: i32) -> TradeData {
    TradeData { item_type, data, offering: false, hidden: false }
}

/// C++ `CvPlayer::buildTradeTable(eOtherPlayer, ourList)`: the items `from` can give `to`.
pub fn trade_table(g: &Game, from: PlayerId, to: PlayerId) -> Vec<TradeData> {
    let d = g.d;
    let mut out = Vec::new();
    let mut add = |t: TradeData, check: bool| {
        if !check || g.player_can_trade_item(from, to, &t, false) {
            out.push(t);
        }
    };
    add(item(TradeableItem::TRADE_GOLD, 0), true);
    add(item(TradeableItem::TRADE_GOLD_PER_TURN, 0), true);
    add(item(TradeableItem::TRADE_MAPS, 0), true);
    add(item(TradeableItem::TRADE_VASSAL, 0), true);
    add(item(TradeableItem::TRADE_OPEN_BORDERS, 0), true);
    add(item(TradeableItem::TRADE_DEFENSIVE_PACT, 0), true);
    add(item(TradeableItem::TRADE_PERMANENT_ALLIANCE, 0), true);
    let (team, to_team) = (g.players[from].team, g.players[to].team);
    if g.at_war(team, to_team) {
        // "We are at war, allow a peace treaty option" (no canTradeItem check).
        add(item(TradeableItem::TRADE_PEACE_TREATY, 0), false);
        add(item(TradeableItem::TRADE_SURRENDER, 0), true);
    }
    for t in d.techs.keys() {
        add(item(TradeableItem::TRADE_TECHNOLOGIES, t.idx() as i32), true);
    }
    for b in d.bonuses.keys() {
        add(item(TradeableItem::TRADE_RESOURCES, b.idx() as i32), true);
    }
    for &c in &g.players[from].cities {
        add(item(TradeableItem::TRADE_CITIES, c.0 as i32), true);
    }
    if !g.players[from].human {
        for kind in [TradeableItem::TRADE_PEACE, TradeableItem::TRADE_WAR, TradeableItem::TRADE_EMBARGO] {
            for t in g.civ_team_ids() {
                if g.team_is_alive(t) && t != team && t != to_team {
                    add(item(kind, t.0 as i32), true);
                }
            }
        }
    }
    for c in d.civics.keys() {
        add(item(TradeableItem::TRADE_CIVIC, c.idx() as i32), true);
    }
    for r in d.religions.keys() {
        add(item(TradeableItem::TRADE_RELIGION, r.idx() as i32), true);
    }
    out
}

/// The most gold `from` can give `to` (`AI_maxGoldTrade`: a human's gold; see the module docs).
pub fn max_gold(g: &Game, from: PlayerId, _to: PlayerId) -> i32 {
    g.players[from].gold.max(0)
}

/// The most gold per turn `from` can give (`AI_maxGoldPerTurnTrade`: for a human
/// `max(0, min(goldRate + gold / PEACE_TREATY_LENGTH, goldRate))`).
pub fn max_gold_per_turn(g: &Game, from: PlayerId, _to: PlayerId) -> i32 {
    let rate = g.player_calculate_gold_rate(from);
    let length = g.d.defines.peace_treaty_length.max(1);
    (rate + g.players[from].gold / length).min(rate).max(0)
}

fn item_json(g: &Game, from: PlayerId, to: PlayerId, t: &TradeData) -> Json {
    let mut j = crate::jobj! {
        "item" => Json::idx(t.item_type),
        "data" => t.data,
        "denial" => Json::opt_idx(g.player_get_trade_denial(from, to, t)),
    };
    match t.item_type {
        TradeableItem::TRADE_GOLD => {
            j.set("max", max_gold(g, from, to));
        }
        TradeableItem::TRADE_GOLD_PER_TURN => {
            j.set("max", max_gold_per_turn(g, from, to));
        }
        // The diplomacy screen names the cities either side offers.
        TradeableItem::TRADE_CITIES if t.data >= 0 && g.cities.contains(CityId(t.data as u32)) => {
            j.set("name", crate::view::city_name(g, CityId(t.data as u32)));
        }
        _ => {}
    }
    j
}

/// C++ `CvPlayerAI::AI_isWillingToTalk(ePlayer)`: whether the AI `ai` talks to `other` (the exe
/// opens the diplomacy screen only then; the contact button's help says "refuses to talk"). At
/// war it refuses until its war counter reaches its leader's `iRefuseToTalkWarThreshold` (doubled
/// for a war it chose, shortened when the other side is clearly winning), and a vassal never
/// talks at war. In peace C++ also refuses while it remembers the other stopping trade with it
/// (`MEMORY_STOPPED_TRADING_RECENT`); that branch is not ported: the AI memory decay
/// (`CvPlayerAI::AI_doCounter`) is AI code that ffh-ai does not run, so the memory would never
/// fade and the AI would refuse to talk for the rest of the game.
pub fn ai_willing_to_talk(g: &Game, ai: PlayerId, other: PlayerId) -> bool {
    let (team, their) = (g.players[ai].team, g.players[other].team);
    if their == team || g.teams[their].vassal[team] || g.teams[team].vassal[their] {
        return true;
    }
    if g.team_is_human(team) {
        return false;
    }
    if g.at_war(team, their) {
        let threshold = g.players[ai].personality_type.map_or(0, |l| g.d.leader_heads[l].refuse_to_talk_war_threshold);
        let mut refuse = threshold * if g.team_ai_is_chosen_war(team, their) { 2 } else { 1 };
        let ours = 1 + g.teams[team].ai_war_success[their];
        let theirs = 1 + g.teams[their].ai_war_success[team];
        if theirs > ours * 2 {
            refuse *= 20 + (80 * ours * 2) / theirs.max(1);
            refuse /= 100;
        }
        if g.teams[team].ai_at_war_counter[their] < refuse {
            return false;
        }
        if g.team_is_a_vassal(team) {
            return false;
        }
    }
    true
}

/// Why the human cannot trade with `with` (or `Ok`): a met, alive civilization it can contact
/// and that is willing to talk ([`ai_willing_to_talk`]).
pub fn contact(g: &Game, h: PlayerId, with: PlayerId) -> Result<(), String> {
    if with.idx() >= g.num_civ_players() || with == h || g.player_is_barbarian(with) {
        return Err(format!("player {} is not another civilization", with.0));
    }
    let team = g.players[h].team;
    if !g.players[with].alive || !g.teams[team].has_met[g.players[with].team] {
        return Err(format!("player {} is unknown or dead", with.0));
    }
    if !g.player_can_contact(h, with) {
        return Err(format!("player {} cannot be contacted now", with.0));
    }
    if !g.players[with].human && !ai_willing_to_talk(g, with, h) {
        return Err(format!("player {} refuses to talk", with.0));
    }
    Ok(())
}

/// `trade {with}`: the trade tables both ways (`we_give` from the human's table, `they_give`
/// from theirs), with denials and the gold maxima.
pub fn trade(v: &Viewer, with: PlayerId) -> Result<Json, QueryErr> {
    let g = v.g;
    if with.idx() >= g.num_civ_players() || with == v.h || g.player_is_barbarian(with) || !v.player_met(with) {
        return Err(invalid(format!("field `with`: player {} is not a civilization the human knows", with.0)));
    }
    let can = contact(g, v.h, with);
    let (ours, theirs) = if can.is_ok() { (trade_table(g, v.h, with), trade_table(g, with, v.h)) } else { (Vec::new(), Vec::new()) };
    Ok(crate::jobj! {
        "with" => with,
        "contact" => can.is_ok(),
        "why_not" => can.err(),
        "war" => g.at_war(v.team, g.players[with].team),
        "we_give" => Json::Arr(ours.iter().map(|t| item_json(g, v.h, with, t)).collect()),
        "they_give" => Json::Arr(theirs.iter().map(|t| item_json(g, with, v.h, t)).collect()),
    })
}

/// The deal checks of `propose_deal` (the diplomacy screen only offers these): `to` can be
/// contacted and is willing to talk ([`contact`]); something is traded; every item is in its
/// side's trade table (gold and gold per turn as an amount from 1 to the side's maximum), not
/// twice, and passes `canTradeItem(.., bTestDenial = true)`; a dual item (peace treaty, open
/// borders, defensive pact, permanent alliance) is on both sides, as the screen puts it; at war,
/// the deal carries the peace treaty or a capitulation (see the module docs).
pub fn check_deal(g: &Game, h: PlayerId, to: PlayerId, give: &[TradeData], get: &[TradeData]) -> Result<(), crate::command::CmdErr> {
    use crate::command::CmdErr;
    if to.idx() >= g.num_players() {
        return Err(CmdErr::Invalid(format!("field `to`: player {} out of range", to.0)));
    }
    contact(g, h, to).map_err(CmdErr::Cannot)?;
    if give.is_empty() && get.is_empty() {
        return Err(CmdErr::Invalid("an empty deal".into()));
    }
    let side = |list: &[TradeData], from: PlayerId, other: PlayerId, field: &str| -> Result<(), CmdErr> {
        let table = trade_table(g, from, other);
        for (i, t) in list.iter().enumerate() {
            if list[..i].iter().any(|u| u.item_type == t.item_type && u.data == t.data) || (Deal::is_gold(t.item_type) && list[..i].iter().any(|u| u.item_type == t.item_type)) {
                return Err(CmdErr::Invalid(format!("field `{field}`: {} twice", crate::names::name(g.d, t.item_type))));
            }
            let listed = match t.item_type {
                TradeableItem::TRADE_GOLD | TradeableItem::TRADE_GOLD_PER_TURN => {
                    let max = if t.item_type == TradeableItem::TRADE_GOLD { max_gold(g, from, other) } else { max_gold_per_turn(g, from, other) };
                    if !(1..=max).contains(&t.data) {
                        return Err(CmdErr::Cannot(format!("field `{field}`: {} {} (1 to {max} possible)", crate::names::name(g.d, t.item_type), t.data)));
                    }
                    table.iter().any(|u| u.item_type == t.item_type)
                }
                _ => table.iter().any(|u| u.item_type == t.item_type && u.data == t.data),
            };
            if !listed {
                return Err(CmdErr::Cannot(format!("field `{field}`: {} {} is not tradeable", crate::names::name(g.d, t.item_type), t.data)));
            }
            if !g.player_can_trade_item(from, other, t, true) {
                return Err(CmdErr::Cannot(format!("field `{field}`: {} {} is refused", crate::names::name(g.d, t.item_type), t.data)));
            }
        }
        Ok(())
    };
    side(give, h, to, "give")?;
    side(get, to, h, "get")?;
    for (list, other, field) in [(give, get, "give"), (get, give, "get")] {
        for t in list {
            if Deal::is_dual(t.item_type, false) && !other.iter().any(|u| u.item_type == t.item_type) {
                return Err(CmdErr::Invalid(format!(
                    "field `{field}`: {} goes on both sides of a deal",
                    crate::names::name(g.d, t.item_type)
                )));
            }
        }
    }
    // At war every deal makes peace (`CvDeal::addTrades`): it has to be a peace deal.
    if g.at_war(g.players[h].team, g.players[to].team) {
        let has = |kind: TradeableItem| give.iter().chain(get).any(|t| t.item_type == kind);
        if !has(TradeableItem::TRADE_PEACE_TREATY) && !has(TradeableItem::TRADE_SURRENDER) {
            return Err(CmdErr::Invalid("at war a deal makes peace: it needs the peace treaty (both sides) or a capitulation".into()));
        }
    }
    Ok(())
}

/// A trade item from the protocol: `{"item": TRADE_X or index, "data": n}` (`data` 0 when
/// absent).
pub fn trade_item_from_json(d: &GameData, j: &Json) -> Result<TradeData, crate::json::FieldError> {
    let item_type = crate::names::field::<TradeableItem>(d, j, "item")?;
    Ok(item(item_type, j.get_or("data", 0)?))
}

pub fn trade_item_json(t: &TradeData) -> Json {
    crate::jobj! { "item" => Json::idx(t.item_type), "data" => t.data }
}
