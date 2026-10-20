//! Doviello. They keep casters, their Shamans (the Doviello Adept, no Mage Guild needed):
//! [`super::unique_casters`], the owner's rule that civilizations use their unique units (CLAUDE.md
//! "AI changes"). Their War Machine and Bison Riders come from the generic building-gated units
//! (`production::unit_building`).
//!
//! Tried and not kept (docs/AI_LOG.md): holding Wild Hunt (a wolf for every unit of ours) for the
//! strike, a city in peril or 20 turns of war, with the wolves made army units in the stacks; no
//! Worker -> Beastman conversion unless the worker is about to be captured; a gold reserve for the
//! upgrade spells; a Siege Workshop and Stables for the War Machine and the Bison Rider (never
//! reached in the measured games; generic now).

use ffh_core::game::Game;
use ffh_core::ids::*;

use super::CivPlay;
use crate::production::Choice;
use crate::Ai;

pub struct Doviello;

impl CivPlay for Doviello {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        super::unique_casters(g, c, threat)
    }
}
