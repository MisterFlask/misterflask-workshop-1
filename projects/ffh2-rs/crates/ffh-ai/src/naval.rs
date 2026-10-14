//! Naval play: settling other landmasses by ship, sea exploration.
//!
//! When no city site is left on our landmasses (`sites.rs` then also scores coastal plots of
//! other landmasses), a coastal city trains a settler and a transport (C++ `UNITAI_SETTLER_SEA`).
//! The settler walks to a coastal city and boards a transport there (`loadUnit`, with its
//! escort when there is room); the transport sails next to the best overseas site it can
//! reach and the cargo lands on it (a move from the ship onto the plot, as C++
//! `groupAmphibMove` orders); the settler founds the city on its next turn. An empty
//! transport sails to a coastal city where a settler waits, else waits in port.
//!
//! A sea explorer (`UNITAI_EXPLORE_SEA`) reveals coasts while our landmass is full and nothing
//! overseas is known yet.

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::sites;
use crate::units::{alive, hold, move_to, nearest_city, skip};
use crate::view::View;
use crate::Ai;

/// Can unit type `info` carry land units (a transport)?
pub fn is_transport_type(info: &UnitInfo) -> bool {
    info.domain_type == Some(DomainType::DOMAIN_SEA) && info.cargo_space > 0 && info.domain_cargo == Some(DomainType::DOMAIN_LAND) && info.special_cargo.is_none()
}

/// Our transports (alive), and transports being trained.
pub fn num_transports(g: &Game, p: PlayerId) -> i32 {
    let alive = g.players[p].units.iter().filter(|&&u| is_transport_type(&g.d.units[g.units[u].unit_type]) && g.units[u].ai_type != Some(UnitAIType::UNITAI_EXPLORE_SEA)).count() as i32;
    alive + g.players[p].ai_num_train_ai_units[UnitAIType::UNITAI_SETTLER_SEA] + g.players[p].ai_num_train_ai_units[UnitAIType::UNITAI_ASSAULT_SEA]
}

pub fn is_coastal_city(g: &Game, c: CityId) -> bool {
    g.city_is_coastal(c, g.d.defines.min_water_size_for_ocean)
}

/// A settler of ours waiting for a ship: on a coastal city plot, not loaded.
pub fn settler_waiting(g: &Game, p: PlayerId) -> Option<PlotIdx> {
    g.players[p].units.iter().copied().find_map(|u| {
        let unit = &g.units[u];
        if !g.d.units[unit.unit_type].found || g.unit_is_cargo(u) {
            return None;
        }
        let q = g.unit_plot(u);
        g.map.plots[q].plot_city.filter(|&c| g.cities[c].owner == p && is_coastal_city(g, c)).map(|_| q)
    })
}

/// Is our home land full while there is unknown or unsettled land across the water?
/// (Research and production use it to want ships.)
pub fn home_is_full(g: &Game, p: PlayerId) -> bool {
    let pl = &g.players[p];
    let Some(cap) = pl.capital_city.filter(|&c| g.cities.contains(c)) else { return false };
    let Some(area) = g.map.plots[g.city_plot(cap)].area else { return false };
    let a = &g.map.areas[area];
    let unowned = a.num_tiles - a.num_owned_tiles;
    unowned * 100 < a.num_tiles * 25 || unowned < 20
}

/// A settler with no site it can walk to, while overseas sites exist: walk to a coastal city
/// and board a ship there. False if there is nothing overseas to settle.
pub fn settler_overseas(ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
    let p = v.p;
    if sites::overseas_sites(ai, g, p).is_empty() {
        return false;
    }
    let here = g.unit_plot(u);
    let in_port = g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == p && is_coastal_city(g, c));
    if in_port {
        let ship = g.map.plots[here].units.iter().copied().find(|&t| g.units[t].owner == p && is_transport_type(&g.d.units[g.units[t].unit_type]) && g.unit_can_load_unit(u, t, here));
        if let Some(t) = ship {
            crate::trace!(g, p, "settler {:?} boards {:?} at {:?}", u, t, g.map.xy(here));
            let others: Vec<UnitId> = g.groups[grp].units.iter().copied().filter(|&o| o != u).collect();
            g.unit_load_unit(u, t);
            // The escort comes along when there is room.
            for o in others {
                if g.units.contains(o) && g.unit_can_load_unit(o, t, here) {
                    g.unit_load_unit(o, t);
                }
            }
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                skip(g, grp);
            }
            return true;
        }
        crate::trace!(g, p, "settler {:?} waits for a ship at {:?}", u, g.map.xy(here));
        skip(g, grp);
        return true;
    }
    // Walk to the nearest coastal city.
    let reach = v.flood.run(g, u, 12, false);
    let port = g.players[p]
        .cities
        .iter()
        .copied()
        .filter(|&c| is_coastal_city(g, c))
        .filter_map(|c| reach.iter().find(|r| r.plot == g.city_plot(c)).map(|r| (r.turns, c)))
        .min();
    match port {
        Some((_, c)) => {
            move_to(g, grp, g.city_plot(c), MOVE_SAFE_TERRITORY);
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                skip(g, grp);
            }
        }
        None => {
            crate::trace!(g, p, "settler {:?} at {:?}: overseas sites but no port in reach", u, g.map.xy(here));
            return false;
        }
    }
    true
}

/// One decision for a transport led by `u`.
pub fn transport(ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let p = v.p;
    let here = g.unit_plot(u);
    let cargo = g.unit_get_cargo_units(u);
    let carries_settler = cargo.iter().any(|&c| g.d.units[g.units[c].unit_type].found);
    crate::trace!(g, p, "transport {:?} at {:?}: cargo {} (settler {}), moves {}", u, g.map.xy(here), cargo.len(), carries_settler, g.unit_moves_left(u));
    if carries_settler {
        // The best overseas site we can sail next to.
        let reach = v.flood.run(g, u, 25, false);
        let mut best: Option<(PlotIdx, PlotIdx, i64)> = None;
        for (site, value) in sites::sites(ai, g, p) {
            if v.is_claimed(site) {
                continue;
            }
            let (sx, sy) = g.map.xy(site);
            if g.plot_distance(sx, sy, g.map.xy(here).0, g.map.xy(here).1) == 1 {
                best = Some((site, here, i64::MAX));
                break;
            }
            let Some(r) = reach
                .iter()
                .filter(|r| g.plot_is_water(r.plot) && g.map.plots[r.plot].revealed[v.team] && {
                    let (rx, ry) = g.map.xy(r.plot);
                    g.plot_distance(rx, ry, sx, sy) == 1
                })
                .min_by_key(|r| (r.turns, r.plot))
            else {
                continue;
            };
            let score = value as i64 * 10 / (r.turns as i64 + 2);
            if best.map_or(true, |(_, _, s)| score > s) {
                best = Some((site, r.plot, score));
            }
        }
        if let Some((site, water, _)) = best {
            v.claimed.push(site);
            if water == here {
                // Land: settler first, then the escort.
                crate::trace!(g, p, "transport {:?} lands its cargo on {:?}", u, g.map.xy(site));
                let mut cargo = cargo;
                cargo.sort_by_key(|&c| (!g.d.units[g.units[c].unit_type].found, c));
                for c in cargo {
                    if !g.units.contains(c) {
                        continue;
                    }
                    let Some(cg) = g.units[c].group else { continue };
                    g.group_clear_mission_queue(cg);
                    g.group_set_activity_type(cg, ActivityType::ACTIVITY_AWAKE);
                    move_to(g, cg, site, MOVE_IGNORE_DANGER);
                }
                if alive(g, grp) && g.group_ready_to_move(grp, false) {
                    skip(g, grp);
                }
                return;
            }
            crate::trace!(g, p, "transport {:?} sails toward {:?}", u, g.map.xy(site));
            move_to(g, grp, water, 0);
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                skip(g, grp);
            }
            return;
        }
    }
    if !cargo.is_empty() && !carries_settler {
        // Cargo we have no plan for: back to port, unload there.
        if g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == p) {
            g.unit_unload_all(u);
            hold(g, grp);
            return;
        }
    }
    if cargo.is_empty() {
        // Pick up a waiting settler.
        if let Some(q) = settler_waiting(g, p) {
            if q == here {
                skip(g, grp);
            } else {
                move_to(g, grp, q, 0);
                if alive(g, grp) && g.group_ready_to_move(grp, false) {
                    skip(g, grp);
                }
            }
            return;
        }
    }
    // Wait in port.
    port(g, p, grp, here);
}

fn port(g: &mut Game, p: PlayerId, grp: GroupId, here: PlotIdx) {
    if g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == p) {
        hold(g, grp);
        return;
    }
    let (x, y) = g.map.xy(here);
    let target = g.players[p].cities.iter().copied().filter(|&c| is_coastal_city(g, c)).min_by_key(|&c| (g.plot_distance(x, y, g.cities[c].x, g.cities[c].y), c)).or_else(|| nearest_city(g, p, here));
    match target {
        Some(c) => {
            move_to(g, grp, g.city_plot(c), 0);
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                skip(g, grp);
            }
        }
        None => skip(g, grp),
    }
}

/// A sea explorer: the reachable water plot that reveals the most, else it becomes a
/// transport (if it can carry) or waits in port.
pub fn sea_explore(ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let team = v.team;
    let reach = v.flood.run(g, u, 10, false);
    let mut best: Option<(PlotIdx, i32)> = None;
    for r in &reach {
        if r.turns == 0 || !g.plot_is_water(r.plot) || !g.map.plots[r.plot].revealed[team] || v.is_claimed(r.plot) || v.danger_at(r.plot) > 0 {
            continue;
        }
        let (x, y) = g.map.xy(r.plot);
        let mut unrevealed = 0;
        for dx in -2..=2 {
            for dy in -2..=2 {
                if let Some(q) = g.plot_xy(x, y, dx, dy) {
                    if !g.map.plots[q].revealed[team] {
                        unrevealed += 1;
                    }
                }
            }
        }
        if unrevealed == 0 {
            continue;
        }
        let score = unrevealed * 100 / (r.turns + 1);
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((r.plot, score));
        }
    }
    if let Some((q, _)) = best {
        v.claimed.push(q);
        move_to(g, grp, q, 0);
        if alive(g, grp) && g.group_ready_to_move(grp, false) {
            skip(g, grp);
        }
        return;
    }
    if is_transport_type(&g.d.units[g.units[u].unit_type]) {
        g.unit_ai_set_unit_ai_type(u, UnitAIType::UNITAI_SETTLER_SEA);
        transport(ai, g, v, grp, u);
        return;
    }
    let here = g.unit_plot(u);
    port(g, v.p, grp, here);
}
