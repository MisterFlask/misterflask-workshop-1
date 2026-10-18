//! Random-event choices (C++ `CvPlayerAI::AI_chooseEvent` / `AI_eventValue`).
//!
//! The rules offer an AI player the trigger's events it can do, in trigger order. Without this
//! the controller default took the first, and FfH lists the decline / pay / lose-a-unit answer
//! first in many triggers: War Games disbanded the unit, the Strange Advisor added 3% inflation
//! for good, Clairone's first answer turned down a free Great Artist, and every Adaptive leader
//! (Varn, Cardith, Cassiel) became Aggressive every 100 turns.
//!
//! `value` follows `AI_eventValue` term by term, in one unit (gold) instead of the C++ mix of
//! gold, hammers x100 and per-turn yields x20, and without its random draws (the AI draws no
//! RNG): `rand(iAIValue)` is its mean, the final +0-20% is dropped. Effects that live in Python
//! callbacks are not valued, as in C++, except the Adaptive trait choice (`trait_value`, from the
//! trait's data). Ties keep the trigger's order.

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::*;
use ffh_data::*;

/// Turns a permanent change is counted for (at normal speed).
const PERMANENT_TURNS: i64 = 50;
/// Turns an Adaptive trait choice lasts (`EVENTTRIGGER_TRAIT_ADAPTIVE` recurs every 100 turns).
const TRAIT_TURNS: i64 = 100;
/// A unit whose production cost is -1 (great people).
const GREAT_PERSON_VALUE: i64 = 600;
/// A building whose production cost is -1 (shrines).
const SPECIAL_BUILDING_VALUE: i64 = 300;
/// Value of an option that would declare a war we did not plan.
const UNWANTED: i64 = -1_000_000;

pub fn choose(g: &Game, p: PlayerId, t: &EventTriggeredData, options: &[EventType]) -> Option<EventType> {
    let mut best: Option<(EventType, i64)> = None;
    for &e in options {
        let v = value(g, p, t, e, 0);
        crate::trace!(g, p, "event {} option {} value {}", g.d.event_triggers[t.trigger].base.type_, g.d.events[e].base.type_, v);
        if best.map_or(true, |(_, b)| v > b) {
            best = Some((e, v));
        }
    }
    best.map(|(e, _)| e)
}

/// Per-turn gold value of one point of each yield and commerce.
const YIELD_W: [i64; YieldType::NUM] = [2, 2, 1];
const COMMERCE_W: [i64; CommerceType::NUM] = [1, 1, 1, 1];

fn speed_pct(g: &Game) -> i64 {
    g.d.game_speeds[g.game_speed_type()].victory_delay_percent.max(1) as i64
}

fn permanent(g: &Game) -> i64 {
    PERMANENT_TURNS * speed_pct(g) / 100
}

/// Hammers as gold, at this game's speed (`train_percent` / `construct_percent`).
fn unit_value(g: &Game, ut: UnitType) -> i64 {
    let cost = g.d.units[ut].production_cost as i64;
    if cost < 0 {
        return GREAT_PERSON_VALUE * speed_pct(g) / 100;
    }
    2 * cost * g.d.game_speeds[g.game_speed_type()].train_percent as i64 / 100
}

fn building_value(g: &Game, b: BuildingType) -> i64 {
    let cost = g.d.buildings[b].production_cost as i64;
    if cost < 0 {
        return SPECIAL_BUILDING_VALUE * speed_pct(g) / 100;
    }
    2 * cost * g.d.game_speeds[g.game_speed_type()].construct_percent as i64 / 100
}

/// Gold per turn of one point of happiness (health) in city `c`: only a city at or near its
/// cap gains from it (an empire-wide change counts every city).
fn cap_w(margin: i32, at_cap: i64) -> i64 {
    if margin <= 0 {
        at_cap
    } else if margin <= 2 {
        1
    } else {
        0
    }
}

fn happy_w(g: &Game, c: CityId) -> i64 {
    cap_w(g.city_happy_level(c) - g.city_unhappy_level(c, 0), 3)
}

fn health_w(g: &Game, c: CityId) -> i64 {
    cap_w(g.city_good_health(c) - g.city_bad_health(c, false, 0), 2)
}

/// Sum of `w` over the event's city, or over all our cities for an empire-wide effect.
fn cities_w(g: &Game, p: PlayerId, city: Option<CityId>, city_effect: bool, w: fn(&Game, CityId) -> i64) -> i64 {
    if city_effect {
        city.map_or(1, |c| w(g, c))
    } else {
        g.players[p].cities.iter().filter(|&&c| g.cities.contains(c)).map(|&c| w(g, c)).sum()
    }
}

/// Gold value of what an event's Python callback does, for the callbacks that come up and
/// whose effect is clear (the rest, as in C++, count 0).
fn python_value(g: &Game, p: PlayerId, ev: &EventInfo) -> i64 {
    match ev.python_callback.as_str() {
        // A hostile Giant Spider on the plot / a Hill Giant raiding the city.
        "doSpiderMine3" => -150,
        "doPigGiant3" => -250,
        // A free technology of our choice.
        "applyIronOrb3" => g.player_get_current_research(p).map_or(0, |t| g.team_get_research_cost(g.players[p].team, t) as i64),
        // The city secedes as a new civilization.
        "doCitySplit1" => -3000,
        "doJudgementRight" => 10 * 2,
        "doJudgementWrong" => -3 * 2,
        _ => 0,
    }
}

fn value(g: &Game, p: PlayerId, t: &EventTriggeredData, e: EventType, depth: u32) -> i64 {
    let d = g.d;
    let ev = &d.events[e];
    let team = g.players[p].team;
    let civ = g.player_get_civilization_type(p);
    let num_cities = g.players[p].num_cities() as i64;
    let city = t.city.filter(|&c| g.cities.contains(c) && g.cities[c].owner == p);
    let unit = t.unit.filter(|&u| g.units.contains(u) && g.units[u].owner == p);
    let plot = g.map.plot_xy(t.plot_x, t.plot_y);
    let perm = permanent(g);

    // C++ refuses a war on a civ it likes or one stronger than its defence; this AI plans its
    // own wars (one at a time), so an event never starts one.
    if ev.declare_war {
        if let Some(o) = t.other_player {
            if !g.at_war(team, g.players[o].team) {
                return UNWANTED;
            }
        }
    }

    let mut v: i64 = ev.ai_value as i64 / 2;
    v += (g.player_get_event_cost(p, e, t.other_player, false) + g.player_get_event_cost(p, e, t.other_player, true)) as i64 / 2;
    v += ev.espionage_points as i64 / 2;

    if ev.tech_percent != 0 {
        if let Some(tech) = g.player_get_best_event_tech(p, e, t.other_player) {
            v += g.team_get_research_cost(team, tech) as i64 * ev.tech_percent as i64 / 100;
        }
    }

    if let (Some(class), Some(civ)) = (ev.unit_class, civ) {
        if let Some(ut) = d.civilizations[civ].civilization_units(class, d.unit_classes[class].default_unit_index) {
            v += ev.num_units as i64 * unit_value(g, ut);
        }
    }
    if ev.disband_unit {
        if let Some(u) = unit {
            v -= unit_value(g, g.units[u].unit_type) + 10 * g.units[u].experience as i64;
        }
    }

    if let (Some(class), Some(_), Some(civ)) = (ev.building_class, city, civ) {
        if let Some(b) = d.civilizations[civ].civilization_buildings(class, &d.building_classes) {
            v += ev.building_change as i64 * building_value(g, b);
        }
    }

    if ev.golden_age {
        let pop: i64 = g.players[p].cities.iter().map(|&c| g.cities[c].population as i64).sum();
        v += g.player_get_golden_age_length(p) as i64 * (pop + num_cities);
    }

    // Building yield, commerce, happiness and health changes: in the event's city, or in every
    // city that has the building (C++ counts each change once, built or not).
    let has_class = |class: BuildingClassType| -> i64 {
        if ev.city_effect {
            city.map_or(0, |c| g.city_is_has_building_class(c, class) as i64)
        } else {
            g.players[p].cities.iter().filter(|&&c| g.city_is_has_building_class(c, class)).count() as i64
        }
    };
    let mut per_turn: i64 = 0;
    for ch in &ev.building_yield_changes {
        per_turn += has_class(ch.building_class) * ch.change as i64 * YIELD_W[ch.yield_type.0 as usize];
    }
    for ch in &ev.building_commerce_changes {
        per_turn += has_class(ch.building_class) * ch.change as i64 * COMMERCE_W[ch.commerce_type.0 as usize];
    }
    for &(class, ch) in &ev.building_happy_changes {
        per_turn += has_class(class) * ch as i64;
    }
    for &(class, ch) in &ev.building_health_changes {
        per_turn += has_class(class) * ch as i64;
    }

    // City (or empire-wide) effects.
    let (cities, pop) = if ev.city_effect {
        (1, city.map_or(5, |c| g.cities[c].population as i64))
    } else if !ev.other_player_city_effect {
        (num_cities, 0)
    } else {
        (0, 0)
    };
    if cities > 0 {
        if ev.city_effect && city.is_some() {
            for (s, &n) in ev.free_specialist_count.iter_enumerated() {
                if n > 0 {
                    let sp = &d.specialists[s];
                    let yv: i64 = (0..YieldType::NUM).map(|y| sp.yield_change[y] as i64 * YIELD_W[y]).sum();
                    let cv: i64 = (0..CommerceType::NUM).map(|k| sp.commerce_change[k] as i64 * COMMERCE_W[k]).sum();
                    per_turn += n as i64 * (yv + cv).max(1);
                }
            }
        }
        let hw = cities_w(g, p, city, ev.city_effect, happy_w);
        let lw = cities_w(g, p, city, ev.city_effect, health_w);
        per_turn += ev.happy as i64 * hw + ev.health as i64 * lw;
        v += cities * ev.food as i64;
        v += cities * ev.food_percent as i64 / 4;
        v += cities * ev.population_change as i64 * 4 * YIELD_W[0] * 10;
        // A city in revolt produces nothing; hurry anger is one unhappy citizen per level for
        // HURRY_ANGER_DIVISOR turns, decaying.
        v -= cities * ev.revolt_turns as i64 * (4 + pop * 3);
        let anger_turns = d.defines.hurry_anger_divisor as i64 * d.game_speeds[g.game_speed_type()].hurry_conscript_anger_percent as i64 / 100;
        v -= ev.hurry_anger as i64 * anger_turns * hw.max(cities) / 2;
        v += ev.happy_turns as i64 * hw * d.defines.temp_happy.max(1) as i64;
        v += cities * ev.culture as i64 / 2;
    }
    v += -ev.crime as i64 * 2;

    let bonus_val = ev.bonus.map_or(0, |b| g.player_ai_bonus_val(p, b, 0) as i64);
    if let Some(pl) = plot {
        if ev.feature_change != 0 {
            if let Some(old) = g.map.plots[pl].feature_type {
                let new = if ev.feature_change > 0 { ev.feature.map_or(0, |f| d.features[f].health_percent) } else { 0 };
                v += (new - d.features[old].health_percent) as i64 * speed_pct(g) / 100;
            }
        }
        v += ev.improvement_change.signum() as i64 * 30 * speed_pct(g) / 100;
        v += ev.route_change.signum() as i64 * 10 * speed_pct(g) / 100;
        v += ev.bonus_change.signum() as i64 * bonus_val * 10 * speed_pct(g) / 100;
        let worked = g.map.plots[pl].working_city.is_some_and(|c| g.cities.contains(c) && g.cities[c].owner == p);
        for y in 0..YieldType::NUM {
            let n = ev.plot_extra_yield[y] as i64;
            if n != 0 {
                per_turn += if worked { n * YIELD_W[y] } else { n * YIELD_W[y] / 2 };
            }
        }
    }
    if ev.bonus_revealed.is_some() {
        v += bonus_val * 5 * speed_pct(g) / 100;
    }

    if unit.is_some() {
        v += 10 * ev.unit_experience as i64;
        v -= 10 * ev.unit_immobile_turns as i64;
    }

    // Free promotions for a unit combat (or class): every unit of ours that lacks it, plus the
    // ones still to come.
    let mut promo = 0;
    for (uc, pr) in ev.unit_combat_promotion.iter_enumerated() {
        let Some(pr) = *pr else { continue };
        for &u in &g.players[p].units {
            if g.units.contains(u) && g.unit_get_unit_combat_type(u) == Some(uc) && !g.units[u].has_promotion[pr] {
                promo += 4 * g.unit_base_combat_str(u).max(1) as i64;
            }
        }
        promo += num_cities * 20;
    }
    for (class, pr) in ev.unit_class_promotion.iter_enumerated() {
        let Some(pr) = *pr else { continue };
        for &u in &g.players[p].units {
            if g.units.contains(u) && d.units[g.units[u].unit_type].unit_class_type == Some(class) && !g.units[u].has_promotion[pr] {
                promo += 4 * g.unit_base_combat_str(u).max(1) as i64;
            }
        }
    }
    v += promo * speed_pct(g) / 100;

    per_turn += 2 * ev.free_unit_support as i64;
    if ev.inflation_modifier != 0 {
        let costs = g.player_calculate_pre_inflated_costs(p) as i64;
        v -= 2 * perm * ev.inflation_modifier as i64 * costs / 100;
    }

    // FfH's Armageddon counter: the good fear it, the evil feed it.
    if ev.global_counter != 0 {
        let good = d.ty::<AlignmentType>("ALIGNMENT_GOOD");
        let evil = d.ty::<AlignmentType>("ALIGNMENT_EVIL");
        let a = g.players[p].alignment;
        let sign = if a.is_some() && a == good { -1 } else if a.is_some() && a == evil { 1 } else { 0 };
        v += sign * 15 * ev.global_counter as i64;
    }

    if let Some(r) = t.religion {
        let mut rv = 15;
        if g.player_get_state_religion(p) == Some(r) {
            rv += 15;
        }
        if g.player_has_holy_city(p, r) {
            rv += 15;
        }
        v += ev.convert_own_cities as i64 * rv * speed_pct(g) / 100;
        v += ev.convert_other_cities as i64 * rv / 3;
    }

    if let Some(o) = t.other_player {
        let liked = crate::diplomacy::attitude(g, p, o) >= AttitudeType::ATTITUDE_CAUTIOUS;
        let sign = if liked { 1 } else { -1 };
        v += sign * 30 * ev.attitude_modifier as i64;
        if let Some(b) = ev.bonus_gift {
            v -= g.player_ai_bonus_val(p, b, -1) as i64 * 5;
        }
    }

    v += per_turn * perm;
    v += python_value(g, p, ev);
    if let Some(tr) = adaptive_trait(d, e) {
        v += trait_value(g, p, tr) * TRAIT_TURNS * speed_pct(g) / 100;
    }

    // Follow-up and cancelled events (C++ recurses once per level; a self-reference counts the
    // value so far twice).
    if depth < 2 {
        let own = v;
        for (other, &chance) in ev.additional_event_chance.iter_enumerated() {
            if chance > 0 {
                v += if other == e { chance as i64 * own / 50 } else { chance as i64 * value(g, p, t, other, depth + 1) / 100 };
            }
        }
        for (other, &chance) in ev.clear_event_chance.iter_enumerated() {
            if chance > 0 {
                v -= if other == e { chance as i64 * own / 50 } else { chance as i64 * value(g, p, t, other, depth + 1) / 100 };
            }
        }
    }
    v
}

/// The trait an Adaptive leader's event picks (`EVENT_TRAIT_X` runs Python `doTraitX`, which
/// swaps the selectable trait for `TRAIT_X`).
fn adaptive_trait(d: &GameData, e: EventType) -> Option<TraitType> {
    let ev = &d.events[e];
    if !ev.python_callback.starts_with("doTrait") {
        return None;
    }
    let name = ev.base.type_.strip_prefix("EVENT_")?;
    d.ty::<TraitType>(name).filter(|&tr| d.traits[tr].selectable)
}

/// Gold per turn a trait is worth to player `p` now, from the trait's data: extra yield on plots
/// at the threshold (Financial), civic upkeep (Organized), commerce per city (Creative), health
/// and happiness per city (Expansive, Charismatic), great people (Philosophical), wonders
/// (Industrious), free promotions (Aggressive, Raiders, Arcane, Spiritual) and no anarchy
/// (Spiritual). A trait the leader has for good adds nothing.
pub fn trait_value(g: &Game, p: PlayerId, tr: TraitType) -> i64 {
    let d = g.d;
    let info = &d.traits[tr];
    let pl = &g.players[p];
    if pl.traits[tr] && pl.personality_type.and_then(|l| d.leader_heads[l].permanent_trait) == Some(tr) {
        return 0;
    }
    let cities: Vec<CityId> = pl.cities.iter().copied().filter(|&c| g.cities.contains(c)).collect();
    let n = cities.len() as i64;
    let mut v: i64 = 0;
    for y in 0..YieldType::NUM {
        let th = info.extra_yield_threshold[y];
        if th <= 0 {
            continue;
        }
        let mut count = 0i64;
        for &c in &cities {
            for k in 0..NUM_CITY_PLOTS {
                let Some(q) = g.city_index_plot(c, k) else { continue };
                if g.city_is_working_plot(c, q) && g.map.plots[q].yields[y] >= th {
                    count += 1;
                }
            }
        }
        v += count * YIELD_W[y];
    }
    if info.upkeep_modifier != 0 {
        v -= g.player_get_civic_upkeep(p, None, true) as i64 * info.upkeep_modifier as i64 / 100;
    }
    for k in 0..CommerceType::NUM {
        v += n * info.commerce_change[k] as i64 * COMMERCE_W[k];
        if info.commerce_modifier[k] != 0 {
            v += g.player_get_commerce_rate(p, CommerceType(k as u16)) as i64 * info.commerce_modifier[k] as i64 / 100;
        }
    }
    for &c in &cities {
        v += info.health as i64 * health_w(g, c) + info.happiness as i64 * happy_w(g, c);
    }
    if info.great_people_rate_modifier != 0 {
        let gpp: i64 = cities.iter().map(|&c| g.city_great_people_rate(c) as i64).sum();
        v += 2 * gpp * info.great_people_rate_modifier as i64 / 100;
    }
    let wonders = info.max_global_building_production_modifier.max(info.max_player_building_production_modifier) as i64;
    if wonders > 0 {
        v += g.player_calculate_total_yield(p, YieldType::YIELD_PRODUCTION) as i64 * wonders / 1000;
    }
    if info.free_promotion.iter().any(|&b| b) {
        let mut units = 0i64;
        for &u in &pl.units {
            if g.units.contains(u) && g.unit_get_unit_combat_type(u).is_some_and(|uc| info.free_promotion_unit_combat[uc]) {
                units += 1;
            }
        }
        v += (units + n) / 3;
    }
    if info.level_experience_modifier < 0 {
        v += pl.units.len() as i64 * -info.level_experience_modifier as i64 / 250;
    }
    if info.max_anarchy == 0 {
        let out = g.player_calculate_total_yield(p, YieldType::YIELD_COMMERCE) + g.player_calculate_total_yield(p, YieldType::YIELD_PRODUCTION);
        v += out as i64 / 40;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triggered(g: &Game, p: PlayerId, trigger: &str, unit: Option<UnitId>) -> EventTriggeredData {
        let (plot_x, plot_y) = unit.map_or((-1, -1), |u| (g.units[u].x, g.units[u].y));
        EventTriggeredData {
            id: TriggeredId(0),
            trigger: g.d.t::<EventTriggerType>(trigger),
            turn: g.game_turn(),
            player: p,
            city: None,
            plot_x,
            plot_y,
            unit,
            other_player: None,
            other_player_city: None,
            religion: None,
            corporation: None,
            building: None,
            text: String::new(),
            global_text: String::new(),
        }
    }

    // The first option is not the default answer: FfH lists the costly one first in these.
    #[test]
    fn keeps_the_unit_and_takes_the_great_artist() {
        let d = ffh_data::global();
        let mut g = Game::new(d, ffh_core::setup::GameSetup::standard(d, 6, "WORLDSIZE_SMALL", 5));
        g.map_script = std::sync::Arc::new(ffh_core::mapscript::FractalMapScript::default());
        g.game_init();
        let p = g.player_ids().find(|&p| g.players[p].alive && !g.player_is_barbarian(p)).unwrap();
        let u = g.players[p].units.iter().copied().find(|&u| d.units[g.units[u].unit_type].combat > 0).expect("a starting fighter");
        let ev = |n: &str| d.t::<EventType>(n);

        let t = triggered(&g, p, "EVENTTRIGGER_WAR_GAMES", Some(u));
        let opts = [ev("EVENT_WAR_GAMES_1"), ev("EVENT_WAR_GAMES_2")];
        assert_eq!(choose(&g, p, &t, &opts), Some(ev("EVENT_WAR_GAMES_2")), "lose an experience point, not the unit");

        let t = triggered(&g, p, "EVENTTRIGGER_CLAIRONE", None);
        let opts = [ev("EVENT_CLAIRONE_1"), ev("EVENT_CLAIRONE_2")];
        assert_eq!(choose(&g, p, &t, &opts), Some(ev("EVENT_CLAIRONE_2")), "a free Great Artist is worth some anger");
    }

    #[test]
    fn adaptive_events_name_selectable_traits() {
        let d = ffh_data::global();
        let fin = d.t::<EventType>("EVENT_TRAIT_FINANCIAL");
        assert_eq!(adaptive_trait(d, fin), Some(d.t::<TraitType>("TRAIT_FINANCIAL")));
        let war_games = d.t::<EventType>("EVENT_WAR_GAMES_1");
        assert_eq!(adaptive_trait(d, war_games), None);
    }
}
