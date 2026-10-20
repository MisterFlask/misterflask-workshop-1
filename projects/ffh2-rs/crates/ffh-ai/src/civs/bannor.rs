//! Bannor: Donal recruits. His Recruit (once: population / 3 + 1 units in his city) is valued by
//! `magic::recruit_value` (FfH's terms never cast it), and [`Bannor::turn`] casts it before our
//! units move: a unit trained at the end of our turn first moves on the next, and the army had
//! taken 3 of 9 Donals out of their city before the end-of-turn cast.
//!
//! Crusade (civic, needs a war with a civ; +25% unit production, Demagogs, Rally; but no
//! settlers, workers or diplomacy with enemies, and lost at peace) is adopted by the generic AI
//! in any war, through its `iAIWeight` of 1000 a city. Holding it back to wars we are clearly
//! winning on power ([`super::CivPlay::civic_value`]: ours at least 1.2x the enemies' sum, kept
//! once adopted) halved how often it was taken (15 -> 7 of 40 full games) and changed nothing
//! measurable (share@end +10, t 0.16; see docs/AI_LOG.md), so it was not kept.

use ffh_core::game::Game;
use ffh_data::*;

use super::CivPlay;
use crate::view::View;
use crate::Ai;

pub struct Bannor;

impl CivPlay for Bannor {
    fn turn(&self, _ai: &Ai, g: &mut Game, v: &mut View) {
        let Some(recruit) = g.d.ty::<SpellType>("SPELL_RECRUIT") else { return };
        for u in g.players[v.p].units.clone() {
            if g.units.contains(u) && g.unit_can_cast(u, recruit, false) {
                let value = crate::magic::recruit_value(g, u);
                if value > 0 {
                    crate::magic::cast(g, u, recruit, "civ", value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ffh_core::ids::*;

    use super::*;
    use crate::campaign::tests::one_landmass_game;

    #[test]
    fn donal_recruits_before_he_moves() {
        let d = ffh_data::global();
        let mut g = one_landmass_game(17);
        let us = PlayerId(0);
        g.player_found(us, 4, 5);
        let city = g.players[us].cities[0];
        g.city_set_population(city, 6);
        let donal = g.player_init_unit(us, d.t::<UnitType>("UNIT_DONAL"), 4, 5, Some(UnitAIType::UNITAI_HERO), None);
        let before = g.players[us].units.len();
        let mut v = View::new(&g, us);
        Bannor.turn(&Ai::default(), &mut g, &mut v);
        assert_eq!(g.players[us].units.len(), before + 3, "population 6: 6 / 3 + 1 recruits");
        assert!(g.units[donal].has_casted && g.unit_moves_left(donal) > 0, "recruited, free to march");
    }
}
