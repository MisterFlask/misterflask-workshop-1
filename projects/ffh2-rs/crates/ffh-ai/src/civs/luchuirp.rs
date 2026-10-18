//! Luchuirp.
//!
//! Golems: the Wood Golem (Axeman class, 6 strength for 90 hammers) and the Gargoyle (Longbowman
//! class, 8/8, +25% city defense) need a Sculptor's Studio, a specific building
//! (`PrereqBuilding`), which the generic production never builds (it only builds the buildings
//! of `PrereqBuildingClass` units). [`Luchuirp::production`] builds it where it opens a unit a
//! third stronger than the city's best (the generic test). The Studio is also the Luchuirp
//! Training Yard, which the later golems (Clockwork, Nullstone, Bone) need; the generic logic
//! already sees those.
//!
//! Gifts of Nantosuelta leaves a Golden Hammer in each city, which can become a free Engineer
//! there (`SPELL_GIVE_HAMMER_TO_CRAFTSMAN`) or +1 strength on a unit that takes it
//! (`SPELL_TAKE_GOLDEN_HAMMER_UNIT`). FfH's end-of-turn spell choice valued the take higher, so
//! the garrison warriors took nearly every hammer; units in our own cities now leave it, and the
//! hammer gives itself.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::Choice;
use crate::Ai;

pub struct Luchuirp;

impl CivPlay for Luchuirp {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        super::unit_building(g, c, threat, if std::env::var("FFH_LUC_EXP").is_ok_and(|v| v == "G") { &["UNIT_GARGOYLE"] } else { &["UNIT_WOOD_GOLEM", "UNIT_GARGOYLE"] }).map(Choice::Construct)
    }

    fn spell_value(&self, g: &Game, u: UnitId, spell: SpellType, value: i32) -> i32 {
        let here = g.unit_plot(u);
        let in_own_city = g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == g.units[u].owner);
        if in_own_city && g.d.ty::<SpellType>("SPELL_TAKE_GOLDEN_HAMMER_UNIT") == Some(spell) {
            return 0;
        }
        value
    }
}
