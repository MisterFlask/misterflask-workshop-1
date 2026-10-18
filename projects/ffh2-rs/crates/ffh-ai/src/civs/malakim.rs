//! Malakim. The Citadel of Light (`BUILDING_CITADEL_OF_LIGHT`, Sorcery, 240 hammers) burns, every
//! turn, the plot within 2 of its city holding the most enemy strength at war (20% fire damage,
//! up to 40%; `ev_citadel_of_light`). Its effect lives in Python, so its generic value is the +5%
//! defense only and it was built in no game. The AI values it in a frontier city -- within 6
//! plots of a city of a civilization we are at war with or prepare a war against -- as a
//! mid-value building (about 1 to 2 hammers a turn of enemy damage), in at most a third of its
//! cities.
//!
//! Religious Fervor (world spell) is left to the generic play (cast the first turn the gate
//! opens): holding it for a war measured worse (docs/civ-ai-d.md).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;

pub struct Malakim;

impl CivPlay for Malakim {
    fn building_value(&self, g: &Game, c: CityId, b: BuildingType, value: i32) -> i32 {
        if g.d.ty::<BuildingType>("BUILDING_CITADEL_OF_LIGHT") != Some(b) {
            return value;
        }
        let p = g.cities[c].owner;
        let cities = &g.players[p].cities;
        let have = cities.iter().filter(|&&o| g.cities[o].num_real_building[b] > 0 || g.city_production_building(o) == Some(b)).count();
        if have * 3 >= cities.len() {
            return value;
        }
        if frontier(g, c) {
            value.max(250)
        } else {
            value
        }
    }
}

/// Within 6 plots of a city of a civilization we are at war with or prepare a war against.
fn frontier(g: &Game, c: CityId) -> bool {
    let p = g.cities[c].owner;
    let team = g.players[p].team;
    let barb = g.barbarian_team();
    let preparing = crate::diplomacy::preparing_target(g, team);
    let (x, y) = (g.cities[c].x, g.cities[c].y);
    g.player_ids().into_iter().any(|o| {
        let ot = g.players[o].team;
        ot != team
            && ot != barb
            && g.players[o].alive
            && (g.at_war(team, ot) || preparing == Some(ot))
            && g.players[o].cities.iter().any(|&oc| g.plot_distance(x, y, g.cities[oc].x, g.cities[oc].y) <= 6)
    })
}
