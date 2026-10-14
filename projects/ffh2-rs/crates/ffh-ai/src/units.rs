//! Unit control: every group that needs orders this turn gets exactly one decision by its
//! head unit's role (`Unit::ai_type`). Decisions are issued as missions through the rules
//! (`pushMission`), so the AI can only do what a human could.

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::production::garrison;
use crate::reach::Reached;
use crate::sites;
use crate::view::{attack_power, View};
use crate::Ai;

// ----------------------------------------------------------------------------
// Mission helpers
// ----------------------------------------------------------------------------

pub fn push(g: &mut Game, grp: GroupId, mission: MissionType, d1: i32, d2: i32, flags: i32, append: bool) {
    g.group_push_mission(grp, mission, d1, d2, flags, append, false, None, None, None);
}

pub fn move_to(g: &mut Game, grp: GroupId, plot: PlotIdx, flags: i32) {
    let (x, y) = g.map.xy(plot);
    push(g, grp, MissionType::MISSION_MOVE_TO, x, y, flags, false);
}

pub fn skip(g: &mut Game, grp: GroupId) {
    push(g, grp, MissionType::MISSION_SKIP, -1, -1, 0, false);
}

pub(crate) fn can_start(g: &Game, grp: GroupId, mission: MissionType, d1: i32, d2: i32) -> bool {
    g.group_can_start_mission(grp, mission, d1, d2, None, false)
}

/// Fortify (or sleep, or skip) in place.
pub fn hold(g: &mut Game, grp: GroupId) {
    for m in [MissionType::MISSION_FORTIFY, MissionType::MISSION_SLEEP] {
        if can_start(g, grp, m, -1, -1) {
            push(g, grp, m, -1, -1, 0, false);
            return;
        }
    }
    skip(g, grp);
}

pub(crate) fn alive(g: &Game, grp: GroupId) -> bool {
    g.groups.contains(grp) && !g.groups[grp].units.is_empty()
}

// ----------------------------------------------------------------------------
// Roles
// ----------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Role {
    Garrison,
    Settler,
    Worker,
    SeaWorker,
    Army,
    Explore,
    GreatPerson,
    Missionary,
    Caster,
    Animal,
    Naval,
    Other,
}

fn role_of(g: &Game, u: UnitId) -> Role {
    let unit = &g.units[u];
    let info = &g.d.units[unit.unit_type];
    if g.unit_get_domain_type(u) == DomainType::DOMAIN_SEA {
        return if matches!(unit.ai_type, Some(UnitAIType::UNITAI_WORKER_SEA)) || info.unit_ai_type[UnitAIType::UNITAI_WORKER_SEA] { Role::SeaWorker } else { Role::Naval };
    }
    let ai = unit.ai_type.or(info.default_unit_ai_type).unwrap_or(UnitAIType::UNITAI_UNKNOWN);
    match ai {
        UnitAIType::UNITAI_SETTLE => Role::Settler,
        UnitAIType::UNITAI_WORKER => Role::Worker,
        UnitAIType::UNITAI_CITY_DEFENSE | UnitAIType::UNITAI_CITY_COUNTER | UnitAIType::UNITAI_CITY_SPECIAL => Role::Garrison,
        UnitAIType::UNITAI_EXPLORE => Role::Explore,
        UnitAIType::UNITAI_MISSIONARY => Role::Missionary,
        UnitAIType::UNITAI_PROPHET | UnitAIType::UNITAI_ARTIST | UnitAIType::UNITAI_SCIENTIST | UnitAIType::UNITAI_GENERAL | UnitAIType::UNITAI_MERCHANT | UnitAIType::UNITAI_ENGINEER => Role::GreatPerson,
        UnitAIType::UNITAI_MAGE | UnitAIType::UNITAI_WARWIZARD | UnitAIType::UNITAI_TERRAFORMER | UnitAIType::UNITAI_MANA_UPGRADE | UnitAIType::UNITAI_MEDIC => Role::Caster,
        UnitAIType::UNITAI_ANIMAL => Role::Animal,
        _ => {
            if info.found {
                Role::Settler
            } else if info.combat > 0 || info.combat_defense > 0 {
                Role::Army
            } else if info.builds.iter().any(|&b| b) {
                Role::Worker
            } else {
                Role::Other
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Turn driver
// ----------------------------------------------------------------------------

/// Decide for every group of `v.p` that needs orders, in role priority order.
pub fn play(ai: &Ai, g: &mut Game, v: &mut View) {
    let p = v.p;
    let mut order: Vec<(Role, GroupId)> = Vec::new();
    for &grp in &g.players[p].groups {
        if let Some(head) = g.group_get_head_unit(grp) {
            order.push((role_of(g, head), grp));
        }
    }
    order.sort();
    if crate::trace_on(p) {
        for &(role, grp) in &order {
            if role == Role::Settler {
                let head = g.group_get_head_unit(grp).unwrap();
                crate::trace!(g, p, "settler group {:?} at {:?} missions {} activity {:?}", grp, g.map.xy(g.unit_plot(head)), g.groups[grp].mission_queue.len(), g.groups[grp].activity_type);
            }
        }
    }

    for (role, grp) in order {
        if !alive(g, grp) {
            continue;
        }
        let replan = !g.groups[grp].mission_queue.is_empty()
            && match role {
                // Settlers re-plan every turn (danger and sites change).
                Role::Settler => g.groups[grp].mission_queue[0].mission_type == MissionType::MISSION_MOVE_TO,
                // Workers drop their job when enemies come near.
                Role::Worker => g.group_get_head_unit(grp).is_some_and(|h| crate::workers::worker_in_danger(g, v, g.unit_plot(h))),
                _ => false,
            };
        if g.groups[grp].force_update || replan {
            // Settlers re-plan every turn (danger and sites change).
            g.groups[grp].force_update = false;
            g.group_clear_mission_queue(grp);
            g.group_set_activity_type(grp, ActivityType::ACTIVITY_AWAKE);
        }
        if !g.group_ready_to_move(grp, false) {
            continue;
        }
        let Some(head) = g.group_get_head_unit(grp) else { continue };
        upkeep(g, head);
        if !alive(g, grp) || !g.units.contains(head) {
            continue;
        }
        match role {
            Role::Settler => settler(ai, g, v, grp, head),
            Role::Worker => crate::workers::worker(g, v, grp, head, false),
            Role::SeaWorker => crate::workers::worker(g, v, grp, head, true),
            Role::Garrison => garrison_unit(g, v, grp, head),
            Role::Army => crate::military::army(g, v, grp, head),
            Role::Explore => explore(g, v, grp, head),
            Role::GreatPerson => great_person(g, v, grp, head),
            Role::Missionary => missionary(g, v, grp, head),
            Role::Caster => caster(g, v, grp, head),
            Role::Animal => barbarian(g, v, grp, head),
            Role::Naval | Role::Other => hold(g, grp),
        }
    }
    finalize(g, p);
}

/// Make sure no group is left ready to move, so the turn can end (C++ `AI_update` always
/// ends a unit's turn with a mission, `MISSION_SKIP` as the fallback).
pub fn finalize(g: &mut Game, p: PlayerId) {
    for grp in g.players[p].groups.clone() {
        if alive(g, grp) && g.group_ready_to_move(grp, true) {
            skip(g, grp);
        }
    }
}

/// Promotions and upgrades before the unit acts.
fn upkeep(g: &mut Game, u: UnitId) {
    // Promote while possible (FfH units may have several picks).
    for _ in 0..8 {
        if !g.units.contains(u) || !g.units[u].promotion_ready {
            break;
        }
        let Some(best) = best_promotion(g, u) else { break };
        g.unit_promote(u, Some(best), None);
    }
    if !g.units.contains(u) {
        return;
    }
    // Upgrade when gold is comfortable.
    let p = g.units[u].owner;
    let ut = g.units[u].unit_type;
    let civ = g.player_get_civilization_type(p);
    let mut best: Option<(UnitType, i32)> = None;
    for (class, &ok) in g.d.units[ut].upgrade_unit_class.iter_enumerated() {
        if !ok {
            continue;
        }
        let Some(target) = civ.and_then(|c| g.d.civ_unit(c, class)) else { continue };
        if !g.unit_can_upgrade(u, Some(target), false) {
            continue;
        }
        let power = g.d.units[target].combat + g.d.units[target].combat_defense;
        if best.map_or(true, |(_, b)| power > b) {
            best = Some((target, power));
        }
    }
    if let Some((target, _)) = best {
        if g.players[p].gold > 100 {
            g.unit_upgrade(u, Some(target));
            // C++ AI_upgrade: upgrade(eBestUnit); doDelayedDeath(); -- the old unit goes now.
            if g.units.contains(u) {
                g.unit_do_delayed_death(u);
            }
        }
    }
}

/// Value of a promotion for this unit (AI heuristic; C++ `AI_promotionValue` is not ported).
pub fn promotion_value(g: &Game, u: UnitId, promo: PromotionType) -> i32 {
    let info = &g.d.promotions[promo];
    let unit = &g.units[u];
    let caster = matches!(unit.ai_type, Some(UnitAIType::UNITAI_MAGE) | Some(UnitAIType::UNITAI_WARWIZARD));
    let defender = matches!(unit.ai_type, Some(UnitAIType::UNITAI_CITY_DEFENSE) | Some(UnitAIType::UNITAI_CITY_COUNTER));
    let mut v = info.ai_weight;
    v += info.combat_percent * 2 + info.extra_combat_str * 40 + info.extra_combat_defense * if defender { 40 } else { 15 };
    v += info.city_defense_percent * if defender { 3 } else { 0 };
    v += info.city_attack_percent * if defender { 0 } else { 2 };
    v += info.first_strikes_change * 30 + info.chance_first_strikes_change * 15;
    v += info.moves_change * if defender { 5 } else { 60 };
    v += info.withdrawal_change * 2;
    v += info.combat_heal_percent / 2 + info.same_tile_heal_change + info.adjacent_tile_heal_change;
    v += info.spell_damage_modify * if caster { 2 } else { 0 };
    v += info.damage_type_combat.iter().sum::<i32>() * 40;
    if caster {
        v += info.spell_caster_xp * 3;
    }
    v.max(1)
}

fn best_promotion(g: &Game, u: UnitId) -> Option<PromotionType> {
    let mut best: Option<(PromotionType, i32)> = None;
    for promo in g.d.promotions.keys() {
        if !g.unit_can_promote(u, Some(promo), None) {
            continue;
        }
        let v = promotion_value(g, u, promo);
        if best.map_or(true, |(_, b)| v > b) {
            best = Some((promo, v));
        }
    }
    best.map(|(p, _)| p)
}

// ----------------------------------------------------------------------------
// Settlers
// ----------------------------------------------------------------------------

fn settler(ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
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

// ----------------------------------------------------------------------------
// Workers
// ----------------------------------------------------------------------------

// ----------------------------------------------------------------------------
// Military
// ----------------------------------------------------------------------------

pub(crate) fn nearest_city(g: &Game, p: PlayerId, from: PlotIdx) -> Option<CityId> {
    let (x, y) = g.map.xy(from);
    g.players[p].cities.iter().copied().min_by_key(|&c| (g.plot_distance(x, y, g.cities[c].x, g.cities[c].y), c))
}

/// Defenders a city wants (garrison quota).
fn garrison_quota(g: &Game, v: &View, c: CityId) -> i32 {
    let plot = g.city_plot(c);
    let mut q = 1 + i32::from(g.city_is_capital(c));
    if v.danger_at(plot) > 0 {
        q += 1 + v.danger_at(plot) / 1000;
    }
    q.min(5)
}

fn garrison_unit(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let p = v.p;
    let here = g.unit_plot(u);
    if let Some(c) = g.map.plots[here].plot_city.filter(|&c| g.cities[c].owner == p) {
        let have = v.garrison.get(&c).copied().unwrap_or(0);
        if have < garrison_quota(g, v, c) || attack_adjacent(g, v, grp, u, 800) {
            *v.garrison.entry(c).or_insert(0) += 1;
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                hold(g, grp);
            }
            return;
        }
    }
    // Go to the city that most needs a defender.
    let reach = v.flood.run(g, u, 8, false);
    let mut best: Option<(CityId, i32)> = None;
    for &c in &g.players[p].cities {
        let cp = g.city_plot(c);
        let Some(r) = reach.iter().find(|r| r.plot == cp) else { continue };
        let have = v.garrison.get(&c).copied().unwrap_or(0) + garrison(g, p, cp).0 * i32::from(cp != here);
        let need = garrison_quota(g, v, c) - have;
        if need <= 0 {
            continue;
        }
        let score = need * 100 - r.turns * 10;
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((c, score));
        }
    }
    match best {
        Some((c, _)) => {
            *v.garrison.entry(c).or_insert(0) += 1;
            move_to(g, grp, g.city_plot(c), 0);
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                hold(g, grp);
            }
        }
        None => {
            if g.map.plots[here].plot_city.is_some() {
                hold(g, grp);
            } else if let Some(c) = nearest_city(g, p, here) {
                move_to(g, grp, g.city_plot(c), 0);
                if alive(g, grp) && g.group_ready_to_move(grp, false) {
                    hold(g, grp);
                }
            } else {
                hold(g, grp);
            }
        }
    }
}

/// Attack an adjacent (one move) enemy if the odds are at least `min_odds` (of 1000).
/// Returns true if an attack was made.
pub(crate) fn attack_adjacent(g: &mut Game, v: &View, grp: GroupId, u: UnitId, min_odds: i32) -> bool {
    if attack_power(g, u) == 0 || !g.unit_can_attack(u) || g.unit_moves_left(u) <= 0 {
        return false;
    }
    let here = g.unit_plot(u);
    let (x, y) = g.map.xy(here);
    let mut best: Option<(PlotIdx, i32)> = None;
    for dir in 0..DirectionType::NUM {
        let Some(q) = g.plot_direction(x, y, DirectionType::from_usize(dir)) else { continue };
        if !g.plot_is_visible_enemy_unit_for_unit(q, u) {
            continue;
        }
        if !g.unit_can_move_or_attack_into(u, q, false) {
            continue;
        }
        let Some(def) = g.plot_get_best_defender(q, None, Some(g.units[u].owner), Some(u), false, false, false) else { continue };
        let odds = g.get_combat_odds(u, def);
        let city_bonus = if g.map.plots[q].plot_city.is_some() { 100 } else { 0 };
        let score = odds + city_bonus;
        if odds >= min_odds && best.map_or(true, |(_, s)| score > s) {
            best = Some((q, score));
        }
    }
    let _ = v;
    if let Some((q, _)) = best {
        move_to(g, grp, q, MOVE_DIRECT_ATTACK);
        true
    } else {
        false
    }
}

/// Reachable plot next to a visible enemy unit we are likely to beat, close to our lands.
fn best_hunt_target(g: &Game, v: &View, u: UnitId, reach: &[Reached]) -> Option<Reached> {
    let my_power = attack_power(g, u);
    if my_power == 0 {
        return None;
    }
    let mut best: Option<(Reached, i32)> = None;
    for r in reach {
        if r.turns == 0 || !g.plot_is_visible_enemy_unit_for_unit(r.plot, u) {
            continue;
        }
        let owner = g.map.plots[r.plot].owner;
        if owner.is_some_and(|o| o != v.p) && g.map.plots[r.plot].plot_city.is_some() {
            continue; // cities are war targets, not hunting targets
        }
        let enemy_power: i32 = g.map.plots[r.plot].units.iter().map(|&e| attack_power(g, e).max(crate::view::defense_power(g, e))).max().unwrap_or(0);
        if enemy_power * 10 > my_power * 8 {
            continue;
        }
        let score = 1000 - r.turns * 100 + if owner == Some(v.p) { 300 } else { 0 };
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((*r, score));
        }
    }
    best.map(|(r, _)| r)
}

fn explore(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    if attack_adjacent(g, v, grp, u, 750) {
        return;
    }
    if !alive(g, grp) || !g.units.contains(u) {
        return;
    }
    let team = v.team;
    let reach = v.flood.run(g, u, 8, false);
    let mut best: Option<(PlotIdx, i32)> = None;
    for r in &reach {
        if r.turns == 0 && r.plot == g.unit_plot(u) {
            continue;
        }
        if v.danger_at(r.plot) > attack_power(g, u) || v.is_claimed(r.plot) {
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
        let goody = if g.plot_is_revealed_goody(r.plot, Some(team)) { 60 } else { 0 };
        if unrevealed == 0 && goody == 0 {
            continue;
        }
        let score = (unrevealed * 10 + goody * 10) * 10 / (r.turns + 1);
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((r.plot, score));
        }
    }
    match best {
        Some((plot, _)) => {
            v.claimed.push(plot);
            move_to(g, grp, plot, 0);
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                skip(g, grp);
            }
        }
        None => {
            // Nothing left to explore: become a regular army unit.
            g.unit_ai_set_unit_ai_type(u, UnitAIType::UNITAI_ATTACK);
            if let Some(ng) = g.units[u].group {
                crate::military::army(g, v, ng, u);
            }
        }
    }
}

pub(crate) fn barbarian(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    // Barbarians and animals are aggressive: attack anything they can beat.
    if attack_adjacent(g, v, grp, u, 500) {
        return;
    }
    if !alive(g, grp) || !g.units.contains(u) {
        return;
    }
    let reach = v.flood.run(g, u, 4, true);
    if let Some(r) = best_hunt_target(g, v, u, &reach) {
        move_to(g, grp, r.plot, 0);
        if alive(g, grp) && g.group_ready_to_move(grp, false) {
            skip(g, grp);
        }
        return;
    }
    // Wander: a deterministic pseudo-random reachable plot.
    let n = reach.len();
    if n > 1 {
        let k = (u.0 as usize).wrapping_mul(2654435761).wrapping_add(g.game_turn() as usize * 40503) % n;
        let target = reach[k].plot;
        move_to(g, grp, target, 0);
        if alive(g, grp) && g.group_ready_to_move(grp, false) {
            skip(g, grp);
        }
        return;
    }
    skip(g, grp);
}

// ----------------------------------------------------------------------------
// Great people, missionaries, casters
// ----------------------------------------------------------------------------

fn great_person(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let p = v.p;
    let here = g.unit_plot(u);
    let Some(capital) = g.players[p].capital_city else {
        hold(g, grp);
        return;
    };
    let cp = g.city_plot(capital);
    if here != cp {
        move_to(g, grp, cp, MOVE_SAFE_TERRITORY);
        if alive(g, grp) && g.group_ready_to_move(grp, false) {
            skip(g, grp);
        }
        return;
    }
    // Lightbulb a tech, else settle as a super specialist, else start a golden age.
    if can_start(g, grp, MissionType::MISSION_DISCOVER, -1, -1) {
        push(g, grp, MissionType::MISSION_DISCOVER, -1, -1, 0, false);
        return;
    }
    let ut = g.units[u].unit_type;
    let specs: Vec<SpecialistType> = g.d.units[ut].great_peoples.iter_enumerated().filter(|(_, &b)| b).map(|(s, _)| s).collect();
    for s in specs {
        if can_start(g, grp, MissionType::MISSION_JOIN, s.i(), -1) {
            push(g, grp, MissionType::MISSION_JOIN, s.i(), -1, 0, false);
            return;
        }
    }
    if can_start(g, grp, MissionType::MISSION_GOLDEN_AGE, -1, -1) {
        push(g, grp, MissionType::MISSION_GOLDEN_AGE, -1, -1, 0, false);
        return;
    }
    hold(g, grp);
}

fn missionary(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let p = v.p;
    let ut = g.units[u].unit_type;
    let religions: Vec<ReligionType> = g.d.units[ut].religion_spreads.iter_enumerated().filter(|(_, &n)| n > 0).map(|(r, _)| r).collect();
    let reach = v.flood.run(g, u, 10, false);
    let mut best: Option<(PlotIdx, ReligionType, i32)> = None;
    for &c in &g.players[p].cities {
        let cp = g.city_plot(c);
        let Some(r) = reach.iter().find(|r| r.plot == cp) else { continue };
        for &rel in &religions {
            if g.cities[c].has_religion[rel] || v.is_claimed(cp) {
                continue;
            }
            let score = g.cities[c].population * 10 - r.turns * 5;
            if best.map_or(true, |(_, _, s)| score > s) {
                best = Some((cp, rel, score));
            }
        }
    }
    let Some((plot, rel, _)) = best else {
        hold(g, grp);
        return;
    };
    v.claimed.push(plot);
    if plot != g.unit_plot(u) {
        move_to(g, grp, plot, MOVE_SAFE_TERRITORY);
        if !alive(g, grp) {
            return;
        }
        push(g, grp, MissionType::MISSION_SPREAD, rel.i(), -1, 0, true);
    } else {
        push(g, grp, MissionType::MISSION_SPREAD, rel.i(), -1, 0, false);
    }
    if alive(g, grp) && g.group_ready_to_move(grp, false) {
        skip(g, grp);
    }
}

/// Casters stay where the fighting is (spells are cast at the end of the turn, see
/// `casting.rs`): with the army when at war, else in the most threatened city.
fn caster(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let p = v.p;
    let here = g.unit_plot(u);
    if let Some(target) = crate::military::war_target(g, v) {
        let tp = g.city_plot(target);
        let (tx, ty) = g.map.xy(tp);
        let reach = v.flood.run(g, u, 15, false);
        if let Some(r) = reach.iter().filter(|r| {
            let (rx, ry) = g.map.xy(r.plot);
            g.plot_distance(rx, ry, tx, ty) == 1 && g.map.plots[r.plot].units.iter().any(|&o| g.units[o].owner == p && attack_power(g, o) > 0)
        }).min_by_key(|r| (r.turns, r.plot)) {
            let plot = r.plot;
            if plot != here {
                move_to(g, grp, plot, 0);
            }
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                skip(g, grp);
            }
            return;
        }
    }
    let worst = g.players[p].cities.iter().copied().max_by_key(|&c| (v.danger_at(g.city_plot(c)), std::cmp::Reverse(c)));
    match worst {
        Some(c) if g.city_plot(c) != here => {
            move_to(g, grp, g.city_plot(c), MOVE_SAFE_TERRITORY);
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                skip(g, grp);
            }
        }
        _ => skip(g, grp),
    }
}
