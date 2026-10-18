//! Elohim.
//!
//! **Sanctuary** (world spell, `spellSanctuary`): for 30 turns no unit of another team can enter
//! our land, and every foreign unit in it is thrown out. The AI-only gate of the Python allows it
//! at war with two civs or more; cast at once it fell on the turn the second war began, whether
//! or not an enemy was near. Held until it pays: cast when an enemy civ unit stands in our land
//! within 3 plots of one of our cities, when one of our cities is in peril, or after 15 turns of
//! two wars.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::{held, CivPlay};
use crate::view::View;
use crate::Ai;

pub struct Elohim;

impl CivPlay for Elohim {
    fn turn(&self, _ai: &Ai, g: &mut Game, v: &mut View) {
        sanctuary(g, v);
    }

    fn spell_value(&self, g: &Game, _u: UnitId, spell: SpellType, value: i32) -> i32 {
        if Some(spell) == held::spell(g, "SPELL_SANCTUARY") {
            return 0;
        }
        value
    }
}

fn sanctuary(g: &mut Game, v: &mut View) {
    let p = v.p;
    let Some(spell) = held::spell(g, "SPELL_SANCTUARY") else { return };
    if held::caster(g, p, spell).is_none() {
        return;
    }
    // (The gate: two civ wars; the second is the younger.)
    let mut wars: Vec<i32> = v.civ_enemies.iter().map(|&e| g.teams[v.team].ai_at_war_counter[e]).collect();
    wars.sort();
    let two_wars_for = if wars.len() >= 2 { wars[wars.len() - 2] } else { 0 };
    let why = if enemy_in_land(g, v) {
        "enemies in our land"
    } else if held::civ_peril(v) {
        "a city in peril"
    } else if two_wars_for >= held::speed_turns(g, 15) {
        "fallback"
    } else {
        return;
    };
    held::cast(g, p, spell, why);
}

/// A visible unit of a civ we are at war with on a plot of ours within 3 plots of our cities.
fn enemy_in_land(g: &Game, v: &View) -> bool {
    let p = v.p;
    g.players[p].cities.iter().any(|&c| {
        let (x, y) = (g.cities[c].x, g.cities[c].y);
        (-3..=3).any(|i| {
            (-3..=3).any(|j| {
                g.plot_xy(x, y, i, j).is_some_and(|q| {
                    g.map.plots[q].owner == Some(p)
                        && g.plot_is_visible(q, v.team)
                        && g.map.plots[q].units.iter().any(|&u| v.civ_enemies.contains(&g.unit_get_team(u)) && !g.unit_is_invisible(u, v.team, false))
                })
            })
        })
    })
}
