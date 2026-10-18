//! Svartalfar. Their `MaintainFeatures` keeps the forest under every improvement they build on
//! their own land (`CvPlot::setImprovementType` with FfH's check), and workers only build there,
//! so they never clear forest.
//!
//! Veil of Night (`SPELL_VEIL_OF_NIGHT`, world spell; for the AI only with 25+ units and at peace
//! with every civilization) gives every unit with strength Hidden Nationality for good: such a
//! unit cannot capture a city (`isNoCapture`), is hostile to every team and sees every foreign
//! unit as an enemy (`isAlwaysHostile`). FfH's own AI parked its HN units (`HNgroupMove` is a
//! skip). This AI has no HN logic: the end-of-turn spell choice cast it the first turn the gate
//! opened, after which the whole standing army could no longer take a city. The AI does not
//! cast it.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;

pub struct Svartalfar;

impl CivPlay for Svartalfar {
    fn spell_value(&self, g: &Game, _u: UnitId, spell: SpellType, value: i32) -> i32 {
        if g.d.ty::<SpellType>("SPELL_VEIL_OF_NIGHT") == Some(spell) {
            return 0;
        }
        value
    }
}
