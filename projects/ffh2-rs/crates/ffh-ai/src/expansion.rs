//! Expansion: when a city builds settlers (`settler_choice`, called by `production::decide`),
//! the escort that leaves with each settler (`prepare_escorts`, once per turn), and where
//! settlers go (`settler`, the settler role of `units::play`). See docs/AI.md "Settlers and
//! escorts".
//!
//! A settler leaves a city with an escort that covers the danger on its way and at the site,
//! and the escort becomes the new city's first garrison: a city founded without a defender
//! builds one at one or two hammers a turn while the barbarians walk in.

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::production::{best_unit, count_ai, garrison, Choice, Role};
use crate::sites;
use crate::units::{alive, can_start, hold, move_to, push, skip};
use crate::view::{attack_power, defense_power, View};
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
/// defense. Any visible threat stops settlers: letting them go under a held threat founded
/// more cities but lost more to the barbarians (measured twice, see docs/AI.md).
pub fn settler_choice(ai: &Ai, g: &Game, c: CityId, p: PlayerId, threat: i32, defense: i32) -> SettlerPlan {
    use SettlerPlan::No;
    let _ = defense;
    let ctx = facts(g, c);
    let pl = &g.players[p];
    if threat > 0 || g.city_is_occupation(c) {
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
        if ctx.broke && crate::economy::spend_pct(g, p) < 20 {
            return No;
        }
        return match best_unit(g, c, Role::Settler) {
            Some(u) => SettlerPlan::Now(Choice::Train(u, UnitAIType::UNITAI_SETTLE)),
            None => No,
        };
    }
    let mut max_settlers = ((ctx.n_cities + 1) / 2).max(1).min(area_sites);
    // C++ AI_chooseProduction: no settlers while in financial trouble.
    if ctx.broke && crate::economy::spend_pct(g, p) < 20 {
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
// Escorts
// ----------------------------------------------------------------------------

/// Turns a settler without an escort waits in a city (and an escorted one waits for one that
/// covers the risk) before it settles for less: unescorted only where nothing can reach it,
/// escorted where the escort covers half the risk.
const ESCORT_PATIENCE: i32 = 10;

/// Once per turn (from `play_turn`): a city about to finish a settler (3 turns or less), or
/// holding one that waits, with no fighter to spare trains the cheapest adequate one: right
/// after the settler in production, at the head of the queue when a settler waits.
pub fn prepare_escorts(g: &mut Game, p: PlayerId) {
    for c in g.players[p].cities.clone() {
        if !g.cities.contains(c) || g.cities[c].order_queue.is_empty() {
            continue;
        }
        let plot = g.city_plot(c);
        let building_settler = g.city_production_unit(c).is_some_and(|u| g.d.units[u].found) && g.city_production_turns_left(c) <= 3;
        let waiting = g.map.plots[plot].units.iter().any(|&u| {
            let unit = &g.units[u];
            unit.owner == p && g.d.units[unit.unit_type].found && !g.unit_is_cargo(u) && g.game_turn() > unit.game_turn_created
        });
        if !building_settler && !waiting {
            continue;
        }
        if escort_on_plot(g, p, plot, None).is_some() {
            continue;
        }
        let queued = g.cities[c].order_queue.iter().any(|o| o.order_type == OrderType::ORDER_TRAIN && g.d.units[UnitType::from_usize(o.data1 as usize)].combat > 0);
        if queued {
            continue;
        }
        let Some(u) = escort_unit(g, c) else { continue };
        crate::trace!(g, p, "{}: escort {} for the settler", g.cities[c].name, g.d.units[u].base.type_);
        g.city_push_order(c, OrderType::ORDER_TRAIN, u.i(), UnitAIType::UNITAI_ATTACK.i(), false, false, building_settler, false);
    }
}

/// The cheapest unit this city can train that defends at least 70% as well as its best one.
fn escort_unit(g: &Game, c: CityId) -> Option<UnitType> {
    let d = g.d;
    let mut cands: Vec<(UnitType, i64, i32)> = Vec::new();
    for u in d.units.keys() {
        let info = &d.units[u];
        if info.domain_type != Some(DomainType::DOMAIN_LAND) || info.combat <= 0 || info.found {
            continue;
        }
        if matches!(info.default_unit_ai_type, Some(UnitAIType::UNITAI_HERO) | Some(UnitAIType::UNITAI_MAGE) | Some(UnitAIType::UNITAI_EXPLORE)) {
            continue;
        }
        if !g.city_offers_unit(c, u) || !g.city_can_train(c, u, false, false, false, false) {
            continue;
        }
        cands.push((u, crate::production::type_city_defense(info), g.city_production_turns_left_unit(c, u, 0)));
    }
    let best_def = cands.iter().map(|&(_, d, _)| d).max()?;
    cands.into_iter().filter(|&(_, d, _)| d * 10 >= best_def * 7).min_by_key(|&(u, d, t)| (t, std::cmp::Reverse(d), u)).map(|(u, _, _)| u)
}

/// Summed defense of the group's fighters (what protects the settler).
fn escort_power(g: &Game, grp: GroupId) -> i32 {
    g.groups[grp].units.iter().filter(|&&o| !g.d.units[g.units[o].unit_type].found).map(|&o| defense_power(g, o)).sum()
}

/// Does the group hold a unit that can fight?
fn escorted(g: &Game, grp: GroupId) -> bool {
    g.groups[grp].units.iter().any(|&o| attack_power(g, o) > 0 || defense_power(g, o) > 0 && !g.d.units[g.units[o].unit_type].found)
}

/// A fighter on `plot` that can leave with a settler: a field unit, or a garrison unit the
/// city can spare (two defenders stay in the capital, one elsewhere, and they still hold the
/// visible threat). Field units first; of the garrison, the weakest that will do.
fn escort_on_plot(g: &Game, p: PlayerId, plot: PlotIdx, grp: Option<GroupId>) -> Option<UnitId> {
    let city = g.map.plots[plot].plot_city.filter(|&c| g.cities[c].owner == p);
    let (garrison_n, garrison_def) = if city.is_some() { garrison(g, p, plot) } else { (0, 0) };
    let threat = if city.is_some() { crate::production::local_threat(g, p, plot, 3) } else { 0 };
    let keep = if city.is_some_and(|c| g.city_is_capital(c)) { 2 } else { 1 };
    let mut best: Option<(UnitId, i32)> = None;
    for &o in &g.map.plots[plot].units {
        let unit = &g.units[o];
        if unit.owner != p || grp.is_some_and(|grp| unit.group == Some(grp)) || g.unit_get_domain_type(o) != DomainType::DOMAIN_LAND {
            continue;
        }
        let info = &g.d.units[unit.unit_type];
        if info.combat <= 0 || info.found || g.unit_moves_left(o) <= 0 || g.unit_is_cargo(o) {
            continue;
        }
        let def = defense_power(g, o);
        let is_garrison = matches!(unit.ai_type, Some(UnitAIType::UNITAI_CITY_DEFENSE) | Some(UnitAIType::UNITAI_CITY_COUNTER) | Some(UnitAIType::UNITAI_CITY_SPECIAL));
        let ok = match unit.ai_type {
            Some(UnitAIType::UNITAI_ATTACK) | Some(UnitAIType::UNITAI_COUNTER) | Some(UnitAIType::UNITAI_RESERVE) | Some(UnitAIType::UNITAI_ATTACK_CITY) => true,
            _ => is_garrison && garrison_n > keep && (garrison_def - def) * 3 / 2 >= threat * 2,
        };
        if !ok {
            continue;
        }
        let score = if is_garrison { -def } else { 100_000 + def };
        if best.map_or(true, |(_, b)| score > b) {
            best = Some((o, score));
        }
    }
    best.map(|(o, _)| o)
}

/// Highest danger on the straight line from `from` (excluded) to `to` (included), our cities
/// excepted: what a settler walking there may meet.
fn path_danger(g: &Game, v: &View, from: PlotIdx, to: PlotIdx) -> i32 {
    let (x0, y0) = g.map.xy(from);
    let (x1, y1) = g.map.xy(to);
    let dx = g.dx_wrap(x1 - x0);
    let dy = g.dy_wrap(y1 - y0);
    let n = dx.abs().max(dy.abs());
    let mut worst = 0;
    for i in 1..=n {
        let Some(q) = g.plot_xy(x0, y0, dx * i / n, dy * i / n) else { continue };
        if g.map.plots[q].plot_city.is_some_and(|c| g.cities[c].owner == v.p) {
            continue;
        }
        worst = worst.max(v.danger_at(q));
    }
    worst
}

/// Move toward a city site, remembering it as the group's mission target (`MISSIONAI_FOUND`,
/// saved with the game, so a resumed game re-plans identically).
fn move_to_site(g: &mut Game, grp: GroupId, site: PlotIdx, flags: i32) {
    let (x, y) = g.map.xy(site);
    g.group_push_mission(grp, MissionType::MISSION_MOVE_TO, x, y, flags, false, false, Some(MissionAIType::MISSIONAI_FOUND), Some(site), None);
}

// ----------------------------------------------------------------------------
// Settlers
// ----------------------------------------------------------------------------

/// One decision for a settler group: pick up an escort, choose the best site whose risk the
/// escort covers, and go (or found); wait in a city when no site is safe enough.
pub fn settler(ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let p = v.p;
    let here = g.unit_plot(u);
    let first_city = g.players[p].cities.is_empty();
    let candidates = sites::sites(ai, g, p);
    let reach = v.flood.run(g, u, if first_city { 3 } else { 10 }, false);
    let in_city = g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == p);
    let previous = if g.groups[grp].ai_mission_ai_type == Some(MissionAIType::MISSIONAI_FOUND) { g.group_ai_get_mission_ai_plot(grp) } else { None };
    // Candidate sites in reach: (site, score, risk). The risk is the worst danger on the way, or
    // twice the danger at the site (a new city has to hold it).
    let mut scored: Vec<(PlotIdx, i64, i32)> = Vec::new();
    for &(site, value) in &candidates {
        if v.is_claimed(site) {
            continue;
        }
        let Some(r) = reach.iter().find(|r| r.plot == site) else { continue };
        let risk = if first_city || site == here { 0 } else { path_danger(g, v, here, site).max(v.danger_at(site) * 2) };
        // Founding on the first turn matters more than a slightly better plot.
        let mut score = value as i64 * 10 / (r.turns as i64 + if first_city { 1 } else { 3 });
        // The site we were already walking to keeps its pull (re-planning every turn from a new
        // position otherwise flips between sites of similar value).
        if previous == Some(site) {
            score = score * 3 / 2;
        }
        scored.push((site, score, risk));
    }
    // An escort from this plot, and a second one when the best site needs more than the first.
    if !first_city {
        for _ in 0..2 {
            let power = escort_power(g, grp);
            let need = scored.iter().max_by_key(|&&(q, s, _)| (s, std::cmp::Reverse(q))).map_or(0, |&(_, _, r)| r);
            if (power > 0 && need <= power) || scored.is_empty() {
                break;
            }
            let Some(e) = escort_on_plot(g, p, here, Some(grp)) else { break };
            crate::trace!(g, p, "settler {:?} takes escort {:?} (power {}, risk {})", u, e, power, need);
            g.unit_join_group(e, Some(grp), true);
        }
    }
    let power = escort_power(g, grp);
    let waited = g.game_turn() - g.units[u].game_turn_created;
    let tolerance = if power > 0 && waited > ESCORT_PATIENCE { power * 2 } else { power };
    // A new city needs a defender from its first turn: an unescorted settler leaves a city only
    // after waiting long.
    let must_wait = power == 0 && in_city && waited <= ESCORT_PATIENCE && !first_city;
    let mut best: Option<(PlotIdx, i64)> = None;
    for &(site, score, risk) in &scored {
        if risk > tolerance || must_wait && site != here {
            continue;
        }
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
    // No site we can walk to: across the water.
    if best.is_none() && !first_city && scored.is_empty() && crate::naval::settler_overseas(ai, g, v, grp, u) {
        return;
    }
    let Some((site, _)) = best.or_else(|| {
        // No scored site: the first settler founds where it stands if it can.
        if first_city && g.unit_can_found(u, here, false) { Some((here, 0)) } else { None }
    }) else {
        if !scored.is_empty() {
            // Sites, but none safe enough: wait in a city for an escort, or go back to one.
            if in_city {
                crate::trace!(g, p, "settler {:?} waits for an escort (power {})", u, power);
                v.escort_requests.push(here);
                skip(g, grp);
                return;
            }
            if let Some(c) = crate::units::nearest_city(g, p, here) {
                crate::trace!(g, p, "settler {:?} falls back to {}", u, g.cities[c].name);
                move_to(g, grp, g.city_plot(c), 0);
                if alive(g, grp) && g.group_ready_to_move(grp, false) {
                    skip(g, grp);
                }
                return;
            }
        }
        // Nowhere to go: wait in a city.
        hold(g, grp);
        return;
    };
    v.claimed.push(site);
    crate::trace!(g, p, "settler {:?} at {:?} -> site {:?} (danger here {}, site {}, escort {})", u, g.map.xy(here), g.map.xy(site), v.danger_at(here), v.danger_at(site), power);
    if site == here {
        if can_start(g, grp, MissionType::MISSION_FOUND, -1, -1) {
            push(g, grp, MissionType::MISSION_FOUND, -1, -1, 0, false);
        } else {
            skip(g, grp);
        }
        return;
    }
    move_to_site(g, grp, site, MOVE_SAFE_TERRITORY);
    if alive(g, grp) && g.group_ready_to_move(grp, false) && g.unit_moves_left(u) > 0 && (escorted(g, grp) || v.danger_at(site) == 0) {
        // No safe path (it crosses a neighbour's or unrevealed land): the direct way.
        move_to_site(g, grp, site, 0);
    }
    if first_city && alive(g, grp) && g.unit_plot(u) == here && g.unit_can_found(u, here, false) && can_start(g, grp, MissionType::MISSION_FOUND, -1, -1) {
        // The better site is out of reach after all (the rules give an unescorted settler no
        // path through danger): the capital goes where the settler stands, not nowhere.
        crate::trace!(g, p, "settler {:?} cannot reach {:?}: founds in place", u, g.map.xy(site));
        push(g, grp, MissionType::MISSION_FOUND, -1, -1, 0, false);
    } else if alive(g, grp) && g.group_ready_to_move(grp, false) {
        // Path failed (or no moves left): try again next turn.
        skip(g, grp);
    } else if alive(g, grp) && g.unit_plot(u) == site && can_start(g, grp, MissionType::MISSION_FOUND, -1, -1) {
        push(g, grp, MissionType::MISSION_FOUND, -1, -1, 0, true);
    }
}
