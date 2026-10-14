//! Diplomacy: when to declare war, when to make peace, which deals to accept.
//!
//! Relations are a small explicit score (alignment, religion, shared borders, war history)
//! instead of the C++ attitude machinery. All decisions are deterministic functions of the
//! game state (no RNG draws), so the AI does not perturb the rules' random stream.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::TradeData;
use ffh_data::*;

use crate::view::View;

/// Relation of player `p` towards `other`: < 0 hostile, > 0 friendly.
pub fn relation(g: &Game, p: PlayerId, other: PlayerId) -> i32 {
    let a = &g.players[p];
    let b = &g.players[other];
    let mut r = 0;
    match (a.alignment, b.alignment) {
        (Some(x), Some(y)) if x != y => {
            r -= (x.idx() as i32 - y.idx() as i32).abs() * 2;
        }
        (Some(_), Some(_)) => r += 1,
        _ => {}
    }
    match (g.player_get_state_religion(p), g.player_get_state_religion(other)) {
        (Some(x), Some(y)) if x == y => r += 2,
        (Some(_), Some(_)) => r -= 1,
        _ => {}
    }
    let (ta, tb) = (a.team, b.team);
    if g.teams[ta].ai_at_war_counter[tb] > 0 {
        r -= 1;
    }
    if g.team_is_open_borders(ta, tb) {
        r += 1;
    }
    r
}

fn capital_xy(g: &Game, p: PlayerId) -> Option<(i32, i32)> {
    g.players[p].capital_city.map(|c| (g.cities[c].x, g.cities[c].y))
}

/// Distance from our capital to the nearest city of `other`.
fn distance_to(g: &Game, p: PlayerId, other: PlayerId) -> i32 {
    let Some((x, y)) = capital_xy(g, p) else { return i32::MAX };
    g.players[other].cities.iter().map(|&c| g.plot_distance(x, y, g.cities[c].x, g.cities[c].y)).min().unwrap_or(i32::MAX)
}

fn is_team_leader(g: &Game, p: PlayerId) -> bool {
    let team = g.players[p].team;
    g.player_ids().into_iter().find(|&q| g.players[q].alive && g.players[q].team == team) == Some(p)
}

pub fn manage(g: &mut Game, v: &View) {
    let p = v.p;
    if v.barbarian || !is_team_leader(g, p) {
        return;
    }
    let team = v.team;
    let turn = g.game_turn();

    // Peace when a war drags on without advantage.
    for &enemy in &v.civ_enemies {
        let counter = g.teams[team].ai_at_war_counter[enemy];
        if counter < 10 {
            continue;
        }
        let ours = g.team_get_power(team, true).max(1);
        let theirs = g.team_get_power(enemy, true).max(1);
        if ours > theirs * 2 && counter < 40 {
            continue; // winning: keep going
        }
        let Some(other) = g.player_ids().into_iter().find(|&q| g.players[q].alive && g.players[q].team == enemy && g.players[q].human == false) else { continue };
        let item = TradeData { item_type: TradeableItem::TRADE_PEACE_TREATY, data: 0, offering: false, hidden: false };
        if !g.player_can_trade_item(p, other, &item, true) {
            continue;
        }
        let ours_list = vec![item.clone()];
        let theirs_list = vec![item];
        let accept = g.controller(other).consider_offer(g, other, p, &ours_list, &theirs_list);
        if accept {
            g.implement_deal(p, other, &ours_list, &theirs_list, false);
        }
    }

    // War: rarely, against a weaker, disliked neighbour we can reach.
    if !v.civ_enemies.is_empty() || turn < 60 || (turn + p.0 as i32 * 3) % 5 != 0 {
        return;
    }
    let leader = g.players[p].personality_type.map(|l| &g.d.leader_heads[l]);
    let war_rand = leader.map_or(100, |l| l.max_war_rand.max(1));
    // Deterministic stand-in for `getSorenRandNum(iMaxWarRand) == 0`, checked every 5 turns.
    let roll = ((turn as u32).wrapping_mul(2654435761) ^ (p.0 as u32).wrapping_mul(40503)) % (war_rand as u32 / 5 + 1);
    if roll != 0 {
        return;
    }
    let ours = g.team_get_power(team, true);
    let mut best: Option<(TeamId, i64)> = None;
    for other in g.player_ids() {
        let op = &g.players[other];
        if !op.alive || op.team == team || g.player_is_barbarian(other) {
            continue;
        }
        let ot = op.team;
        if !g.teams[team].has_met[ot] || !g.team_can_declare_war(team, ot) {
            continue;
        }
        let rel = relation(g, p, other);
        if rel > 0 {
            continue;
        }
        let theirs = g.team_get_power(ot, true).max(1);
        if (ours as i64) * 10 < (theirs as i64) * 14 {
            continue;
        }
        let dist = distance_to(g, p, other);
        if dist > 20 {
            continue;
        }
        let score = (ours as i64) * 100 / theirs as i64 - dist as i64 * 5 - rel as i64 * 20;
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((ot, score));
        }
    }
    if let Some((target, _)) = best {
        g.team_declare_war(team, target, true, Some(WarPlanType::WARPLAN_TOTAL));
    }
}

/// C++ `AI_considerOffer` equivalent: accept peace when the war is not going our way or has
/// lasted long, open borders with friends, and gifts.
pub fn consider_offer(g: &Game, p: PlayerId, from: PlayerId, they_give: &[TradeData], we_give: &[TradeData]) -> bool {
    if we_give.is_empty() {
        return true;
    }
    let team = g.players[p].team;
    let their_team = g.players[from].team;
    let all = |items: &[TradeData], kind: TradeableItem| items.iter().all(|t| t.item_type == kind);
    if all(they_give, TradeableItem::TRADE_PEACE_TREATY) && all(we_give, TradeableItem::TRADE_PEACE_TREATY) {
        let counter = g.teams[team].ai_at_war_counter[their_team];
        let ours = g.team_get_power(team, true).max(1);
        let theirs = g.team_get_power(their_team, true).max(1);
        return counter >= 10 && (ours * 10 <= theirs * 12 || counter >= 30);
    }
    if all(they_give, TradeableItem::TRADE_OPEN_BORDERS) && all(we_give, TradeableItem::TRADE_OPEN_BORDERS) {
        return !g.at_war(team, their_team) && relation(g, p, from) >= 1;
    }
    false
}

/// C++ `AI_conquerCity`: raze small, distant conquests we could not hold.
pub fn raze_conquered_city(g: &Game, p: PlayerId, c: CityId) -> bool {
    let city = &g.cities[c];
    if city.population > 2 || g.player_is_barbarian(p) {
        return false;
    }
    let (x, y) = (city.x, city.y);
    let nearest = g.players[p].cities.iter().filter(|&&o| o != c).map(|&o| g.plot_distance(x, y, g.cities[o].x, g.cities[o].y)).min().unwrap_or(i32::MAX);
    let holy = g.d.religions.keys().any(|r| g.get_holy_city(r) == Some(c));
    nearest > 10 && !holy && city.num_world_wonders == 0
}
