//! City production (C++ `CvCityAI::AI_chooseProduction`, FfH's `CvGameUtils.AI_chooseProduction`
//! Python): a priority list -- garrison, emergency defense, the first worker, settlers while
//! good sites remain, minimal army, more workers, buildings by marginal value per turn of
//! production, army to its target, casters, missionaries -- then a process. Whatever happens,
//! a city is never left without an order (`fallback`).

use ffh_core::defs::{CITY_HOME_PLOT, NUM_CITY_PLOTS};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::sites;
use crate::view::type_attack_power;
use crate::Ai;

#[derive(Copy, Clone, Debug)]
pub enum Choice {
    Train(UnitType, UnitAIType),
    Construct(BuildingType),
    Create(ProjectType),
    Maintain(ProcessType),
}

pub fn choose(ai: &Ai, g: &mut Game, c: CityId) {
    let p = g.cities[c].owner;
    if g.player_is_barbarian(p) {
        // FfH barbarian cities pick from a fixed list (CustomFunctions.BarbCityProduction).
        if g.cf_barb_city_production(c) {
            return;
        }
    }
    if let Some(choice) = decide(ai, g, c) {
        crate::trace!(g, p, "{} size {} builds {}", g.cities[c].name, g.cities[c].population, choice_name(g, choice));
        push(g, c, choice);
    }
    if g.cities[c].order_queue.is_empty() {
        // Never leave a city idle: anything it can make.
        if let Some(choice) = fallback(g, c) {
            push(g, c, choice);
        }
    }
}

pub fn choice_name(g: &Game, choice: Choice) -> String {
    match choice {
        Choice::Train(u, ai) => format!("{} ({:?})", g.d.units[u].base.type_, ai),
        Choice::Construct(b) => g.d.buildings[b].base.type_.clone(),
        Choice::Create(pr) => g.d.projects[pr].base.type_.clone(),
        Choice::Maintain(pr) => g.d.processes[pr].base.type_.clone(),
    }
}

pub fn push(g: &mut Game, c: CityId, choice: Choice) {
    match choice {
        Choice::Train(u, unit_ai) => g.city_push_order(c, OrderType::ORDER_TRAIN, u.i(), unit_ai.i(), false, false, false, false),
        Choice::Construct(b) => g.city_push_order(c, OrderType::ORDER_CONSTRUCT, b.i(), -1, false, false, false, false),
        Choice::Create(pr) => g.city_push_order(c, OrderType::ORDER_CREATE, pr.i(), -1, false, false, false, false),
        Choice::Maintain(pr) => g.city_push_order(c, OrderType::ORDER_MAINTAIN, pr.i(), -1, false, false, false, false),
    }
}

/// Once per turn (C++ `CvCityAI::AI_doTurn` sets production dirty on danger): a city under
/// threat that is not building a defender switches to one (the progress on the old item is
/// kept by the rules).
pub fn emergency(ai: &Ai, g: &mut Game, p: PlayerId) {
    for c in g.players[p].cities.clone() {
        if !g.cities.contains(c) || g.cities[c].order_queue.is_empty() {
            continue;
        }
        let plot = g.city_plot(c);
        let threat = local_threat(g, p, plot, 2);
        if threat == 0 {
            continue;
        }
        let (defenders, defense) = garrison(g, p, plot);
        let building_fighter = g.city_production_unit(c).is_some_and(|u| g.d.units[u].combat > 0);
        if building_fighter || defense * 3 / 2 >= threat && defenders > 0 {
            continue;
        }
        crate::trace!(g, p, "{} under threat {} (defense {}): switching to defense", g.cities[c].name, threat, defense);
        g.city_clear_order_queue(c);
        choose(ai, g, c);
    }
}

/// Last resort when the priority list found nothing: the cheapest useful building, else the
/// strongest land unit per hammer, else any process.
fn fallback(g: &Game, c: CityId) -> Option<Choice> {
    let d = g.d;
    let mut best: Option<(BuildingType, i32)> = None;
    for b in d.buildings.keys() {
        if !building_allowed(g, c, b) || !g.city_can_construct(c, b, false, false, false) {
            continue;
        }
        let turns = g.city_production_turns_left_building(c, b, 0);
        if best.map_or(true, |(_, t)| turns < t) {
            best = Some((b, turns));
        }
    }
    if let Some((b, _)) = best {
        return Some(Choice::Construct(b));
    }
    if let Some(u) = best_unit(g, c, Role::Defense) {
        return Some(Choice::Train(u, UnitAIType::UNITAI_CITY_DEFENSE));
    }
    if let Some(u) = best_unit(g, c, Role::Attack) {
        return Some(Choice::Train(u, UnitAIType::UNITAI_ATTACK));
    }
    d.processes.keys().find(|&pr| g.city_can_maintain(c, pr, false)).map(Choice::Maintain)
}

/// Our units on the city plot able to defend it, and their summed defense power.
pub fn garrison(g: &Game, p: PlayerId, plot: PlotIdx) -> (i32, i32) {
    let mut n = 0;
    let mut power = 0;
    for &u in &g.map.plots[plot].units {
        let unit = &g.units[u];
        if unit.owner != p {
            continue;
        }
        let info = &g.d.units[unit.unit_type];
        if info.combat_defense <= 0 && info.combat <= 0 {
            continue;
        }
        if matches!(unit.ai_type, Some(UnitAIType::UNITAI_CITY_DEFENSE) | Some(UnitAIType::UNITAI_CITY_COUNTER) | Some(UnitAIType::UNITAI_CITY_SPECIAL)) || info.only_defensive {
            n += 1;
            power += crate::view::defense_power(g, u);
        }
    }
    (n, power)
}

/// Summed attack power of visible enemy units within `range` plots.
pub fn local_threat(g: &Game, p: PlayerId, plot: PlotIdx, range: i32) -> i32 {
    let team = g.players[p].team;
    let (x, y) = g.map.xy(plot);
    let mut threat = 0;
    for dx in -range..=range {
        for dy in -range..=range {
            let Some(q) = g.plot_xy(x, y, dx, dy) else { continue };
            if !g.plot_is_visible(q, team) {
                continue;
            }
            for &u in &g.map.plots[q].units {
                let ut = g.units[u].owner;
                let their_team = g.players[ut].team;
                // Animals cannot enter owned land: no threat to a city.
                if g.d.units[g.units[u].unit_type].animal {
                    continue;
                }
                if their_team != team && g.at_war(team, their_team) {
                    // Nearer units weigh more (distance 1: full, 3: a third).
                    let dist = dx.abs().max(dy.abs()).max(1);
                    threat += crate::view::attack_power(g, u) * (range + 1 - dist) / range;
                }
            }
        }
    }
    threat
}

/// A barbarian lair or a (non-animal) barbarian unit within `range` plots.
pub fn barbarians_near(g: &Game, p: PlayerId, plot: PlotIdx, range: i32) -> bool {
    let team = g.players[p].team;
    let barb = g.barbarian_player();
    let (x, y) = g.map.xy(plot);
    for dx in -range..=range {
        for dy in -range..=range {
            let Some(q) = g.plot_xy(x, y, dx, dy) else { continue };
            let pl = &g.map.plots[q];
            if !pl.revealed[team] {
                continue;
            }
            if pl.improvement_type.is_some_and(|i| g.d.improvements[i].spawn_unit_type.is_some_and(|u| !g.d.units[u].animal)) {
                return true;
            }
            if g.plot_is_visible(q, team) && pl.units.iter().any(|&u| g.units[u].owner == barb && !g.d.units[g.units[u].unit_type].animal) {
                return true;
            }
        }
    }
    false
}

pub fn count_ai(g: &Game, p: PlayerId, unit_ai: UnitAIType) -> i32 {
    let pl = &g.players[p];
    pl.ai_num_ai_units[unit_ai] + pl.ai_num_train_ai_units[unit_ai]
}

/// Can our workers build anything useful yet (an improvement or route whose tech we have)?
pub fn has_worker_builds(g: &Game, p: PlayerId) -> bool {
    let team = g.players[p].team;
    g.d.builds.iter().any(|b| (b.improvement.is_some() || b.route.is_some()) && b.tech_prereq.map_or(true, |t| g.team_is_has_tech(team, Some(t))))
}

/// Plots worked by our cities that have no improvement (a measure of worker demand).
pub fn unimproved_worked_plots(g: &Game, p: PlayerId) -> i32 {
    let mut n = 0;
    for &c in &g.players[p].cities {
        for i in 0..NUM_CITY_PLOTS {
            if i == CITY_HOME_PLOT || !g.cities[c].working_plot[i] {
                continue;
            }
            if let Some(q) = g.city_index_plot(c, i) {
                let plot = &g.map.plots[q];
                if plot.improvement_type.is_none() && plot.plot_type != PlotType::PLOT_OCEAN {
                    n += 1;
                }
            }
        }
    }
    n
}

/// Buildings the AI never builds by itself: a new palace (moves the capital), buildings with
/// no effect but a cost the generic valuation cannot see the downside of.
fn building_allowed(g: &Game, c: CityId, b: BuildingType) -> bool {
    let info = &g.d.buildings[b];
    if info.capital {
        // Only if we have lost our palace.
        return g.players[g.cities[c].owner].capital_city.map_or(true, |cap| !g.cities.contains(cap));
    }
    true
}

fn civ_enemies_at_war(g: &Game, p: PlayerId) -> bool {
    let team = g.players[p].team;
    let barb = g.barbarian_team();
    g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t))
}

/// The owner cannot pay for more: research slider pushed down to cover costs.
pub fn financial_trouble(g: &Game, p: PlayerId) -> bool {
    g.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH) && g.players[p].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()] < 40 && g.players[p].cities.len() > 1
}

/// City state used by the decision.
struct Ctx {
    p: PlayerId,
    n_cities: i32,
    capital: bool,
    pop: i32,
    threat: i32,
    defenders: i32,
    defense: i32,
    food_surplus: i32,
    production: i32,
    happy_room: i32,
    at_war: bool,
    broke: bool,
}

fn decide(ai: &Ai, g: &Game, c: CityId) -> Option<Choice> {
    let d = g.d;
    let city = &g.cities[c];
    let p = city.owner;
    let pl = &g.players[p];
    let plot = g.city_plot(c);
    let barbarian = g.player_is_barbarian(p);
    let leader = pl.personality_type.map(|l| &d.leader_heads[l]);
    let (defenders, defense) = garrison(g, p, plot);
    let ctx = Ctx {
        p,
        n_cities: pl.cities.len() as i32,
        capital: g.city_is_capital(c),
        pop: city.population,
        threat: local_threat(g, p, plot, 3),
        defenders,
        defense,
        food_surplus: g.city_yield_rate(c, YieldType::YIELD_FOOD) - g.city_food_consumption(c, false, 0),
        production: g.city_yield_rate(c, YieldType::YIELD_PRODUCTION),
        happy_room: g.city_happy_level(c) - g.city_unhappy_level(c, 0),
        at_war: civ_enemies_at_war(g, p),
        broke: financial_trouble(g, p),
    };
    crate::trace!(g, p, "{}: threat {} defenders {} ({}) food {:+} prod {} happy {}", city.name, ctx.threat, ctx.defenders, ctx.defense, ctx.food_surplus, ctx.production, ctx.happy_room);
    if ctx.threat > 0 && crate::trace_on(p) {
        let (x, y) = g.map.xy(plot);
        for dx in -3..=3 {
            for dy in -3..=3 {
                let Some(q) = g.plot_xy(x, y, dx, dy) else { continue };
                for &u in &g.map.plots[q].units {
                    if g.units[u].owner != p {
                        crate::trace!(g, p, "   threat unit {} owner {:?} at ({},{}) impr {:?} visible {}", d.units[g.units[u].unit_type].base.type_, g.units[u].owner, dx, dy, g.map.plots[q].improvement_type.map(|i| d.improvements[i].base.type_.clone()), g.plot_is_visible(q, pl.team));
                    }
                }
            }
        }
    }
    let flavor = |name: &str| match (leader, d.ty::<FlavorType>(name)) {
        (Some(l), Some(f)) => l.flavor_value[f],
        _ => 0,
    };
    let mil_flavor = flavor("FLAVOR_MILITARY");

    // Garrison quota: one, the capital and threatened cities more, cities near barbarian lairs
    // or barbarians more.
    let mut needed = 1 + i32::from(ctx.capital && (ctx.n_cities > 1 || g.game_turn() > 30)) + i32::from(ctx.threat > 0);
    if barbarians_near(g, p, plot, 5) {
        needed += 1;
    }
    if ctx.at_war {
        needed += 1;
    }
    if let Some(l) = leader {
        if l.ai_value_defense > 100 && ctx.n_cities > 3 {
            needed += 1;
        }
    }

    // 1. Missing or overwhelmed garrison.
    // Fortified defenders in a city fight at about 1.5x their strength.
    if ctx.defenders == 0 || (ctx.threat > 0 && ctx.defense * 3 / 2 < ctx.threat && ctx.defenders < needed + 2) {
        if let Some(u) = best_unit(g, c, Role::Defense) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_CITY_DEFENSE));
        }
    }
    if barbarian {
        return best_unit(g, c, Role::Attack).map(|u| Choice::Train(u, UnitAIType::UNITAI_ATTACK));
    }

    let workers = count_ai(g, p, UnitAIType::UNITAI_WORKER);
    let can_improve = has_worker_builds(g, p);

    // 2. The first worker, once workers have something to do.
    if workers == 0 && can_improve && ctx.threat == 0 && (ctx.pop >= 2 || ctx.food_surplus <= 1) {
        if let Some(u) = best_unit(g, c, Role::Worker) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_WORKER));
        }
    }

    // 3. Settlers while good sites remain in reach.
    if let Some(choice) = settler_choice(ai, g, c, &ctx) {
        return Some(choice);
    }

    // 4. Extra defenders up to the quota (a broke empire keeps one per city unless threatened).
    if ctx.broke && ctx.threat == 0 {
        needed = needed.min(1 + i32::from(ctx.capital));
    }
    if ctx.defenders < needed {
        if let Some(u) = best_unit(g, c, Role::Defense) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_CITY_DEFENSE));
        }
    }

    // 5. A small mobile force to clear barbarians and animals near our lands.
    let attackers = count_ai(g, p, UnitAIType::UNITAI_ATTACK) + count_ai(g, p, UnitAIType::UNITAI_ATTACK_CITY) + count_ai(g, p, UnitAIType::UNITAI_COLLATERAL);
    let min_army = if ctx.broke { 1 + ctx.n_cities / 4 } else { 1 + ctx.n_cities / 2 };
    if attackers < min_army && ctx.production >= 3 {
        if let Some(u) = best_unit(g, c, Role::Attack) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_ATTACK));
        }
    }

    // 6. More workers, as the land we work needs improving.
    if can_improve && ctx.threat == 0 {
        let unimproved = unimproved_worked_plots(g, p);
        let target = (ctx.n_cities.min(3) + (unimproved + 1) / 3).min(ctx.n_cities * 2).max(1);
        if workers < target && ctx.pop >= 2 {
            if let Some(u) = best_unit(g, c, Role::Worker) {
                return Some(Choice::Train(u, UnitAIType::UNITAI_WORKER));
            }
        }
    }

    // 7. Heroes are unique and strong.
    if let Some(u) = best_unit(g, c, Role::Hero) {
        return Some(Choice::Train(u, UnitAIType::UNITAI_HERO));
    }

    // 8. Army target (larger at war).
    let mut army_target = ctx.n_cities * (2 + mil_flavor / 5) / 2 + 1;
    if ctx.at_war {
        army_target = army_target * 2 + 2;
    } else if ctx.broke {
        army_target = min_army;
    }
    let army_short = attackers < army_target;
    // Interleave army and buildings in peacetime (deterministic: no RNG draw from the AI).
    if army_short && (ctx.at_war || (g.game_turn() + c.0 as i32) % 3 == 0) {
        if let Some(u) = best_unit(g, c, Role::Attack) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_ATTACK));
        }
    }

    // 9. Buildings by marginal value per turn of production.
    let mut best_building: Option<(BuildingType, i32)> = None;
    for b in d.buildings.keys() {
        if !building_allowed(g, c, b) || !g.city_can_construct(c, b, false, false, false) {
            continue;
        }
        let value = building_value(g, c, b, ctx.threat);
        if value <= 0 {
            continue;
        }
        let turns = g.city_production_turns_left_building(c, b, 0).max(1);
        // Nothing that ties a city up for ages (wonders belong in productive cities).
        if turns > 50 {
            continue;
        }
        let score = value * 100 / (turns + 3);
        if best_building.map_or(true, |(_, s)| score > s) {
            best_building = Some((b, score));
        }
    }
    if let Some((b, score)) = best_building {
        if score >= 40 || !army_short {
            return Some(Choice::Construct(b));
        }
    }

    // 10. Casters and missionaries.
    let mages = count_ai(g, p, UnitAIType::UNITAI_MAGE) + count_ai(g, p, UnitAIType::UNITAI_WARWIZARD);
    let mage_target = (ctx.n_cities + 2) / 4 + leader.map_or(0, |l| i32::from(l.ai_value_mage > 100));
    if mages < mage_target {
        if let Some(u) = best_unit(g, c, Role::Mage) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_MAGE));
        }
    }
    if let Some(r) = g.player_get_state_religion(p) {
        let missing = pl.cities.iter().filter(|&&cc| !g.cities[cc].has_religion[r]).count() as i32;
        if missing > 0 && count_ai(g, p, UnitAIType::UNITAI_MISSIONARY) == 0 {
            if let Some(u) = best_missionary(g, c, r) {
                return Some(Choice::Train(u, UnitAIType::UNITAI_MISSIONARY));
            }
        }
    }

    if army_short {
        if let Some(u) = best_unit(g, c, Role::Attack) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_ATTACK));
        }
    }
    if let Some((b, _)) = best_building {
        return Some(Choice::Construct(b));
    }
    if let Some(pr) = best_project(g, c) {
        return Some(Choice::Create(pr));
    }
    // 11. Convert production to research (else gold).
    let mut best_process: Option<(ProcessType, i32)> = None;
    for pr in d.processes.keys() {
        if !g.city_can_maintain(c, pr, false) {
            continue;
        }
        let m = &d.processes[pr].production_to_commerce_modifier;
        let v = m[CommerceType::COMMERCE_RESEARCH.idx()] * 2 + m[CommerceType::COMMERCE_GOLD.idx()] * 2 + m[CommerceType::COMMERCE_CULTURE.idx()];
        if best_process.map_or(true, |(_, s)| v > s) {
            best_process = Some((pr, v));
        }
    }
    best_process.map(|(pr, _)| Choice::Maintain(pr))
}

/// Settlers: while sites remain in this city's area, a bounded number at a time, from cities
/// big enough to pay for them (food goes into settler production, so growth stops).
fn settler_choice(ai: &Ai, g: &Game, c: CityId, ctx: &Ctx) -> Option<Choice> {
    let p = ctx.p;
    let pl = &g.players[p];
    if ctx.threat > 0 || g.city_is_occupation(c) {
        return None;
    }
    let settlers = count_ai(g, p, UnitAIType::UNITAI_SETTLE);
    let max_cities = if pl.max_cities > 0 { pl.max_cities } else { i32::MAX };
    if ctx.n_cities + settlers >= max_cities {
        return None;
    }
    let area = g.map.plots[g.city_plot(c)].area;
    let sites = sites::sites(ai, g, p);
    let area_sites = sites.iter().filter(|(q, _)| g.map.plots[*q].area == area).count() as i32;
    if area_sites == 0 {
        return None;
    }
    let mut max_settlers = ((ctx.n_cities + 1) / 2).max(1).min(area_sites);
    // C++ AI_chooseProduction: no settlers while in financial trouble.
    if ctx.broke && g.players[p].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()] < 20 {
        return None;
    }
    if ctx.at_war {
        max_settlers = (max_settlers + 1) / 2;
    }
    if settlers >= max_settlers {
        return None;
    }
    // Size: grow first while growth is cheap and happy; a city at its happy cap gains nothing
    // from growing, so it builds settlers at once.
    let min_pop = if ctx.happy_room <= 0 { 1 } else if ctx.food_surplus >= 4 && ctx.pop < 3 { 3 } else { 2 };
    if ctx.pop < min_pop {
        return None;
    }
    // A city with almost no output would take forever; let better cities build them.
    if ctx.n_cities > 1 && ctx.food_surplus.max(0) + ctx.production < 4 {
        return None;
    }
    best_unit(g, c, Role::Settler).map(|u| Choice::Train(u, UnitAIType::UNITAI_SETTLE))
}

/// Projects worth building by default (positive AI weight, not a world-ending or self-harming
/// ritual the generic logic cannot judge).
fn best_project(g: &Game, c: CityId) -> Option<ProjectType> {
    let d = g.d;
    let mut best: Option<(ProjectType, i32)> = None;
    for pr in d.projects.keys() {
        let info = &d.projects[pr];
        if info.ai_weight <= 0 || !g.city_can_create(c, pr, false, false) {
            continue;
        }
        let name = info.base.type_.as_str();
        // Rituals with large side effects: left to dedicated logic.
        if matches!(name, "PROJECT_SAMHAIN" | "PROJECT_THE_DRAW" | "PROJECT_STIR_FROM_SLUMBER" | "PROJECT_BANE_DIVINE" | "PROJECT_GLORY_EVERLASTING" | "PROJECT_PURGE_THE_UNFAITHFUL") {
            continue;
        }
        let turns = g.city_production_turns_left_project(c, pr, 0);
        if turns > 40 {
            continue;
        }
        let v = info.ai_weight * 100 / (turns + 5);
        if best.map_or(true, |(_, b)| v > b) {
            best = Some((pr, v));
        }
    }
    best.map(|(pr, _)| pr)
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Role {
    Defense,
    Attack,
    Worker,
    Settler,
    Hero,
    Mage,
    Explore,
}

fn is_hero(info: &UnitInfo) -> bool {
    info.default_unit_ai_type == Some(UnitAIType::UNITAI_HERO)
}

fn role_ok(info: &UnitInfo, role: Role) -> bool {
    let has = |a: UnitAIType| info.unit_ai_type[a];
    let fighter = info.combat > 0 && !info.found && !is_hero(info) && info.default_unit_ai_type != Some(UnitAIType::UNITAI_MAGE);
    match role {
        Role::Defense => {
            fighter && (has(UnitAIType::UNITAI_CITY_DEFENSE) || has(UnitAIType::UNITAI_CITY_COUNTER) || has(UnitAIType::UNITAI_ATTACK) || has(UnitAIType::UNITAI_COUNTER))
                && info.default_unit_ai_type != Some(UnitAIType::UNITAI_EXPLORE)
        }
        Role::Attack => fighter && !info.only_defensive && (has(UnitAIType::UNITAI_ATTACK) || has(UnitAIType::UNITAI_ATTACK_CITY)),
        Role::Worker => has(UnitAIType::UNITAI_WORKER) && info.combat == 0,
        Role::Settler => has(UnitAIType::UNITAI_SETTLE) && info.found,
        Role::Hero => is_hero(info) && info.combat > 0,
        Role::Mage => info.default_unit_ai_type == Some(UnitAIType::UNITAI_MAGE) || has(UnitAIType::UNITAI_WARWIZARD),
        Role::Explore => has(UnitAIType::UNITAI_EXPLORE),
    }
}

/// Defense strength of a unit type in a city (FfH units have a separate defense value).
fn type_city_defense(info: &UnitInfo) -> i64 {
    let dmg: i32 = info.damage_type_combat.iter().sum();
    let base = if info.combat_defense > 0 { info.combat_defense } else { info.combat };
    let mut v = ((base + dmg) * 100) as i64 * (100 + info.city_defense_modifier.max(-50)) as i64 / 100;
    v += v * (info.first_strikes * 2 + info.chance_first_strikes) as i64 / 20;
    v
}

/// Best unit this city can train for `role`, by power per production turn.
pub fn best_unit(g: &Game, c: CityId, role: Role) -> Option<UnitType> {
    let d = g.d;
    let mut best: Option<(UnitType, i64)> = None;
    for u in d.units.keys() {
        let info = &d.units[u];
        if info.domain_type != Some(DomainType::DOMAIN_LAND) || !role_ok(info, role) {
            continue;
        }
        if !g.city_can_train(c, u, false, false, false, false) {
            continue;
        }
        let turns = g.city_production_turns_left_unit(c, u, 0).max(1) as i64;
        let power = match role {
            Role::Defense => type_city_defense(info),
            Role::Attack | Role::Hero => {
                let mut v = type_attack_power(d, u) as i64;
                v += v * (info.moves.max(1) as i64 - 1) / 4;
                v += v * (info.first_strikes * 2 + info.chance_first_strikes) as i64 / 20;
                v += v * info.withdrawal_probability as i64 / 200;
                v += info.collateral_damage as i64;
                v
            }
            Role::Worker => 100 + info.work_rate as i64,
            Role::Settler => 100,
            Role::Mage | Role::Explore => 100 + info.moves as i64 * 10 + info.tier as i64 * 50,
        } + info.ai_weight as i64;
        if power <= 0 {
            continue;
        }
        // Strength matters more than cost: weaker but cheaper units only win by a margin.
        let score = match role {
            Role::Worker | Role::Settler => power * 1000 / turns,
            _ => power * power / 100 * 1000 / (turns + 4),
        };
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((u, score));
        }
    }
    best.map(|(u, _)| u)
}

fn best_missionary(g: &Game, c: CityId, r: ReligionType) -> Option<UnitType> {
    g.d.units.keys().filter(|&u| {
        let info = &g.d.units[u];
        info.unit_ai_type[UnitAIType::UNITAI_MISSIONARY] && info.religion_spreads[r] > 0
    }).find(|&u| g.city_can_train(c, u, false, false, false, false))
}

/// Marginal value of building `b` in city `c` (roughly: value points per turn, a hammer being
/// worth 15 as in the governor).
pub fn building_value(g: &Game, c: CityId, b: BuildingType, threat: i32) -> i32 {
    let d = g.d;
    let info = &d.buildings[b];
    let city = &g.cities[c];
    let p = city.owner;
    let n_cities = g.players[p].cities.len() as i32;
    let pop = city.population;
    let food = g.city_yield_rate(c, YieldType::YIELD_FOOD).max(0);
    let prod = g.city_yield_rate(c, YieldType::YIELD_PRODUCTION).max(0);
    let comm = g.city_yield_rate(c, YieldType::YIELD_COMMERCE).max(0);
    // Value of one point per turn of each yield / commerce type.
    let yw = [20, 15, 7];
    let rates = [food, prod, comm];
    let mut v = 0;
    for i in 0..YieldType::NUM {
        v += info.yield_change[i] * yw[i] * 10;
        v += info.yield_modifier[i] * rates[i] * yw[i] / 10;
        v += info.global_yield_modifier[i] * rates[i] * yw[i] * n_cities.min(8) / 20;
    }
    let cw = [8, 9, 4, 1];
    for i in 0..CommerceType::NUM {
        let rate = g.city_commerce_rate(c, CommerceType::from_usize(i)).max(0);
        v += (info.commerce_change[i] + info.obsolete_safe_commerce_change[i]) * cw[i] * 10;
        v += info.commerce_modifier[i] * rate * cw[i] / 10;
        v += info.global_commerce_modifier[i] * rate * cw[i] * n_cities.min(8) / 20;
    }
    // Culture: border growth matters in a new city (cheap culture buildings only).
    let culture = info.commerce_change[CommerceType::COMMERCE_CULTURE.idx()];
    if culture > 0 && info.production_cost <= 120 && city.culture_level.map_or(true, |l| l.i() <= 1) {
        v += culture.min(3) * 150;
    }

    // Happiness and health, worth a citizen when the city is at its cap.
    let happy_room = g.city_happy_level(c) - g.city_unhappy_level(c, 0);
    let mut happy = info.happiness + info.area_happiness + info.global_happiness + info.state_religion_happiness * i32::from(info.state_religion.is_some());
    for bonus in d.bonuses.keys() {
        if info.bonus_happiness_changes[bonus] != 0 && g.city_has_bonus(c, bonus) {
            happy += info.bonus_happiness_changes[bonus];
        }
    }
    if happy != 0 {
        let per = if happy_room <= 0 { 180 } else if happy_room <= 1 { 100 } else { 35 };
        v += happy * per;
    }
    v += (info.area_happiness * 30 + info.global_happiness * 50) * (n_cities - 1).max(0);
    let health_room = g.city_good_health(c) - g.city_bad_health(c, false, 0);
    let mut health = info.health + info.area_health + info.global_health;
    for bonus in d.bonuses.keys() {
        if info.bonus_health_changes[bonus] != 0 && g.city_has_bonus(c, bonus) {
            health += info.bonus_health_changes[bonus];
        }
    }
    if health != 0 {
        let per = if health_room <= 0 { 120 } else if health_room <= 2 { 60 } else { 20 };
        v += health * per;
    }
    // Specialists slots in a big city.
    let slots: i32 = info.specialist_count.iter().sum();
    v += slots * 15 * (pop - 4).clamp(0, 8);
    v += info.great_people_rate_change * 40 + info.great_people_rate_modifier * pop / 4;
    v += info.free_specialist * 120;
    v += info.defense_modifier * if threat > 0 { 6 } else { 1 } / 2;
    // Experience matters in cities that train units.
    let xp: i32 = info.free_experience + info.unit_combat_free_experience.iter().sum::<i32>().min(4) + info.domain_free_experience[DomainType::DOMAIN_LAND];
    v += xp * prod.min(15) * 4;
    v += info.military_production_modifier * prod / 4;
    v += info.trade_routes * 60;
    // Granary-like food storage: worth more while the city grows.
    v += info.food_kept * food.min(12) / 4;
    v += info.worker_speed_modifier / 2;
    v += info.maintenance_modifier.min(0) * -g.city_maintenance(c).max(0) / 100 * 8;
    v += info.heal_rate_change * 2;
    v += info.ai_weight;
    if let Some(l) = g.players[p].personality_type.map(|l| &d.leader_heads[l]) {
        if info.building_class_type.is_some() && (l.favorite_early_wonder == info.building_class_type || l.favorite_late_wonder == info.building_class_type) {
            v += 400;
        }
        if l.favorite_wonder == Some(b) {
            v += 300;
        }
    }
    v
}
