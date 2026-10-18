//! Doviello: no civilization-specific play. Tried and not kept (docs/civ-ai-b.md): holding Wild
//! Hunt (a wolf for every unit of ours) for the strike, a city in peril or 20 turns of war, with
//! the wolves made army units in the stacks; no Worker -> Beastman conversion unless the worker
//! is about to be captured; a gold reserve for the upgrade spells; a Siege Workshop and Stables
//! for the War Machine and the Bison Rider (never reached in the measured games).

use super::CivPlay;

pub struct Doviello;

impl CivPlay for Doviello {}
