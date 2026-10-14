//! Research choice: value of what a tech unlocks for this player, divided by its remaining
//! cost, with one level of lookahead and the leader's FfH personality hints (favorite early
//! techs and religion, flavors).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::values::values;

pub fn choose(g: &Game, p: PlayerId) -> Option<TechType> {
    let team = g.players[p].team;
    let mut best: Option<TechType> = None;
    let mut best_score = i64::MIN;
    for t in g.d.techs.keys() {
        if g.team_is_has_tech(team, Some(t)) || !g.player_can_research(p, t, false) {
            continue;
        }
        let mut value = tech_value(g, p, t) as i64;
        // Lookahead: a share of what the techs this one leads to are worth.
        for &child in &values(g.d).tech_children[t] {
            if !g.team_is_has_tech(team, Some(child)) {
                value += tech_value(g, p, child) as i64 / 4;
            }
        }
        let left = g.team_get_research_left(team, t).max(1) as i64;
        let score = value * 10_000 / left;
        if score > best_score {
            best_score = score;
            best = Some(t);
        }
    }
    best
}

/// What tech `t` is worth to player `p`, before cost.
pub fn tech_value(g: &Game, p: PlayerId, t: TechType) -> i32 {
    let d = g.d;
    let info = &d.techs[t];
    let vals = values(d);
    let unlocks = &vals.tech_unlocks[t];
    let pl = &g.players[p];
    let civ = g.player_get_civilization_type(p);
    let num_cities = pl.cities.len() as i32;
    let leader = pl.personality_type.map(|l| &d.leader_heads[l]);

    let mut value = 100 + info.ai_weight;

    for &u in &unlocks.units {
        let Some(class) = d.units[u].unit_class_type else { continue };
        if civ.and_then(|c| d.civ_unit(c, class)) != Some(u) {
            continue;
        }
        let power = vals.unit_power[u];
        value += power / 8;
        if d.units[u].found {
            value += 400;
        }
        if !d.units[u].builds.iter().all(|&b| !b) {
            value += 150;
        }
    }
    for &b in &unlocks.buildings {
        let Some(class) = d.buildings[b].building_class_type else { continue };
        if civ.and_then(|c| d.civ_building(c, class)) != Some(b) {
            continue;
        }
        value += vals.building_value[b] * num_cities.clamp(1, 8) / 4;
    }
    value += 120 * unlocks.builds.len() as i32;
    value += 60 * unlocks.civics.len() as i32;
    if let Some(r) = unlocks.religion {
        if g.religion_game_turn_founded[r] == -1 {
            value += 500;
            if let Some(l) = leader {
                if l.favorite_religion == Some(r) || l.favorite_early_religion == Some(r) {
                    value += 2500;
                }
                value += l.religion_weight_modifier[r] * 5;
            }
        }
    }
    if let Some(l) = leader {
        let early = pl.current_era.idx() <= 1;
        if early {
            for fav in [l.favorite_early_tech1, l.favorite_early_tech2, l.favorite_early_tech3, l.favorite_early_mil_tech] {
                if fav == Some(t) {
                    value += 800;
                }
            }
        }
        if l.favorite_tech == Some(t) {
            value += 300;
        }
        let flavor: i32 = l.flavor_value.iter().zip(info.flavor_value.iter()).map(|(a, b)| a * b).sum();
        value += flavor * 4;
    }
    // Economy techs (trade routes, worker speed, health/happiness).
    value += info.trade_routes * 150 + info.happiness * 120 * num_cities.max(1) / 2 + info.health * 80 * num_cities.max(1) / 2;
    value += info.worker_speed_modifier * 3;
    value.max(1)
}
