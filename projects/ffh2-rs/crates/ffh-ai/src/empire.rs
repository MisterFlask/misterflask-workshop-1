//! Empire-level choices: research queue, civics, state religion, commerce sliders.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

pub fn manage(g: &mut Game, p: PlayerId) {
    research(g, p);
    civics(g, p);
    religion(g, p);
    sliders(g, p);
}

fn research(g: &mut Game, p: PlayerId) {
    if !g.players[p].research_queue.is_empty() {
        return;
    }
    if let Some(t) = crate::research::choose(g, p) {
        g.player_push_research(p, t, true);
    }
}

/// Value of running `civic` (AI heuristic).
fn civic_value(g: &Game, p: PlayerId, civic: CivicType) -> i32 {
    let d = g.d;
    let info = &d.civics[civic];
    let pl = &g.players[p];
    let n = pl.cities.len() as i32;
    let pop = pl.total_population;
    let mut v = info.ai_weight * n.max(1);
    if let Some(l) = pl.personality_type.map(|l| &d.leader_heads[l]) {
        if l.favorite_civic == Some(civic) {
            v += 300 + 50 * n;
        }
    }
    let yw = [3, 3, 2];
    for i in 0..YieldType::NUM {
        v += info.yield_modifier[i] * pop * yw[i] / 10;
        v += info.capital_yield_modifier[i] * yw[i];
    }
    for i in 0..CommerceType::NUM {
        v += info.commerce_modifier[i] * pl.commerce_rate[i].max(0) / 25;
        v += info.capital_commerce_modifier[i];
    }
    v += info.largest_city_happiness * 20 + info.extra_health * 15 * n;
    if pl.state_religion_count > 0 || g.player_get_state_religion(p).is_some() {
        v += info.state_religion_happiness * 20 * n;
    }
    v += info.non_state_religion_happiness * 10 * n;
    v += info.free_experience * 15 + info.free_specialist * 30 * n;
    v += info.trade_routes * 25 * n;
    v += info.worker_speed_modifier / 2 + info.military_production_modifier / 2;
    v -= info.war_weariness_modifier / 4;
    v -= g.player_get_single_civic_upkeep(p, Some(civic), true) * 10;
    v
}

fn civics(g: &mut Game, p: PlayerId) {
    if g.players[p].revolution_timer > 0 || g.player_is_anarchy(p) {
        return;
    }
    // Re-evaluate at most every 10 turns per player (staggered), as revolutions cost anarchy.
    if (g.game_turn() + p.0 as i32) % 10 != 0 {
        return;
    }
    let d = g.d;
    let current = g.players[p].civics.clone();
    let mut new = current.clone();
    let mut gain = 0;
    for opt in d.civic_options.keys() {
        let mut best: Option<(CivicType, i32)> = current[opt].map(|c| (c, civic_value(g, p, c)));
        let cur_value = best.map_or(i32::MIN, |(_, v)| v);
        for c in d.civics.keys() {
            if d.civics[c].civic_option_type != Some(opt) || !g.player_can_do_civics(p, Some(c)) {
                continue;
            }
            let v = civic_value(g, p, c);
            if best.map_or(true, |(_, b)| v > b) {
                best = Some((c, v));
            }
        }
        if let Some((c, v)) = best {
            if Some(c) != current[opt] {
                new[opt] = Some(c);
                gain += v - cur_value.max(0);
            }
        }
    }
    if new != current && gain > 100 && g.player_can_revolution(p, Some(&new)) {
        g.player_revolution(p, &new, false);
    }
}

fn religion(g: &mut Game, p: PlayerId) {
    if !g.player_can_change_religion(p) {
        return;
    }
    let d = g.d;
    let current = g.player_get_state_religion(p);
    let fav = g.players[p].personality_type.and_then(|l| d.leader_heads[l].favorite_religion);
    let mut best: Option<(ReligionType, i32)> = None;
    for r in d.religions.keys() {
        let count = g.players[p].has_religion_count[r];
        if count == 0 || !g.player_can_convert(p, Some(r)) && current != Some(r) {
            continue;
        }
        let mut v = count * 10;
        if fav == Some(r) {
            v += 25;
        }
        if g.player_has_holy_city(p, r) {
            v += 20;
        }
        if current == Some(r) {
            v += 15; // conversion costs anarchy
        }
        if best.map_or(true, |(_, b)| v > b) {
            best = Some((r, v));
        }
    }
    if let Some((r, _)) = best {
        if current != Some(r) && g.player_can_convert(p, Some(r)) {
            g.player_convert(p, Some(r));
        }
    }
}

/// Highest research rate that keeps the treasury from running dry.
fn sliders(g: &mut Game, p: PlayerId) {
    if !g.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH) {
        return;
    }
    let gold = g.players[p].gold;
    let reserve = 20 + 5 * g.players[p].cities.len() as i32;
    let mut pct = 100;
    while pct >= 0 {
        if g.players[p].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()] != pct {
            g.player_set_commerce_percent(p, CommerceType::COMMERCE_RESEARCH, pct);
        }
        let rate = g.player_calculate_gold_rate(p);
        if rate >= 0 || gold + rate * 10 > reserve {
            break;
        }
        pct -= 10;
    }
}
