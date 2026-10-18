//! Workers (C++ `CvUnitAI::AI_workerMove` / `AI_bestPlotBuild`, new valuation): improve the
//! plots our cities work, connect resources, and build roads between our cities.
//!
//! A build's value is the yield change of its improvement weighted by what the working city
//! needs (food while it can grow, production, commerce; improvements that grow, like cottages,
//! count their grown yield in part), plus the value of a resource it connects; plots being
//! worked now count fully, others at a discount. Roads come from a per-turn plan
//! ([`road_plan`]): the next missing road plot between two unconnected cities of ours, and
//! plots that would link an improved resource to its city. FfH's AI-only build bans
//! (`CvGameUtils.canBuild`) are respected: no forts, no farms for Fallow civs, no workshops
//! before Guilds (unless Fallow), no cottages under Aristocracy, no windmills under Arete.

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::units::{alive, hold, move_to, nearest_city, push, skip};
use crate::view::View;

/// FfH `CvGameUtils.canBuild` for AI players: false = the AI must not use this build here.
pub fn ai_build_allowed(g: &Game, p: PlayerId, plot: PlotIdx, build: BuildType) -> bool {
    let d = g.d;
    let name = d.builds[build].base.type_.as_str();
    let pl = &g.players[p];
    let civic = |opt: &str, civ: &str| -> bool {
        match (d.ty::<CivicOptionType>(opt), d.ty::<CivicType>(civ)) {
            (Some(o), Some(c)) => pl.civics[o] == Some(c),
            _ => false,
        }
    };
    let fallow = d.ty::<TraitType>("TRAIT_FALLOW").is_some_and(|t| pl.traits[t]);
    match name {
        "BUILD_FORT" => false,
        "BUILD_WINDMILL" => !civic("CIVICOPTION_LABOR", "CIVIC_ARETE"),
        "BUILD_WORKSHOP" => fallow || d.ty::<TechType>("TECH_GUILDS").is_some_and(|t| g.team_is_has_tech(pl.team, Some(t))),
        "BUILD_COTTAGE" => !civic("CIVICOPTION_GOVERNMENT", "CIVIC_ARISTOCRACY"),
        "BUILD_FARM" => !fallow,
        "BUILD_ROAD" => !g.plot_is_water(plot),
        _ => true,
    }
}

/// Roads worth building this turn: (plot, value).
pub fn road_plan(g: &Game, p: PlayerId) -> Vec<(PlotIdx, i32)> {
    let d = g.d;
    let team = g.players[p].team;
    let can_road = d.builds.iter().any(|b| b.route.is_some() && b.improvement.is_none() && b.tech_prereq.map_or(true, |t| g.team_is_has_tech(team, Some(t))));
    if !can_road {
        return Vec::new();
    }
    let cities = &g.players[p].cities;
    let mut out: Vec<(PlotIdx, i32)> = Vec::new();
    let add = |out: &mut Vec<(PlotIdx, i32)>, q: PlotIdx, v: i32| {
        if let Some(e) = out.iter_mut().find(|(o, _)| *o == q) {
            e.1 = e.1.max(v);
        } else {
            out.push((q, v));
        }
    };
    // Greedy step from `from` toward `to` along land we may build on: the first plot without a
    // route.
    let next_road = |from: PlotIdx, to: PlotIdx| -> Option<PlotIdx> {
        let (tx, ty) = g.map.xy(to);
        let mut cur = from;
        for _ in 0..12 {
            let (x, y) = g.map.xy(cur);
            let mut best: Option<(PlotIdx, i32)> = None;
            for dir in 0..DirectionType::NUM {
                let Some(q) = g.plot_direction(x, y, DirectionType::from_usize(dir)) else { continue };
                if g.plot_is_water(q) || g.plot_is_impassable(q) {
                    continue;
                }
                let owner = g.map.plots[q].owner;
                if owner.is_some_and(|o| o != p) {
                    continue;
                }
                let (qx, qy) = g.map.xy(q);
                let dist = g.plot_distance(qx, qy, tx, ty) * 10 - i32::from(g.map.plots[q].route_type.is_some()) * 3;
                if best.map_or(true, |(bq, bd)| dist < bd || dist == bd && q < bq) {
                    best = Some((q, dist));
                }
            }
            let (q, _) = best?;
            if q == to {
                return None;
            }
            if g.map.plots[q].route_type.is_none() {
                return Some(q);
            }
            cur = q;
        }
        None
    };
    for (i, &a) in cities.iter().enumerate() {
        let pa = g.city_plot(a);
        for &b in cities.iter().skip(i + 1) {
            let pb = g.city_plot(b);
            if g.map.plots[pa].area != g.map.plots[pb].area {
                continue;
            }
            let (ax, ay) = g.map.xy(pa);
            let (bx, by) = g.map.xy(pb);
            if g.plot_distance(ax, ay, bx, by) > 8 || g.city_is_connected_to(a, b) {
                continue;
            }
            let v = 60 + 8 * (g.cities[a].population + g.cities[b].population).min(20);
            if let Some(q) = next_road(pa, pb) {
                add(&mut out, q, v);
            }
            if let Some(q) = next_road(pb, pa) {
                add(&mut out, q, v);
            }
        }
        // Improved resources not yet linked to this city.
        for k in 0..NUM_CITY_PLOTS {
            if k == CITY_HOME_PLOT {
                continue;
            }
            let Some(q) = g.city_index_plot(a, k) else { continue };
            let plot = &g.map.plots[q];
            if plot.owner != Some(p) || plot.working_city != Some(a) {
                continue;
            }
            let (Some(bonus), Some(imp)) = (plot.bonus_type, plot.improvement_type) else { continue };
            if !d.improvements[imp].improvement_bonus[bonus].bonus_trade || g.plot_is_connected_to(q, a) {
                continue;
            }
            if plot.route_type.is_none() && !g.plot_is_water(q) {
                add(&mut out, q, 90);
            } else if let Some(r) = next_road(q, pa) {
                add(&mut out, r, 90);
            }
        }
    }
    out
}

/// Value of performing `build` on `plot` for player `p` (0 = not worth it).
fn build_value(g: &Game, v: &View, p: PlayerId, plot: PlotIdx, build: BuildType) -> i32 {
    let d = g.d;
    let info = &d.builds[build];
    let pl = &g.map.plots[plot];
    if let Some(imp) = info.improvement {
        if pl.improvement_type == Some(imp) {
            return 0;
        }
        // What the working city needs.
        let city = pl.working_city.filter(|&c| g.cities[c].owner == p);
        let (food_w, prod_w) = match city {
            Some(c) => {
                let surplus = g.city_yield_rate(c, YieldType::YIELD_FOOD) - g.city_food_consumption(c, false, 0);
                let happy_room = g.city_happy_level(c) - g.city_unhappy_level(c, 0);
                let food_w = if surplus < 2 && happy_room > 0 { 30 } else if happy_room > 0 { 22 } else { 12 };
                (food_w, 18)
            }
            None => (20, 18),
        };
        // Improvements that grow (cottages) count their grown form for two thirds.
        let mut last = imp;
        for _ in 0..4 {
            match d.improvements[last].improvement_upgrade {
                Some(next) if next != imp => last = next,
                _ => break,
            }
        }
        // Commerce matters more to an empire that cannot afford its research.
        let research = crate::economy::spend_pct(g, p);
        let commerce_w = if research < 50 { 11 * (200 - 2 * research) / 100 } else { 11 };
        let weights = [food_w, prod_w, commerce_w];
        let mut value = 0;
        let mut new_total = 0;
        for y in 0..YieldType::NUM {
            let yt = YieldType::from_usize(y);
            let mut new = g.plot_calculate_improvement_yield_change(plot, imp, yt, Some(p), false, false);
            if last != imp {
                let grown = g.plot_calculate_improvement_yield_change(plot, last, yt, Some(p), false, false);
                new = (new + 2 * grown) / 3;
            }
            let old = pl.improvement_type.map_or(0, |o| g.plot_calculate_improvement_yield_change(plot, o, yt, Some(p), false, false));
            value += (new - old) * weights[y];
            new_total += pl.yields[y] - old + new;
        }
        // A plot nobody would work (total yield 2 or less) is not worth improving.
        if new_total <= 2 && pl.bonus_type.is_none() {
            return 0;
        }
        if let Some(b) = pl.bonus_type {
            let revealed = d.bonuses[b].tech_reveal.map_or(true, |t| g.team_is_has_tech(v.team, Some(t)));
            let traded_now = pl.improvement_type.is_some_and(|o| d.improvements[o].improvement_bonus[b].bonus_trade);
            if revealed && d.improvements[imp].improvement_bonus[b].bonus_trade && !traded_now {
                value += 100 + g.player_ai_bonus_val(p, b, 1).min(400) / 2;
            } else if revealed && traded_now {
                // Never replace the improvement that connects a resource with one that does not.
                return 0;
            }
        }
        if value <= 0 {
            return 0;
        }
        let worked = g.plot_is_being_worked(plot);
        if !worked {
            value = value * 6 / 10;
        }
        if pl.improvement_type.is_some() && value < 60 {
            return 0; // do not churn improvements
        }
        value
    } else if info.route.is_some() {
        if pl.route_type.is_some() {
            return 0;
        }
        v.roads.iter().find(|(q, _)| *q == plot).map_or(0, |&(_, val)| val)
    } else {
        0
    }
}

/// Is a worker on this plot in danger (enemy units can reach it next turn)?
pub fn worker_in_danger(g: &Game, v: &View, plot: PlotIdx) -> bool {
    v.danger_at(plot) > 0 && g.map.plots[plot].plot_city.is_none()
}

/// Where a threatened worker goes this turn: a city of ours it can reach, else a plot no
/// visible enemy can reach, else one our soldiers hold, else the least dangerous; nearer our
/// nearest city first. None when we have no city.
fn refuge(g: &Game, v: &mut View, u: UnitId) -> Option<PlotIdx> {
    let p = v.p;
    let here = g.unit_plot(u);
    let city = nearest_city(g, p, here)?;
    let (cx, cy) = (g.cities[city].x, g.cities[city].y);
    let reach = v.flood.run(g, u, 0, false);
    reach
        .iter()
        .map(|r| r.plot)
        .filter(|&q| g.map.plots[q].owner.map_or(true, |o| o == p))
        .min_by_key(|&q| {
            let plot = &g.map.plots[q];
            let ours = plot.plot_city.is_some_and(|c| g.cities[c].owner == p);
            let danger = v.danger_at(q);
            let held = plot.units.iter().any(|&o| g.units[o].owner == p && g.d.units[g.units[o].unit_type].combat > 0);
            let rank = if ours { 0 } else if danger == 0 { 1 } else if held { 2 } else { 3 };
            let (x, y) = g.map.xy(q);
            (rank, if rank == 3 { danger } else { 0 }, g.plot_distance(x, y, cx, cy), q)
        })
}

pub fn worker(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId, sea: bool) {
    worker_with(g, v, grp, u, sea, &|_, _, _| true);
}

/// [`worker`] considering only the builds `allow` accepts on a plot (automated workers of
/// human players: network only, one city, safe automation).
pub fn worker_with(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId, sea: bool, allow: &dyn Fn(&Game, PlotIdx, BuildType) -> bool) {
    let p = v.p;
    let here = g.unit_plot(u);
    // Flee danger first.
    if !sea && worker_in_danger(g, v, here) {
        crate::trace!(g, p, "worker {:?} at {:?} flees", u, g.map.xy(here));
        if let Some(q) = refuge(g, v, u) {
            if q != here {
                // Our own danger map chose the plot: the rules' AI path finder would refuse a
                // path for a non-combat group that starts or ends next to the enemy (pathValid's
                // AI_getPlotDanger), and the worker would stay where it is to be captured.
                move_to(g, grp, q, MOVE_IGNORE_DANGER);
            }
            if alive(g, grp) && g.group_ready_to_move(grp, true) {
                skip(g, grp);
            }
            return;
        }
    }
    if sea && crate::civs::sea_worker(g, v, grp, u) { return; } // civ-mechanics
    let ut = g.units[u].unit_type;
    let work_rate = g.d.units[ut].work_rate.max(1);
    let reach = v.flood.run(g, u, 8, false);
    let builds: Vec<BuildType> = g.d.builds.keys().filter(|&b| g.d.units[ut].builds[b]).collect();
    let mut best: Option<(PlotIdx, BuildType, i64)> = None;
    for r in &reach {
        let q = r.plot;
        let pl = &g.map.plots[q];
        if pl.owner != Some(p) || v.danger_at(q) > 0 || v.is_claimed(q) {
            continue;
        }
        if sea != g.plot_is_water(q) {
            continue;
        }
        let in_radius = pl.working_city.is_some_and(|c| g.cities[c].owner == p);
        let road = v.roads.iter().any(|(rq, _)| *rq == q);
        if !in_radius && !road {
            continue;
        }
        for &b in &builds {
            if !ai_build_allowed(g, p, q, b) || !allow(g, q, b) || !g.unit_can_build(u, q, b, false) {
                continue;
            }
            let value = build_value(g, v, p, q, b);
            if value <= 0 {
                continue;
            }
            let time = g.d.builds[b].time.max(1) / work_rate;
            let score = value as i64 * 100 / (r.turns as i64 * 2 + time as i64 + 3);
            if best.map_or(true, |(_, _, s)| score > s) {
                best = Some((q, b, score));
            }
        }
    }
    let Some((plot, build, _)) = best else {
        crate::trace!(g, p, "worker {:?} at {:?}: nothing to build ({} plots in reach)", u, g.map.xy(here), reach.len());
        // Nothing to do: wait in a city.
        if g.map.plots[here].plot_city.is_none() {
            if let Some(c) = nearest_city(g, p, here) {
                move_to(g, grp, g.city_plot(c), 0);
            }
        }
        if alive(g, grp) && g.group_ready_to_move(grp, true) {
            hold(g, grp);
        }
        return;
    };
    v.claimed.push(plot);
    crate::trace!(g, p, "worker {:?} at {:?}: {} at {:?}", u, g.map.xy(here), g.d.builds[build].base.type_, g.map.xy(plot));
    if plot != here {
        move_to(g, grp, plot, MOVE_SAFE_TERRITORY);
        if !alive(g, grp) {
            return;
        }
        push(g, grp, MissionType::MISSION_BUILD, build.i(), -1, 0, true);
    } else {
        push(g, grp, MissionType::MISSION_BUILD, build.i(), -1, 0, false);
    }
    if alive(g, grp) && g.group_ready_to_move(grp, true) {
        skip(g, grp);
    }
}
