//! Infernal (joins mid-game when the first team learns Infernal Pact). Fallow (`bIgnoreFood`):
//! their cities produce no food and never grow, so population comes only from a new city's
//! starting 3 and from Manes (`SPELL_ADD_TO_CITY`: a Manes in its own city sacrifices itself for
//! +1 population; Demonic Citizens leave no unhappiness or unhealthiness). FfH's spell value (50
//! for the population against the caster's worth) is negative, so the starting Manes and the ones
//! gifted by deaths stayed weak (2+1) army units all game. The AI walks lone Manes to the nearest
//! city and adds there those the city's garrison does not need (`units::garrison_quota`, which
//! rises with the danger around the city): the owner's rule, civilizations use their unique
//! mechanics (CLAUDE.md "AI changes").
//!
//! First tried in round 5 (commit f20b509) keeping only one other defender per city: fresh seeds
//! 5001-5064 (the 60 games where the Infernals appear) population +5.0 (t 2.23) but units -6.2
//! (t -2.77), alive 0.8 -> 0.7 (t -1.66), share -29 (t -0.68) -- the Manes were most of their army
//! (docs/AI_LOG.md); back since 2026-10-20 with the garrison quota.

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
            let Some(c) = g.map.plots[here].plot_city.filter(|&c| g.cities[c].owner == p) else { continue };
            let others = g.map.plots[here].units.iter().filter(|&&o| o != u && g.units[o].owner == p && g.unit_can_defend(o, Some(here))).count() as i32;
            if others < crate::units::garrison_quota(g, v, c) {
                continue;
            }
            crate::trace!(g, p, "Manes added to city at {:?}", g.map.xy(here));
            crate::magic::cast(g, u, spell, "civ", 0);
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
