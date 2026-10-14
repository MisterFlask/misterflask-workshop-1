//! Candidate city sites for a player (C++ `AI_updateCitySites`, bounded): plots near our
//! cities (or settlers, before the first city) scored once per turn with the ported
//! `AI_foundValue`, then picked greedily with city spacing. The result is a pure function of
//! the game state; the cache only avoids recomputing it within one `update()` call.

use std::collections::BTreeMap;

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::Ai;

/// Maximum distance from an existing city at which sites are considered.
const SITE_RANGE: i32 = 7;
/// Maximum distance from a coastal city of ours at which overseas sites are considered.
const OVERSEAS_RANGE: i32 = 14;
/// Number of sites kept.
const MAX_SITES: usize = 6;

#[derive(Clone, Default)]
pub struct SiteCache {
    /// Game turn slice (`update()` call) the sites were computed in: a cache never outlives
    /// one `update()`, so a game saved between updates and resumed with a fresh controller
    /// recomputes them from the same state and decides identically.
    pub slice: i32,
    pub sites: Vec<(PlotIdx, i32)>,
}

pub type SiteCaches = BTreeMap<PlayerId, SiteCache>;

pub fn sites(ai: &Ai, g: &Game, p: PlayerId) -> Vec<(PlotIdx, i32)> {
    let slice = g.turn_slice;
    if let Some(c) = ai.sites.lock().unwrap().get(&p) {
        if c.slice == slice {
            return c.sites.clone();
        }
    }
    let s = compute(g, p);
    ai.sites.lock().unwrap().insert(p, SiteCache { slice, sites: s.clone() });
    s
}

pub fn num_sites(ai: &Ai, g: &Game, p: PlayerId) -> i32 {
    sites(ai, g, p).len() as i32
}

/// Sites on landmasses where we have no city (reachable only by sea).
pub fn overseas_sites(ai: &Ai, g: &Game, p: PlayerId) -> Vec<(PlotIdx, i32)> {
    let home: Vec<Option<AreaId>> = g.players[p].cities.iter().map(|&c| g.map.plots[g.city_plot(c)].area).collect();
    sites(ai, g, p).into_iter().filter(|&(q, _)| !home.contains(&g.map.plots[q].area)).collect()
}

fn compute(g: &Game, p: PlayerId) -> Vec<(PlotIdx, i32)> {
    let pl = &g.players[p];
    let team = pl.team;
    let mut centers: Vec<(i32, i32, i32)> = pl.cities.iter().map(|&c| (g.cities[c].x, g.cities[c].y, SITE_RANGE)).collect();
    if centers.is_empty() {
        for &u in &pl.units {
            if g.d.units[g.units[u].unit_type].found {
                centers.push((g.units[u].x, g.units[u].y, 3));
            }
        }
    }
    let mut seen = vec![false; g.map.num_plots()];
    let mut scored: Vec<(PlotIdx, i32)> = Vec::new();
    for &(cx, cy, range) in &centers {
        for dx in -range..=range {
            for dy in -range..=range {
                let Some(q) = g.plot_xy(cx, cy, dx, dy) else { continue };
                if seen[q.idx()] {
                    continue;
                }
                seen[q.idx()] = true;
                let plot = &g.map.plots[q];
                if !plot.revealed[team] || plot.plot_type == PlotType::PLOT_OCEAN || plot.owner.is_some_and(|o| o != p) {
                    continue;
                }
                let (x, y) = g.map.xy(q);
                let v = g.player_ai_found_value(p, x, y, -1, false, &[]);
                if v > 0 {
                    scored.push((q, v));
                }
            }
        }
    }
    let min_value = if pl.cities.is_empty() { 1 } else { g.player_ai_min_found_value(p) };
    // No site left on our landmasses: look across the water (coastal plots of other
    // landmasses within OVERSEAS_RANGE of our coastal cities), for settlers carried by ship.
    let home_areas: Vec<Option<AreaId>> = pl.cities.iter().map(|&c| g.map.plots[g.city_plot(c)].area).collect();
    let home_left = scored.iter().any(|&(q, v)| v >= min_value && home_areas.contains(&g.map.plots[q].area));
    if !home_left && !pl.cities.is_empty() {
        let min_water = g.d.defines.min_water_size_for_ocean;
        for &c in &pl.cities {
            if !g.city_is_coastal(c, min_water) {
                continue;
            }
            let (cx, cy) = (g.cities[c].x, g.cities[c].y);
            for dx in -OVERSEAS_RANGE..=OVERSEAS_RANGE {
                for dy in -OVERSEAS_RANGE..=OVERSEAS_RANGE {
                    let Some(q) = g.plot_xy(cx, cy, dx, dy) else { continue };
                    if seen[q.idx()] {
                        continue;
                    }
                    seen[q.idx()] = true;
                    let plot = &g.map.plots[q];
                    if !plot.revealed[team] || plot.owner.is_some() || home_areas.contains(&plot.area) || !g.plot_is_coastal_land(q, min_water) {
                        continue;
                    }
                    let (x, y) = g.map.xy(q);
                    let v = g.player_ai_found_value(p, x, y, -1, false, &[]);
                    if v > 0 {
                        scored.push((q, v));
                    }
                }
            }
        }
    }
    // Best first; ties by plot index for determinism.
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let min_spacing = g.d.defines.min_city_range.max(2) + 1;
    let mut out: Vec<(PlotIdx, i32)> = Vec::new();
    for (q, v) in scored {
        if v < min_value {
            break;
        }
        let (x, y) = g.map.xy(q);
        if out.iter().any(|&(o, _)| {
            let (ox, oy) = g.map.xy(o);
            g.plot_distance(x, y, ox, oy) < min_spacing
        }) {
            continue;
        }
        out.push((q, v));
        if out.len() >= MAX_SITES {
            break;
        }
    }
    out
}
