//! Lanun (`SPELL_PIRATE_COVE`, C++ `CvUnitAI::AI_buildPirateCove`): a work boat turns an empty
//! coast plot of ours into a Pirate Cove (+2 commerce, growing into a Pirate Harbor and a Pirate
//! Port, 3/2/6 on coast). [`Lanun::production`] trains a work boat for a city with a valid cove
//! plot it works; [`Lanun::sea_worker`] takes it there and casts.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::{best_unit, count_ai, Choice, Role};
use crate::units::{alive, move_to, skip};
use crate::view::View;
use crate::Ai;

pub struct Lanun;

impl CivPlay for Lanun {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        cove_boat(g, c, threat)
    }

    fn sea_worker(&self, g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
        sea_worker(g, v, grp, u)
    }
}


/// Lanun: a work boat for a city that works a plot where a Pirate Cove can go (C++
/// `AI_neededSeaWorkers` adds one), one boat at a time.
fn cove_boat(g: &Game, c: CityId, threat: i32) -> Option<Choice> {
    let p = g.cities[c].owner;
    if threat > 0 || g.cities[c].population < 3 || count_ai(g, p, UnitAIType::UNITAI_WORKER_SEA) > 0 {
        return None;
    }
    g.d.ty::<SpellType>("SPELL_PIRATE_COVE")?;
    let has_site = (0..ffh_core::defs::NUM_CITY_PLOTS).any(|i| g.city_index_plot(c, i).is_some_and(|q| g.map.plots[q].working_city == Some(c) && cove_site(g, q, p)));
    if !has_site {
        return None;
    }
    best_unit(g, c, Role::SeaWorker).map(|u| Choice::Train(u, UnitAIType::UNITAI_WORKER_SEA))
}


/// C++ `CvPlot::isPirateCoveValid` (water next to land, ours, empty, no cove within 2 plots)
/// and a plot the cove improvement can go on (coast).
fn cove_site(g: &Game, q: PlotIdx, p: PlayerId) -> bool {
    g.plot_is_pirate_cove_valid(q, p) && g.d.ty::<ImprovementType>("IMPROVEMENT_PIRATE_COVE").is_some_and(|i| g.plot_can_have_improvement(q, i, None, false))
}

/// A Lanun work boat with no sea resource left to improve goes to the best plot for a Pirate
/// Cove (worked plots of big cities first) and casts there. True if it was given orders.
fn sea_worker(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
    let p = v.p;
    let Some(spell) = g.d.ty::<SpellType>("SPELL_PIRATE_COVE") else { return false };
    let here = g.unit_plot(u);
    if cove_site(g, here, p) && g.unit_can_cast(u, spell, false) {
        crate::trace!(g, p, "work boat {:?}: pirate cove at {:?}", u, g.map.xy(here));
        g.unit_cast(u, spell);
        if alive(g, grp) && g.group_ready_to_move(grp, true) {
            skip(g, grp);
        }
        return true;
    }
    let reach = v.flood.run(g, u, 8, false);
    // Resources in reach first (the generic sea worker improves those our cities can work).
    let ut = g.units[u].unit_type;
    let resource = |q: PlotIdx| {
        let pl = &g.map.plots[q];
        pl.owner == Some(p)
            && pl.working_city.is_some_and(|c| g.cities[c].owner == p)
            && v.danger_at(q) == 0
            && pl.improvement_type.is_none()
            && pl.bonus_type.is_some()
            && g.d.builds.keys().any(|b| g.d.units[ut].builds[b] && g.d.builds[b].improvement.is_some() && g.unit_can_build(u, q, b, false))
    };
    if let Some(r) = reach.iter().find(|r| resource(r.plot)) {
        crate::trace!(g, p, "work boat {:?}: resource at {:?} first", u, g.map.xy(r.plot));
        return false;
    }
    let mut best: Option<(PlotIdx, i64)> = None;
    for r in &reach {
        let q = r.plot;
        if v.danger_at(q) > 0 || v.is_claimed(q) || !cove_site(g, q, p) {
            continue;
        }
        let Some(c) = g.map.plots[q].working_city.filter(|&c| g.cities[c].owner == p) else { continue };
        let value = 10 + g.cities[c].population + if g.plot_is_being_worked(q) { 10 } else { 0 };
        let score = value as i64 * 100 / (r.turns as i64 + 2);
        if best.map_or(true, |(bq, s)| score > s || score == s && q < bq) {
            best = Some((q, score));
        }
    }
    let Some((plot, _)) = best else { return false };
    v.claimed.push(plot);
    crate::trace!(g, p, "work boat {:?} at {:?}: to pirate cove site {:?}", u, g.map.xy(here), g.map.xy(plot));
    move_to(g, grp, plot, 0);
    if !alive(g, grp) || !g.units.contains(u) {
        return true;
    }
    if g.unit_plot(u) == plot && g.unit_can_cast(u, spell, false) {
        g.unit_cast(u, spell);
    }
    if alive(g, grp) && g.group_ready_to_move(grp, true) {
        skip(g, grp);
    }
    true
}
