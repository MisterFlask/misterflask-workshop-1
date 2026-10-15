//! Diplomacy: war plans, war declarations, peace, which deals to accept.
//!
//! - The rules keep the C++ AI counters (`team_ai_do_counter`: at war / at peace / war plan
//!   state); war success is counted by combat.
//! - War (C++ `AI_doWar`, simplified): when at peace, every few turns an aggressive enough
//!   leader picks a reachable, weaker, disliked neighbour and starts preparing
//!   (`WARPLAN_PREPARING_TOTAL`: production builds an army, stacks gather on the rally city
//!   nearest the target); the war is declared once the army is ready (or preparation drags on).
//!   The willingness is a deterministic function of the turn and the leader's `iMaxWarRand`,
//!   so the AI draws nothing from the game RNG.
//! - Peace: when the war goes badly (war success and power against us), or drags on without
//!   gains, a peace treaty is offered through a real deal the other side's controller judges.
//!
//! Relations are a small explicit score (alignment, religion, shared borders, war history)
//! instead of the C++ attitude machinery.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::TradeData;
use ffh_data::*;

use crate::view::View;

/// Relation of player `p` towards `other`: < 0 hostile, > 0 friendly.
pub fn relation(g: &Game, p: PlayerId, other: PlayerId) -> i32 {
    let a = &g.players[p];
    let b = &g.players[other];
    let mut r = 0;
    match (a.alignment, b.alignment) {
        (Some(x), Some(y)) if x != y => {
            r -= (x.idx() as i32 - y.idx() as i32).abs() * 2;
        }
        (Some(_), Some(_)) => r += 1,
        _ => {}
    }
    match (g.player_get_state_religion(p), g.player_get_state_religion(other)) {
        (Some(x), Some(y)) if x == y => r += 2,
        (Some(_), Some(_)) => r -= 1,
        _ => {}
    }
    let (ta, tb) = (a.team, b.team);
    if g.teams[ta].ai_at_war_counter[tb] > 0 {
        r -= 1;
    }
    if g.team_is_open_borders(ta, tb) {
        r += 1;
    }
    r
}

/// Attitude from the relation score (C++ `AI_getAttitude` thresholds are not reproduced).
pub fn attitude(g: &Game, p: PlayerId, towards: PlayerId) -> AttitudeType {
    if p == towards {
        return AttitudeType::ATTITUDE_FRIENDLY;
    }
    let mut r = relation(g, p, towards);
    if g.at_war(g.players[p].team, g.players[towards].team) {
        r -= 2;
    }
    match r {
        i32::MIN..=-4 => AttitudeType::ATTITUDE_FURIOUS,
        -3..=-1 => AttitudeType::ATTITUDE_ANNOYED,
        0 => AttitudeType::ATTITUDE_CAUTIOUS,
        1..=2 => AttitudeType::ATTITUDE_PLEASED,
        _ => AttitudeType::ATTITUDE_FRIENDLY,
    }
}

fn is_team_leader(g: &Game, p: PlayerId) -> bool {
    let team = g.players[p].team;
    g.player_ids().into_iter().find(|&q| g.players[q].alive && g.players[q].team == team) == Some(p)
}

/// The alive (non-barbarian) leader player of a team.
fn team_leader(g: &Game, t: TeamId) -> Option<PlayerId> {
    g.player_ids().into_iter().find(|&q| g.players[q].alive && g.players[q].team == t)
}

/// Our attack units (army roles), alive or in training.
pub fn army_size(g: &Game, p: PlayerId) -> i32 {
    let pl = &g.players[p];
    let mut n = 0;
    for a in [UnitAIType::UNITAI_ATTACK, UnitAIType::UNITAI_ATTACK_CITY, UnitAIType::UNITAI_COLLATERAL, UnitAIType::UNITAI_COUNTER] {
        n += pl.ai_num_ai_units[a];
    }
    n
}

/// Army we want before declaring a prepared war.
pub fn army_needed_for_war(g: &Game, p: PlayerId, target: TeamId) -> i32 {
    let their_cities = g.teams[target].num_cities;
    (4 + g.players[p].cities.len() as i32 / 2 + their_cities / 2).min(14)
}

/// The team we are preparing war against (C++ `WARPLAN_PREPARING_*`).
pub fn preparing_target(g: &Game, team: TeamId) -> Option<TeamId> {
    g.team_ids().into_iter().find(|&o| o != team && g.team_is_alive(o) && !g.teams[team].at_war[o] && g.team_ai_is_sneak_attack_preparing(team, o))
}

/// Distance (plots) from our nearest city to their nearest city, if both have cities in a
/// shared land area.
fn reach_distance(g: &Game, team: TeamId, other: TeamId) -> Option<i32> {
    let mut best: Option<i32> = None;
    for p in g.player_ids() {
        if !g.players[p].alive || g.players[p].team != team {
            continue;
        }
        for &c in &g.players[p].cities {
            let ac = g.map.plots[g.city_plot(c)].area;
            for q in g.player_ids() {
                if !g.players[q].alive || g.players[q].team != other {
                    continue;
                }
                for &o in &g.players[q].cities {
                    if g.map.plots[g.city_plot(o)].area != ac {
                        continue;
                    }
                    let d = g.plot_distance(g.cities[c].x, g.cities[c].y, g.cities[o].x, g.cities[o].y);
                    best = Some(best.map_or(d, |b: i32| b.min(d)));
                }
            }
        }
    }
    best
}

pub fn manage(g: &mut Game, v: &View) {
    let p = v.p;
    if v.barbarian || !is_team_leader(g, p) {
        return;
    }
    let team = v.team;
    let turn = g.game_turn();

    let plan = crate::victory::plan(g, p); // victory-pursuit
    // Peace when the war goes badly or drags on without gains.
    for &enemy in &v.civ_enemies {
        if want_peace(g, team, enemy, &plan) {
            let Some(other) = team_leader(g, enemy).filter(|&q| !g.players[q].human) else { continue };
            let item = TradeData { item_type: TradeableItem::TRADE_PEACE_TREATY, data: 0, offering: false, hidden: false };
            if !g.player_can_trade_item(p, other, &item, true) {
                continue;
            }
            let ours_list = vec![item.clone()];
            let theirs_list = vec![item];
            let accept = g.controller(other).consider_offer(g, other, p, &ours_list, &theirs_list);
            if accept {
                crate::trace!(g, p, "peace with team {:?} after {} turns", enemy, g.teams[team].ai_at_war_counter[enemy]);
                g.implement_deal(p, other, &ours_list, &theirs_list, false);
            }
        }
    }

    // Trade with friendly neighbours every few turns (staggered): technologies swapped one for
    // one at similar cost, open borders.
    if (turn + p.0 as i32) % 5 == 0 {
        trade(g, p);
    }

    // One war at a time.
    if !v.civ_enemies.is_empty() {
        return;
    }

    // A prepared war: declare when the army is ready, or give up when the target is gone.
    if let Some(target) = preparing_target(g, team) {
        let reachable = reach_distance(g, team, target).is_some();
        if !reachable || !g.team_can_declare_war(team, target) {
            g.team_ai_set_war_plan(team, target, None, true);
            return;
        }
        let ready = army_size(g, p) >= army_needed_for_war(g, p, target);
        let waited = g.teams[team].ai_war_plan_state_counter[target];
        if ready && waited >= 5 || waited >= 40 {
            crate::trace!(g, p, "declares war on team {:?} (army {}, prepared {} turns)", target, army_size(g, p), waited);
            let plan = if g.teams[team].ai_war_plan[target] == Some(WarPlanType::WARPLAN_PREPARING_LIMITED) { WarPlanType::WARPLAN_LIMITED } else { WarPlanType::WARPLAN_TOTAL };
            g.team_declare_war(team, target, true, Some(plan));
        }
        return;
    }

    // Start preparing a war: rarely, against a weaker, disliked neighbour we can reach.
    if turn < 50 || (turn + p.0 as i32 * 3) % 5 != 0 || g.players[p].cities.len() < 2 {
        return;
    }
    // victory-pursuit: a war our victory plan calls for (a rival about to win, our domination
    // or conquest) needs no roll and no grudge.
    if let Some((target, why)) = victory_war(g, p, &plan) {
        crate::trace!(g, p, "prepares war on team {:?} for the victory plan ({})", target, why);
        g.team_ai_set_war_plan(team, target, Some(WarPlanType::WARPLAN_PREPARING_TOTAL), false);
        return;
    }
    let leader = g.players[p].personality_type.map(|l| &g.d.leader_heads[l]);
    // C++ AI_doWar's three kinds of war, each with its own leader odds (deterministic stand-in
    // for `getSorenRandNum(iRand) == 0`, checked every 5 turns): total war against a weaker
    // neighbour, limited war at closer odds, and dogpiling on a civ already at war.
    let roll = |salt: u32, rand: i32| -> bool {
        let h = (turn as u32).wrapping_mul(2654435761) ^ (p.0 as u32).wrapping_mul(40503) ^ salt.wrapping_mul(97);
        h % (rand.max(1) as u32 / 5 + 1) == 0
    };
    #[derive(PartialEq, Clone, Copy)]
    enum Kind {
        Total,
        Limited,
        Dogpile,
    }
    let kind = if roll(0, leader.map_or(100, |l| l.max_war_rand)) {
        Kind::Total
    } else if roll(1, leader.map_or(100, |l| l.limited_war_rand)) {
        Kind::Limited
    } else if roll(2, leader.map_or(100, |l| l.dogpile_war_rand)) {
        Kind::Dogpile
    } else {
        return;
    };
    // A broke empire only picks on clearly weaker neighbours (a captured city pays for itself
    // in plunder and land); a bankrupt one not at all.
    let broke = crate::production::financial_trouble(g, p);
    if broke && g.player_calculate_gold_rate(p) < 0 && crate::economy::spend_pct(g, p) == 0 {
        crate::trace!(g, p, "war roll passed but bankrupt");
        return;
    }
    let margin = if broke { 75 } else { 100 };
    let ours = g.team_get_power(team, true).max(1) as i64;
    let mut best: Option<(TeamId, i64)> = None;
    for other in g.player_ids() {
        let op = &g.players[other];
        if !op.alive || op.team == team || g.player_is_barbarian(other) || op.cities.is_empty() {
            continue;
        }
        let ot = op.team;
        if !g.teams[team].has_met[ot] || !g.team_can_declare_war(team, ot) {
            continue;
        }
        // Peace treaties hold for a while.
        if g.teams[team].ai_at_peace_counter[ot] < 10 && g.teams[team].ai_has_met_counter[ot] > 10 {
            continue;
        }
        let rel = relation(g, p, other);
        if rel > 1 {
            continue;
        }
        let theirs = g.team_get_defensive_power(ot).max(1) as i64;
        // Power checks as in C++ (their defensive power against ours times the leader ratio).
        let ok = match kind {
            Kind::Total => theirs * 2 / 3 < ours * leader.map_or(100, |l| l.max_war_nearby_power_ratio) as i64 * margin / 10000,
            Kind::Limited => theirs < ours * leader.map_or(100, |l| l.limited_war_power_ratio) as i64 * margin / 10000,
            Kind::Dogpile => {
                // Their enemies' power counts on our side.
                let at_war_with: i64 = g.team_ids().into_iter().filter(|&o| o != ot && o != team && g.team_is_alive(o) && o != g.barbarian_team() && g.at_war(o, ot)).map(|o| g.team_get_power(o, true) as i64).sum();
                at_war_with > 0 && theirs < (ours + at_war_with) * margin / 100
            }
        };
        if !ok {
            continue;
        }
        let Some(dist) = reach_distance(g, team, ot) else { continue };
        if dist > 16 {
            continue;
        }
        if !can_take_cities(g, p, other) {
            continue;
        }
        let score = ours * 100 / theirs - dist as i64 * 6 - rel as i64 * 20 + op.cities.len() as i64 * 10;
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((ot, score));
        }
    }
    if let Some((target, _)) = best {
        let plan = if kind == Kind::Total { WarPlanType::WARPLAN_PREPARING_TOTAL } else { WarPlanType::WARPLAN_PREPARING_LIMITED };
        crate::trace!(g, p, "prepares war on team {:?} ({:?})", target, plan);
        g.team_ai_set_war_plan(team, target, Some(plan), false);
    }
}

fn tech_item(t: TechType) -> TradeData {
    TradeData { item_type: TradeableItem::TRADE_TECHNOLOGIES, data: t.i(), offering: false, hidden: false }
}

/// Tech swaps and open borders with the AI leaders of teams we know and like well enough.
fn trade(g: &mut Game, p: PlayerId) {
    let team = g.players[p].team;
    for other in g.player_ids() {
        let op = &g.players[other];
        if other == p || !op.alive || op.human || op.team == team || g.player_is_barbarian(other) || !is_team_leader(g, other) {
            continue;
        }
        let ot = op.team;
        if !g.teams[team].has_met[ot] || g.at_war(team, ot) || relation(g, p, other) < 0 {
            continue;
        }
        // Open borders (trade routes, passage).
        if !g.team_is_open_borders(team, ot) && relation(g, p, other) >= 1 {
            let item = TradeData { item_type: TradeableItem::TRADE_OPEN_BORDERS, data: 0, offering: false, hidden: false };
            if g.player_can_trade_item(p, other, &item, true) && g.player_can_trade_item(other, p, &item, true) {
                let ours = vec![item.clone()];
                let theirs = vec![item];
                if g.controller(other).consider_offer(g, other, p, &ours, &theirs) {
                    crate::trace!(g, p, "open borders with {:?}", other);
                    g.implement_deal(p, other, &ours, &theirs, false);
                }
            }
        }
        // The tech of theirs we value most, for the tech of ours closest in cost.
        let cost = |t: TechType| g.team_get_research_cost(team, t).max(1);
        let mut want: Option<(TechType, i32)> = None;
        for t in g.d.techs.keys() {
            if g.team_is_has_tech(team, Some(t)) || !g.player_can_research(p, t, false) || !g.player_can_trade_item(other, p, &tech_item(t), true) {
                continue;
            }
            let c = cost(t);
            if want.map_or(true, |(_, b)| c > b) {
                want = Some((t, c));
            }
        }
        let Some((get, get_cost)) = want else { continue };
        let mut give: Option<(TechType, i32)> = None;
        for t in g.d.techs.keys() {
            if !g.team_is_has_tech(team, Some(t)) || g.team_is_has_tech(ot, Some(t)) || !g.player_can_trade_item(p, other, &tech_item(t), true) {
                continue;
            }
            let diff = (cost(t) - get_cost).abs();
            if give.map_or(true, |(_, b)| diff < b) {
                give = Some((t, diff));
            }
        }
        let Some((give, _)) = give else { continue };
        let ours = vec![tech_item(give)];
        let theirs = vec![tech_item(get)];
        if g.controller(other).consider_offer(g, other, p, &ours, &theirs) {
            crate::trace!(g, p, "trades {} for {} with {:?}", g.d.techs[give].base.type_, g.d.techs[get].base.type_, other);
            g.implement_deal(p, other, &ours, &theirs, false);
        }
    }
}

/// Can our units realistically take `other`'s cities: our best trainable attacker against
/// their best city defender (fortified, with city bonuses), or siege engines to wear the
/// walls down.
fn can_take_cities(g: &Game, p: PlayerId, other: PlayerId) -> bool {
    let d = g.d;
    let Some(c) = g.players[p].capital_city.filter(|&c| g.cities.contains(c)) else { return false };
    let mut best_attack = 0;
    let mut siege = false;
    for u in d.units.keys() {
        let info = &d.units[u];
        if info.domain_type != Some(DomainType::DOMAIN_LAND) || info.combat <= 0 || info.only_defensive {
            continue;
        }
        if !g.city_can_train(c, u, false, false, false, false) {
            continue;
        }
        best_attack = best_attack.max((info.combat + info.damage_type_combat.iter().sum::<i32>()) * (100 + info.city_attack_modifier.max(0)));
        siege |= info.bombard_rate > 0;
    }
    // Their weakest city (the one we would attack first): its best defender.
    let team = g.players[p].team;
    let mut weakest: Option<i32> = None;
    for &oc in &g.players[other].cities {
        let cp = g.city_plot(oc);
        if !g.map.plots[cp].revealed[team] {
            continue;
        }
        let mut best = 0;
        for &e in &g.map.plots[cp].units {
            if g.units[e].owner != other {
                continue;
            }
            let info = &d.units[g.units[e].unit_type];
            if info.combat <= 0 {
                continue;
            }
            let def = if info.combat_defense > 0 { info.combat_defense } else { info.combat } + info.damage_type_combat.iter().sum::<i32>();
            best = best.max(def * (100 + info.city_defense_modifier.max(0)));
        }
        weakest = Some(weakest.map_or(best, |w: i32| w.min(best)));
    }
    let Some(best_defense) = weakest else { return false };
    // Fortified in a city: roughly +50%; siege engines take most of that away.
    let needed = best_defense * if siege { 85 } else { 120 } / 100;
    best_attack >= needed
}

/// The war our victory plan calls for (victory-pursuit): against the rival about to win, else,
/// when our goal is domination or conquest, against the weakest civ we can reach -- any civ we
/// can reach over land within 20 plots whose weakest city we can take, whose defensive power
/// is below ours (the rival's: below 1.5x ours). Returns the team and why.
fn victory_war(g: &Game, p: PlayerId, plan: &crate::victory::Plan) -> Option<(TeamId, &'static str)> {
    let team = g.players[p].team;
    let broke = crate::production::financial_trouble(g, p);
    if broke && g.player_calculate_gold_rate(p) < 0 {
        return None;
    }
    let ours = g.team_get_power(team, true).max(1) as i64;
    let ok = |other: PlayerId, ratio_pct: i64| -> Option<i64> {
        let ot = g.players[other].team;
        if !g.players[other].alive || ot == team || g.player_is_barbarian(other) || g.players[other].cities.is_empty() {
            return None;
        }
        if !g.teams[team].has_met[ot] || !g.team_can_declare_war(team, ot) {
            return None;
        }
        if g.teams[team].ai_at_peace_counter[ot] < 10 && g.teams[team].ai_has_met_counter[ot] > 10 {
            return None;
        }
        let theirs = g.team_get_defensive_power(ot).max(1) as i64;
        if theirs * 100 > ours * ratio_pct {
            return None;
        }
        let dist = reach_distance(g, team, ot)?;
        if dist > 20 || !can_take_cities(g, p, other) {
            return None;
        }
        Some(ours * 100 / theirs - dist as i64 * 6)
    };
    if let Some((q, _)) = plan.rival {
        if ok(q, 150).is_some() {
            return Some((g.players[q].team, "a rival close to victory"));
        }
    }
    if !crate::victory::wants_conquest(plan) {
        return None;
    }
    let mut best: Option<(TeamId, i64)> = None;
    for other in g.player_ids() {
        if let Some(score) = ok(other, 100) {
            let ot = g.players[other].team;
            if best.map_or(true, |(_, s)| score > s) {
                best = Some((ot, score));
            }
        }
    }
    best.map(|(t, _)| (t, "domination or conquest"))
}

/// Should `team` seek peace with `enemy`?
fn want_peace(g: &Game, team: TeamId, enemy: TeamId, plan: &crate::victory::Plan) -> bool {
    let counter = g.teams[team].ai_at_war_counter[enemy];
    if counter < 10 {
        return false;
    }
    let ours = g.team_get_power(team, true).max(1) as i64;
    let theirs = g.team_get_power(enemy, true).max(1) as i64;
    let success = g.teams[team].ai_war_success[enemy] as i64;
    let their_success = g.teams[enemy].ai_war_success[team] as i64;
    // Losing: they hit harder and are not weaker.
    if their_success > success + 10 && ours * 10 < theirs * 12 {
        return true;
    }
    // victory-pursuit: a war for our domination or conquest, or against a rival about to win,
    // goes on while it is not lost.
    if crate::victory::wants_conquest(plan) || crate::victory::rival_team(g, plan) == Some(enemy) {
        return false;
    }
    // Clearly winning: keep going (a long war still ends eventually).
    if ours > theirs * 2 && counter < 60 {
        return false;
    }
    // Stalemate.
    counter >= 40 && success < their_success * 2 + 20 || counter >= 80
}

/// C++ `AI_considerOffer` equivalent: accept peace when the war is not going our way or has
/// lasted long, open borders with friends, and gifts.
pub fn consider_offer(g: &Game, p: PlayerId, from: PlayerId, they_give: &[TradeData], we_give: &[TradeData]) -> bool {
    if we_give.is_empty() {
        return true;
    }
    let team = g.players[p].team;
    let their_team = g.players[from].team;
    let all = |items: &[TradeData], kind: TradeableItem| items.iter().all(|t| t.item_type == kind);
    if all(they_give, TradeableItem::TRADE_PEACE_TREATY) && all(we_give, TradeableItem::TRADE_PEACE_TREATY) {
        let counter = g.teams[team].ai_at_war_counter[their_team];
        if counter < 10 {
            return false;
        }
        // We accept unless we are clearly winning.
        let ours = g.team_get_power(team, true).max(1) as i64;
        let theirs = g.team_get_power(their_team, true).max(1) as i64;
        let success = g.teams[team].ai_war_success[their_team] as i64;
        let their_success = g.teams[their_team].ai_war_success[team] as i64;
        let winning = ours > theirs * 3 / 2 && success > their_success + 20;
        // victory-pursuit: a war for our domination or conquest, or against a rival about to
        // win, ends only when it is lost (or after 80 turns).
        let plan = crate::victory::plan(g, p);
        if crate::victory::wants_conquest(&plan) || crate::victory::rival_team(g, &plan) == Some(their_team) {
            let losing = their_success > success + 10 && ours * 10 < theirs * 12;
            return losing || counter >= 80;
        }
        return !winning || counter >= 60;
    }
    if all(they_give, TradeableItem::TRADE_OPEN_BORDERS) && all(we_give, TradeableItem::TRADE_OPEN_BORDERS) {
        return !g.at_war(team, their_team) && relation(g, p, from) >= 1;
    }
    // Technology for technology: worth at least four fifths of what we give, with a civ we do
    // not dislike, and never to the one we are preparing a war against.
    if all(they_give, TradeableItem::TRADE_TECHNOLOGIES) && all(we_give, TradeableItem::TRADE_TECHNOLOGIES) {
        if g.at_war(team, their_team) || relation(g, p, from) < 0 || preparing_target(g, team) == Some(their_team) {
            return false;
        }
        let value = |items: &[TradeData]| items.iter().map(|t| g.team_get_research_cost(team, TechType::from_usize(t.data as usize))).sum::<i32>();
        return value(they_give) * 5 >= value(we_give) * 4;
    }
    false
}

/// C++ `AI_conquerCity`: raze small, distant conquests we could not hold.
pub fn raze_conquered_city(g: &Game, p: PlayerId, c: CityId) -> bool {
    let city = &g.cities[c];
    if city.population > 2 || g.player_is_barbarian(p) {
        return false;
    }
    let (x, y) = (city.x, city.y);
    let nearest = g.players[p].cities.iter().filter(|&&o| o != c).map(|&o| g.plot_distance(x, y, g.cities[o].x, g.cities[o].y)).min().unwrap_or(i32::MAX);
    let holy = g.d.religions.keys().any(|r| g.get_holy_city(r) == Some(c));
    nearest > 10 && !holy && city.num_world_wonders == 0
}
