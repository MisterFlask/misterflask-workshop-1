//! Early expansion: when a city builds settlers (`settler_choice`, called by
//! `production::decide`) and where settlers go with their escorts (`settler`, the settler
//! role of `units::play`). See docs/AI.md "Settlers and expansion".

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::production::{best_unit, count_ai, garrison, Choice, Role};
use crate::sites;
use crate::units::{alive, can_start, hold, move_to, push, skip};
use crate::view::{attack_power, View};
use crate::Ai;

// ----------------------------------------------------------------------------
// Production
// ----------------------------------------------------------------------------

#[derive(PartialEq, Eq)]
pub enum SettlerPlan {
    No,
    /// Build one now.
    Now(Choice),
    /// Once the city has grown.
    Soon,
}

/// The city facts the settler decision reads (the same values `production::decide` uses).
struct CityFacts {
    n_cities: i32,
    pop: i32,
    food_surplus: i32,
    production: i32,
    happy_room: i32,
    at_war: bool,
    broke: bool,
}

fn facts(g: &Game, c: CityId) -> CityFacts {
    let city = &g.cities[c];
    let p = city.owner;
    let pl = &g.players[p];
    let team = pl.team;
    let barb = g.barbarian_team();
    let civ_war = g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t));
    CityFacts {
        n_cities: pl.cities.len() as i32,
        pop: city.population,
        food_surplus: g.city_yield_rate(c, YieldType::YIELD_FOOD) - g.city_food_consumption(c, false, 0),
        production: g.city_yield_rate(c, YieldType::YIELD_PRODUCTION),
        happy_room: g.city_happy_level(c) - g.city_unhappy_level(c, 0),
        at_war: civ_war || crate::diplomacy::preparing_target(g, team).is_some(),
        broke: crate::production::financial_trouble(g, p),
    }
}

/// Settlers: while sites remain in this city's area, a bounded number at a time, from cities
/// big enough to pay for them (food goes into settler production, so growth stops).
/// `threat`/`defense`: the visible threat around city `c` (of player `p`) and its garrison's
/// defense.
pub fn settler_choice(ai: &Ai, g: &Game, c: CityId, p: PlayerId, threat: i32, defense: i32) -> SettlerPlan {
    use SettlerPlan::No;
    let _ = defense;
    let safe = threat == 0;
    let ctx = facts(g, c);
    let pl = &g.players[p];
    if !safe || g.city_is_occupation(c) {
        return No;
    }
    let settlers = count_ai(g, p, UnitAIType::UNITAI_SETTLE);
    let max_cities = if pl.max_cities > 0 { pl.max_cities } else { i32::MAX };
    if ctx.n_cities + settlers >= max_cities {
        return No;
    }
    let area = g.map.plots[g.city_plot(c)].area;
    let sites = sites::sites(ai, g, p);
    let area_sites = sites.iter().filter(|(q, _)| g.map.plots[*q].area == area).count() as i32;
    let pop = ctx.pop;
    if area_sites == 0 {
        // Overseas: one settler at a time, from a coastal city that has (or can build) a ship.
        let overseas = sites::overseas_sites(ai, g, p);
        if overseas.is_empty() || settlers > 0 || !crate::naval::is_coastal_city(g, c) || pop < 3 {
            return No;
        }
        if crate::naval::num_transports(g, p) == 0 && best_unit(g, c, Role::Transport).is_none() {
            return No;
        }
        if ctx.broke && g.players[p].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()] < 20 {
            return No;
        }
        return match best_unit(g, c, Role::Settler) {
            Some(u) => SettlerPlan::Now(Choice::Train(u, UnitAIType::UNITAI_SETTLE)),
            None => No,
        };
    }
    let mut max_settlers = ((ctx.n_cities + 1) / 2).max(1).min(area_sites);
    // C++ AI_chooseProduction: no settlers while in financial trouble.
    if ctx.broke && g.players[p].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()] < 20 {
        return No;
    }
    if ctx.at_war {
        max_settlers = (max_settlers + 1) / 2;
    }
    if settlers >= max_settlers {
        return No;
    }
    // Size: grow first while growth is cheap and happy (a settler eats the food surplus as
    // production, so a size-2 city takes 30+ turns for one, a size-4 city half that); a city at
    // its happy cap gains nothing from growing, so it builds settlers at once.
    let min_pop = if ctx.happy_room <= 0 { 1 } else if ctx.food_surplus >= 3 { 4 } else { 3 };
    if pop < min_pop {
        return if settlers == 0 && ctx.n_cities <= 3 { SettlerPlan::Soon } else { No };
    }
    // A city with almost no output would take forever; let better cities build them.
    if ctx.n_cities > 1 && ctx.food_surplus.max(0) + ctx.production < 4 {
        return No;
    }
    let Some(u) = best_unit(g, c, Role::Settler) else { return No };
    // A slow city leaves settlers to a much faster one on the same landmass (it would hold the
    // settler slot for ages while the fast city waits).
    let turns = g.city_production_turns_left_unit(c, u, 0);
    let fastest = pl
        .cities
        .iter()
        .filter(|&&o| o != c && g.map.plots[g.city_plot(o)].area == area && !g.city_is_occupation(o))
        .map(|&o| g.city_production_turns_left_unit(o, u, 0))
        .min()
        .unwrap_or(i32::MAX);
    if turns > 15 && turns > fastest.saturating_mul(2).saturating_add(4) {
        return No;
    }
    SettlerPlan::Now(Choice::Train(u, UnitAIType::UNITAI_SETTLE))
}

// ----------------------------------------------------------------------------
// Settlers
// ----------------------------------------------------------------------------

pub fn settler(ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let p = v.p;
    let here = g.unit_plot(u);
    let first_city = g.players[p].cities.is_empty();
    let candidates = sites::sites(ai, g, p);
    let reach = v.flood.run(g, u, if first_city { 3 } else { 10 }, false);
    let mut best: Option<(PlotIdx, i64)> = None;
    for &(site, value) in &candidates {
        if v.is_claimed(site) {
            continue;
        }
        let Some(r) = reach.iter().find(|r| r.plot == site) else { continue };
        // Danger at the site: only an escorted settler goes (or founds where it stands).
        if v.danger_at(site) > 0 && !first_city && site != here && !escorted(g, grp) {
            continue;
        }
        // Founding on the first turn matters more than a slightly better plot.
        let score = value as i64 * 10 / (r.turns as i64 + if first_city { 1 } else { 3 });
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((site, score));
        }
    }
    // The first city: found in place unless a clearly better site is a step away.
    if first_city && g.unit_can_found(u, here, false) {
        let (x, y) = g.map.xy(here);
        let here_value = g.player_ai_found_value(p, x, y, -1, false, &[]) as i64;
        let better = best.is_some_and(|(site, _)| {
            let r = reach.iter().find(|r| r.plot == site).map_or(9, |r| r.turns);
            let (sx, sy) = g.map.xy(site);
            r == 0 && (g.player_ai_found_value(p, sx, sy, -1, false, &[]) as i64) * 100 > here_value * 125
        });
        if !better {
            best = Some((here, 0));
        }
    }
    if best.is_none() && !first_city && crate::naval::settler_overseas(ai, g, v, grp, u) {
        return;
    }
    let Some((site, _)) = best.or_else(|| {
        // No scored site: the first settler founds where it stands if it can.
        if first_city && g.unit_can_found(u, here, false) { Some((here, 0)) } else { None }
    }) else {
        // Nowhere to go: wait in a city.
        hold(g, grp);
        return;
    };
    v.claimed.push(site);
    crate::trace!(g, p, "settler {:?} at {:?} -> site {:?} (danger here {}, site {})", u, g.map.xy(here), g.map.xy(site), v.danger_at(here), v.danger_at(site));
    if site != here && !first_city && !escorted(g, grp) {
        // Leaving our lands without an escort gets settlers eaten by animals and barbarians:
        // take a spare fighter from this plot, else wait for one (a while) in a city.
        if let Some(e) = find_escort(g, v, here, grp) {
            crate::trace!(g, p, "settler {:?} takes escort {:?}", u, e);
            g.unit_join_group(e, Some(grp), true);
        } else if g.map.plots[here].plot_city.is_some() && g.game_turn() - g.units[u].game_turn_created < 8 {
            v.escort_requests.push(here);
            hold_one_turn(g, grp);
            return;
        }
    }
    if site == here {
        if can_start(g, grp, MissionType::MISSION_FOUND, -1, -1) {
            push(g, grp, MissionType::MISSION_FOUND, -1, -1, 0, false);
        } else {
            skip(g, grp);
        }
        return;
    }
    move_to(g, grp, site, MOVE_SAFE_TERRITORY);
    if alive(g, grp) && g.group_ready_to_move(grp, false) && g.unit_moves_left(u) > 0 && (escorted(g, grp) || v.danger_at(site) == 0) {
        // No safe path (it crosses a neighbour's or unrevealed land): the direct way.
        move_to(g, grp, site, 0);
    }
    if alive(g, grp) && g.group_ready_to_move(grp, false) {
        // Path failed (or no moves left): try again next turn.
        skip(g, grp);
    } else if alive(g, grp) && g.unit_plot(u) == site && can_start(g, grp, MissionType::MISSION_FOUND, -1, -1) {
        push(g, grp, MissionType::MISSION_FOUND, -1, -1, 0, true);
    }
}

/// Does the group hold a unit that can fight?
fn escorted(g: &Game, grp: GroupId) -> bool {
    g.groups[grp].units.iter().any(|&o| attack_power(g, o) > 0 || crate::view::defense_power(g, o) > 0 && !g.d.units[g.units[o].unit_type].found)
}

/// A fighter on `plot` that can leave: not the last garrison of a city there.
fn find_escort(g: &Game, v: &View, plot: PlotIdx, grp: GroupId) -> Option<UnitId> {
    let p = v.p;
    let city = g.map.plots[plot].plot_city.filter(|&c| g.cities[c].owner == p);
    let garrison_n = city.map_or(0, |_| garrison(g, p, plot).0);
    // The capital keeps two defenders, other cities one.
    let keep = if city.is_some_and(|c| g.city_is_capital(c)) { 2 } else { 1 };
    let mut best: Option<(UnitId, i32)> = None;
    for &o in &g.map.plots[plot].units {
        let unit = &g.units[o];
        if unit.owner != p || unit.group == Some(grp) || g.unit_get_domain_type(o) != DomainType::DOMAIN_LAND {
            continue;
        }
        let info = &g.d.units[unit.unit_type];
        if info.combat <= 0 || info.found || g.unit_moves_left(o) <= 0 {
            continue;
        }
        let is_garrison = matches!(unit.ai_type, Some(UnitAIType::UNITAI_CITY_DEFENSE) | Some(UnitAIType::UNITAI_CITY_COUNTER) | Some(UnitAIType::UNITAI_CITY_SPECIAL));
        let ok = match unit.ai_type {
            Some(UnitAIType::UNITAI_ATTACK) | Some(UnitAIType::UNITAI_COUNTER) | Some(UnitAIType::UNITAI_RESERVE) | Some(UnitAIType::UNITAI_ATTACK_CITY) => true,
            _ => is_garrison && garrison_n > keep,
        };
        if !ok {
            continue;
        }
        // Field units before garrison units.
        let d = crate::view::defense_power(g, o) + if is_garrison { 0 } else { 100_000 };
        if best.map_or(true, |(_, b)| d > b) {
            best = Some((o, d));
        }
    }
    best.map(|(o, _)| o)
}

/// End this group's turn without fortifying (it acts again next turn).
fn hold_one_turn(g: &mut Game, grp: GroupId) {
    skip(g, grp);
}
