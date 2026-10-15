//! Economy estimates shared by production, the sliders and disbanding: what one more unit
//! costs per turn and whether the empire can carry it at a decent research rate.
//!
//! FfH unit support (C++ `CvPlayer::calculateUnitCost`): units beyond the free ones (handicap,
//! civics, population) cost `getGoldPerUnit` (+ `getGoldPerMilitaryUnit` for military
//! units), scaled by the handicap and AI percentages, then by inflation. An AI that keeps
//! training units it cannot pay for ends with its research slider at zero and nothing left to
//! build but more units; production asks [`can_afford_unit`] before every optional unit.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

/// Gold per turn (x100) one more unit would cost: 0 while free unit slots remain.
pub fn marginal_unit_cost_x100(g: &Game, p: PlayerId, military: bool) -> i32 {
    let pl = &g.players[p];
    let b = g.player_calculate_unit_cost_breakdown(p);
    let mut raw = 0;
    if pl.num_units() >= b.free_units {
        raw += pl.gold_per_unit;
    }
    if military && pl.num_military_units >= b.free_military_units {
        raw += pl.gold_per_military_unit;
    }
    if raw <= 0 {
        return 0;
    }
    // The handicap / AI / era percentages, as they apply now.
    let base = b.base_unit_cost + b.military_cost + b.extra_cost;
    let scaled = if base > 0 { raw * 100 * b.cost / base } else { raw * 100 };
    scaled * (100 + g.player_calculate_inflation_rate(p).max(0)) / 100
}

/// The share of the commerce the sliders spend on research and culture: what the AI reads as
/// "research" when it judges its finances. A culture slider the victory planner sets (see
/// `victory::culture_slider`) is a choice, not a cost: counting only research, the empire took
/// itself for bankrupt and disbanded its army.
pub fn spend_pct(g: &Game, p: PlayerId) -> i32 {
    let pct = &g.players[p].commerce_percent;
    pct[CommerceType::COMMERCE_RESEARCH.idx()] + pct[CommerceType::COMMERCE_CULTURE.idx()]
}

/// Estimated net gold per turn with research and culture together at `research_pct` (see
/// [`spend_pct`]; the culture slider stays where it is, gold takes the rest).
pub fn gold_at_research(g: &Game, p: PlayerId, research_pct: i32) -> i32 {
    let pl = &g.players[p];
    if !g.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH) {
        return g.player_calculate_gold_rate(p);
    }
    let research_pct = (research_pct - pl.commerce_percent[CommerceType::COMMERCE_CULTURE.idx()]).max(0);
    let gold = g.player_get_commerce_rate(p, CommerceType::COMMERCE_GOLD);
    let research = g.player_get_commerce_rate(p, CommerceType::COMMERCE_RESEARCH);
    let cur_gold_pct = pl.commerce_percent[CommerceType::COMMERCE_GOLD.idx()];
    let cur_research_pct = pl.commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()];
    let flexible = (cur_gold_pct + cur_research_pct).max(1);
    let new_gold_pct = (flexible - research_pct).max(0);
    let est_gold = (gold + research) * new_gold_pct / flexible;
    est_gold + pl.gold_per_turn - g.player_calculate_inflated_costs(p)
}

/// Can the empire pay for one more (military) unit and still research at `research_pct`?
pub fn can_afford_unit_at(g: &Game, p: PlayerId, military: bool, research_pct: i32) -> bool {
    let cost = marginal_unit_cost_x100(g, p, military);
    if cost == 0 {
        return true;
    }
    gold_at_research(g, p, research_pct) * 100 >= cost
}

/// [`can_afford_unit_at`] with the research rate the AI wants to keep: 60% in peace, 30% at
/// war (or while preparing one).
pub fn can_afford_unit(g: &Game, p: PlayerId, military: bool, at_war: bool) -> bool {
    can_afford_unit_at(g, p, military, if at_war { 30 } else { 60 })
}
