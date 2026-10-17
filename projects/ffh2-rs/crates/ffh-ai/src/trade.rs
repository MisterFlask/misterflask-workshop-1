//! Diplomacy trade valuation: what a resource (bonus), gold, or gold-per-turn is worth in a deal,
//! and whether we will give a resource away. A port of the `CvPlayerAI` trade functions
//! (`AI_bonusTradeVal`, `AI_bonusTrade`, `AI_dealVal`, `AI_goldTradeValuePercent`,
//! `AI_goldPerTurnTradeVal`, `AI_maxGoldTrade`, `AI_maxGoldPerTurnTrade`).
//!
//! The heavy part -- the value of *holding* a resource (`AI_bonusVal`/`AI_baseBonusVal`, the
//! unit/building/era enablement scan, the FfH mana and evil-resource terms) -- is already ported
//! faithfully in `ffh-core` (`Game::player_ai_bonus_val`); this module is the thin trade layer on
//! top. Integer math mirrors the DLL exactly (each `/ 100` truncates at that step).
//!
//! Deviations from the C++, forced by what ffh-core has ported (see docs/PORTING.md):
//! - `AI_techTradeVal` is unported; the whole AI already uses `team_get_research_cost` as its
//!   tech-value proxy (research.rs, diplomacy.rs), so `deal_val` uses the same proxy.
//! - `AI_isFinancialTrouble` is simplified to `production::financial_trouble`.
//! - `AI_getAttitude` is this AI's own `diplomacy::attitude` (the C++ attitude machinery is not
//!   ported); it returns the right enum, so the refuse-threshold comparisons are faithful in shape.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::TradeData;
use ffh_data::*;

use crate::diplomacy::attitude;

/// A resource (bonus) trade item.
pub fn bonus_item(b: BonusType) -> TradeData {
    TradeData { item_type: TradeableItem::TRADE_RESOURCES, data: b.i(), offering: false, hidden: false }
}

/// A one-off gold trade item of `amount` gold.
pub fn gold_item(amount: i32) -> TradeData {
    TradeData { item_type: TradeableItem::TRADE_GOLD, data: amount, offering: false, hidden: false }
}

/// A gold-per-turn trade item.
pub fn gold_per_turn_item(per_turn: i32) -> TradeData {
    TradeData { item_type: TradeableItem::TRADE_GOLD_PER_TURN, data: per_turn, offering: false, hidden: false }
}

/// C++ `AI_goldTradeValuePercent`: 200, or 300 when our finances are stretched (we value gold more
/// when broke). `AI_isFinancialTrouble` is the simplified `production::financial_trouble`.
pub fn gold_trade_value_percent(g: &Game, p: PlayerId) -> i32 {
    let mut v = 2;
    if crate::production::financial_trouble(g, p) {
        v += 1;
    }
    100 * v
}

/// C++ `AI_goldPerTurnTradeVal`: gold-per-turn valued over a peace-treaty's length, in the same
/// units as a one-off gold value.
pub fn gold_per_turn_trade_val(g: &Game, p: PlayerId, gpt: i32) -> i32 {
    let mut v = gpt * g.d.defines.peace_treaty_length;
    v = v * gold_trade_value_percent(g, p) / 100;
    v
}

/// C++ `AI_bonusTradeVal`: the value, to player `p` trading with `other`, of a single resource in a
/// deal. `change` is +1 when `p` gains the bonus, -1 when it loses it (passed to the ported
/// `player_ai_bonus_val`). Scaled by the smaller of the two empires (small empires value a resource
/// less), by the resource's data `ai_trade_modifier`, halved if `other` is our non-capitulated
/// vassal, and spread over a treaty's length.
pub fn bonus_trade_val(g: &Game, p: PlayerId, bonus: BonusType, other: PlayerId, change: i32) -> i32 {
    let mut v = g.player_ai_bonus_val(p, bonus, change);
    let cities = g.players[p].num_cities().min(g.players[other].num_cities());
    v = v * ((cities + 3) * 30) / 100;
    v = v * (g.d.bonuses[bonus].ai_trade_modifier + 100).max(0) / 100;
    let (team, ot) = (g.players[p].team, g.players[other].team);
    if g.teams[ot].vassal[team] && !g.teams[ot].capitulated {
        v /= 2;
    }
    v * g.d.defines.peace_treaty_length
}

/// Does any unit or building anywhere take `bonus` as a prerequisite (and-bonus or one of its
/// or-bonuses)? A "strategic" resource -- one that unlocks something -- which the AI guards more
/// jealously than a mere happiness/health resource. Scans all content (not civ-filtered), as C++
/// `AI_bonusTrade` does.
fn is_strategic(g: &Game, bonus: BonusType) -> bool {
    let d = g.d;
    d.units.keys().any(|u| {
        let info = &d.units[u];
        info.prereq_and_bonus == Some(bonus) || info.prereq_or_bonuses.iter().any(|&b| b == bonus)
    }) || d.buildings.keys().any(|b| {
        let info = &d.buildings[b];
        info.prereq_and_bonus == Some(bonus) || info.prereq_or_bonuses.iter().any(|&x| x == Some(bonus))
    })
}

/// C++ `AI_bonusTrade`: may player `p` give `bonus` to `other`? `None` is NO_DENIAL (yes). The AI
/// never gives a resource to its worst enemy, nor a strategic/happiness/health resource to a civ it
/// does not like well enough (the leader's per-category refuse thresholds, eased by one step toward
/// a civ of our alignment or favourite religion -- the FfH "better diplomacy" tweak). A resource
/// the recipient already has earns no thanks (NO_GAIN), so it is not offered. (Corporations are
/// absent in FfH 0.41, so `player_ai_corporation_bonus_val` is always 0 and those C++ branches are
/// dead -- transcribed literally, the zeros collapse them.)
pub fn bonus_trade_denial(g: &Game, p: PlayerId, bonus: BonusType, other: PlayerId) -> Option<DenialType> {
    let (team, ot) = (g.players[p].team, g.players[other].team);
    if g.teams[team].vassal[ot] || g.at_war(team, ot) || team == ot {
        return None;
    }
    // The receiver already has it (corporation value is always 0 without corporations).
    if g.player_get_num_available_bonuses(other, bonus) > 0 && g.player_ai_corporation_bonus_val(other, bonus) <= 0 {
        return Some(if g.players[other].human { DenialType::DENIAL_JOKING } else { DenialType::DENIAL_NO_GAIN });
    }
    if g.players[p].human {
        return None;
    }
    if g.teams[team].ai_worst_enemy == Some(ot) {
        return Some(DenialType::DENIAL_WORST_ENEMY);
    }
    if g.player_ai_corporation_bonus_val(p, bonus) > 0 {
        return Some(DenialType::DENIAL_JOKING); // dead without corporations
    }
    let att = attitude(g, p, other).i();
    // FfH: one step more generous toward a civ of our alignment or favourite religion.
    let same = g.players[other].alignment == g.players[p].alignment
        || g.players[other].favorite_religion == g.players[p].favorite_religion;
    let i_mod = i32::from(same);
    let leader = g.players[p].personality_type.map(|l| &g.d.leader_heads[l]);
    let refuse = |thr: Option<AttitudeType>| att + i_mod <= opt_to_i32(thr);
    if is_strategic(g, bonus) && leader.is_some_and(|l| refuse(l.strategic_bonus_refuse_attitude_threshold)) {
        return Some(DenialType::DENIAL_ATTITUDE);
    }
    if g.d.bonuses[bonus].happiness > 0 && leader.is_some_and(|l| refuse(l.happiness_bonus_refuse_attitude_threshold)) {
        return Some(DenialType::DENIAL_ATTITUDE);
    }
    if g.d.bonuses[bonus].health > 0 && leader.is_some_and(|l| refuse(l.health_bonus_refuse_attitude_threshold)) {
        return Some(DenialType::DENIAL_ATTITUDE);
    }
    None
}

/// Does a trade list contain gold or gold-per-turn? Used to refuse gold-for-gold.
pub fn is_gold_deal(list: &[TradeData]) -> bool {
    list.iter().any(|t| matches!(t.item_type, TradeableItem::TRADE_GOLD | TradeableItem::TRADE_GOLD_PER_TURN))
}

/// C++ `AI_dealVal`: the total value, to player `p` trading with `other`, of everything in `list`
/// (what `p` would receive). `change` is +1 for a gain (passed through to the per-resource value).
/// Scoped to the item kinds the AI actually trades -- technologies (via the research-cost proxy,
/// as `AI_techTradeVal` is unported), resources, gold, gold-per-turn; other kinds are valued 0
/// here and handled by `consider_offer`'s own peace/open-borders branches. The at-war `AI_endWarVal`
/// term is unreachable on the resource/gold paths (they are gated to peacetime), so it is omitted.
pub fn deal_val(g: &Game, p: PlayerId, other: PlayerId, list: &[TradeData], ignore_annual: bool, change: i32) -> i32 {
    let team = g.players[p].team;
    let mut value = 0;
    for item in list {
        match item.item_type {
            TradeableItem::TRADE_TECHNOLOGIES => {
                value += g.team_get_research_cost(team, TechType::from_usize(item.data as usize));
            }
            TradeableItem::TRADE_RESOURCES => {
                if !ignore_annual {
                    value += bonus_trade_val(g, p, BonusType::from_usize(item.data as usize), other, change);
                }
            }
            TradeableItem::TRADE_GOLD => {
                value += item.data * gold_trade_value_percent(g, p) / 100;
            }
            TradeableItem::TRADE_GOLD_PER_TURN => {
                if !ignore_annual {
                    value += gold_per_turn_trade_val(g, p, item.data);
                }
            }
            // C++ `AI_dealVal` TRADE_WAR (CvPlayerAI.cpp:6877): what `p` charges to declare war on
            // the item's target team as a favour to `other` -- our price for a war bribe.
            TradeableItem::TRADE_WAR => {
                value += crate::diplomacy::declare_war_trade_val(g, team, TeamId(item.data as u8), g.players[other].team);
            }
            // AI: other item kinds (maps, cities, ...) are not valued here; the resource/gold offer
            // paths never build them, and consider_offer handles peace/open-borders itself.
            _ => {}
        }
    }
    value
}

/// C++ `AI_maxGoldTrade`: the most one-off gold player `p` will hand `other` -- bounded by what it
/// can field (population, how long known, leader's generosity), what it has already given this
/// rival, and a buffer of ~12 turns of research it will not spend. Rounded down to a clean
/// multiple. (`p` is always an AI giving to a different team here, so only that branch is ported.)
pub fn max_gold_trade(g: &Game, p: PlayerId, other: PlayerId) -> i32 {
    let (team, ot) = (g.players[p].team, g.players[other].team);
    let leader = match g.players[p].personality_type {
        Some(l) => &g.d.leader_heads[l],
        None => return 0,
    };
    let mut max = g.players[p].total_population;
    max *= g.teams[team].ai_has_met_counter[ot] + 10;
    max = max * leader.max_gold_trade_percent / 100;
    max -= g.players[p].ai_gold_traded_to[other];
    let mut buffer = -g.player_calculate_gold_rate(p) * 12;
    buffer = buffer * g.d.game_speeds[g.game_speed_type()].research_percent / 100;
    let gold = g.players[p].gold;
    max = max.min(gold - buffer).min(gold);
    let remainder = g.d.defines.diplomacy_value_remainder.max(1);
    max -= max.rem_euclid(remainder);
    max.max(0)
}

/// C++ `AI_maxGoldPerTurnTrade`: the most gold-per-turn player `p` will commit to `other`, capped
/// by its actual income.
pub fn max_gold_per_turn_trade(g: &Game, p: PlayerId, other: PlayerId) -> i32 {
    let leader = match g.players[p].personality_type {
        Some(l) => &g.d.leader_heads[l],
        None => return 0,
    };
    let mut max = g.players[p].total_population * leader.max_gold_per_turn_trade_percent / 100;
    max += g.players[p].gold_per_turn_by_player[other].min(0);
    max.min(g.player_calculate_gold_rate(p)).max(0)
}

/// Build the loser's tribute (what they hand the winner) to close an end-war-value gap of `want`
/// value units, drawn from `loser`'s inventory and bounded by [`max_gold_trade`] (loser -> winner).
/// C++ `AI_doDiplo` peace construction (CvPlayerAI.cpp:835-900): a tech, then gold. Deterministic:
/// the costliest tech `loser` can hand over (substitutes the C++ `rand(10000)` pick), tie-broken by
/// index; then one-off gold. Cities are omitted (`AI_cityTradeVal` unported).
///
/// Deviation from the C++ `AI_doDiplo` quirk (deliberate, recorded in docs/AI.md): the original
/// adds gold at FACE value to its own value balance (CvPlayerAI.cpp:863) because the proposing AI
/// then implements the deal unilaterally. Here AI-AI deals are judged by the *recipient's*
/// `consider_offer`, whose `deal_val` scales gold by `gold_trade_value_percent` (200-300%). Built at
/// face value the gold would read as 2-3x overpriced and the recipient would always refuse, so the
/// feature would never fire. We therefore build the gold that is *worth* the remaining gap under
/// `deal_val` -- the `AI_counterPropose` scaling (CvPlayerAI.cpp:7182) -- so the deal balances for
/// the judge. Returns the items the loser adds to their side of the deal.
pub fn peace_tribute(g: &Game, winner: PlayerId, loser: PlayerId, want: i32) -> Vec<TradeData> {
    let mut items = Vec::new();
    if want <= 0 {
        return items;
    }
    let winner_team = g.players[winner].team;
    let tech_item = |t: TechType| TradeData { item_type: TradeableItem::TRADE_TECHNOLOGIES, data: t.i(), offering: false, hidden: false };
    let tech_val = |t: TechType| g.team_get_research_cost(winner_team, t);
    // Costliest tech the loser can give us. C++ quirk: the original picks a random tradeable tech
    // (1 + rand(10000)); we pick deterministically by value, tie-broken by lowest index.
    let tech = g
        .d
        .techs
        .keys()
        .filter(|&t| g.player_can_trade_item(loser, winner, &tech_item(t), true))
        .max_by_key(|&t| (tech_val(t), std::cmp::Reverse(t)));
    let mut got = 0;
    if let Some(t) = tech {
        items.push(tech_item(t));
        got += tech_val(t);
    }
    // Gold worth the remaining gap to the winner under deal_val (see the deviation note above),
    // capped by what the loser will hand over.
    let remaining = want - got;
    if remaining > 0 {
        let pct = gold_trade_value_percent(g, winner).max(1);
        let gold = (remaining * 100 / pct).min(max_gold_trade(g, loser, winner));
        if gold > 0 {
            let item = gold_item(gold);
            if g.player_can_trade_item(loser, winner, &item, true) {
                items.push(item);
            }
        }
    }
    items
}

/// Build a deal for `p` to acquire a resource from `other`: the resource of theirs `p` most wants
/// and they can spare, paid for with the cheapest surplus resource of ours they would take for it
/// (a balanced swap), or -- failing that -- with gold, at the least price that clears their
/// valuation and within [`max_gold_trade`]. Returns `(we_give, they_give)`, or `None` when there is
/// nothing worth acquiring (or no affordable payment). Deterministic: best wanted resource by value
/// then index, cheapest acceptable give likewise. The caller gates (peace, liking, contact cadence)
/// and runs `consider_offer` / `implement_deal`; `other`'s own `consider_offer` is the fairness
/// judge, so a thin or refused offer simply does nothing. We give only a genuine surplus
/// (`num_tradeable > 1`, keeping a copy) and never a resource [`bonus_trade_denial`] blocks.
pub fn buy_resource_offer(g: &Game, p: PlayerId, other: PlayerId) -> Option<(Vec<TradeData>, Vec<TradeData>)> {
    let get = g
        .d
        .bonuses
        .keys()
        .filter(|&b| {
            g.player_get_num_tradeable_bonuses(other, b) > 1
                && bonus_trade_val(g, p, b, other, 1) > 0
                && g.player_can_trade_item(other, p, &bonus_item(b), true)
        })
        .max_by_key(|&b| (bonus_trade_val(g, p, b, other, 1), std::cmp::Reverse(b)))?;
    let want = bonus_trade_val(g, p, get, other, 1);
    let give = g
        .d
        .bonuses
        .keys()
        .filter(|&b| {
            b != get
                && g.player_get_num_tradeable_bonuses(p, b) > 1
                && bonus_trade_denial(g, p, b, other).is_none()
                && g.player_can_trade_item(p, other, &bonus_item(b), true)
                && bonus_trade_val(g, other, b, p, 1) >= want
        })
        .min_by_key(|&b| (bonus_trade_val(g, other, b, p, 1), b));
    let ours = if let Some(give) = give {
        vec![bonus_item(give)]
    } else {
        let pct = gold_trade_value_percent(g, other).max(1);
        let mut gold = (want * 100 + pct - 1) / pct;
        let rem = g.d.defines.diplomacy_value_remainder.max(1);
        if gold % rem != 0 {
            gold += rem - gold % rem;
        }
        gold = gold.min(max_gold_trade(g, p, other));
        let item = gold_item(gold);
        if gold <= 0 || !g.player_can_trade_item(p, other, &item, true) {
            return None;
        }
        vec![item]
    };
    Some((ours, vec![bonus_item(get)]))
}
