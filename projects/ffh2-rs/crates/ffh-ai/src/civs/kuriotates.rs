//! Kuriotates (`CvCity::init`, FfH): the Sprawling trait limits them to 3 full cities on a
//! standard map (radius 3, as for every AI); past the limit a new city is a *settlement* (radius
//! 2, -75% yields, nothing to build, but no maintenance or civic upkeep, and it claims land and
//! resources). The ported C++ `AI_foundValue` already scores settlement-sized sites for a
//! sprawling player at its limit and favours sites away from our cities. What the AI does:
//! - [`Kuriotates::production`] builds one settler at a time for a settlement site on the city's
//!   landmass, from a full city of size 5+ (or at its happy cap) at peace;
//! - [`Kuriotates::site_value`]: at the limit, no settlement site within 5 plots of a full city.
//!   A plot is worked by the city nearest to it (`CvPlot::updateWorkingCity`), so a settlement
//!   inside a full city's radius-3 ring takes plots the full city could work and leaves them
//!   nearly idle (a size-1 settlement works one plot at a quarter of its yield): measured, full
//!   cities stayed smaller (population at turn 300 37 against 41 without settlements, commerce
//!   117 against 159);
//! - settlements cannot train units: the nearest full city trains a defender for a settlement that
//!   has none (the garrison logic walks it there);
//! - after losing a full city, the next city founded is a full one, but the generic settler step
//!   counts settlements towards the limit and stops: a settler for a full city then.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::{best_unit, count_ai, Choice, Role};
use crate::Ai;

pub struct Kuriotates;

impl CivPlay for Kuriotates {
    fn production(&self, ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        settlement_defender(g, c).or_else(|| full_city_settler(ai, g, c, threat)).or_else(|| settlement_settler(ai, g, c, threat))
    }

    fn site_value(&self, g: &Game, p: PlayerId, q: PlotIdx, value: i32) -> i32 {
        let pl = &g.players[p];
        let full = pl.cities.len() as i32 - g.player_get_num_settlements(p);
        if pl.max_cities <= 0 || full < pl.max_cities {
            return value;
        }
        // A full city's radius (3) plus a settlement's (2): closer, they share plots.
        let (x, y) = g.map.xy(q);
        if pl.cities.iter().any(|&c| !g.cities[c].settlement && g.plot_distance(x, y, g.cities[c].x, g.cities[c].y) <= 5) {
            return 0;
        }
        value
    }
}

/// Below the limit of full cities while settlements make up the rest (a full city was lost): the
/// next city founded is a full one (`CvCity::init` counts full cities), but the generic settler
/// step counts settlements too and stops. One settler at a time from a city of size 4+ (or at
/// its happy cap) with a site on its landmass.
fn full_city_settler(ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
    let p = g.cities[c].owner;
    let pl = &g.players[p];
    if pl.max_cities <= 0 || threat > 0 || g.cities[c].settlement || g.city_is_occupation(c) {
        return None;
    }
    let settlements = g.player_get_num_settlements(p);
    let full = pl.cities.len() as i32 - settlements;
    if settlements == 0 || full >= pl.max_cities || count_ai(g, p, UnitAIType::UNITAI_SETTLE) > 0 {
        return None;
    }
    if crate::production::financial_trouble(g, p) && crate::economy::spend_pct(g, p) < 20 {
        return None;
    }
    let happy_room = g.city_happy_level(c) - g.city_unhappy_level(c, 0);
    if g.cities[c].population < 4 && happy_room > 0 {
        return None;
    }
    let area = g.map.plots[g.city_plot(c)].area;
    if !crate::sites::sites(ai, g, p).iter().any(|&(q, _)| g.map.plots[q].area == area) {
        return None;
    }
    let u = best_unit(g, c, Role::Settler)?;
    if g.city_production_turns_left_unit(c, u, 0) > 25 {
        return None;
    }
    crate::trace!(g, p, "{}: settler for a full city ({} full of {})", g.cities[c].name, full, pl.max_cities);
    Some(Choice::Train(u, UnitAIType::UNITAI_SETTLE))
}

/// Kuriotates at their city limit: one settler at a time for a settlement site on this
/// city's landmass, from a city of size 5+ (or at its happy cap) at peace.
fn settlement_settler(ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
    let p = g.cities[c].owner;
    let pl = &g.players[p];
    if pl.max_cities <= 0 || threat > 0 || g.cities[c].settlement || g.city_is_occupation(c) {
        return None;
    }
    let full = pl.cities.len() as i32 - g.player_get_num_settlements(p);
    if full < pl.max_cities || count_ai(g, p, UnitAIType::UNITAI_SETTLE) > 0 {
        return None;
    }
    if crate::production::financial_trouble(g, p) {
        return None;
    }
    let team = pl.team;
    let barb = g.barbarian_team();
    if g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t)) {
        return None;
    }
    let happy_room = g.city_happy_level(c) - g.city_unhappy_level(c, 0);
    if g.cities[c].population < 5 && happy_room > 0 {
        return None;
    }
    let area = g.map.plots[g.city_plot(c)].area;
    if !crate::sites::sites(ai, g, p).iter().any(|&(q, _)| g.map.plots[q].area == area) {
        return None;
    }
    let u = best_unit(g, c, Role::Settler)?;
    if g.city_production_turns_left_unit(c, u, 0) > 20 {
        return None;
    }
    Some(Choice::Train(u, UnitAIType::UNITAI_SETTLE))
}


fn civ_war(g: &Game, p: PlayerId) -> bool {
    let team = g.players[p].team;
    let barb = g.barbarian_team();
    g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t))
}

/// Settlements cannot train: the nearest full city trains a defender for one that has none
/// (the garrison logic walks it there), unless one is already on its way.
fn settlement_defender(g: &Game, c: CityId) -> Option<Choice> {
    let p = g.cities[c].owner;
    let pl = &g.players[p];
    if pl.max_cities <= 0 || g.cities[c].settlement {
        return None;
    }
    let area = g.map.plots[g.city_plot(c)].area;
    let empty: Vec<CityId> = pl.cities.iter().copied().filter(|&s| g.cities[s].settlement && g.map.plots[g.city_plot(s)].area == area && crate::production::garrison(g, p, g.city_plot(s)).0 == 0).collect();
    if empty.is_empty() {
        return None;
    }
    let moving = pl.units.iter().filter(|&&u| g.units[u].ai_type == Some(UnitAIType::UNITAI_CITY_DEFENSE) && g.map.plots[g.unit_plot(u)].plot_city.is_none()).count() as i32
        + pl.ai_num_train_ai_units[UnitAIType::UNITAI_CITY_DEFENSE];
    if moving >= empty.len() as i32 {
        return None;
    }
    let dist = |a: CityId, b: CityId| g.plot_distance(g.cities[a].x, g.cities[a].y, g.cities[b].x, g.cities[b].y);
    let nearest_full = |s: CityId| pl.cities.iter().copied().filter(|&o| !g.cities[o].settlement).min_by_key(|&o| (dist(o, s), o));
    if !empty.iter().any(|&s| nearest_full(s) == Some(c)) || !crate::economy::can_afford_unit(g, p, true, civ_war(g, p)) {
        return None;
    }
    best_unit(g, c, Role::Defense).map(|u| Choice::Train(u, UnitAIType::UNITAI_CITY_DEFENSE))
}
