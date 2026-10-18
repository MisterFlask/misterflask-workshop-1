//! Malakim. Religious Fervor (`SPELL_RELIGIOUS_FERVOR`, world spell; for the AI only once 5 cities
//! have the state religion) puts a tier-3 priest of the state religion in every city, each with as
//! many experience points as there are cities with the religion. The end-of-turn spell choice cast
//! it the first turn the gate opened, often at peace, where the priests sit in the most threatened
//! city and cost unit support. The AI holds it for a war (at war with a civilization or preparing
//! one), when the priests follow the army; at peace it casts it once twice the gate's cities have
//! the religion (more and stronger priests).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::{world_spell, CivPlay};
use crate::view::View;
use crate::Ai;

pub struct Malakim;

const SPELL: &str = "SPELL_RELIGIOUS_FERVOR";

impl CivPlay for Malakim {
    fn spell_value(&self, g: &Game, _u: UnitId, spell: SpellType, value: i32) -> i32 {
        if world_spell::is(g, spell, SPELL) {
            return 0;
        }
        value
    }

    fn turn(&self, _ai: &Ai, g: &mut Game, v: &mut View) {
        let p = v.p;
        if g.players[p].feat_accomplished[FeatType::FEAT_GLOBAL_SPELL] {
            return;
        }
        let Some(r) = g.player_get_state_religion(p) else { return };
        let religious = g.players[p].cities.iter().filter(|&&c| g.cities[c].has_religion[r]).count();
        if world_spell::war_or_preparing(g, p) || religious >= 10 {
            world_spell::cast(g, p, SPELL);
        }
    }
}
