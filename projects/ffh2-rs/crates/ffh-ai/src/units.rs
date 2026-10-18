//! Unit control: every group that needs orders this turn gets exactly one decision by its
//! head unit's role (`Unit::ai_type`). Decisions are issued as missions through the rules
//! (`pushMission`), so the AI can only do what a human could.

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::production::garrison;
use crate::reach::Reached;
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
    Transport,
    SeaExplore,
    Naval,
    Other,
}

fn role_of(g: &Game, u: UnitId) -> Role {
    let unit = &g.units[u];
    let info = &g.d.units[unit.unit_type];
    if g.unit_get_domain_type(u) == DomainType::DOMAIN_SEA {
        return if matches!(unit.ai_type, Some(UnitAIType::UNITAI_WORKER_SEA)) || info.unit_ai_type[UnitAIType::UNITAI_WORKER_SEA] && info.combat == 0 {
            Role::SeaWorker
        } else if unit.ai_type == Some(UnitAIType::UNITAI_EXPLORE_SEA) {
            Role::SeaExplore
        } else if crate::naval::is_transport_type(info) {
            Role::Transport
        } else {
            Role::Naval
        };
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
    // Promotions and upgrades for units that will not get orders this turn (fortified
    // garrisons, sentries); the others get theirs before acting.
    if !v.barbarian {
        for u in g.players[p].units.clone() {
            if !g.units.contains(u) {
                continue;
            }
            let Some(grp) = g.units[u].group else { continue };
            if g.group_ready_to_move(grp, false) {
                continue;
            }
            upkeep(g, u);
        }
    }
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
        // A settler never sleeps through a turn (a site may have opened up); cargo excepted.
        let sleeping_settler = role == Role::Settler
            && g.groups[grp].activity_type == ActivityType::ACTIVITY_SLEEP
            && g.group_get_head_unit(grp).is_some_and(|h| !g.unit_is_cargo(h));
        // Units aboard a ship go where the ship takes them (the transport lands them).
        if g.group_get_head_unit(grp).is_some_and(|h| g.unit_is_cargo(h) && g.unit_get_domain_type(h) == DomainType::DOMAIN_LAND) {
            g.groups[grp].force_update = false;
            continue;
        }
        if g.groups[grp].force_update || replan || sleeping_settler {
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
        if !v.barbarian && crate::civs::unit(ai, g, v, grp, head) { continue; } // civ-mechanics
        match role {
            Role::Settler => crate::expansion::settler(ai, g, v, grp, head),
            Role::Worker => crate::workers::worker(g, v, grp, head, false),
            Role::SeaWorker => crate::workers::worker(g, v, grp, head, true),
            Role::Garrison => garrison_unit(g, v, grp, head),
            Role::Army => crate::military::army(g, v, grp, head),
            Role::Explore => explore(g, v, grp, head),
            Role::GreatPerson => great_person(g, v, grp, head),
            Role::Missionary => missionary(g, v, grp, head),
            Role::Caster => caster(g, v, grp, head),
            Role::Animal => barbarian(g, v, grp, head),
            Role::Transport if !v.barbarian => crate::naval::transport(ai, g, v, grp, head),
            Role::SeaExplore if !v.barbarian => crate::naval::sea_explore(ai, g, v, grp, head),
            Role::Transport | Role::SeaExplore => hold(g, grp),
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
        let defender = matches!(g.units[u].ai_type, Some(UnitAIType::UNITAI_CITY_DEFENSE) | Some(UnitAIType::UNITAI_CITY_COUNTER));
        let power = if defender { crate::production::type_city_defense(&g.d.units[target]) as i32 } else { crate::view::type_attack_power(g.d, target) };
        if best.map_or(true, |(_, b)| power > b) {
            best = Some((target, power));
        }
    }
    if let Some((target, _)) = best {
        // Keep a reserve for a few turns of deficit; defenders first.
        let reserve = 20 + 3 * g.players[p].cities.len() as i32;
        let defender = matches!(g.units[u].ai_type, Some(UnitAIType::UNITAI_CITY_DEFENSE) | Some(UnitAIType::UNITAI_CITY_COUNTER));
        let price = g.unit_upgrade_price(u, target);
        if g.players[p].gold - price >= if defender { reserve } else { reserve * 2 } {
            crate::trace!(g, p, "upgrade {} -> {} ({} gold)", g.d.units[ut].base.type_, g.d.units[target].base.type_, price);
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
        let v = crate::civs::promotion_value(g, u, promo, promotion_value(g, u, promo)); // civ-mechanics
        if best.map_or(true, |(_, b)| v > b) {
            best = Some((promo, v));
        }
    }
    best.map(|(p, _)| p)
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
    match explore_target(g, v, u) {
        Some(plot) => {
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

/// The land explorer's next goal: the reachable plot (8 turns) that reveals the most unrevealed
/// plots or a goody hut, per turn of travel, away from danger and other explorers' goals.
pub(crate) fn explore_target(g: &Game, v: &mut View, u: UnitId) -> Option<PlotIdx> {
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
    best.map(|(plot, _)| plot)
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
    if crate::victory::great_work(g, v, grp, u) { return; } // victory-pursuit
    let here = g.unit_plot(u);
    let Some(capital) = g.player_get_capital_city(p) else {
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

pub(crate) fn missionary(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
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
    if crate::victory::mana_node(g, v, grp, u) { return; } // victory-pursuit
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
