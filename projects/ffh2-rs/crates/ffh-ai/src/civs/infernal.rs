//! Infernal (joins mid-game when the first team learns Infernal Pact). Fallow (`bIgnoreFood`):
//! their cities produce no food and never grow, so population comes only from a new city's
//! starting 3 and from Manes (`SPELL_ADD_TO_CITY`: a Manes in its own city sacrifices itself for
//! +1 population; Demonic Citizens leave no unhappiness or unhealthiness). FfH's spell value
//! (50 for the population against the caster's worth) is negative, so the starting Manes and the
//! ones gifted by deaths stayed weak (2+1) army units all game. The AI now walks lone Manes to the
//! nearest city and adds them there, keeping one other defender in the city.
//!
//! Measured over the 5 games (of 80 full games, Malakim and Svartalfar forced, seeds 1-40) where
//! the Infernals appear on both sides: population at the end 15.8 -> 53.4 (t 2.44), cities 2.4 ->
//! 4.4, world share 785 -> 1316 (t 1.93), Manes left at the end 10 -> 0.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::view::View;
use crate::Ai;

pub struct Infernal;

const SPELL: &str = "SPELL_ADD_TO_CITY";

fn is_manes(g: &Game, u: UnitId) -> bool {
    g.d.ty::<UnitClassType>("UNITCLASS_MANES").is_some_and(|mc| g.d.units[g.units[u].unit_type].unit_class_type == Some(mc))
}

impl CivPlay for Infernal {
    fn turn(&self, _ai: &Ai, g: &mut Game, v: &mut View) {
        let p = v.p;
        let Some(spell) = g.d.ty::<SpellType>(SPELL) else { return };
        for u in g.players[p].units.clone() {
            if !g.units.contains(u) || !is_manes(g, u) || !g.unit_can_cast(u, spell, false) {
                continue;
            }
            let here = g.unit_plot(u);
            let defenders = g.map.plots[here].units.iter().filter(|&&o| o != u && g.units[o].owner == p && g.unit_can_defend(o, Some(here))).count();
            if defenders == 0 {
                continue;
            }
            crate::trace!(g, p, "Manes added to city at {:?}", g.map.xy(here));
            g.unit_cast(u, spell);
        }
    }

    fn unit(&self, _ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, head: UnitId) -> bool {
        if !is_manes(g, head) || g.groups[grp].units.len() != 1 {
            return false;
        }
        let here = g.unit_plot(head);
        if g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == v.p) {
            return false;
        }
        let Some(c) = crate::units::nearest_city(g, v.p, here) else { return false };
        crate::units::move_to(g, grp, g.city_plot(c), 0);
        if crate::units::alive(g, grp) && g.group_ready_to_move(grp, false) {
            crate::units::skip(g, grp);
        }
        true
    }
}
