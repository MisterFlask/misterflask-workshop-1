//! Civilization-specific play (item `civ-mechanics`): rules some civilizations have that the
//! generic AI does not see. Every branch is guarded by the player's civilization (resolved by
//! its type name on each call), so games without these civilizations play exactly as before.
//!
//! - **Kuriotates** (`CvCity::init`, FfH): past the city limit (3 on a standard map) a new city
//!   is a settlement (radius 2, -75% yields, nothing to build, but no maintenance or civic
//!   upkeep, and it claims land and resources). [`production`] builds one settler at a time
//!   for the sites `sites.rs` already scores for them (C++ `AI_foundValue` switches a
//!   sprawling player at its limit to settlement-sized sites and favours sites away from our
//!   cities).
//! - **Lanun** (`SPELL_PIRATE_COVE`, C++ `CvUnitAI::AI_buildPirateCove`): a work boat turns an
//!   empty coast plot of ours into a Pirate Cove (+2 commerce, growing into a Pirate Harbor and
//!   a Pirate Port, 3/2/6 on coast). [`production`] trains a work boat for a city with a valid
//!   cove plot it works; [`sea_worker`] takes it there and casts.
//! - **Ljosalfar / Svartalfar**: nothing to do here. Their `MaintainFeatures` keeps the forest
//!   under every improvement they build on their own land (`CvPlot::setImprovementType`
//!   with FfH's check), and workers only build there, so they never clear forest.
//! - **Khazad**: nothing either. Saving gold for their Dwarven Vaults (`cf_do_turn_khazad`)
//!   was tried and measured: more population, fewer techs, the same score by turn 300 (see
//!   docs/AI.md).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::production::{best_unit, count_ai, Choice, Role};
use crate::units::{alive, move_to, skip};
use crate::view::View;
use crate::Ai;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Civ {
    Kuriotates,
    Lanun,
    Other,
}

/// The player's civilization as far as civilization-specific play goes: `Civ::Other` for
/// civilizations without any, and for those turned off by `FFH_AI_CIV_OFF` (see [`enabled`]).
/// Every civilization-specific branch of the AI goes through this (or [`enabled`]).
pub fn civ(g: &Game, p: PlayerId) -> Civ {
    if !enabled(g, p) {
        return Civ::Other;
    }
    match g.player_get_civilization_type(p).map(|c| g.d.civilizations[c].base.type_.as_str()) {
        Some("CIVILIZATION_KURIOTATES") => Civ::Kuriotates,
        Some("CIVILIZATION_LANUN") => Civ::Lanun,
        _ => Civ::Other,
    }
}

/// False when the player's civilization-specific play is turned off: `FFH_AI_CIV_OFF` lists
/// civilizations (`CIVILIZATION_KHAZAD` or `KHAZAD`, comma-separated, or `all`) that play as
/// generic civs. A measurement switch (civ X with and without its logic, against the same
/// opponents, in one binary); it is read once and never changes during a game, so play stays
/// deterministic, and the run cache keys runs by every `FFH_*` variable.
pub fn enabled(g: &Game, p: PlayerId) -> bool {
    static OFF: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let off = OFF.get_or_init(|| {
        std::env::var("FFH_AI_CIV_OFF")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().trim_start_matches("CIVILIZATION_").to_ascii_uppercase())
            .filter(|s| !s.is_empty())
            .collect()
    });
    if off.is_empty() {
        return true;
    }
    let Some(c) = g.player_get_civilization_type(p) else { return true };
    let name = g.d.civilizations[c].base.type_.trim_start_matches("CIVILIZATION_");
    !off.iter().any(|o| o == "ALL" || o == name)
}

// ----------------------------------------------------------------------------
// Production: Kuriotates settlements, Lanun cove boats
// ----------------------------------------------------------------------------

/// Civilization-specific production, asked after the settler step of `production::decide`.
pub fn production(ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
    let p = g.cities[c].owner;
    match civ(g, p) {
        Civ::Kuriotates => settlement_settler(ai, g, c, threat),
        Civ::Lanun => cove_boat(g, c, threat),
        _ => None,
    }
}

/// Kuriotates at their city limit: one settler at a time for a settlement site on this
/// city's landmass, from a city of size 5+ (or at its happy cap) at peace.
fn settlement_settler(ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
    let p = g.cities[c].owner;
    let pl = &g.players[p];
    if pl.max_cities <= 0 || threat > 0 || g.cities[c].settlement || g.city_is_occupation(c) {
        return None;
    }
    let full = pl.cities.len() as i32 - g.player_get_num_settlements(p);
    if full < pl.max_cities || count_ai(g, p, UnitAIType::UNITAI_SETTLE) > 0 {
        return None;
    }
    if crate::production::financial_trouble(g, p) {
        return None;
    }
    let team = pl.team;
    let barb = g.barbarian_team();
    if g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t)) {
        return None;
    }
    let happy_room = g.city_happy_level(c) - g.city_unhappy_level(c, 0);
    if g.cities[c].population < 5 && happy_room > 0 {
        return None;
    }
    let area = g.map.plots[g.city_plot(c)].area;
    if !crate::sites::sites(ai, g, p).iter().any(|&(q, _)| g.map.plots[q].area == area) {
        return None;
    }
    let u = best_unit(g, c, Role::Settler)?;
    if g.city_production_turns_left_unit(c, u, 0) > 20 {
        return None;
    }
    Some(Choice::Train(u, UnitAIType::UNITAI_SETTLE))
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

// ----------------------------------------------------------------------------
// Lanun work boats: Pirate Coves
// ----------------------------------------------------------------------------

/// C++ `CvPlot::isPirateCoveValid` (water next to land, ours, empty, no cove within 2 plots)
/// and a plot the cove improvement can go on (coast).
fn cove_site(g: &Game, q: PlotIdx, p: PlayerId) -> bool {
    g.plot_is_pirate_cove_valid(q, p) && g.d.ty::<ImprovementType>("IMPROVEMENT_PIRATE_COVE").is_some_and(|i| g.plot_can_have_improvement(q, i, None, false))
}

/// A Lanun work boat with no sea resource left to improve goes to the best plot for a Pirate
/// Cove (worked plots of big cities first) and casts there. True if it was given orders.
pub fn sea_worker(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
    let p = v.p;
    if civ(g, p) != Civ::Lanun {
        return false;
    }
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
