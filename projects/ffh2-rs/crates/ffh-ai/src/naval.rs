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

use crate::production::{best_unit, Choice, Role};
use crate::sites;
use crate::units::{alive, hold, move_to, nearest_city, skip};
use crate::view::View;
use crate::Ai;

/// Maximum plot distance from one of our coastal cities at which an overseas enemy city is
/// considered a sea-assault target (the transport's own flood decides the real path).
const SEA_RANGE: i32 = 24;

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

// ----------------------------------------------------------------------------
// Amphibious invasion targets
// ----------------------------------------------------------------------------

/// An overseas enemy (or barbarian) city we can ship an army to and take: it sits on a
/// landmass where we have no city, is within sea-reach of one of our coastal cities, and its
/// weakest defender our best trainable attacker can beat. A pure function of the game state
/// (no cache, no RNG, deterministic ordering).
#[derive(Clone, Debug)]
pub struct Amph {
    /// The enemy city to take and its plot.
    pub city: CityId,
    pub plot: PlotIdx,
    /// Its owner (a civ or the barbarians).
    pub owner: PlayerId,
    /// Our coastal city the assault launches from (the nearest to the target) and its plot.
    pub staging: CityId,
    pub staging_plot: PlotIdx,
    /// Attackers the city wants (rough estimate from what we see of its garrison).
    pub need: i32,
    /// What taking it is worth, and the value-for-cost score (closest/most-valuable first).
    pub value: i64,
    pub dist: i32,
    pub score: i64,
    /// Our army units already on its landmass (a landing in progress): keeps the invasion on
    /// one target instead of flipping between overseas cities each turn.
    pub landed: i32,
}

/// Our army units (able to attack, on land) within two plots of city plot `cp`: a landing in
/// progress against that specific city.
fn army_near_city(g: &Game, p: PlayerId, cp: PlotIdx) -> i32 {
    let (cx, cy) = g.map.xy(cp);
    g.players[p]
        .units
        .iter()
        .filter(|&&u| {
            if !g.units.contains(u) || g.unit_is_cargo(u) || !crate::military::is_army_unit(g, u) {
                return false;
            }
            let q = g.unit_plot(u);
            let (x, y) = g.map.xy(q);
            g.map.plots[q].area == g.map.plots[cp].area && g.plot_distance(x, y, cx, cy) <= 2
        })
        .count() as i32
}

/// Transports of ours dedicated to the assault (`UNITAI_ASSAULT_SEA`), alive or in training.
fn assault_ships(g: &Game, p: PlayerId) -> i32 {
    let alive = g.players[p].units.iter().filter(|&&u| is_transport_type(&g.d.units[g.units[u].unit_type]) && g.units[u].ai_type == Some(UnitAIType::UNITAI_ASSAULT_SEA)).count() as i32;
    alive + g.players[p].ai_num_train_ai_units[UnitAIType::UNITAI_ASSAULT_SEA]
}

/// What taking city `c` is worth (population, wonders -- no victory-plan coupling here).
fn amph_value(g: &Game, c: CityId) -> i64 {
    let city = &g.cities[c];
    400 + city.population as i64 * 100 + city.num_world_wonders as i64 * 300
}

/// Attackers the overseas city `c` wants: twice what we see of its garrison, plus one (the
/// same rough estimate the siege campaigns use; an unseen garrison is assumed by age/population).
fn amph_need(g: &Game, p: PlayerId, c: CityId) -> i32 {
    let team = g.players[p].team;
    let cp = g.city_plot(c);
    let n_def = if g.plot_is_visible(cp, team) {
        crate::campaign::visible_defenders(g, team, cp).len() as i32
    } else if g.player_is_barbarian(g.cities[c].owner) {
        (1 + (g.game_turn() - g.cities[c].game_turn_acquired) / 25).clamp(1, 6)
    } else {
        (2 + g.cities[c].population / 5).min(5)
    };
    crate::campaign::rough_need(n_def)
}

/// The best overseas city of `other` we can ship an army to and take (used both when choosing a
/// war to start and, once at war, to run the assault). Independent of war status so the war
/// trigger can ask "is there a sea-reachable city of theirs we could take?".
pub fn amphibious_city(g: &Game, p: PlayerId, other: PlayerId) -> Option<Amph> {
    if other == p || !g.players[other].alive || g.players[other].cities.is_empty() {
        return None;
    }
    let team = g.players[p].team;
    // Our coastal cities (launch points) and the landmasses we already hold a city on.
    let ports: Vec<CityId> = g.players[p].cities.iter().copied().filter(|&c| is_coastal_city(g, c)).collect();
    if ports.is_empty() {
        return None;
    }
    // We must be able to carry an army across (a transport now, or one we can build): no point
    // declaring or preparing an amphibious war we have no ships for (good transports come late).
    if num_transports(g, p) == 0 && !ports.iter().any(|&c| best_unit(g, c, Role::Transport).is_some()) {
        return None;
    }
    let our_areas: Vec<Option<AreaId>> = g.players[p].cities.iter().map(|&c| g.map.plots[g.city_plot(c)].area).collect();
    let mut best: Option<Amph> = None;
    for &c in &g.players[other].cities {
        let cp = g.city_plot(c);
        if !g.map.plots[cp].revealed[team] {
            continue;
        }
        // A landmass where we have no city, reachable only by sea, and coastal (so a transport
        // can land the army next to it).
        if our_areas.contains(&g.map.plots[cp].area) || !g.plot_is_adjacent_to_water(cp) {
            continue;
        }
        let (cx, cy) = (g.cities[c].x, g.cities[c].y);
        let Some((dist, staging)) = ports.iter().map(|&o| (g.plot_distance(cx, cy, g.cities[o].x, g.cities[o].y), o)).min() else { continue };
        if dist > SEA_RANGE {
            continue;
        }
        // Our best trainable attacker must be able to beat its weakest defender.
        if !crate::diplomacy::can_take_city(g, p, c) {
            continue;
        }
        let need = amph_need(g, p, c);
        let value = amph_value(g, c);
        let score = value * 1000 / (need as i64 * 100 + dist as i64 * 25 + 200);
        let landed = army_near_city(g, p, cp);
        // A city we are already landing near outranks a higher-scoring one we have not reached:
        // the invasion stays on one target instead of ferrying troops to a new coast each turn.
        let key = (landed > 0, score, std::cmp::Reverse(c));
        let better = best.as_ref().map_or(true, |b| key > (b.landed > 0, b.score, std::cmp::Reverse(b.city)));
        if better {
            best = Some(Amph { city: c, plot: cp, owner: other, staging, staging_plot: g.city_plot(staging), need, value, dist, score, landed });
        }
    }
    best
}

/// The best amphibious target for player `p` among the civs (and barbarians) we are at war with
/// or preparing war against. Recomputed each turn from the game state.
pub fn amphibious_target(g: &Game, p: PlayerId) -> Option<Amph> {
    if g.player_is_barbarian(p) {
        return None;
    }
    let team = g.players[p].team;
    let barb = g.barbarian_team();
    let preparing = crate::diplomacy::preparing_target(g, team);
    let mut best: Option<Amph> = None;
    for other in g.player_ids() {
        if other == p || !g.players[other].alive {
            continue;
        }
        let ot = g.players[other].team;
        if ot == team {
            continue;
        }
        // Barbarians only when we are at war with them; civs when at war or preparing one.
        let enemy = if g.player_is_barbarian(other) { g.at_war(team, barb) } else { g.at_war(team, ot) || preparing == Some(ot) };
        if !enemy {
            continue;
        }
        let Some(amph) = amphibious_city(g, p, other) else { continue };
        let key = (amph.landed > 0, amph.score, std::cmp::Reverse(amph.city));
        let better = best.as_ref().map_or(true, |b| key > (b.landed > 0, b.score, std::cmp::Reverse(b.city)));
        if better {
            best = Some(amph);
        }
    }
    best
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

/// A settler of ours whose last decision was to wait in port for a ship (no site it can walk
/// to, `settler_overseas`), not one that only stands in a coastal city (waiting for an escort).
pub fn settler_waits_for_ship(g: &Game, p: PlayerId) -> bool {
    g.players[p].units.iter().any(|&u| {
        g.d.units[g.units[u].unit_type].found
            && !g.unit_is_cargo(u)
            && g.units[u].group.is_some_and(|grp| g.groups[grp].ai_mission_ai_type == Some(MissionAIType::MISSIONAI_LOAD_SETTLER))
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
        // The turn is skipped as a settler waiting for a ship (C++ `MISSIONAI_LOAD_SETTLER`,
        // which research reads: `settler_waits_for_ship`).
        let load = Some(MissionAIType::MISSIONAI_LOAD_SETTLER);
        g.group_push_mission(grp, MissionType::MISSION_SKIP, -1, -1, 0, false, false, load, Some(here), None);
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

// ----------------------------------------------------------------------------
// Amphibious invasion: army side (one decision, from `military::army`)
// ----------------------------------------------------------------------------

/// End the group's turn if it is still waiting for orders.
fn finish(g: &mut Game, grp: GroupId) {
    if alive(g, grp) && g.group_ready_to_move(grp, true) {
        skip(g, grp);
    }
}

/// Is this army group one of the stacks committed to the amphibious assault? The army units on
/// the staging landmass, nearest the staging port first, are committed up to the force the
/// target wants (so the whole army is not shipped off for one small city). Deterministic.
fn committed(g: &Game, p: PlayerId, amph: &Amph, grp: GroupId) -> bool {
    let staging_area = g.map.plots[amph.staging_plot].area;
    let (sx, sy) = g.map.xy(amph.staging_plot);
    // Army groups on the staging landmass, not already aboard a ship.
    let mut groups: Vec<(i32, GroupId, i32)> = Vec::new(); // (dist to port, group, unit count)
    for &gr in &g.players[p].groups {
        let Some(head) = g.group_get_head_unit(gr) else { continue };
        if g.unit_is_cargo(head) || !crate::military::is_army_unit(g, head) {
            continue;
        }
        let hp = g.unit_plot(head);
        if g.map.plots[hp].area != staging_area {
            continue;
        }
        let (hx, hy) = g.map.xy(hp);
        groups.push((g.plot_distance(hx, hy, sx, sy), gr, g.groups[gr].units.len() as i32));
    }
    groups.sort_by_key(|&(d, gr, _)| (d, gr));
    let mut n = 0;
    for &(_, gr, count) in &groups {
        if n >= amph.need {
            break;
        }
        if gr == grp {
            return true;
        }
        n += count;
    }
    false
}

/// A committed army group boards the transport waiting in the staging port (the whole stack,
/// like a settler's escort), else waits for one to be built or to arrive.
fn board(g: &mut Game, p: PlayerId, grp: GroupId, u: UnitId, here: PlotIdx) {
    let ship = g.map.plots[here].units.iter().copied().find(|&t| {
        g.units[t].owner == p
            && is_transport_type(&g.d.units[g.units[t].unit_type])
            // Not already carrying a settler on its own errand.
            && !g.unit_get_cargo_units(t).iter().any(|&c| g.d.units[g.units[c].unit_type].found)
            && g.unit_can_load_unit(u, t, here)
    });
    if let Some(t) = ship {
        crate::trace!(g, p, "assault unit {:?} boards {:?} at {:?}", u, t, g.map.xy(here));
        let others: Vec<UnitId> = g.groups[grp].units.iter().copied().filter(|&o| o != u).collect();
        g.unit_load_unit(u, t);
        for o in others {
            if g.units.contains(o) && g.unit_can_load_unit(o, t, here) {
                g.unit_load_unit(o, t);
            }
        }
        if alive(g, grp) && g.group_ready_to_move(grp, false) {
            skip(g, grp);
        }
    } else {
        crate::trace!(g, p, "assault unit {:?} waits for a ship at {:?}", u, g.map.xy(here));
        skip(g, grp);
    }
}

/// A stack landed on the target's landmass takes the city. Successive trips land only the
/// ship's cargo, so a lone stack rarely covers a city; the landed units therefore concentrate on
/// one rally plot next to the city (`military::army` merges army units sharing a plot) and the
/// massed stack strikes once it can take the city -- or holds, rather than throwing itself away,
/// when the revealed defenders are too strong (reusing `military::assess`/`attack_ok`/`assault`).
fn assault_landed(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId, amph: &Amph) -> bool {
    let here = g.unit_plot(u);
    let tp = amph.plot;
    let (tx, ty) = g.map.xy(tp);
    // Land plots beside the city we can stand on, and how many of our army units each holds.
    let mut adj: Vec<(PlotIdx, i32)> = Vec::new();
    for dir in 0..DirectionType::NUM {
        let Some(q) = g.plot_direction(tx, ty, DirectionType::from_usize(dir)) else { continue };
        if g.plot_is_water(q) || g.plot_is_impassable(q) || g.map.plots[q].plot_city.is_some() {
            continue;
        }
        if q != here && !g.unit_can_move_into(u, q, false, false, false) {
            continue;
        }
        let ours = g.map.plots[q].units.iter().filter(|&&o| g.units[o].owner == v.p && crate::military::is_army_unit(g, o)).count() as i32;
        adj.push((q, ours));
    }
    // Rally on the adjacent plot already holding the most of our army (ties: best defence, then
    // lowest index), so the trickle of landings piles onto one plot and merges.
    let rally = adj.iter().max_by_key(|&&(q, n)| (n, g.plot_defense_modifier(q, Some(v.team), true), std::cmp::Reverse(q))).map(|&(q, _)| q);
    let dist = {
        let (hx, hy) = g.map.xy(here);
        g.plot_distance(hx, hy, tx, ty)
    };
    if dist <= 1 {
        // Beside the city: strike if this (merged) stack can take it, else gather on the rally
        // plot for the next landings (a lone unit beside a strong city must not be thrown away).
        let can_strike = crate::military::assess(g, grp, tp).is_some_and(|a| crate::military::attack_ok(&a, true));
        if can_strike {
            let name = g.cities[amph.city].name.clone();
            crate::trace!(g, v.p, "landed stack {:?} ({} units) assaults {} at {:?}", grp, g.groups[grp].units.len(), name, g.map.xy(tp));
            crate::military::assault(g, v, grp, tp);
            let taken = !g.cities.contains(amph.city) || g.cities[amph.city].owner == v.p;
            crate::trace!(g, v.p, "landed assault on {}: {}", name, if taken { "taken" } else { "holds" });
        } else if rally.is_some_and(|r| r != here) {
            move_to(g, grp, rally.unwrap(), 0);
        } else {
            // Wait awake for the next landings to pile on (fortifying would stop re-evaluation).
            skip(g, grp);
        }
        finish(g, grp);
        return true;
    }
    // Inland (or landed a step away): move to the rally plot next to the city.
    let reach = v.flood.run(g, u, 10, false);
    let target_plot = rally.or_else(|| {
        reach
            .iter()
            .filter(|r| {
                let (rx, ry) = g.map.xy(r.plot);
                g.plot_distance(rx, ry, tx, ty) == 1 && !g.plot_is_water(r.plot) && g.map.plots[r.plot].plot_city.is_none()
            })
            .min_by_key(|r| (r.turns, std::cmp::Reverse(g.plot_defense_modifier(r.plot, Some(v.team), true)), r.plot))
            .map(|r| r.plot)
    });
    match target_plot {
        Some(q) if q != here => move_to(g, grp, q, 0),
        Some(_) => skip(g, grp),
        None => move_to(g, grp, tp, MOVE_IGNORE_DANGER),
    }
    finish(g, grp);
    true
}

/// One decision for an army stack when an amphibious invasion is on. True if it was handled
/// (board for the crossing, or take the city once landed); false to leave the stack to the
/// normal land logic (`military::army`).
pub fn amphibious(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
    let p = v.p;
    let Some(amph) = v.amphibious.clone() else { return false };
    if !g.cities.contains(amph.city) || g.cities[amph.city].owner == p {
        return false;
    }
    let here = g.unit_plot(u);
    // Landed on the target's landmass: take the city.
    if g.map.plots[here].area == g.map.plots[amph.plot].area {
        return assault_landed(g, v, grp, u, &amph);
    }
    // On the staging side: only the committed stacks march to the port and board.
    if !committed(g, p, &amph, grp) {
        return false;
    }
    if here == amph.staging_plot {
        board(g, p, grp, u, here);
    } else {
        move_to(g, grp, amph.staging_plot, MOVE_SAFE_TERRITORY);
        if alive(g, grp) && g.group_ready_to_move(grp, false) {
            skip(g, grp);
        }
    }
    true
}

// ----------------------------------------------------------------------------
// Production: an assault fleet for the invasion
// ----------------------------------------------------------------------------

/// Production hook for the staging port: a transport to carry the army overseas when we have
/// none, then a few attackers up to what one assault needs (capped, so the port does not build
/// endlessly). Reuses `num_transports` accounting (a transport in training counts).
pub fn assault_production(g: &Game, c: CityId) -> Option<Choice> {
    let p = g.cities[c].owner;
    if g.player_is_barbarian(p) {
        return None;
    }
    let amph = amphibious_target(g, p)?;
    // Only the staging port (the coastal city nearest the target) builds the fleet.
    if c != amph.staging {
        return None;
    }
    // Transports dedicated to the assault (settlers' transports are often busy ferrying settlers
    // and never free to carry the army, so the assault wants its own). Build enough to carry the
    // whole assault force in one wave -- a single transport only trickles a few units per trip,
    // which rarely masses enough on the beach to take a defended city; a fleet lands the force
    // fast enough to strike. Capped by what one assault needs (ceil(need / cargo per ship)).
    if let Some(u) = best_unit(g, c, Role::Transport) {
        let cargo_each = g.d.units[u].cargo_space.max(1);
        let want = (amph.need + cargo_each - 1) / cargo_each;
        if assault_ships(g, p) < want {
            return Some(Choice::Train(u, UnitAIType::UNITAI_ASSAULT_SEA));
        }
    }
    // Enough attackers for one assault (defenders x2, from `need`).
    if crate::diplomacy::army_size(g, p) < amph.need {
        if let Some(u) = best_unit(g, c, Role::CityAttack).or_else(|| best_unit(g, c, Role::Attack)) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_ATTACK));
        }
    }
    None
}

/// One decision for a transport led by `u`.
pub fn transport(ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let p = v.p;
    let here = g.unit_plot(u);
    let cargo = g.unit_get_cargo_units(u);
    let carries_settler = cargo.iter().any(|&c| g.d.units[g.units[c].unit_type].found);
    // A land fighter aboard (not a settler): an assault to land on an enemy coast.
    let carries_army = !carries_settler
        && cargo.iter().any(|&c| {
            let info = &g.d.units[g.units[c].unit_type];
            !info.found && info.combat > 0 && g.unit_get_domain_type(c) == DomainType::DOMAIN_LAND
        });
    crate::trace!(g, p, "transport {:?} at {:?}: cargo {} (settler {}, army {}), moves {}", u, g.map.xy(here), cargo.len(), carries_settler, carries_army, g.unit_moves_left(u));
    if carries_army {
        if let Some(amph) = v.amphibious.clone() {
            transport_assault(g, v, grp, u, &amph, &cargo);
            return;
        }
        // The target is gone: carry the army back to port and unload (below).
    }
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
        // An assault is on: wait in the staging port for the army to board (it boards the ship
        // itself, like a settler and its escort).
        if let Some(amph) = v.amphibious.clone() {
            if here == amph.staging_plot {
                port(g, p, grp, here);
            } else {
                move_to(g, grp, amph.staging_plot, 0);
                if alive(g, grp) && g.group_ready_to_move(grp, false) {
                    skip(g, grp);
                }
            }
            return;
        }
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

/// A transport carrying an assault army sails to a water plot next to the target enemy city and
/// lands the army on a land plot beside it (the sail/land of `transport`'s settler branch, with
/// the landing sites being the land plots around the enemy city rather than a settle site).
fn transport_assault(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId, amph: &Amph, cargo: &[UnitId]) {
    let p = v.p;
    let here = g.unit_plot(u);
    let (hx, hy) = g.map.xy(here);
    let tp = amph.plot;
    let (tx, ty) = g.map.xy(tp);
    // The target city's name for tracing: another of our stacks may capture or raze it earlier this
    // turn, leaving `amph.city` a stale slab index, so guard the read (only ever hit under tracing).
    let city_name = |g: &Game| if g.cities.contains(amph.city) { g.cities[amph.city].name.clone() } else { "a lost city".to_string() };
    // Land plots beside the city a land unit can disembark onto.
    let sample = cargo.iter().copied().find(|&c| g.unit_get_domain_type(c) == DomainType::DOMAIN_LAND);
    let army_on = |l: PlotIdx| g.map.plots[l].units.iter().filter(|&&o| g.units[o].owner == p && crate::military::is_army_unit(g, o)).count() as i32;
    let reach = v.flood.run(g, u, 25, false);
    // The landing site to disembark onto: whichever reachable land plot beside the city already
    // holds the most of our army (so successive landings pile onto one plot and merge into a stack
    // that can strike, the way `assault_landed` already rallies them), then the soonest reachable,
    // then the strongest ground. We still land whenever any plot is reachable -- only if none is do
    // we sail on toward the city -- so a trickle concentrates without ever failing to get ashore.
    let mut best: Option<(PlotIdx, PlotIdx, i32, i32)> = None; // (land site, water plot, our army, turns)
    for dir in 0..DirectionType::NUM {
        let Some(l) = g.plot_direction(tx, ty, DirectionType::from_usize(dir)) else { continue };
        if g.plot_is_water(l) || g.plot_is_impassable(l) || g.map.plots[l].plot_city.is_some() {
            continue;
        }
        if sample.is_some_and(|c| !g.unit_can_move_into(c, l, false, false, false)) {
            continue;
        }
        let (lx, ly) = g.map.xy(l);
        let (water, turns) = if g.plot_is_water(here) && g.plot_distance(hx, hy, lx, ly) == 1 {
            (here, -1)
        } else {
            match reach
                .iter()
                .filter(|r| g.plot_is_water(r.plot) && g.map.plots[r.plot].revealed[v.team] && {
                    let (rx, ry) = g.map.xy(r.plot);
                    g.plot_distance(rx, ry, lx, ly) == 1
                })
                .min_by_key(|r| (r.turns, r.plot))
            {
                Some(r) => (r.plot, r.turns),
                None => continue,
            }
        };
        let a = army_on(l);
        let def = g.plot_defense_modifier(l, Some(v.team), true);
        let key = (a, std::cmp::Reverse(turns), def, std::cmp::Reverse(l));
        let better = best.map_or(true, |(bl, _, ba, bt)| key > (ba, std::cmp::Reverse(bt), g.plot_defense_modifier(bl, Some(v.team), true), std::cmp::Reverse(bl)));
        if better {
            best = Some((l, water, a, turns));
        }
    }
    let Some((site, water, _, _)) = best else {
        // No reachable coast yet: sail toward the city (the reached water plot nearest it).
        if let Some(r) = reach.iter().filter(|r| g.plot_is_water(r.plot) && r.turns > 0).min_by_key(|r| {
            let (rx, ry) = g.map.xy(r.plot);
            (g.plot_distance(rx, ry, tx, ty), r.turns, r.plot)
        }) {
            crate::trace!(g, p, "assault transport {:?} sails toward {} at {:?}", u, city_name(g), g.map.xy(tp));
            move_to(g, grp, r.plot, 0);
        }
        if alive(g, grp) && g.group_ready_to_move(grp, false) {
            skip(g, grp);
        }
        return;
    };
    if water == here {
        crate::trace!(g, p, "assault transport {:?} lands its army on {:?} next to {}", u, g.map.xy(site), city_name(g));
        for c in cargo.iter().copied() {
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
    crate::trace!(g, p, "assault transport {:?} sails toward the coast of {} ({:?})", u, city_name(g), g.map.xy(site));
    move_to(g, grp, water, 0);
    if alive(g, grp) && g.group_ready_to_move(grp, false) {
        skip(g, grp);
    }
}

pub(crate) fn port(g: &mut Game, p: PlayerId, grp: GroupId, here: PlotIdx) {
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
    if let Some(q) = sea_explore_target(g, v, u) {
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

/// The sea explorer's next goal: the reachable revealed water plot (10 turns) that reveals the
/// most, per turn of travel, away from danger and other explorers' goals.
pub(crate) fn sea_explore_target(g: &Game, v: &mut View, u: UnitId) -> Option<PlotIdx> {
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
    best.map(|(q, _)| q)
}
