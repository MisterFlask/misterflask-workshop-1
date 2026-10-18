//! Luchuirp: Gifts of Nantosuelta leaves a Golden Hammer in each city, which can become a free
//! Engineer there (`SPELL_GIVE_HAMMER_TO_CRAFTSMAN`) or +1 strength on a unit that takes it
//! (`SPELL_TAKE_GOLDEN_HAMMER_UNIT`). FfH's end-of-turn spell choice valued the take higher, so
//! the garrison warriors took nearly every hammer; units in our own cities now leave it, and the
//! hammer gives itself.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;

pub struct Luchuirp;

impl CivPlay for Luchuirp {
    fn spell_value(&self, g: &Game, u: UnitId, spell: SpellType, value: i32) -> i32 {
        let here = g.unit_plot(u);
        let in_own_city = g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == g.units[u].owner);
        if in_own_city && g.d.ty::<SpellType>("SPELL_TAKE_GOLDEN_HAMMER_UNIT") == Some(spell) {
            return 0;
        }
        value
    }
}
