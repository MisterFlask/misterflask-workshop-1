//! Civilization-specific play (item `civ-mechanics`): rules some civilizations have that the
//! generic AI does not see. Each civilization has a module here implementing [`CivPlay`]; the
//! generic AI calls the hooks below at fixed points, and a civilization overrides only the hooks
//! it needs (the defaults change nothing). Every hook dispatches on [`civ`], so games without a
//! civilization play exactly as before (same checksums), and `FFH_AI_CIV_OFF` turns a
//! civilization's logic off for measurement ([`enabled`]).
//!
//! Hooks and where the generic AI calls them:
//! - [`production`]: `production::decide`, right after the settler step;
//! - [`turn`]: once per turn in `play_turn`, after the empire, diplomacy and the view, before any
//!   unit moves (empire-level actions: world spells, rituals, ...);
//! - [`unit`]: `units::play`, before a group's role logic (true = the group got its orders);
//! - [`sea_worker`]: `workers::worker` for sea workers;
//! - [`spell_value`]: `casting::choose_spell`, the value of each castable spell (0 or less: not
//!   cast by the end-of-turn spell choice);
//! - [`building_value`]: `production::building_value` (what production, research and spells use);
//! - [`tech_value`]: `research::tech_value`;
//! - [`site_value`]: `sites::compute`, the value of a city site.
//!
//! What each civilization does is described in its module (docs/AI.md, "Civilization-specific").

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::production::Choice;
use crate::view::View;
use crate::Ai;

mod amurites;
mod balseraphs;
mod bannor;
mod calabim;
mod clan_of_embers;
mod doviello;
mod elohim;
mod grigori;
mod hippus;
mod illians;
mod infernal;
mod khazad;
mod kuriotates;
mod lanun;
mod ljosalfar;
mod luchuirp;
mod malakim;
mod mercurians;
mod sheaim;
mod sidar;
mod svartalfar;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Civ {
    Amurites,
    Balseraphs,
    Bannor,
    Calabim,
    ClanOfEmbers,
    Doviello,
    Elohim,
    Grigori,
    Hippus,
    Illians,
    Infernal,
    Khazad,
    Kuriotates,
    Lanun,
    Ljosalfar,
    Luchuirp,
    Malakim,
    Mercurians,
    Sheaim,
    Sidar,
    Svartalfar,
    Other,
}

const NAMES: [(&str, Civ); 21] = [
    ("AMURITES", Civ::Amurites),
    ("BALSERAPHS", Civ::Balseraphs),
    ("BANNOR", Civ::Bannor),
    ("CALABIM", Civ::Calabim),
    ("CLAN_OF_EMBERS", Civ::ClanOfEmbers),
    ("DOVIELLO", Civ::Doviello),
    ("ELOHIM", Civ::Elohim),
    ("GRIGORI", Civ::Grigori),
    ("HIPPUS", Civ::Hippus),
    ("ILLIANS", Civ::Illians),
    ("INFERNAL", Civ::Infernal),
    ("KHAZAD", Civ::Khazad),
    ("KURIOTATES", Civ::Kuriotates),
    ("LANUN", Civ::Lanun),
    ("LJOSALFAR", Civ::Ljosalfar),
    ("LUCHUIRP", Civ::Luchuirp),
    ("MALAKIM", Civ::Malakim),
    ("MERCURIANS", Civ::Mercurians),
    ("SHEAIM", Civ::Sheaim),
    ("SIDAR", Civ::Sidar),
    ("SVARTALFAR", Civ::Svartalfar),
];

/// Per civilization type: the [`Civ`] whose logic it plays (`Civ::Other` when it has none or is
/// listed in `FFH_AI_CIV_OFF`). Built once from the game data and the environment.
fn table(d: &GameData) -> &'static [Civ] {
    static TABLE: std::sync::OnceLock<Vec<Civ>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let off: Vec<String> = std::env::var("FFH_AI_CIV_OFF")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().trim_start_matches("CIVILIZATION_").to_ascii_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        d.civilizations
            .keys()
            .map(|c| {
                let name = d.civilizations[c].base.type_.trim_start_matches("CIVILIZATION_");
                if off.iter().any(|o| o == "ALL" || o == name) {
                    return Civ::Other;
                }
                NAMES.iter().find(|(n, _)| *n == name).map_or(Civ::Other, |&(_, civ)| civ)
            })
            .collect()
    })
}

/// The player's civilization as far as civilization-specific play goes: `Civ::Other` for
/// civilizations without any, and for those turned off by `FFH_AI_CIV_OFF` (see [`enabled`]).
pub fn civ(g: &Game, p: PlayerId) -> Civ {
    g.player_get_civilization_type(p).map_or(Civ::Other, |c| table(g.d)[c.idx()])
}

/// False when the player's civilization-specific play is turned off: `FFH_AI_CIV_OFF` lists
/// civilizations (`CIVILIZATION_KHAZAD` or `KHAZAD`, comma-separated, or `all`) that play as
/// generic civs. A measurement switch (civ X with and without its logic, against the same
/// opponents, in one binary); it is read once and never changes during a game, so play stays
/// deterministic, and the run cache keys runs by every `FFH_*` variable. The research flavours
/// of `tech_plan.rs` (older, per-civilization beelines) are not covered.
pub fn enabled(g: &Game, p: PlayerId) -> bool {
    civ(g, p) != Civ::Other
}

/// A civilization's own play. Every method defaults to "nothing special".
#[allow(unused_variables)]
pub trait CivPlay: Sync {
    /// A production choice, asked right after the settler step of `production::decide`.
    fn production(&self, ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        None
    }
    /// Empire-level actions, once per turn before any unit moves.
    fn turn(&self, ai: &Ai, g: &mut Game, v: &mut View) {}
    /// Orders for a group (`head` is its head unit) before its role's logic; true if given.
    fn unit(&self, ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, head: UnitId) -> bool {
        false
    }
    /// Orders for a sea worker; true if given.
    fn sea_worker(&self, g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
        false
    }
    /// The end-of-turn spell choice's value of casting `spell` with `u` (`value` is FfH's
    /// estimate); 0 or less and it is not cast there.
    fn spell_value(&self, g: &Game, u: UnitId, spell: SpellType, value: i32) -> i32 {
        value
    }
    /// What building `b` is worth in city `c` (`value` is the generic estimate).
    fn building_value(&self, g: &Game, c: CityId, b: BuildingType, value: i32) -> i32 {
        value
    }
    /// What researching `t` is worth (`value` is the generic estimate, before cost).
    fn tech_value(&self, g: &Game, p: PlayerId, t: TechType, value: i32) -> i32 {
        value
    }
    /// What a city site at `q` is worth to `p` (`value` is `AI_foundValue`; 0 or less: no site).
    fn site_value(&self, g: &Game, p: PlayerId, q: PlotIdx, value: i32) -> i32 {
        value
    }
}

struct Generic;
impl CivPlay for Generic {}

fn play(c: Civ) -> &'static dyn CivPlay {
    match c {
        Civ::Amurites => &amurites::Amurites,
        Civ::Balseraphs => &balseraphs::Balseraphs,
        Civ::Bannor => &bannor::Bannor,
        Civ::Calabim => &calabim::Calabim,
        Civ::ClanOfEmbers => &clan_of_embers::ClanOfEmbers,
        Civ::Doviello => &doviello::Doviello,
        Civ::Elohim => &elohim::Elohim,
        Civ::Grigori => &grigori::Grigori,
        Civ::Hippus => &hippus::Hippus,
        Civ::Illians => &illians::Illians,
        Civ::Infernal => &infernal::Infernal,
        Civ::Khazad => &khazad::Khazad,
        Civ::Kuriotates => &kuriotates::Kuriotates,
        Civ::Lanun => &lanun::Lanun,
        Civ::Ljosalfar => &ljosalfar::Ljosalfar,
        Civ::Luchuirp => &luchuirp::Luchuirp,
        Civ::Malakim => &malakim::Malakim,
        Civ::Mercurians => &mercurians::Mercurians,
        Civ::Sheaim => &sheaim::Sheaim,
        Civ::Sidar => &sidar::Sidar,
        Civ::Svartalfar => &svartalfar::Svartalfar,
        Civ::Other => &Generic,
    }
}

fn of(g: &Game, p: PlayerId) -> &'static dyn CivPlay {
    play(civ(g, p))
}

// ----------------------------------------------------------------------------
// Hooks (called by the generic AI)
// ----------------------------------------------------------------------------

pub fn production(ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
    of(g, g.cities[c].owner).production(ai, g, c, threat)
}

pub fn turn(ai: &Ai, g: &mut Game, v: &mut View) {
    of(g, v.p).turn(ai, g, v)
}

pub fn unit(ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, head: UnitId) -> bool {
    of(g, v.p).unit(ai, g, v, grp, head)
}

pub fn sea_worker(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
    of(g, v.p).sea_worker(g, v, grp, u)
}

pub fn spell_value(g: &Game, u: UnitId, spell: SpellType, value: i32) -> i32 {
    of(g, g.units[u].owner).spell_value(g, u, spell, value)
}

pub fn building_value(g: &Game, c: CityId, b: BuildingType, value: i32) -> i32 {
    of(g, g.cities[c].owner).building_value(g, c, b, value)
}

pub fn tech_value(g: &Game, p: PlayerId, t: TechType, value: i32) -> i32 {
    of(g, p).tech_value(g, p, t, value)
}

pub fn site_value(g: &Game, p: PlayerId, q: PlotIdx, value: i32) -> i32 {
    of(g, p).site_value(g, p, q, value)
}
