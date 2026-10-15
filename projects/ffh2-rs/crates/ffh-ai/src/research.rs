//! Research choice: what a tech is worth to this player now, per turn of research.
//!
//! The value of a tech is the sum of what it unlocks, measured against the player's
//! situation (after C++ `CvPlayerAI::AI_techValue` and FfH's `CvGameUtils.AI_chooseTech`):
//! - improvements our workers could build on the land our cities work, especially those
//!   connecting resources (FfH's "starting resources" logic: Agriculture for wheat, Animal
//!   Husbandry for pigs, Mining for gold and hills ...);
//! - a better city defender or attacker, weighted by the military pressure we see (barbarians
//!   near our cities, wars);
//! - buildings for our cities (happiness weighted up when cities are at their cap), religions
//!   still unfounded (the leader's favorite above all), civics, trade routes, health and
//!   happiness;
//! - the leader's FfH hints (favorite techs, early religion, flavors).
//!
//! The score is value per turn of research, plus a quarter of what the tech leads to.

use ffh_core::defs::NUM_CITY_PLOTS;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::values::values;

/// Per-choice snapshot of the player's situation.
struct Ctx {
    team: TeamId,
    civ: Option<CivilizationType>,
    num_cities: i32,
    /// Plots in our cities' radii (owned by us or nobody).
    plots: Vec<PlotIdx>,
    best_defense: i64,
    best_attack: i64,
    /// Military pressure: 1 (peace, no barbarians seen) .. 6.
    pressure: i64,
    /// Some city cannot grow because of unhappiness.
    unhappy_cities: i32,
    rate: i32,
    /// Our landmass is full (see `naval::home_is_full`).
    home_full: bool,
    /// We can already build a ship that carries land units.
    has_transport: bool,
    /// Cities with no building left to build.
    idle_cities: i32,
    /// We know a process (production to research or gold).
    has_process: bool,
}

/// Unit types `p` could train with its techs plus `extra` (ignores resources and buildings).
fn unit_available(g: &Game, p: PlayerId, ctx: &Ctx, u: UnitType, extra: &[TechType]) -> bool {
    let d = g.d;
    let info = &d.units[u];
    let Some(class) = info.unit_class_type else { return false };
    if ctx.civ.and_then(|c| d.civ_unit(c, class)) != Some(u) {
        return false;
    }
    if info.domain_type != Some(DomainType::DOMAIN_LAND) || info.production_cost <= 0 || info.found {
        return false;
    }
    if info.prereq_religion.is_some() || info.state_religion.is_some() || info.prereq_building.is_some() || info.min_level > 0 {
        return false;
    }
    if info.default_unit_ai_type == Some(UnitAIType::UNITAI_HERO) || d.unit_classes[class].max_player_instances > 0 || d.unit_classes[class].max_global_instances > 0 {
        return false;
    }
    let has = |t: TechType| extra.contains(&t) || g.team_is_has_tech(ctx.team, Some(t));
    if let Some(t) = info.prereq_and_tech {
        if !has(t) {
            return false;
        }
    }
    info.prereq_and_techs.iter().all(|&t| has(t)) && !g.player_is_barbarian(p)
}

fn unit_defense(info: &UnitInfo) -> i64 {
    if info.combat <= 0 || !(info.unit_ai_type[UnitAIType::UNITAI_CITY_DEFENSE] || info.unit_ai_type[UnitAIType::UNITAI_ATTACK] || info.unit_ai_type[UnitAIType::UNITAI_CITY_COUNTER]) {
        return 0;
    }
    let base = if info.combat_defense > 0 { info.combat_defense } else { info.combat } + info.damage_type_combat.iter().sum::<i32>();
    (base * (100 + info.city_defense_modifier.max(0))) as i64
}

fn unit_attack(info: &UnitInfo) -> i64 {
    if info.combat <= 0 || info.only_defensive || !(info.unit_ai_type[UnitAIType::UNITAI_ATTACK] || info.unit_ai_type[UnitAIType::UNITAI_ATTACK_CITY]) {
        return 0;
    }
    let base = info.combat + info.damage_type_combat.iter().sum::<i32>();
    (base * 100 + base * 100 * (info.moves.max(1) - 1) / 4) as i64
}

fn best_military(g: &Game, p: PlayerId, ctx: &Ctx, extra: &[TechType]) -> (i64, i64) {
    let mut def = 0;
    let mut att = 0;
    for u in g.d.units.keys() {
        if unit_available(g, p, ctx, u, extra) {
            def = def.max(unit_defense(&g.d.units[u]));
            att = att.max(unit_attack(&g.d.units[u]));
        }
    }
    (def, att)
}

fn make_ctx(g: &Game, p: PlayerId) -> Ctx {
    let pl = &g.players[p];
    let team = pl.team;
    let mut plots = Vec::new();
    let mut unhappy_cities = 0;
    for &c in &pl.cities {
        for i in 0..NUM_CITY_PLOTS {
            if let Some(q) = g.city_index_plot(c, i) {
                let owner = g.map.plots[q].owner;
                if (owner.is_none() || owner == Some(p)) && !plots.contains(&q) {
                    plots.push(q);
                }
            }
        }
        if g.city_happy_level(c) - g.city_unhappy_level(c, 0) <= 0 {
            unhappy_cities += 1;
        }
    }
    // Military pressure: barbarian (non-animal) and enemy units seen near our cities.
    let mut seen = 0i64;
    let barb = g.barbarian_player();
    for &u in &g.players[barb].units {
        let unit = &g.units[u];
        if g.d.units[unit.unit_type].animal {
            continue;
        }
        if pl.cities.iter().any(|&c| g.plot_distance(unit.x, unit.y, g.cities[c].x, g.cities[c].y) <= 8) {
            seen += 1;
        }
    }
    let at_war = g.teams.keys().any(|t| t != team && t != g.barbarian_team() && g.teams[t].alive_count > 0 && g.at_war(team, t));
    let pressure = (1 + seen / 2 + if at_war { 3 } else { 0 } + i64::from(g.game_turn() > 25) + i64::from(g.game_turn() > 60)).min(6);
    let mut ctx = Ctx {
        team,
        civ: g.player_get_civilization_type(p),
        num_cities: pl.cities.len() as i32,
        plots,
        best_defense: 0,
        best_attack: 0,
        pressure,
        unhappy_cities,
        rate: g.player_calculate_research_rate(p, None).max(1),
        home_full: crate::naval::home_is_full(g, p) || crate::naval::settler_waiting(g, p).is_some(),
        has_transport: false,
        idle_cities: pl.cities.iter().filter(|&&c| !g.d.buildings.keys().any(|b| g.city_can_construct(c, b, false, false, false))).count() as i32,
        has_process: g.d.processes.iter().any(|pr| pr.tech_prereq.map_or(true, |t| g.team_is_has_tech(team, Some(t)))),
    };
    ctx.has_transport = g.d.units.keys().any(|u| {
        let info = &g.d.units[u];
        crate::naval::is_transport_type(info)
            && info.unit_class_type.and_then(|cl| ctx.civ.and_then(|c| g.d.civ_unit(c, cl))) == Some(u)
            && info.prereq_and_tech.map_or(true, |t| g.team_is_has_tech(team, Some(t)))
    });
    let (def, att) = best_military(g, p, &ctx, &[]);
    ctx.best_defense = def;
    ctx.best_attack = att;
    ctx
}

/// Value added to a tech our victory goal needs, and the research turns it is charged at most.
const GOAL_TECH_BONUS: i64 = 4000;
const GOAL_TECH_TURNS: i64 = 15;

pub fn choose(g: &Game, p: PlayerId) -> Option<TechType> {
    let team = g.players[p].team;
    if g.players[p].cities.is_empty() {
        // FfH: AI_chooseTech returns Agriculture before the first city.
        if let Some(t) = g.d.ty::<TechType>("TECH_AGRICULTURE") {
            if !g.team_is_has_tech(team, Some(t)) && g.player_can_research(p, t, false) {
                return Some(t);
            }
        }
    }
    let ctx = make_ctx(g, p);
    // victory-pursuit: the techs our victory goal needs (victory.rs).
    let goal_techs = crate::victory::goal_techs(g, p, &crate::victory::plan_ours(g, p));
    let mut best: Option<TechType> = None;
    let mut best_score = i64::MIN;
    for t in g.d.techs.keys() {
        if g.team_is_has_tech(team, Some(t)) || !g.player_can_research(p, t, false) {
            continue;
        }
        let mut value = tech_value(g, p, t, &ctx) as i64;
        // Lookahead: a share of what the techs this one leads to are worth.
        for &child in &values(g.d).tech_children[t] {
            if !g.team_is_has_tech(team, Some(child)) {
                value += tech_value_static(g, p, child) as i64 / 4;
                // A better unit one step further (Mining -> Bronze Working) counts in part.
                let or_ok = g.d.techs[child].prereq_or_techs.is_empty() || g.d.techs[child].prereq_or_techs.iter().any(|&o| o == t || g.team_is_has_tech(team, Some(o)));
                if or_ok {
                    let others_known = g.d.techs[child].prereq_and_techs.iter().all(|&a| a == t || g.team_is_has_tech(team, Some(a)));
                    if others_known {
                        value += military_value(g, p, &ctx, &[t, child]) as i64 / 2 - military_value(g, p, &ctx, &[t]) as i64 / 2;
                    }
                }
            }
        }
        let left = g.team_get_research_left(team, t).max(1) as i64;
        let mut turns = left / ctx.rate as i64;
        if goal_techs.contains(&t) {
            // A beeline: worth more, and a long research no longer counts against it.
            value = value * 2 + GOAL_TECH_BONUS;
            turns = turns.min(GOAL_TECH_TURNS);
        }
        let score = value * 1000 / (turns + 4);
        if crate::trace_on(p) && std::env::var("FFH_AI_TRACE_TECH").is_ok() {
            crate::trace!(g, p, "  tech {} value {} turns {} score {}", g.d.techs[t].base.type_, value, turns, score);
        }
        if score > best_score {
            best_score = score;
            best = Some(t);
        }
    }
    if let Some(t) = best {
        crate::trace!(g, p, "research {} (score {})", g.d.techs[t].base.type_, best_score);
    }
    best
}

/// Improvements unlocked by `t`: value of building them on our land.
fn builds_value(g: &Game, p: PlayerId, t: TechType, ctx: &Ctx) -> i32 {
    let d = g.d;
    let mut total = 0;
    for (b, binfo) in d.builds.iter_enumerated() {
        if binfo.tech_prereq != Some(t) {
            continue;
        }
        let _ = b;
        if binfo.route.is_some() {
            // Roads connect cities (trade, resources) and speed up movement.
            total += 40 * ctx.num_cities.clamp(1, 8);
            continue;
        }
        let Some(imp) = binfo.improvement else { continue };
        // Improvements that grow (cottages) count their grown form for two thirds.
        let mut last = imp;
        for _ in 0..4 {
            match d.improvements[last].improvement_upgrade {
                Some(next) if next != imp => last = next,
                _ => break,
            }
        }
        let research = crate::economy::spend_pct(g, p);
        let yw = [20, 15, if research < 50 { 9 * (200 - 2 * research) / 100 } else { 9 }];
        let mut gains: Vec<i32> = Vec::new();
        for &q in &ctx.plots {
            let plot = &g.map.plots[q];
            if plot.plot_city.is_some() {
                continue;
            }
            if let Some(cur) = plot.improvement_type {
                // Keep existing improvements unless a resource waits for a better one.
                let bonus_waits = plot.bonus_type.is_some_and(|bn| !d.improvements[cur].improvement_bonus[bn].bonus_trade);
                if cur == imp || !bonus_waits {
                    continue;
                }
            }
            if plot.improvement_type == Some(imp) || !g.plot_can_have_improvement(q, imp, Some(ctx.team), false) {
                continue;
            }
            let mut v = 0;
            for y in 0..YieldType::NUM {
                let yt = YieldType::from_usize(y);
                let mut new = g.plot_calculate_improvement_yield_change(q, imp, yt, Some(p), false, false);
                if last != imp {
                    new = (new + 2 * g.plot_calculate_improvement_yield_change(q, last, yt, Some(p), false, false)) / 3;
                }
                let old = plot.improvement_type.map_or(0, |o| g.plot_calculate_improvement_yield_change(q, o, yt, Some(p), false, false));
                v += (new - old) * yw[y];
            }
            if let Some(bonus) = plot.bonus_type {
                let revealed = d.bonuses[bonus].tech_reveal.map_or(true, |rt| g.team_is_has_tech(ctx.team, Some(rt)));
                let traded_now = plot.improvement_type.is_some_and(|o| d.improvements[o].improvement_bonus[bonus].bonus_trade);
                if revealed && !traded_now && d.improvements[imp].improvement_bonus[bonus].bonus_trade {
                    v += 60 + g.player_ai_bonus_val(p, bonus, 1).min(400) / 2;
                }
            }
            if v > 0 {
                gains.push(v);
            }
        }
        gains.sort_unstable_by(|a, b| b.cmp(a));
        // A worker improves a handful of plots before the next tech: count the best few.
        total += gains.iter().take(6).sum::<i32>() * 3;
    }
    total
}

/// Value of `t` without the situational terms (for lookahead).
fn tech_value_static(g: &Game, p: PlayerId, t: TechType) -> i32 {
    let d = g.d;
    let info = &d.techs[t];
    let vals = values(d);
    let unlocks = &vals.tech_unlocks[t];
    let pl = &g.players[p];
    let civ = g.player_get_civilization_type(p);
    let num_cities = pl.cities.len() as i32;
    let leader = pl.personality_type.map(|l| &d.leader_heads[l]);
    let mut value = 50 + info.ai_weight.min(500) / 5;
    for &b in &unlocks.buildings {
        let Some(class) = d.buildings[b].building_class_type else { continue };
        if civ.and_then(|c| d.civ_building(c, class)) != Some(b) {
            continue;
        }
        value += vals.building_value[b].max(0) * num_cities.clamp(1, 6) / 4;
    }
    value += 60 * unlocks.civics.len() as i32;
    if let Some(r) = unlocks.religion {
        if g.religion_game_turn_founded[r] == -1 && !pl.agnostic {
            value += 400;
            if let Some(l) = leader {
                if l.favorite_religion == Some(r) || l.favorite_early_religion == Some(r) {
                    value += 1200;
                }
            }
        }
    }
    if let Some(l) = leader {
        if l.favorite_tech == Some(t) {
            value += 200;
        }
        let flavor: i32 = l.flavor_value.iter().zip(info.flavor_value.iter()).map(|(a, b)| a * b).sum();
        value += flavor * 2;
    }
    value += info.trade_routes * 150 + info.happiness * 80 * num_cities.max(1) + info.health * 50 * num_cities.max(1);
    value += info.worker_speed_modifier * 2;
    value
}

/// A better defender / attacker from `techs`, weighted by the pressure we are under.
fn military_value(g: &Game, p: PlayerId, ctx: &Ctx, techs: &[TechType]) -> i32 {
    let (def, att) = best_military(g, p, ctx, techs);
    let mut value = 0;
    if def > ctx.best_defense {
        value += ((def - ctx.best_defense) * (150 + 100 * ctx.pressure) / 100) as i32;
    }
    if att > ctx.best_attack {
        value += ((att - ctx.best_attack) * (80 + 60 * ctx.pressure) / 100) as i32;
    }
    // FfH AI_chooseTech "early military beeline": past the first turns, get beyond warriors.
    let tier1 = ctx.best_attack <= 400 && ctx.best_defense <= 400;
    if tier1 && g.game_turn() >= 25 && (def > ctx.best_defense || att > ctx.best_attack) {
        value += 500 + 100 * ctx.pressure as i32;
    }
    value.max(0)
}

/// What tech `t` is worth to player `p` now, before cost.
fn tech_value(g: &Game, p: PlayerId, t: TechType, ctx: &Ctx) -> i32 {
    let d = g.d;
    let vals = values(d);
    let unlocks = &vals.tech_unlocks[t];
    let pl = &g.players[p];
    let leader = pl.personality_type.map(|l| &d.leader_heads[l]);
    let mut value = tech_value_static(g, p, t);

    // Land improvements and resources.
    value += builds_value(g, p, t, ctx);

    // Trading with the civs we know: technologies above all.
    let met = g.team_ids().into_iter().filter(|&o| o != ctx.team && o != g.barbarian_team() && g.team_is_alive(o) && g.teams[ctx.team].has_met[o]).count() as i32;
    if d.techs[t].tech_trading {
        value += 250 * met.min(5);
    }
    if d.techs[t].open_borders_trading {
        value += 60 * met.min(5);
    }

    // Civics better than what we run now (City States and Aristocracy cut maintenance).
    for &civic in &unlocks.civics {
        let Some(opt) = d.civics[civic].civic_option_type else { continue };
        let current = pl.civics[opt].map_or(0, |c| crate::empire::civic_value(g, p, c));
        value += ((crate::empire::civic_value(g, p, civic) - current) / 4).clamp(0, 2000);
    }

    // Ships: when our landmass is full, the first transport (and the tech before it).
    if ctx.home_full && !ctx.has_transport {
        let transport_tech = |t: TechType| {
            vals.tech_unlocks[t].units.iter().any(|&u| {
                crate::naval::is_transport_type(&d.units[u]) && d.units[u].unit_class_type.and_then(|cl| ctx.civ.and_then(|c| d.civ_unit(c, cl))) == Some(u)
            })
        };
        if transport_tech(t) {
            value += 1500 + 200 * ctx.num_cities.min(8);
        } else if vals.tech_children[t].iter().any(|&ch| transport_tech(ch)) {
            value += 600;
        }
    }

    // Processes (research, wealth) for cities that have nothing left to build.
    if ctx.idle_cities > 0 && !ctx.has_process && d.processes.iter().any(|pr| pr.tech_prereq == Some(t)) {
        value += 400 * ctx.idle_cities.min(6) + 300;
    }

    // Military: a better defender / attacker, as pressure demands.
    value += military_value(g, p, ctx, &[t]);
    // Casters: the first arcane unit.
    if let Some(l) = leader {
        let civ = ctx.civ;
        let has_mage_now = d.units.keys().any(|u| d.units[u].default_unit_ai_type == Some(UnitAIType::UNITAI_MAGE) && unit_available(g, p, ctx, u, &[]));
        if !has_mage_now
            && unlocks.units.iter().any(|&u| d.units[u].default_unit_ai_type == Some(UnitAIType::UNITAI_MAGE) && d.units[u].unit_class_type.and_then(|cl| civ.and_then(|c| d.civ_unit(c, cl))) == Some(u))
        {
            value += 150 + l.ai_value_mage.max(0) * 2;
        }
    }

    // Buildings for our cities as they are now (libraries where there is research to
    // multiply, markets where there is gold, ...): production's marginal value per turn in
    // the six cities that gain most.
    for &b in &unlocks.buildings {
        let binfo = &d.buildings[b];
        let Some(class) = binfo.building_class_type else { continue };
        if ctx.civ.and_then(|c| d.civ_building(c, class)) != Some(b) || binfo.prereq_religion.is_some() || d.building_classes[class].max_global_instances > 0 {
            continue;
        }
        let mut gains: Vec<i32> = pl.cities.iter().map(|&c| crate::production::building_value(g, c, b, 0).max(0)).collect();
        gains.sort_unstable_by(|a, b| b.cmp(a));
        value += gains.iter().take(6).sum::<i32>() / 4;
    }

    // Happiness buildings when cities are capped.
    if ctx.unhappy_cities > 0 {
        for &b in &unlocks.buildings {
            let binfo = &d.buildings[b];
            let Some(class) = binfo.building_class_type else { continue };
            if ctx.civ.and_then(|c| d.civ_building(c, class)) != Some(b) || binfo.prereq_religion.is_some() {
                continue;
            }
            if binfo.happiness > 0 {
                value += binfo.happiness * 120 * ctx.unhappy_cities.min(6);
            }
        }
    }

    // FfH leader hints for the early game.
    if let Some(l) = leader {
        if pl.current_era.idx() <= 1 {
            for fav in [l.favorite_early_tech1, l.favorite_early_tech2, l.favorite_early_tech3] {
                if fav == Some(t) {
                    value += 300;
                }
            }
            if l.favorite_early_mil_tech == Some(t) {
                value += 150 + 50 * ctx.pressure as i32;
            }
        }
    }
    value.max(1)
}
