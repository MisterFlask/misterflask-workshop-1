//! Field armies: stacks that hunt barbarians and animals near our lands, clear lairs (an AI
//! unit entering a lair destroys it, FfH `CvUnit::setXY`), and take barbarian and enemy cities
//! in coordinated operations (`campaign.rs`).
//!
//! A stack is a selection group led by an army unit; army units meeting on a plot merge (up
//! to [`MAX_STACK`]). Attacks go through the rules' group attack (`MISSION_MOVE_TO` with
//! `MOVE_DIRECT_ATTACK`: the best attacker fights, or a sacrifice when the odds are poor),
//! repeated while the odds of the best attacker against the best defender, given how many
//! attackers remain for how many defenders, make the assault worth it (C++
//! `AI_groupAttack`/`AI_attackCityMove` follow-through).

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::production::garrison;
use crate::units::{alive, hold, move_to, nearest_city, skip};
use crate::view::{attack_power, defense_power, View};

/// Largest stack we build.
pub const MAX_STACK: usize = 8;

/// A target for a field army (computed once per turn in the view).
#[derive(Copy, Clone, Debug)]
pub struct Target {
    pub plot: PlotIdx,
    pub value: i32,
    /// Enemy (non-animal or animal) units visible there.
    pub units: i32,
    pub city: bool,
    pub lair: bool,
}

pub fn is_army_unit(g: &Game, u: UnitId) -> bool {
    let unit = &g.units[u];
    let info = &g.d.units[unit.unit_type];
    info.combat > 0
        && !info.found
        && g.unit_get_domain_type(u) == DomainType::DOMAIN_LAND
        && matches!(
            unit.ai_type,
            Some(UnitAIType::UNITAI_ATTACK) | Some(UnitAIType::UNITAI_ATTACK_CITY) | Some(UnitAIType::UNITAI_COLLATERAL) | Some(UnitAIType::UNITAI_COUNTER) | Some(UnitAIType::UNITAI_RESERVE) | Some(UnitAIType::UNITAI_PILLAGE)
        )
}

/// Units of the group able to attack now.
fn attackers(g: &Game, grp: GroupId) -> Vec<UnitId> {
    g.groups[grp].units.iter().copied().filter(|&u| attack_power(g, u) > 0 && g.unit_can_attack(u) && g.unit_moves_left(u) > 0).collect()
}

/// Fight estimate against a plot.
#[derive(Copy, Clone, Debug)]
pub struct Assessment {
    /// Best odds (per mille) of one of our attackers against their best defender.
    pub best: i32,
    /// Defenders there.
    pub n_def: i32,
    /// Our attackers able to fight now.
    pub n_att: i32,
    /// Expected "defender kills" of the whole stack, per mille: each attacker counts its odds
    /// plus a third of the rest (a lost fight still wounds the defender); collateral
    /// attackers count more against stacks.
    pub effective: i32,
}

/// Fight estimate against plot `q`.
pub(crate) fn assess(g: &Game, grp: GroupId, q: PlotIdx) -> Option<Assessment> {
    let mut atts = attackers(g, grp);
    if atts.is_empty() {
        return None;
    }
    atts.sort_by_key(|&u| (std::cmp::Reverse(attack_power(g, u)), u));
    let owner = g.groups[grp].owner;
    let mut best = 0;
    let mut n_def = 0;
    let mut effective = 0;
    let mut any = false;
    for &u in atts.iter().take(MAX_STACK) {
        if !g.unit_can_move_or_attack_into(u, q, false) {
            continue;
        }
        any = true;
        let Some(def) = g.plot_get_best_defender(q, None, Some(owner), Some(u), false, false, false) else {
            return Some(Assessment { best: 1000, n_def: 0, n_att: atts.len() as i32, effective: 1000 * atts.len() as i32 });
        };
        let odds = g.get_combat_odds(u, def);
        best = best.max(odds);
        n_def = n_def.max(g.plot_get_num_visible_enemy_defenders(q, u));
        effective += odds + (1000 - odds) / 3;
        if g.d.units[g.units[u].unit_type].collateral_damage > 0 {
            effective += 250;
        }
    }
    if !any {
        return None;
    }
    Some(Assessment { best, n_def: n_def.max(1), n_att: atts.len() as i32, effective })
}

/// Is an assault worth it?
pub fn attack_ok(a: &Assessment, city: bool) -> bool {
    if a.best >= 650 {
        return true;
    }
    let need = a.n_def * 1000 + if city { 300 } else { 0 };
    if a.best < 100 {
        return false;
    }
    a.effective >= need && (a.best >= 300 || a.n_att >= a.n_def + 2)
}

/// Attack plot `q` with the group until it falls or the odds turn bad. True if we fought.
pub fn assault(g: &mut Game, v: &View, grp: GroupId, q: PlotIdx) -> bool {
    let mut fought = false;
    let city = g.map.plots[q].plot_city.is_some();
    // Casters of ours around the target cast first when the assault goes ahead (magic.rs).
    if let Some(a) = assess(g, grp, q) {
        if a.n_def > 0 && attack_ok(&a, city) {
            let atts = attackers(g, grp);
            crate::magic::before_attack(g, v, q, &atts);
        }
    }
    for _ in 0..12 {
        if !alive(g, grp) {
            break;
        }
        let Some(head) = g.group_get_head_unit(grp) else { break };
        if !g.plot_is_visible_enemy_unit_for_unit(q, head) {
            break;
        }
        let Some(a) = assess(g, grp, q) else { break };
        let (n_def, n_att) = (a.n_def, a.n_att);
        if n_def == 0 {
            break;
        }
        if !attack_ok(&a, city) {
            break;
        }
        g.groups[grp].ai_group_attack = false;
        let before = g.groups[grp].units.len();
        move_to(g, grp, q, MOVE_DIRECT_ATTACK);
        fought = true;
        if !alive(g, grp) {
            break;
        }
        // Nothing happened (no attack possible): stop.
        if g.groups[grp].units.len() == before && attackers(g, grp).len() as i32 == n_att {
            break;
        }
    }
    if alive(g, grp) {
        g.groups[grp].ai_group_attack = false;
        // The defenders are gone: move in (capture a city, destroy a lair, take workers).
        if let Some(head) = g.group_get_head_unit(grp) {
            if g.unit_plot(head) != q && !g.plot_is_visible_enemy_unit_for_unit(q, head) && g.unit_moves_left(head) > 0 && g.group_can_move_into(grp, q, false) {
                move_to(g, grp, q, 0);
            }
        }
    }
    fought
}

/// Merge other army units on this plot into `grp` (up to [`MAX_STACK`]).
fn merge_here(g: &mut Game, v: &View, grp: GroupId, here: PlotIdx) {
    if g.groups[grp].units.len() >= MAX_STACK {
        return;
    }
    let others: Vec<UnitId> = g.map.plots[here].units.iter().copied().filter(|&o| g.units[o].owner == v.p && g.units[o].group != Some(grp) && is_army_unit(g, o)).collect();
    for o in others {
        if g.groups[grp].units.len() >= MAX_STACK {
            break;
        }
        // Only fresh units join (a group whose members have different moves left would keep
        // the turn from ending).
        if g.unit_plot(o) == here && g.unit_moves_left(o) == g.unit_max_moves(o) && g.unit_can_join_group(o, here, grp) {
            g.unit_join_group(o, Some(grp), true);
        }
    }
}

/// Enemy city our army marches on: this turn's war operation (`campaign.rs`), else the
/// closest known enemy city to any of our cities.
pub fn war_target(g: &Game, v: &View) -> Option<CityId> {
    // (The plan was made earlier this turn: the city may have fallen or been razed since.)
    if let Some(c) = v.campaign.war_target().filter(|&c| g.cities.contains(c) && g.cities[c].owner != v.p) {
        return Some(c);
    }
    let preparing = crate::diplomacy::preparing_target(g, v.team);
    if v.civ_enemies.is_empty() && preparing.is_none() {
        return None;
    }
    let ours: Vec<(i32, i32, Option<AreaId>)> = g.players[v.p].cities.iter().map(|&c| (g.cities[c].x, g.cities[c].y, g.map.plots[g.city_plot(c)].area)).collect();
    let mut best: Option<(CityId, i32)> = None;
    for pl in g.players.iter() {
        if !pl.alive || !(v.civ_enemies.contains(&pl.team) || v.civ_enemies.is_empty() && preparing == Some(pl.team)) {
            continue;
        }
        for &c in &pl.cities {
            let cp = g.city_plot(c);
            if !g.map.plots[cp].revealed[v.team] {
                continue;
            }
            let (cx, cy) = (g.cities[c].x, g.cities[c].y);
            let area = g.map.plots[cp].area;
            let Some(dist) = ours.iter().filter(|o| o.2 == area).map(|&(x, y, _)| g.plot_distance(x, y, cx, cy)).min() else { continue };
            if best.map_or(true, |(bc, bd)| dist < bd || (dist == bd && c < bc)) {
                best = Some((c, dist));
            }
        }
    }
    best.map(|(c, _)| c)
}

/// Stack strength in rough power units (sum of attack power of its fighters).
fn stack_power(g: &Game, grp: GroupId) -> i32 {
    g.groups[grp].units.iter().map(|&u| attack_power(g, u)).sum()
}

/// One decision for an army stack led by `u`.
/// Diagnostic (`FFH_AI_EXPOSURE=1`): report army stacks of `p` left on a plot that a *much
/// stronger* visible enemy unit can reach next turn -- the single strongest enemy attacker in
/// reach out-powers our best defender by half again (`maxatt > 1.5x bestdef`), the "obviously
/// about to be killed" case. `natt` is how many enemy attackers reach the plot and `wipe` whether
/// there are enough to kill every unit in the stack. Skips a garrison in one of our cities and a
/// stack next to its target about to strike this turn; `ctx=op` marks a stack marching/striking in
/// an operation, `ctx=loose` one not in any. Compares single units (not summed power, which
/// over-counts many weak attackers a strong defender beats one at a time). Pure read, printed only
/// when the env var is set, so it has no cost or effect otherwise. Called once per player's turn.
pub fn report_exposure(g: &Game, v: &View, p: PlayerId) {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    if !*ON.get_or_init(|| std::env::var("FFH_AI_EXPOSURE").is_ok()) {
        return;
    }
    let mut seen = std::collections::BTreeSet::new();
    for &u in &g.players[p].units {
        if !g.units.contains(u) {
            continue;
        }
        let Some(grp) = g.units[u].group else { continue };
        if !seen.insert(grp) || !g.groups[grp].units.iter().any(|&o| is_army_unit(g, o)) {
            continue;
        }
        let head = g.group_get_head_unit(grp).unwrap_or(u);
        let here = g.unit_plot(head);
        if g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == p) || crate::campaign::striking(g, v, grp) {
            continue;
        }
        let (hx, hy) = g.map.xy(here);
        let best_def = g.groups[grp].units.iter().map(|&o| defense_power(g, o)).max().unwrap_or(0).max(1);
        // Strongest single visible enemy attacker that can reach this plot next turn, and how many.
        let mut max_att = 0;
        let mut n_att = 0;
        for pl in g.players.iter() {
            if !pl.alive || !v.enemies.contains(&pl.team) {
                continue;
            }
            for &e in &pl.units {
                if !g.units.contains(e) {
                    continue;
                }
                let eu = &g.units[e];
                let att = attack_power(g, e);
                if att == 0 {
                    continue;
                }
                let Some(ep) = g.map.plot_xy(eu.x, eu.y) else { continue };
                if !g.plot_is_visible(ep, v.team) {
                    continue;
                }
                let range = (g.d.units[eu.unit_type].moves.max(1) + 1).min(4);
                if g.plot_distance(eu.x, eu.y, hx, hy) <= range {
                    max_att = max_att.max(att);
                    n_att += 1;
                }
            }
        }
        if max_att * 2 > best_def * 3 {
            let ctx = if crate::campaign::on_the_move(v, grp) { "op" } else { "loose" };
            eprintln!(
                "[EXPOSED t{} p{} grp{}] bestdef={} maxatt={} ratio={} natt={} units={} wipe={} ctx={}",
                g.game_turn(), p.0, grp.0, best_def, max_att, max_att * 100 / best_def, n_att, g.groups[grp].units.len(), n_att >= g.groups[grp].units.len(), ctx
            );
        }
    }
}

pub fn army(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let p = v.p;
    let here = g.unit_plot(u);
    if v.barbarian {
        crate::units::barbarian(g, v, grp, u);
        return;
    }

    // A group attack the rules queued (AI follow-through).
    if g.groups[grp].ai_group_attack {
        let (x, y) = (g.groups[grp].ai_group_attack_x, g.groups[grp].ai_group_attack_y);
        g.groups[grp].ai_group_attack = false;
        if let Some(q) = g.map.plot_xy(x, y) {
            if assault(g, v, grp, q) {
                finish(g, grp);
                return;
            }
        }
    }

    // An undefended city of ours (a fresh one, typically founded by the settler we escorted):
    // its first garrison.
    if let Some(c) = g.map.plots[here].plot_city.filter(|&c| g.cities[c].owner == p) {
        if garrison(g, p, here).0 == 0 && v.garrison.get(&c).copied().unwrap_or(0) == 0 {
            *v.garrison.entry(c).or_insert(0) += 1;
            g.unit_ai_set_unit_ai_type(u, UnitAIType::UNITAI_CITY_DEFENSE);
            if let Some(ng) = g.units[u].group {
                hold(g, ng);
            }
            return;
        }
    }

    merge_here(g, v, grp, here);

    // Heal before anything but a sure kill.
    let max_hp = g.d.defines.max_hit_points.max(1);
    let hurt = g.groups[grp].units.iter().any(|&o| g.units[o].damage * 100 / max_hp > 35);

    // Adjacent targets first (the operation's target city is left to its strike).
    crate::campaign::plan(g, v);
    let op_target = v.campaign.ops.first().map(|o| o.plot);
    if let Some(q) = best_adjacent(g, grp, here, hurt, op_target) {
        assault(g, v, grp, q);
        finish(g, grp);
        return;
    }
    if !alive(g, grp) {
        return;
    }
    // (Not a stack about to strike: the strike estimate counts its wounds, and the city's
    // defenders heal faster than we do.)
    if hurt && !crate::campaign::striking(g, v, grp) && v.danger_at(here) * 2 < g.groups[grp].units.iter().map(|&o| defense_power(g, o)).sum::<i32>().max(1) * 3 {
        if g.group_can_start_mission(grp, MissionType::MISSION_HEAL, -1, -1, None, false) {
            crate::units::push(g, grp, MissionType::MISSION_HEAL, -1, -1, 0, false);
            return;
        }
    }

    // A city of ours in peril within a few turns: defend it (unless the stack is on its way to
    // a siege: campaign.rs).
    if !v.peril.is_empty() && !crate::campaign::on_the_move(v, grp) {
        let reach = v.flood.run(g, u, 5, false);
        if let Some(r) = v.peril.iter().filter_map(|&(q, _)| reach.iter().find(|r| r.plot == q).copied()).min_by_key(|r| (r.turns, r.plot)) {
            crate::trace!(g, p, "stack {:?} defends city at {:?}", grp, g.map.xy(r.plot));
            if r.plot == here {
                hold(g, grp);
            } else {
                move_to(g, grp, r.plot, 0);
                finish(g, grp);
            }
            return;
        }
    }

    // A settler waits for an escort.
    if !v.escort_requests.is_empty() && g.groups[grp].units.len() == 1 {
        let reach = v.flood.run(g, u, 6, false);
        if let Some((i, r)) = v.escort_requests.iter().enumerate().filter_map(|(i, &q)| reach.iter().find(|r| r.plot == q).map(|r| (i, *r))).min_by_key(|(_, r)| (r.turns, r.plot)) {
            v.escort_requests.remove(i);
            if r.plot != here {
                move_to(g, grp, r.plot, 0);
            }
            finish(g, grp);
            return;
        }
    }

    // An amphibious invasion (naval.rs): a committed stack boards the transport in port to
    // cross the water; a stack already landed on the enemy's landmass takes the overseas city
    // (which `campaign.rs` cannot, as it only targets landmasses where we hold a city).
    if crate::naval::amphibious(g, v, grp, u) {
        return;
    }

    // War (or its preparation): the operation against the chosen enemy city; stacks close to
    // a barbarian target act on its operation before hunting (campaign.rs).
    if crate::campaign::act(g, v, grp, u, true) {
        return;
    }

    // Hunt: the best target in reach that the stack can take.
    if hunt(g, v, grp, u) {
        return;
    }

    // Peacetime campaign against a barbarian city (campaign.rs).
    if crate::campaign::act(g, v, grp, u, false) {
        return;
    }

    // Gather: join the nearest other stack within a few turns, else hold in the nearest city.
    if g.groups[grp].units.len() < MAX_STACK {
        let reach = v.flood.run(g, u, 3, false);
        let mut best: Option<(PlotIdx, i32)> = None;
        for r in &reach {
            if r.plot == here {
                continue;
            }
            let n = g.map.plots[r.plot].units.iter().filter(|&&o| g.units[o].owner == p && is_army_unit(g, o)).count() as i32;
            if n == 0 || n as usize >= MAX_STACK {
                continue;
            }
            // Smaller stacks walk to bigger ones (ties: lower plot index gathers).
            let mine = g.groups[grp].units.len() as i32;
            if n < mine || (n == mine && r.plot.idx() > here.idx()) {
                continue;
            }
            let score = n * 10 - r.turns * 5;
            if best.map_or(true, |(_, s)| score > s) {
                best = Some((r.plot, score));
            }
        }
        if let Some((q, _)) = best {
            move_to(g, grp, q, 0);
            finish(g, grp);
            return;
        }
    }
    crate::trace!(g, p, "stack {:?} ({} units, power {}) idle at {:?}; {} targets", grp, g.groups[grp].units.len(), stack_power(g, grp), g.map.xy(here), v.targets.len());
    if g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == p) {
        hold(g, grp);
    } else if let Some(c) = nearest_city(g, p, here) {
        move_to(g, grp, g.city_plot(c), 0);
        finish(g, grp);
    } else {
        hold(g, grp);
    }
}

/// Make sure the group's turn ends.
fn finish(g: &mut Game, grp: GroupId) {
    if alive(g, grp) && g.group_ready_to_move(grp, true) {
        skip(g, grp);
    }
}

/// Best adjacent enemy plot worth attacking now.
fn best_adjacent(g: &Game, grp: GroupId, here: PlotIdx, hurt: bool, skip: Option<PlotIdx>) -> Option<PlotIdx> {
    let head = g.group_get_head_unit(grp)?;
    let (x, y) = g.map.xy(here);
    let mut best: Option<(PlotIdx, i32)> = None;
    for dir in 0..DirectionType::NUM {
        let Some(q) = g.plot_direction(x, y, DirectionType::from_usize(dir)) else { continue };
        if !g.plot_is_visible_enemy_unit_for_unit(q, head) || skip == Some(q) {
            continue;
        }
        // Do not attack into civ cities unless that is the plan (campaign.rs does).
        let city = g.map.plots[q].plot_city;
        if city.is_some_and(|c| !g.player_is_barbarian(g.cities[c].owner)) {
            continue;
        }
        let Some(a) = assess(g, grp, q) else { continue };
        let ok = if hurt { a.best >= 750 } else { attack_ok(&a, city.is_some()) };
        if !ok {
            continue;
        }
        let score = a.best + if city.is_some() { 300 } else { 0 };
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((q, score));
        }
    }
    best.map(|(q, _)| q)
}

/// Peacetime (and barbarian) targets: move toward the best one we can take.
fn hunt(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
    if v.targets.is_empty() {
        return false;
    }
    let here = g.unit_plot(u);
    let power = stack_power(g, grp);
    let n_att = attackers(g, grp).len() as i32;
    if n_att == 0 {
        return false;
    }
    let reach = v.flood.run(g, u, 6, true);
    let mut cands: Vec<(i32, usize, i32)> = Vec::new(); // (score, target index, turns)
    for (i, t) in v.targets.iter().enumerate() {
        let Some(r) = reach.iter().find(|r| r.plot == t.plot) else { continue };
        if v.target_claims.iter().any(|&(q, _)| q == t.plot) && !t.city {
            continue;
        }
        // The operation's target is taken by the operation.
        if t.city && v.campaign.ops.iter().any(|o| o.plot == t.plot) {
            continue;
        }
        // Rough feasibility before exact odds: their strongest defender against our stack.
        let their: i32 = g.map.plots[t.plot].units.iter().filter(|&&e| g.units[e].owner != v.p).map(|&e| defense_power(g, e)).max().unwrap_or(0);
        if their > 0 && their * t.units.max(1) > power * 2 {
            continue;
        }
        if t.city && n_att < 3 {
            continue;
        }
        let score = t.value * 10 / (r.turns + 2);
        cands.push((score, i, r.turns));
    }
    cands.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for &(_, i, _) in cands.iter().take(4) {
        let t = v.targets[i];
        // Exact check where it matters: the best odds against what stands there.
        if t.units > 0 {
            match assess(g, grp, t.plot) {
                Some(a) if attack_ok(&a, t.city) || a.best >= 300 && a.n_att >= 2 => {}
                _ => continue,
            }
        }
        v.target_claims.push((t.plot, grp));
        crate::trace!(g, v.p, "stack {:?} ({} units) hunts {:?} (value {}, units {}, lair {}, city {})", grp, g.groups[grp].units.len(), g.map.xy(t.plot), t.value, t.units, t.lair, t.city);
        let (tx, ty) = g.map.xy(t.plot);
        let (x, y) = g.map.xy(here);
        if g.plot_distance(x, y, tx, ty) <= 1 && t.units > 0 {
            assault(g, v, grp, t.plot);
        } else if t.units == 0 {
            move_to(g, grp, t.plot, 0);
        } else {
            // Next to it, on the best defensive ground we reach first.
            let reach = v.flood.run(g, u, 6, false);
            let staging = reach
                .iter()
                .filter(|r| {
                    let (rx, ry) = g.map.xy(r.plot);
                    g.plot_distance(rx, ry, tx, ty) == 1 && g.map.plots[r.plot].plot_city.map_or(true, |c| g.cities[c].owner == v.p)
                })
                .min_by_key(|r| (r.turns, std::cmp::Reverse(g.plot_defense_modifier(r.plot, Some(v.team), true)), r.plot))
                .copied();
            match staging {
                Some(r) if r.plot != here => move_to(g, grp, r.plot, 0),
                _ => {}
            }
        }
        finish(g, grp);
        return true;
    }
    false
}

/// Targets for field armies near our lands: enemy units outside civ cities, lairs, barbarian
/// cities.
pub fn find_targets(g: &Game, v: &View) -> Vec<Target> {
    let p = v.p;
    let pl = &g.players[p];
    let ours: Vec<(i32, i32)> = pl.cities.iter().map(|&c| (g.cities[c].x, g.cities[c].y)).collect();
    if ours.is_empty() {
        return Vec::new();
    }
    let near = |x: i32, y: i32, r: i32| ours.iter().map(|&(cx, cy)| g.plot_distance(x, y, cx, cy)).min().unwrap_or(i32::MAX) <= r;
    let mut out: Vec<Target> = Vec::new();
    // Enemy units (visible).
    for opl in g.players.iter() {
        if !opl.alive || !v.enemies.contains(&opl.team) {
            continue;
        }
        for &e in &opl.units {
            let unit = &g.units[e];
            let Some(q) = g.map.plot_xy(unit.x, unit.y) else { continue };
            if !g.plot_is_visible(q, v.team) || g.plot_is_water(q) {
                continue;
            }
            if out.iter().any(|t| t.plot == q) {
                continue;
            }
            let city = g.map.plots[q].plot_city;
            if city.is_some_and(|c| !g.player_is_barbarian(g.cities[c].owner)) {
                continue; // civ cities: war logic
            }
            let in_ours = g.map.plots[q].owner == Some(p);
            if !in_ours && !near(unit.x, unit.y, if city.is_some() { 10 } else { 6 }) {
                continue;
            }
            let n = g.map.plots[q].units.iter().filter(|&&o| g.units[o].owner == unit.owner).count() as i32;
            let lair = g.map.plots[q].improvement_type.is_some_and(|i| g.d.improvements[i].spawn_unit_type.is_some());
            let mut value = 150 + 30 * n;
            if in_ours {
                value += 250;
            }
            if lair {
                value += 250;
            }
            if city.is_some() {
                value += 700;
            }
            out.push(Target { plot: q, value, units: n, city: city.is_some(), lair });
        }
    }
    // Empty lairs near our lands.
    let barb_team = g.barbarian_team();
    if v.enemies.contains(&barb_team) {
        for &(cx, cy) in &ours {
            for dx in -6..=6 {
                for dy in -6..=6 {
                    let Some(q) = g.plot_xy(cx, cy, dx, dy) else { continue };
                    let plot = &g.map.plots[q];
                    if !plot.revealed[v.team] || plot.owner.is_some_and(|o| o != p) {
                        continue;
                    }
                    let Some(imp) = plot.improvement_type else { continue };
                    if g.d.improvements[imp].spawn_unit_type.is_none() || out.iter().any(|t| t.plot == q) {
                        continue;
                    }
                    out.push(Target { plot: q, value: 300, units: 0, city: false, lair: true });
                }
            }
        }
    }
    out
}
