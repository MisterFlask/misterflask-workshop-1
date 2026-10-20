//! Balseraphs.
//!
//! - **Loki** (hero, enters any territory) can cast **Disrupt** in a city of another team: 0-2 of
//!   the owner's culture there is lost, and a city left with none becomes ours, without war (its
//!   owner's attitude drops by 4). The generic AI never took him into a foreign city.
//!   [`Balseraphs::unit`] sends him, at peace, to the nearest reachable city of a civ we are not at
//!   war with whose owner culture is low and grows by nothing (Disrupt drains one a turn on
//!   average, so a city producing culture never falls), and Disrupt is cast there. Otherwise he
//!   moves as a caster (`units::role_of`: he defends only): as an army unit he walked alone, and
//!   30 of 42 Lokis were killed.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::units::{alive, hold, move_to};
use crate::view::View;
use crate::Ai;

pub struct Balseraphs;

impl CivPlay for Balseraphs {
    fn unit(&self, _ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, head: UnitId) -> bool {
        if g.d.units[g.units[head].unit_type].base.type_ != "UNIT_LOKI" {
            return false;
        }
        loki(g, v, grp, head)
    }
}

/// A rival civ's city Loki can take with Disrupt: not ours, not at war with us, its owner's
/// culture there below what 20 turns of Disrupt drain (one a turn on average) and not growing.
fn disrupt_target(g: &Game, team: TeamId, c: CityId) -> bool {
    let owner = g.cities[c].owner;
    let t = g.players[owner].team;
    t != team
        && !g.player_is_barbarian(owner)
        && !g.at_war(team, t)
        && g.city_culture(c, owner) < 20
        && g.city_commerce_rate(c, CommerceType::COMMERCE_CULTURE) == 0
}

/// Loki: Disrupt in a target city; else to the nearest target at peace (see the module doc).
fn loki(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
    let p = v.p;
    let Some(spell) = g.d.ty::<SpellType>("SPELL_DISRUPT") else { return false };
    let here = g.unit_plot(u);
    if let Some(c) = g.map.plots[here].plot_city.filter(|&c| disrupt_target(g, v.team, c)) {
        if g.unit_can_cast(u, spell, false) {
            crate::trace!(g, p, "loki: disrupt {} (culture {})", g.cities[c].name, g.city_culture(c, g.cities[c].owner));
            g.unit_cast(u, spell);
            // A city taken is a new city (the old id is gone).
            crate::trace!(g, p, "loki: {}", if g.cities.contains(c) { "not yet" } else { "city taken" });
        }
        if alive(g, grp) && g.group_ready_to_move(grp, true) {
            hold(g, grp);
        }
        return true;
    }
    if !v.civ_enemies.is_empty() || g.units[u].damage > 20 {
        return false;
    }
    let reach = v.flood.run(g, u, 12, false);
    let target = reach.iter().find(|r| g.map.plots[r.plot].plot_city.is_some_and(|c| g.map.plots[r.plot].revealed[v.team] && disrupt_target(g, v.team, c)));
    let Some(r) = target.copied() else { return false };
    crate::trace!(g, p, "loki at {:?}: to {:?} ({} turns)", g.map.xy(here), g.map.xy(r.plot), r.turns);
    move_to(g, grp, r.plot, 0);
    if alive(g, grp) && g.units.contains(u) && g.unit_plot(u) == r.plot && g.unit_can_cast(u, spell, false) {
        g.unit_cast(u, spell);
    }
    if alive(g, grp) && g.group_ready_to_move(grp, true) {
        hold(g, grp);
    }
    true
}
