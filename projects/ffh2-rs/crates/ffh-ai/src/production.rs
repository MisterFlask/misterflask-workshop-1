//! City production (C++ `CvCityAI::AI_chooseProduction`, FfH's `CvGameUtils.AI_chooseProduction`
//! Python): a short priority list (emergency defense, workers, settlers, heroes, army, buildings
//! by marginal value, casters, missionaries), then a process.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::sites;
use crate::view::{type_attack_power, type_defense_power};
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
        push(g, c, choice);
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
                if their_team != team && g.at_war(team, their_team) {
                    threat += crate::view::attack_power(g, u);
                }
            }
        }
    }
    threat
}

fn count_ai(g: &Game, p: PlayerId, unit_ai: UnitAIType) -> i32 {
    let pl = &g.players[p];
    pl.ai_num_ai_units[unit_ai] + pl.ai_num_train_ai_units[unit_ai]
}

fn decide(ai: &Ai, g: &Game, c: CityId) -> Option<Choice> {
    let d = g.d;
    let city = &g.cities[c];
    let p = city.owner;
    let pl = &g.players[p];
    let plot = g.city_plot(c);
    let n_cities = pl.cities.len() as i32;
    let barbarian = g.player_is_barbarian(p);
    let leader = pl.personality_type.map(|l| &d.leader_heads[l]);

    let threat = local_threat(g, p, plot, 3);
    let (defenders, defense) = garrison(g, p, plot);
    let capital = g.city_is_capital(c);
    let mut needed = 1 + i32::from(capital) + i32::from(threat > 0);
    if let Some(l) = leader {
        if l.ai_value_defense > 100 && n_cities > 2 {
            needed += 1;
        }
    }

    // 1. Emergency or missing garrison.
    if defenders == 0 || (threat > 0 && defense < threat && defenders < needed + 2) {
        if let Some(u) = best_unit(g, c, Role::Defense) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_CITY_DEFENSE));
        }
        if let Some(u) = best_unit(g, c, Role::Attack) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_CITY_DEFENSE));
        }
    }
    if barbarian {
        return best_unit(g, c, Role::Attack).map(|u| Choice::Train(u, UnitAIType::UNITAI_ATTACK));
    }

    // 2. Workers.
    let workers = count_ai(g, p, UnitAIType::UNITAI_WORKER);
    let worker_target = if n_cities <= 1 { 1 } else { n_cities * 3 / 2 };
    if workers < worker_target && city.population >= 2 && threat == 0 {
        if let Some(u) = best_unit(g, c, Role::Worker) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_WORKER));
        }
    }

    // 3. Settlers while there is room.
    let settlers = count_ai(g, p, UnitAIType::UNITAI_SETTLE);
    let max_cities = if pl.max_cities > 0 { pl.max_cities } else { i32::MAX };
    if threat == 0 && n_cities + settlers < max_cities && city.population >= 2 {
        let num_sites = sites::num_sites(ai, g, p);
        if settlers < num_sites.min(2) && settlers < (n_cities + 1) / 2 + 1 {
            if let Some(u) = best_unit(g, c, Role::Settler) {
                return Some(Choice::Train(u, UnitAIType::UNITAI_SETTLE));
            }
        }
    }

    // 4. Extra defenders.
    if defenders < needed {
        if let Some(u) = best_unit(g, c, Role::Defense) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_CITY_DEFENSE));
        }
    }

    // 5. Heroes are unique and strong: always take them.
    if let Some(u) = best_unit(g, c, Role::Hero) {
        return Some(Choice::Train(u, UnitAIType::UNITAI_HERO));
    }

    // 6. Army.
    let attackers = count_ai(g, p, UnitAIType::UNITAI_ATTACK) + count_ai(g, p, UnitAIType::UNITAI_ATTACK_CITY);
    let mil_flavor = match (leader, d.ty::<FlavorType>("FLAVOR_MILITARY")) {
        (Some(l), Some(f)) => l.flavor_value[f],
        _ => 0,
    };
    let at_war = g.teams.keys().any(|t| t != pl.team && t != g.barbarian_team() && g.teams[t].alive_count > 0 && g.at_war(pl.team, t));
    let mut army_target = n_cities * (2 + mil_flavor / 5) / 2;
    if at_war {
        army_target = army_target * 2 + 2;
    }
    let army_short = attackers < army_target;
    // Interleave army and buildings in peacetime (deterministic: no RNG draw from the AI).
    if army_short && (at_war || (g.game_turn() + c.0 as i32) % 3 == 0) {
        if let Some(u) = best_unit(g, c, Role::Attack) {
            return Some(Choice::Train(u, UnitAIType::UNITAI_ATTACK));
        }
    }

    // 7. Buildings by marginal value per turn of production.
    let mut best_building: Option<(BuildingType, i32)> = None;
    for b in d.buildings.keys() {
        if !g.city_can_construct(c, b, false, false, false) {
            continue;
        }
        let value = building_value(g, c, b, threat);
        if value <= 0 {
            continue;
        }
        let turns = g.city_production_turns_left_building(c, b, 0).max(1);
        let score = value * 100 / turns;
        if best_building.map_or(true, |(_, s)| score > s) {
            best_building = Some((b, score));
        }
    }
    if let Some((b, score)) = best_building {
        if score >= 40 || !army_short {
            return Some(Choice::Construct(b));
        }
    }

    // 8. Casters and missionaries.
    let mages = count_ai(g, p, UnitAIType::UNITAI_MAGE) + count_ai(g, p, UnitAIType::UNITAI_WARWIZARD);
    let mage_target = (n_cities + 2) / 4 + leader.map_or(0, |l| i32::from(l.ai_value_mage > 100));
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
    for pr in d.projects.keys() {
        if g.city_can_create(c, pr, false, false) && d.projects[pr].ai_weight > 0 {
            return Some(Choice::Create(pr));
        }
    }
    // 9. Convert production to research (else gold).
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
    if let Some((pr, _)) = best_process {
        return Some(Choice::Maintain(pr));
    }
    best_unit(g, c, Role::Defense).map(|u| Choice::Train(u, UnitAIType::UNITAI_CITY_DEFENSE))
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

fn role_ok(info: &UnitInfo, role: Role) -> bool {
    let has = |a: UnitAIType| info.unit_ai_type[a];
    match role {
        Role::Defense => has(UnitAIType::UNITAI_CITY_DEFENSE) && info.combat_defense > 0,
        Role::Attack => (has(UnitAIType::UNITAI_ATTACK) || has(UnitAIType::UNITAI_ATTACK_CITY)) && info.combat > 0,
        Role::Worker => has(UnitAIType::UNITAI_WORKER),
        Role::Settler => has(UnitAIType::UNITAI_SETTLE) && info.found,
        Role::Hero => has(UnitAIType::UNITAI_HERO),
        Role::Mage => has(UnitAIType::UNITAI_MAGE) || has(UnitAIType::UNITAI_WARWIZARD),
        Role::Explore => has(UnitAIType::UNITAI_EXPLORE),
    }
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
            Role::Defense => type_defense_power(d, u) as i64 * (100 + info.city_defense_modifier.max(0)) as i64 / 100,
            Role::Attack | Role::Hero => {
                let mut v = type_attack_power(d, u) as i64;
                v += v * (info.moves.max(1) as i64 - 1) / 4;
                v += v * (info.first_strikes * 2 + info.chance_first_strikes) as i64 / 20;
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
        let score = power * 1000 / (turns + 4);
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

/// Marginal value of building `b` in city `c` (roughly: yield points per turn x 10).
pub fn building_value(g: &Game, c: CityId, b: BuildingType, threat: i32) -> i32 {
    let d = g.d;
    let info = &d.buildings[b];
    let city = &g.cities[c];
    let p = city.owner;
    let food = g.city_yield_rate(c, YieldType::YIELD_FOOD).max(0);
    let prod = g.city_yield_rate(c, YieldType::YIELD_PRODUCTION).max(0);
    let comm = g.city_yield_rate(c, YieldType::YIELD_COMMERCE).max(0);
    let yw = [8, 7, 4];
    let rates = [food, prod, comm];
    let mut v = 0;
    for i in 0..YieldType::NUM {
        v += info.yield_change[i] * yw[i] * 10;
        v += info.yield_modifier[i] * rates[i] * yw[i] / 10;
    }
    let cw = [4, 5, 2, 1];
    for i in 0..CommerceType::NUM {
        v += info.commerce_change[i] * cw[i] * 10;
        v += info.commerce_modifier[i] * city.commerce_rate[i].max(0) * cw[i] / 10;
    }
    let unhappy = g.city_unhappy_level(c, 0) - g.city_happy_level(c);
    if info.happiness > 0 {
        v += info.happiness * if unhappy >= 0 { 120 } else { 30 };
    }
    let unhealthy = g.city_bad_health(c, false, 0) - g.city_good_health(c);
    if info.health > 0 {
        v += info.health * if unhealthy >= 0 { 80 } else { 20 };
    }
    v += info.area_happiness * 40 + info.global_happiness * 60 * g.players[p].cities.len() as i32;
    v += info.defense_modifier * if threat > 0 { 4 } else { 1 } / 2;
    v += info.free_experience * 20;
    v += info.trade_routes * 60;
    v += info.great_people_rate_change * 40;
    v += info.food_kept;
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
