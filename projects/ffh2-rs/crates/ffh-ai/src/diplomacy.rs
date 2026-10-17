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

/// Power ratio (ours * 100 / their defensive power) at which the AI judges itself decisively
/// stronger than a rival -- strong enough to commit to total war (conquering the civ) rather than
/// a limited land grab. Reused as the "stronger than its neighbour" bar for guns-vs-butter siege
/// research (`research::siege_offense`), so both read the same notion of clear superiority.
pub(crate) const TOTAL_WAR_RATIO: i64 = 140;

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
    // Peace when the war goes badly or drags on without gains -- or, when we are clearly winning and
    // the war has run its course (>10 turns), a peace that converts our edge into reparations (tech +
    // gold) instead of a valueless white peace (C++ `AI_doDiplo` peace build). A winner pursuing
    // conquest/domination against this enemy, or winning a total war (press_conquest), keeps fighting
    // to take cities rather than cashing out.
    for &enemy in &v.civ_enemies {
        let Some(other) = team_leader(g, enemy).filter(|&q| !g.players[q].human) else { continue };
        let our_want = end_war_val(g, team, enemy); // iOurValue: how much WE want peace
        let their_want = end_war_val(g, enemy, team); // iTheirValue: how much THEY want peace
        let victory_war = crate::victory::wants_conquest(&plan) || crate::victory::rival_team(g, &plan) == Some(enemy);
        // Don't sell a conquest war we are winning: press on to take cities instead (see press_conquest).
        let winner_initiated = their_want > our_want && g.teams[team].ai_at_war_counter[enemy] > 10 && !victory_war && !press_conquest(g, team, enemy);
        if !want_peace(g, team, enemy, &plan) && !winner_initiated {
            continue;
        }
        let item = TradeData { item_type: TradeableItem::TRADE_PEACE_TREATY, data: 0, offering: false, hidden: false };
        if !g.player_can_trade_item(p, other, &item, true) || !g.player_can_trade_item(other, p, &item, true) {
            continue;
        }
        let mut ours_list = vec![item.clone()];
        let mut theirs_list = vec![item];
        if their_want > our_want {
            // Winning: demand reparations from them (added to their side of the deal).
            theirs_list.extend(crate::trade::peace_tribute(g, p, other, their_want - our_want));
        } else if our_want > their_want {
            // Losing: sweeten the peace with tribute of our own so the winner will accept.
            ours_list.extend(crate::trade::peace_tribute(g, other, p, our_want - their_want));
        }
        let accept = g.controller(other).consider_offer(g, other, p, &ours_list, &theirs_list);
        if accept {
            crate::trace!(g, p, "peace with team {:?} after {} turns (we take {}, we give {})", enemy, g.teams[team].ai_at_war_counter[enemy], theirs_list.len() - 1, ours_list.len() - 1);
            g.implement_deal(p, other, &ours_list, &theirs_list, false);
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
        // Reachable by land (a shared landmass) or by sea (an overseas city we can ship an army to).
        let reachable = reach_distance(g, team, target).is_some()
            || team_leader(g, target).is_some_and(|q| crate::naval::amphibious_city(g, p, q).is_some());
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
    // Greedy expansion by war: prepare war on the reachable rival whose territory we can take at
    // acceptable risk, for the best gain -- no dice roll and no grudge requirement. A civ uses the
    // military power it has to take land whenever the opportunity is good (domination and conquest
    // are this greedy rule run to its conclusion; the victory planner no longer drives wars).
    // Flavour biases the bar, not the drive: a war-eager leader (low C++ iMaxWarRand) strikes at
    // near parity and will hit anyone, a cautious one needs a clear edge and spares civs it likes.
    let broke = crate::production::financial_trouble(g, p);
    if broke && g.player_calculate_gold_rate(p) < 0 && crate::economy::spend_pct(g, p) == 0 {
        crate::trace!(g, p, "would expand by war but is bankrupt");
        return;
    }
    // iMaxWarRand is the leader's propensity to start total war -- LOW = eager. Map it to the power
    // edge we require over the target's defence: eager leaders accept near parity (even a slight
    // underdog, acceptable risk for a warmonger), cautious ones want clear superiority. A broke
    // empire wants a bigger edge (a captured city has to pay for itself in plunder and land).
    let war_rand = leader.map_or(150, |l| l.max_war_rand);
    let need_ratio = (80 + war_rand as i64 / 4).clamp(85, 180) + if broke { 30 } else { 0 };
    let spare_friends = war_rand > 80;
    let ours = g.team_get_power(team, true).max(1) as i64;
    let mut best: Option<(TeamId, i64, bool)> = None;
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
        // A cautious leader does not attack a civ it likes; a war-eager one takes any opening.
        if spare_friends && rel > 1 {
            continue;
        }
        let theirs = g.team_get_defensive_power(ot).max(1) as i64;
        let ratio = ours * 100 / theirs;
        if ratio < need_ratio {
            continue;
        }
        // Reachable and takeable: by land (a shared landmass within 16 plots whose weakest city
        // our best trainable attacker can take), or, failing that, by sea (an overseas city we
        // can ship an army to and take -- `amphibious_city` already checks its takeability). No
        // hardcoded unit or stat; reachability from the flood/distance helpers.
        let land = reach_distance(g, team, ot).filter(|&d| d <= 16 && can_take_cities(g, p, other));
        let dist = match land {
            Some(d) => d,
            None => match crate::naval::amphibious_city(g, p, other) {
                Some(a) => a.dist,
                None => continue,
            },
        };
        // Flavor bias (secondary): prefer striking a rival of the opposed alignment -- the
        // good/evil conflict at the heart of FFH. Added only to the score, so it re-ranks targets
        // we can already reach and take (the reachability/takeability gate above has run); it never
        // makes us attack an unreachable civ, so it cannot trade away the greedy territory-grab.
        let flavor = {
            let good = g.d.ty::<AlignmentType>("ALIGNMENT_GOOD");
            let evil = g.d.ty::<AlignmentType>("ALIGNMENT_EVIL");
            let (us, them) = (g.players[p].alignment, op.alignment);
            if (us == good && them == evil) || (us == evil && them == good) { 30 } else { 0 }
        };
        // Greedy gain: a bigger edge, more and closer cities; minus how much we like them; a
        // flavor nudge toward alignment-opposites.
        let score = ratio - dist as i64 * 6 - rel as i64 * 15 + op.cities.len() as i64 * 10 + flavor;
        let total = ratio >= TOTAL_WAR_RATIO;
        if best.map_or(true, |(_, s, _)| score > s) {
            best = Some((ot, score, total));
        }
    }
    if let Some((target, _, total)) = best {
        let plan = if total { WarPlanType::WARPLAN_PREPARING_TOTAL } else { WarPlanType::WARPLAN_PREPARING_LIMITED };
        crate::trace!(g, p, "prepares war on team {:?} to take territory ({:?}, power ratio {})", target, plan, ours * 100 / g.team_get_defensive_power(target).max(1) as i64);
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
        // A resource of theirs we want, paid for with a surplus of ours or with gold.
        resource_trade(g, p, other);
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

/// Offer `other` a resource trade ([`trade::buy_resource_offer`][crate::trade::buy_resource_offer]
/// builds the deal); the AI leader `other` judges it through its own `consider_offer`.
fn resource_trade(g: &mut Game, p: PlayerId, other: PlayerId) {
    let Some((ours, theirs)) = crate::trade::buy_resource_offer(g, p, other) else { return };
    if g.controller(other).consider_offer(g, other, p, &ours, &theirs) {
        let got = BonusType::from_usize(theirs[0].data as usize);
        crate::trace!(g, p, "buys {} from {:?}", g.d.bonuses[got].base.type_, other);
        g.implement_deal(p, other, &ours, &theirs, false);
    }
}

/// Our best trainable land attacker's effective city-attack strength (x100), and whether we
/// can train a siege engine. `None` when we have no capital to train from. No hardcoded unit:
/// every land attacker the capital can train is weighed (combat + damage types, city-attack
/// bonus), siege engines noted for the bombard discount.
pub(crate) fn best_trainable_attack(g: &Game, p: PlayerId) -> Option<(i32, bool)> {
    let d = g.d;
    let c = g.players[p].capital_city.filter(|&c| g.cities.contains(c))?;
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
    Some((best_attack, siege))
}

/// The strength (x100) of city `c`'s best defender as we see it (fortified city bonuses), or
/// `None` when the city plot is not revealed to our team.
fn city_best_defense(g: &Game, p: PlayerId, c: CityId) -> Option<i32> {
    let team = g.players[p].team;
    let cp = g.city_plot(c);
    if !g.map.plots[cp].revealed[team] {
        return None;
    }
    let other = g.cities[c].owner;
    let mut best = 0;
    for &e in &g.map.plots[cp].units {
        if g.units[e].owner != other {
            continue;
        }
        let info = &g.d.units[g.units[e].unit_type];
        if info.combat <= 0 {
            continue;
        }
        let def = if info.combat_defense > 0 { info.combat_defense } else { info.combat } + info.damage_type_combat.iter().sum::<i32>();
        best = best.max(def * (100 + info.city_defense_modifier.max(0)));
    }
    Some(best)
}

/// Can our best trainable attacker realistically take city `c`: its best defender (fortified in
/// the city, with city bonuses), siege engines lowering the bar. False when we cannot train an
/// attacker or the city plot is not revealed.
pub(crate) fn can_take_city(g: &Game, p: PlayerId, c: CityId) -> bool {
    let Some((best_attack, siege)) = best_trainable_attack(g, p) else { return false };
    let Some(best_defense) = city_best_defense(g, p, c) else { return false };
    // Fortified in a city: roughly +50%; siege engines take most of that away.
    let needed = best_defense * if siege { 85 } else { 120 } / 100;
    best_attack >= needed
}

/// Can our units realistically take `other`'s cities: our best trainable attacker against
/// their weakest revealed city's best defender (fortified, with city bonuses), or siege
/// engines to wear the walls down.
fn can_take_cities(g: &Game, p: PlayerId, other: PlayerId) -> bool {
    let Some((best_attack, siege)) = best_trainable_attack(g, p) else { return false };
    // Their weakest city (the one we would attack first): its best defender.
    let mut weakest: Option<i32> = None;
    for &oc in &g.players[other].cities {
        if let Some(best) = city_best_defense(g, p, oc) {
            weakest = Some(weakest.map_or(best, |w: i32| w.min(best)));
        }
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

/// Power ratio (ours * 100 / their defensive power) at or above which a winner should press a war to
/// taking cities (conquest) rather than cash out for a token tribute -- an *overwhelming* edge (2x),
/// well above the `TOTAL_WAR_RATIO` = 140 bar for merely committing to a war. Below it (a 1-2x edge)
/// the enemy's cities are not reliably stormable, so a tribute peace is a sensible outcome and this
/// does not fire; at or above it, conquest is achievable and selling it for a tech forfeits the
/// primary goal. Tunable by A/B (the siege/launch bar is deliberately lower).
pub(crate) const PRESS_CONQUEST_RATIO: i64 = 200;

/// Are we overwhelmingly winning this war and should press it to taking cities rather than cashing
/// out for tribute? True when our power is at least `PRESS_CONQUEST_RATIO`% of the enemy's defensive
/// power -- a civ this far ahead can take cities, and conquest (the primary goal) must not be sold
/// for a secondary tech. Without this a strong civ tribute-farms a weak victim for the whole game
/// instead of finishing it; the plain victory-plan guard misses it because it keys on the civ's
/// overall victory goal, not on this war's power edge. The war plan is *not* used: an AI's active war
/// plan is overwritten to LIMITED/DOGPILE on declaration (never TOTAL), and the cash-out fires at the
/// first legal turn before war success accrues, so only the immediate power ratio is reliable. Both
/// the offer path (`manage`) and the acceptance path (`consider_offer`) consult this so a winner
/// neither offers nor accepts a peace that abandons a conquest it is winning.
fn press_conquest(g: &Game, team: TeamId, enemy: TeamId) -> bool {
    let ours = g.team_get_power(team, true).max(1) as i64;
    let theirs = g.team_get_defensive_power(enemy).max(1) as i64;
    ours * 100 >= theirs * PRESS_CONQUEST_RATIO
}

/// C++ `CvTeamAI::AI_endWarVal` (CvTeamAI.cpp:992): value to `team` of ending its war with `enemy`.
/// Large when we are weak / losing, cut to a fifth once we dominate (>120% power) -- this gap is
/// what funds reparations at peace. Integer math throughout (each `/` truncates). The
/// warmonger-continue sub-factors needing unported strategy/financial state (DAGGER, financial
/// trouble, the maxWar/limitedWar/dogpileWar rands) are omitted; the land-ratio cut and the
/// war-plan multiplier -- both with ported inputs -- are kept. Cities-as-tribute is not valued here
/// (`AI_cityTradeVal` unported). Widened to i64 like the rest of this module's power math to avoid
/// the C++ `int` overflow on large late-game powers.
pub(crate) fn end_war_val(g: &Game, team: TeamId, enemy: TeamId) -> i32 {
    let mut v: i64 = 100;
    v += g.teams[team].num_cities as i64 * 3;
    v += g.teams[enemy].num_cities as i64 * 3;
    v += g.team_get_total_population(team, true) as i64;
    v += g.team_get_total_population(enemy, true) as i64;
    v += g.teams[enemy].ai_war_success[team] as i64 * 20; // how hard THEY have hit US
    let our_power = g.team_get_power(team, true).max(1) as i64;
    let their_power = g.team_get_defensive_power(enemy).max(1) as i64;
    v *= their_power + 10;
    v /= (our_power + 10).max(1); // FfH (Kael 04/23/2009): divide by OUR power only
    if our_power * 100 > their_power * 120 {
        v /= 5; // FfH: if we are >120% power, peace is worth a fifth
    }
    let our_plan = g.teams[team].ai_war_plan[enemy];
    let their_plan = g.teams[enemy].ai_war_plan[team];
    // Warmonger-continue block: only when we are not human and clearly stronger. C++ also folds in
    // DAGGER / financial-trouble / warRand sub-factors (unported strategy + financial state) --
    // omitted here; only the land-ratio cut has all inputs ported.
    if !g.team_is_human(team) && our_power > 120 * their_power / 100 {
        let land_ratio = g.team_get_total_land(team, true) as i64 * 100 / (g.team_get_total_land(enemy, true).max(1) as i64);
        if land_ratio > 120 {
            v = v * (9 * 100) / (10 * land_ratio);
        }
    }
    // War-plan multiplier (either side's plan toward the other).
    let total = (!g.team_is_human(team) && our_plan == Some(WarPlanType::WARPLAN_TOTAL))
        || (!g.team_is_human(enemy) && their_plan == Some(WarPlanType::WARPLAN_TOTAL));
    let dogpile = (!g.team_is_human(team) && our_plan == Some(WarPlanType::WARPLAN_DOGPILE) && g.team_get_at_war_count(enemy, true) > 1)
        || (!g.team_is_human(enemy) && their_plan == Some(WarPlanType::WARPLAN_DOGPILE) && g.team_get_at_war_count(team, true) > 1);
    if total {
        v *= 2;
    } else if dogpile {
        v = v * 3 / 2;
    }
    // (AGGRESSIVE_AI endangered-cities block omitted: the option is off in our games.)
    let remainder = g.d.defines.diplomacy_value_remainder.max(1) as i64;
    v -= v.rem_euclid(remainder);
    if g.team_is_human(team) {
        v = v.max(remainder);
    }
    v as i32
}

/// Should `team` seek peace with `enemy`?
pub(crate) fn want_peace(g: &Game, team: TeamId, enemy: TeamId, plan: &crate::victory::Plan) -> bool {
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
    // Peace, possibly with reparations: each list carries a peace treaty, and either may also carry
    // tribute (tech / gold). Value both sides as the at-war `AI_endWarVal` baseline plus the tribute
    // (C++ `AI_dealVal`/`AI_considerOffer`, CvPlayerAI.cpp:6815, 7057) and accept when what we get is
    // worth at least what we give.
    let has_peace = |items: &[TradeData]| items.iter().any(|t| t.item_type == TradeableItem::TRADE_PEACE_TREATY);
    if has_peace(they_give) && has_peace(we_give) {
        let counter = g.teams[team].ai_at_war_counter[their_team];
        if counter < 10 {
            return false;
        }
        // Never hand over a resource our own judgement blocks (ported `AI_bonusTrade`).
        for item in we_give {
            if item.item_type == TradeableItem::TRADE_RESOURCES
                && crate::trade::bonus_trade_denial(g, p, BonusType::from_usize(item.data as usize), from).is_some()
            {
                return false;
            }
        }
        let ours = g.team_get_power(team, true).max(1) as i64;
        let theirs = g.team_get_power(their_team, true).max(1) as i64;
        let success = g.teams[team].ai_war_success[their_team] as i64;
        let their_success = g.teams[their_team].ai_war_success[team] as i64;
        // victory-pursuit / press-conquest OVERRIDE: a war for our domination or conquest, against a
        // rival about to win, or a total war we are winning (press_conquest), is not sold for tribute
        // -- it ends only when it is lost (or after 80 turns). Without the press_conquest arm a winning
        // conqueror would simply re-buy peace from the loser's sweetened side of the deal.
        let plan = crate::victory::plan(g, p);
        if crate::victory::wants_conquest(&plan) || crate::victory::rival_team(g, &plan) == Some(their_team) || press_conquest(g, team, their_team) {
            let losing = their_success > success + 10 && ours * 10 < theirs * 12;
            return losing || counter >= 80;
        }
        // Reparations: value each side as the at-war `AI_endWarVal` baseline plus its tribute (C++
        // `AI_dealVal`/`AI_considerOffer`, CvPlayerAI.cpp:6815, 7057). A deal that pays us at least
        // what we give away -- a winner's reparations, or a loser sweetening the peace -- is taken.
        let their_v = end_war_val(g, team, their_team) + crate::trade::deal_val(g, p, from, they_give, false, 1);
        let our_v = end_war_val(g, their_team, team) + crate::trade::deal_val(g, from, p, we_give, false, 1);
        if their_v >= our_v {
            return true;
        }
        // The deal leaves us behind on value. Give away tribute of our own only when it pays
        // (the test above) -- never fall through to accept a peace in which we also hand something
        // over. A BARE peace (we give nothing but the treaty) still ends on plain willingness: we
        // take it unless we are clearly winning, and always once the war has dragged to exhaustion
        // (keeps the pre-reparations war-ending behaviour, so even/stalemated wars still resolve).
        if we_give.iter().any(|t| t.item_type != TradeableItem::TRADE_PEACE_TREATY) {
            return false;
        }
        let winning = ours > theirs * 3 / 2 && success > their_success + 20;
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
    // Resources and gold (and any mixed deal): value both sides in gold-equivalent (C++
    // `AI_dealVal`) and accept when what we receive is worth at least what we give away. A friendly
    // act, so only at peace with a civ we do not dislike and are not about to attack; we refuse
    // gold-for-gold, and never hand over a resource our own judgement blocks (`trade::bonus_trade_denial`,
    // the ported `AI_bonusTrade`: our worst enemy, a resource they already have, or a strategic /
    // happiness / health resource to a civ we do not like enough).
    let tradeable = |items: &[TradeData]| {
        items
            .iter()
            .any(|t| matches!(t.item_type, TradeableItem::TRADE_RESOURCES | TradeableItem::TRADE_GOLD | TradeableItem::TRADE_GOLD_PER_TURN))
    };
    if tradeable(they_give) || tradeable(we_give) {
        if crate::trade::is_gold_deal(they_give) && crate::trade::is_gold_deal(we_give) {
            return false;
        }
        if g.at_war(team, their_team) || relation(g, p, from) < 0 || preparing_target(g, team) == Some(their_team) {
            return false;
        }
        for item in we_give {
            if item.item_type == TradeableItem::TRADE_RESOURCES
                && crate::trade::bonus_trade_denial(g, p, BonusType::from_usize(item.data as usize), from).is_some()
            {
                return false;
            }
        }
        let theirs = crate::trade::deal_val(g, p, from, they_give, false, 1);
        let ours = crate::trade::deal_val(g, from, p, we_give, false, 1);
        return theirs >= ours;
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
