//! Amurites: the arcane civilization (both leaders Arcane; the palace gives Fire, Body and
//! Metamagic mana; Wizards are Mages with a free Spellstaff; the research plan beelines Sorcery).
//! They keep casters so their Wizards come (a Wizard is an Adept upgraded at level 4):
//! [`super::unique_casters`], the owner's rule that civilizations use their unique units
//! (CLAUDE.md "AI changes").
//!
//! First tried in round 5 (commit 5c6fdbe; docs/AI_LOG.md), before casters cast in fights:
//! development seeds 1-40 win 5.0% -> 7.5% (2/1), share at the end +108 (t 1.06); fresh seeds
//! 5001-5064 win 4.7% -> 1.6% (0/2), share at the end -25 (t -0.31). Back since 2026-10-20 under
//! the owner's rule. Arcane Lacuna is never cast: its AI-only gate (7 typed mana nodes on the
//! whole map) is never met (docs/AI.md, "Civilization-specific").

use ffh_core::game::Game;
use ffh_core::ids::*;

use super::CivPlay;
use crate::production::Choice;
use crate::Ai;

pub struct Amurites;

impl CivPlay for Amurites {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        super::unique_casters(g, c, threat)
    }
}
