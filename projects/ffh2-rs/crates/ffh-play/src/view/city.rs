//! The city screen (`city {city}`, design §4.6): the human's own cities only (C++ opens the city
//! screen for the active player's cities; debug reveal opens any city).
//!
//! The happiness reads fill the city rank caches (F11): the query runs under a `QueryGuard`,
//! which restores them, so building a city screen leaves the save bytes as they were (tested).

use ffh_core::ids::*;
use ffh_data::*;

use crate::json::Json;
use crate::query::QueryErr;
use crate::view::{city_name, production, unit_json, Viewer};

fn all<I: Idx>(n: usize) -> impl Iterator<Item = I> {
    (0..n).map(I::from_usize)
}

/// The buildable list: units, buildings, projects and processes the city screen shows
/// (`canTrain/canConstruct/canCreate(.., bTestVisible = true)`), `enabled` when they can be
/// started now, with cost (production needed) and turns.
pub fn buildable(v: &Viewer, c: CityId) -> Json {
    let g = v.g;
    let d = g.d;
    let mut out = Vec::new();
    for u in d.units.keys() {
        if g.city_offers_unit(c, u) && g.city_can_train(c, u, false, true, false, false) {
            out.push(crate::jobj! {
                "order" => Json::idx(OrderType::ORDER_TRAIN), "type" => Json::idx(u),
                "enabled" => g.city_can_train(c, u, false, false, false, false),
                "cost" => g.city_production_needed_unit(c, u), "turns" => g.city_production_turns_left_unit(c, u, 0),
            });
        }
    }
    for b in d.buildings.keys() {
        if g.city_can_construct(c, b, false, true, false) {
            out.push(crate::jobj! {
                "order" => Json::idx(OrderType::ORDER_CONSTRUCT), "type" => Json::idx(b),
                "enabled" => g.city_can_construct(c, b, false, false, false),
                "cost" => g.city_production_needed_building(c, b), "turns" => g.city_production_turns_left_building(c, b, 0),
            });
        }
    }
    for p in d.projects.keys() {
        if g.city_can_create(c, p, false, true) {
            out.push(crate::jobj! {
                "order" => Json::idx(OrderType::ORDER_CREATE), "type" => Json::idx(p),
                "enabled" => g.city_can_create(c, p, false, false),
                "cost" => g.city_production_needed_project(c, p), "turns" => g.city_production_turns_left_project(c, p, 0),
            });
        }
    }
    for p in d.processes.keys() {
        if g.city_can_maintain(c, p, false) {
            out.push(crate::jobj! { "order" => Json::idx(OrderType::ORDER_MAINTAIN), "type" => Json::idx(p), "enabled" => true, "cost" => Json::Null, "turns" => Json::Null });
        }
    }
    Json::Arr(out)
}

/// The production queue with each order's turns (from what is stored, for the head order).
fn queue(v: &Viewer, c: CityId) -> Json {
    let g = v.g;
    let city = &g.cities[c];
    let out = city.order_queue.iter().enumerate().map(|(i, o)| {
        let k = o.data1;
        let ok = |n: usize| k >= 0 && (k as usize) < n;
        let d = g.d;
        let turns = match o.order_type {
            OrderType::ORDER_TRAIN if ok(d.units.len()) => Json::from(g.city_production_turns_left_unit(c, UnitType::from_usize(k as usize), i as i32)),
            OrderType::ORDER_CONSTRUCT if ok(d.buildings.len()) => Json::from(g.city_production_turns_left_building(c, BuildingType::from_usize(k as usize), i as i32)),
            OrderType::ORDER_CREATE if ok(d.projects.len()) => Json::from(g.city_production_turns_left_project(c, ProjectType::from_usize(k as usize), i as i32)),
            _ => Json::Null,
        };
        crate::jobj! { "order" => Json::idx(o.order_type), "type" => k, "save" => o.save, "turns" => turns }
    });
    Json::Arr(out.collect())
}

/// `city {city}`: the full city screen.
pub fn city(v: &Viewer, c: CityId) -> Result<Json, QueryErr> {
    let g = v.g;
    let d = g.d;
    let Some(city) = g.cities.get(c) else { return Err(QueryErr::Gone) };
    if city.owner != v.h && !v.reveal {
        return Err(QueryErr::NotOwner);
    }
    // City plots: yields, worked here or by another city.
    let mut plots = Vec::new();
    for i in 0..g.city_num_city_plots(c).max(0) as usize {
        let Some(p) = g.city_index_plot(c, i) else { continue };
        let pl = &g.map.plots[p];
        plots.push(crate::jobj! {
            "i" => i,
            "x" => pl.x,
            "y" => pl.y,
            "revealed" => v.plot_revealed(p),
            "owner" => v.plot_owner(p),
            "worked" => g.city_is_working_plot(c, p),
            // A click on it works it or stops working it (`alterWorkingPlot`); else, on our
            // plot, it takes the plot from the city working it (the working override).
            "can_work" => g.city_can_work(c, p),
            "worked_by" => pl.working_city.filter(|&w| w != c && v.city_revealed(w)),
            "override" => pl.working_city_override,
            "yields" => if v.plot_visible(p) { Json::from(pl.yields.to_vec()) } else { Json::Null },
        });
    }
    let specialists = d.specialists.keys().map(|s| {
        // The +/- buttons as the city screen shows them (what `do_task CHANGE_SPECIALIST`
        // accepts).
        let (plus, minus) = crate::command::specialist_buttons(g, c, s);
        crate::jobj! {
            "specialist" => Json::idx(s),
            "count" => city.specialist_count[s],
            "forced" => city.force_specialist_count[s],
            "free" => city.free_specialist_count[s],
            "max" => city.max_specialist_count[s],
            "can_add" => g.city_is_specialist_valid(c, s, 1),
            "plus" => plus,
            "minus" => minus,
        }
    });
    let emphasize: Vec<bool> = d.emphasizes.keys().map(|e| city.ai_emphasize[e]).collect();
    let yields = all::<YieldType>(YieldType::NUM).map(|y| {
        crate::jobj! { "yield" => Json::idx(y), "rate" => g.city_yield_rate(c, y), "base" => city.base_yield_rate[y.idx()], "modifier" => city.yield_rate_modifier[y.idx()] }
    });
    let commerces = all::<CommerceType>(CommerceType::NUM).map(|k| crate::jobj! { "commerce" => Json::idx(k), "rate" => g.city_commerce_rate(c, k) });
    let happiness = crate::jobj! {
        "happy" => g.city_happy_level(c),
        "unhappy" => g.city_unhappy_level(c, 0),
        "angry" => g.city_angry_population(c, 0),
        "buildings" => city.building_good_happiness + city.extra_building_good_happiness,
        "buildings_bad" => city.building_bad_happiness + city.extra_building_bad_happiness,
        "religions" => city.religion_good_happiness,
        "religions_bad" => city.religion_bad_happiness,
        "bonuses" => city.bonus_good_happiness,
        "bonuses_bad" => city.bonus_bad_happiness,
        "features" => city.feature_good_happiness,
        "features_bad" => city.feature_bad_happiness,
        "military" => g.city_military_happiness(c),
        "largest_city" => g.city_largest_city_happiness(c),
        "commerce" => g.city_commerce_happiness(c),
        "extra" => city.extra_happiness,
        "war_weariness_percent_anger" => g.city_war_weariness_percent_anger(c),
        "hurry_anger_timer" => city.hurry_anger_timer,
        "conscript_anger_timer" => city.conscript_anger_timer,
    };
    let health = crate::jobj! {
        "good" => g.city_good_health(c),
        "bad" => g.city_bad_health(c, false, 0),
        "unhealthy_population" => g.city_unhealthy_population(c, false, 0),
        "fresh_water" => city.fresh_water_good_health,
        "fresh_water_bad" => city.fresh_water_bad_health,
        "features" => city.feature_good_health,
        "features_bad" => city.feature_bad_health,
        "buildings" => city.building_good_health,
        "buildings_bad" => city.building_bad_health,
        "power" => city.power_good_health,
        "power_bad" => city.power_bad_health,
        "bonuses" => city.bonus_good_health,
        "bonuses_bad" => city.bonus_bad_health,
        "extra" => city.extra_health,
    };
    let buildings: Vec<Json> = d
        .buildings
        .keys()
        .filter(|&b| city.num_real_building[b] > 0 || city.num_free_building[b] > 0)
        .map(|b| crate::jobj! { "building" => Json::idx(b), "real" => city.num_real_building[b], "free" => city.num_free_building[b] })
        .collect();
    let religions: Vec<Json> = d
        .religions
        .keys()
        .filter(|&r| city.has_religion[r])
        .map(|r| crate::jobj! { "religion" => Json::idx(r), "holy" => g.city_is_holy_city(c, r) })
        .collect();
    let plot = g.city_plot(c);
    let garrison: Vec<Json> = g.map.plots[plot].units.iter().filter(|&&u| v.unit_visible(u)).map(|&u| unit_json(v, u)).collect();
    let hurries: Vec<Json> = d
        .hurries
        .keys()
        .filter(|&hu| g.city_can_hurry(c, hu, true))
        .map(|hu| crate::jobj! {
            "hurry" => Json::idx(hu), "enabled" => g.city_can_hurry(c, hu, false),
            "gold" => g.city_hurry_gold(c, hu), "population" => g.city_hurry_population(c, hu),
        })
        .collect();
    Ok(crate::jobj! {
        "id" => c,
        "name" => city_name(g, c),
        "owner" => city.owner,
        "x" => city.x,
        "y" => city.y,
        "size" => city.population,
        "capital" => g.city_is_capital(c),
        "founded" => city.game_turn_founded,
        "culture_level" => Json::opt_idx(city.culture_level),
        "culture" => g.city_culture(c, city.owner),
        "culture_threshold" => g.city_culture_threshold(c),
        "defense" => g.city_defense_modifier(c, false),
        "defense_damage" => city.defense_damage,
        "maintenance" => g.city_maintenance(c),
        "crime" => g.city_crime(c),
        "trade_routes" => g.city_trade_routes(c),
        "disorder" => g.city_is_disorder(c),
        "occupation" => city.occupation_timer,
        "food" => crate::jobj! {
            "stored" => city.food,
            "threshold" => g.city_growth_threshold(c),
            "difference" => g.city_food_difference(c, true),
            "consumption" => g.city_food_consumption(c, false, 0),
            "turns" => g.city_food_turns_left(c),
        },
        "production" => crate::jobj! {
            "current" => production(g, c),
            "stored" => g.city_production(c),
            "needed" => if city.order_queue.is_empty() { Json::Null } else { Json::from(g.city_production_needed(c)) },
            "rate" => g.city_current_production_difference(c, false, true),
            "automated" => city.production_automated,
        },
        "citizens_automated" => city.citizens_automated,
        "emphasize" => emphasize,
        "extra_population" => g.city_extra_population(c),
        "yields" => Json::Arr(yields.collect()),
        "commerces" => Json::Arr(commerces.collect()),
        "happiness" => happiness,
        "health" => health,
        "great_people" => crate::jobj! { "progress" => city.great_people_progress, "rate" => g.city_great_people_rate(c) },
        "plots" => Json::Arr(plots),
        "specialists" => Json::Arr(specialists.collect()),
        "free_specialists" => g.city_total_free_specialists(c),
        "buildings" => Json::Arr(buildings),
        "religions" => Json::Arr(religions),
        "garrison" => Json::Arr(garrison),
        "queue" => queue(v, c),
        "hurries" => Json::Arr(hurries),
        "can_conscript" => g.city_can_conscript(c),
        "buildable" => buildable(v, c),
    })
}
