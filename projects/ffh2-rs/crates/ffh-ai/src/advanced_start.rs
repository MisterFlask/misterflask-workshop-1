//! Advanced start (C++ `CvPlayerAI::AI_doAdvancedStart`, `GAMEOPTION_ADVANCED_START`): spend
//! the start points through the rules' `doAdvancedStartAction`, then exit (the rules turn what
//! is left into gold).
//!
//! The C++ split is kept: about 10% of the points reveal land around the start, the leader's
//! share (10% + `iBuildUnitProb` / 3) goes to units, the rest to cities, their population,
//! culture and improvements, and technologies (at most a quarter of the city points while
//! cities are still being placed). The C++ random draws (tech purchase odds, unit value noise)
//! are replaced by fixed choices: the AI draws nothing from the game RNG.

use ffh_core::defs::{CITY_HOME_PLOT, NUM_CITY_PLOTS};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::player::AdvancedStartAction as A;
use ffh_data::*;

use crate::production::{best_unit, Role};

fn act(g: &mut Game, p: PlayerId, action: A, plot: Option<PlotIdx>, data: i32) {
    let (x, y) = plot.map_or((-1, -1), |q| g.map.xy(q));
    g.player_do_advanced_start_action(p, action, x, y, data, true);
}

fn spent_since(g: &Game, p: PlayerId, mark: i32) -> i32 {
    mark - g.players[p].advanced_start_points
}

/// Spend `p`'s advanced start points, then leave advanced start.
pub fn run(g: &mut Game, p: PlayerId) {
    if g.players[p].advanced_start_points < 0 || g.player_is_barbarian(p) {
        return;
    }
    let Some(start) = g.player_get_starting_plot(p) else {
        act(g, p, A::Exit, None, -1);
        return;
    };
    let total = g.players[p].advanced_start_points;
    let reveal_points = total * 10 / 100;
    let build_unit_prob = g.players[p].personality_type.map_or(0, |l| g.d.leader_heads[l].build_unit_prob);
    let military_points = total * (10 + build_unit_prob / 3) / 100;
    let mut city_points = total - military_points - reveal_points;

    // Capital: on the starting plot, else the best site near it.
    if g.players[p].capital_city.is_none() {
        let site = if g.player_get_advanced_start_city_cost(p, true, Some(start)) > 0 {
            Some(start)
        } else {
            best_site(g, p, start, 6, 1)
        };
        match site {
            Some(q) => place_city(g, p, q),
            None => {
                act(g, p, A::Exit, None, -1);
                return;
            }
        }
        if g.players[p].cities.is_empty() {
            act(g, p, A::Exit, None, -1);
            return;
        }
    } else if let Some(c) = g.players[p].capital_city {
        let q = g.city_plot(c);
        place_city(g, p, q);
    }
    city_points -= total - g.players[p].advanced_start_points;

    // Reveal the land around the start, nearest rings first.
    let mark = g.players[p].advanced_start_points;
    let (sx, sy) = g.map.xy(start);
    'reveal: for r in 1..=12i32 {
        for dx in -r..=r {
            for dy in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let Some(q) = g.plot_xy(sx, sy, dx, dy) else { continue };
                if g.player_get_advanced_start_visibility_cost(p, true, Some(q)) > 0 {
                    act(g, p, A::Visibility, Some(q), -1);
                }
                if spent_since(g, p, mark) > reveal_points || g.players[p].advanced_start_points <= 0 {
                    break 'reveal;
                }
            }
        }
    }

    let mark = g.players[p].advanced_start_points;
    city_points = city_points.min(mark);
    let mut tech_spending = 0;
    // One tech up front in the first era (C++), more while cities are placed (up to a quarter
    // of the city points).
    if buy_tech(g, p, i32::MAX).is_some_and(|c| {
        tech_spending += c;
        true
    }) {}
    for _ in 0..100 {
        if let Some(c) = buy_tech(g, p, city_points / 4 - tech_spending) {
            tech_spending += c;
            for c in g.players[p].cities.clone() {
                let q = g.city_plot(c);
                place_city(g, p, q);
            }
        }
        let n = g.players[p].cities.len() as i32;
        let Some(site) = best_site(g, p, start, 8, 500 + 250 * n) else { break };
        let cost = g.player_get_advanced_start_city_cost(p, true, Some(site));
        if cost > g.players[p].advanced_start_points || spent_since(g, p, mark) + cost > city_points {
            break;
        }
        let before = g.players[p].cities.len();
        place_city(g, p, site);
        if g.players[p].cities.len() == before {
            break;
        }
    }
    // Technologies with the rest of the city points.
    while buy_tech(g, p, city_points - spent_since(g, p, mark)).is_some() {}

    // A land explorer from the city with the most unknown land around.
    if let Some(&c) = g.players[p].cities.first() {
        let area = g.map.plots[g.city_plot(c)].area;
        if area.is_some_and(|a| g.area_get_num_unrevealed_tiles(a, g.players[p].team) > 10) {
            if let Some(u) = best_unit(g, c, Role::Explore) {
                place_unit(g, p, g.city_plot(c), u, UnitAIType::UNITAI_EXPLORE);
            }
        }
    }

    // Buildings: the most valuable per point in each city, while city points last.
    for _ in 0..10 {
        let mut any = false;
        for c in g.players[p].cities.clone() {
            let mut best: Option<(BuildingType, i64)> = None;
            for b in g.d.buildings.keys() {
                let cost = g.player_get_advanced_start_building_cost(p, b, true, Some(c));
                if cost <= 0 || spent_since(g, p, mark) + cost > city_points || cost > g.players[p].advanced_start_points {
                    continue;
                }
                let v = crate::production::building_value(g, c, b, 0) as i64;
                if v <= 0 {
                    continue;
                }
                let score = v * 1000 / cost as i64;
                if best.map_or(true, |(_, s)| score > s) {
                    best = Some((b, score));
                }
            }
            if let Some((b, _)) = best {
                let q = g.city_plot(c);
                act(g, p, A::Building, Some(q), b.i());
                any = true;
            }
        }
        if !any {
            break;
        }
    }

    // Units: defenders, workers and field units in turn, while points last (C++ 10 passes).
    let roles = [(Role::Defense, UnitAIType::UNITAI_CITY_DEFENSE), (Role::Worker, UnitAIType::UNITAI_WORKER), (Role::Attack, UnitAIType::UNITAI_RESERVE), (Role::Attack, UnitAIType::UNITAI_ATTACK)];
    let start_area = g.map.plots[start].area;
    'units: for pass in 0..10 {
        for c in g.players[p].cities.clone() {
            let q = g.city_plot(c);
            if pass > 0 && g.map.plots[q].area != start_area {
                continue;
            }
            let (role, unit_ai) = roles[pass % roles.len()];
            // Workers: one per city is plenty.
            if role == Role::Worker && pass >= roles.len() {
                continue;
            }
            let Some(u) = best_unit(g, c, role) else { continue };
            let cost = g.player_get_advanced_start_unit_cost(p, u, true, Some(q));
            if cost < 0 {
                continue;
            }
            if cost > g.players[p].advanced_start_points {
                break 'units;
            }
            place_unit(g, p, q, u, unit_ai);
        }
    }

    crate::trace!(g, p, "advanced start: {} cities, {} units, {} points left", g.players[p].cities.len(), g.players[p].units.len(), g.players[p].advanced_start_points);
    act(g, p, A::Exit, None, -1);
}

fn place_unit(g: &mut Game, p: PlayerId, q: PlotIdx, u: UnitType, unit_ai: UnitAIType) {
    let before = g.players[p].units.len();
    act(g, p, A::Unit, Some(q), u.i());
    if g.players[p].units.len() > before {
        if let Some(&nu) = g.players[p].units.last() {
            if g.units.contains(nu) && g.units[nu].unit_type == u {
                g.unit_ai_set_unit_ai_type(nu, unit_ai);
            }
        }
    }
}

/// Buy the tech research would pick now if it costs at most `budget`; returns the cost.
fn buy_tech(g: &mut Game, p: PlayerId, budget: i32) -> Option<i32> {
    let t = crate::research::choose(g, p)?;
    if g.d.techs[t].repeat {
        return None;
    }
    let cost = g.player_get_advanced_start_tech_cost(p, Some(t), true);
    if cost <= 0 || cost > budget || cost > g.players[p].advanced_start_points {
        return None;
    }
    act(g, p, A::Tech, None, t.i());
    g.team_is_has_tech(g.players[p].team, Some(t)).then_some(cost)
}

/// Best revealed city site within `range` of `start` worth at least `min_value`.
fn best_site(g: &Game, p: PlayerId, start: PlotIdx, range: i32, min_value: i32) -> Option<PlotIdx> {
    let (sx, sy) = g.map.xy(start);
    let mut best: Option<(PlotIdx, i32)> = None;
    for dx in -range..=range {
        for dy in -range..=range {
            let Some(q) = g.plot_xy(sx, sy, dx, dy) else { continue };
            if !g.map.plots[q].revealed[g.players[p].team] || g.player_get_advanced_start_city_cost(p, true, Some(q)) <= 0 {
                continue;
            }
            let (x, y) = g.map.xy(q);
            let v = g.player_ai_found_value(p, x, y, -1, false, &[]);
            if v >= min_value && best.map_or(true, |(_, b)| v > b) {
                best = Some((q, v));
            }
        }
    }
    best.map(|(q, _)| q)
}

/// C++ `AI_advancedStartPlaceCity`: found (or improve) a city, culture for its borders,
/// improvements for the citizens it can keep happy, population to work them.
fn place_city(g: &mut Game, p: PlayerId, q: PlotIdx) {
    if g.map.plots[q].plot_city.is_none() {
        act(g, p, A::City, Some(q), -1);
    }
    let Some(c) = g.map.plots[q].plot_city.filter(|&c| g.cities[c].owner == p) else { return };
    if g.cities[c].culture_level.map_or(true, |l| l.i() <= 1) {
        act(g, p, A::Culture, Some(q), -1);
    }
    let mut improved = (0..NUM_CITY_PLOTS)
        .filter(|&i| i != CITY_HOME_PLOT)
        .filter_map(|i| g.city_index_plot(c, i))
        .filter(|&pq| g.map.plots[pq].working_city == Some(c) && g.map.plots[pq].improvement_type.is_some())
        .count() as i32;
    let target = g.city_happy_level(c) + g.players[p].current_era.i() / 2;
    while improved < target {
        let Some((pq, imp)) = best_improvement(g, p, c) else { break };
        let before = g.map.plots[pq].improvement_type;
        act(g, p, A::Improvement, Some(pq), imp.i());
        if g.map.plots[pq].improvement_type == before {
            break;
        }
        improved += 1;
        if g.cities[c].population < improved {
            act(g, p, A::Pop, Some(q), -1);
        }
    }
    while improved > g.cities[c].population {
        let cost = g.player_get_advanced_start_pop_cost(p, true, Some(c));
        if cost <= 0 || cost > g.players[p].advanced_start_points || g.city_health_rate(c, false, 0) < 0 {
            break;
        }
        act(g, p, A::Pop, Some(q), -1);
    }
    g.city_assign_working_plots(c);
}

/// The most valuable improvement the city could buy on one of its plots.
fn best_improvement(g: &Game, p: PlayerId, c: CityId) -> Option<(PlotIdx, ImprovementType)> {
    let d = g.d;
    let weights = [20, 15, 9];
    let mut best: Option<(PlotIdx, ImprovementType, i32)> = None;
    for i in 0..NUM_CITY_PLOTS {
        if i == CITY_HOME_PLOT {
            continue;
        }
        let Some(q) = g.city_index_plot(c, i) else { continue };
        let plot = &g.map.plots[q];
        if plot.owner != Some(p) || plot.improvement_type.is_some() || g.plot_is_water(q) {
            continue;
        }
        for imp in d.improvements.keys() {
            let cost = g.player_get_advanced_start_improvement_cost(p, Some(imp), true, Some(q));
            if cost <= 0 || cost > g.players[p].advanced_start_points {
                continue;
            }
            let allowed = d.builds.iter_enumerated().any(|(b, info)| info.improvement == Some(imp) && crate::workers::ai_build_allowed(g, p, q, b));
            if !allowed {
                continue;
            }
            let mut v = 0;
            let mut total = 0;
            for y in 0..YieldType::NUM {
                let change = g.plot_calculate_improvement_yield_change(q, imp, YieldType::from_usize(y), Some(p), false, false);
                v += change * weights[y];
                total += plot.yields[y] + change;
            }
            if let Some(bn) = plot.bonus_type {
                if d.improvements[imp].improvement_bonus[bn].bonus_trade {
                    v += 100;
                }
            }
            if v <= 0 || total <= 2 {
                continue;
            }
            if best.map_or(true, |(_, _, b)| v > b) {
                best = Some((q, imp, v));
            }
        }
    }
    best.map(|(q, imp, _)| (q, imp))
}
