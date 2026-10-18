//! Kuriotates (`CvCity::init`, FfH): past the city limit (3 on a standard map) a new city is a
//! settlement (radius 2, -75% yields, nothing to build, but no maintenance or civic upkeep, and it
//! claims land and resources). [`Kuriotates::production`] builds one settler at a time for the
//! sites `sites.rs` already scores for them (C++ `AI_foundValue` switches a sprawling player at
//! its limit to settlement-sized sites and favours sites away from our cities).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::{best_unit, count_ai, Choice, Role};
use crate::Ai;

pub struct Kuriotates;

impl CivPlay for Kuriotates {
    fn production(&self, ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        settlement_settler(ai, g, c, threat)
    }
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
