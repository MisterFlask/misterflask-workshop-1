//! Doviello. They keep casters, their Shamans (the Doviello Adept, no Mage Guild needed):
//! [`super::unique_casters`], the owner's rule that civilizations use their unique units (CLAUDE.md
//! "AI changes"). Their War Machine and Bison Riders come from the generic building-gated units
//! (`production::unit_building`); the War Machine (Hero promotion) is trained as a hero.
//!
//! Worker -> Beastman (`SPELL_UPGRADE_DOVIELLO_WARRIOR`; the rules' AI gate: at war, with as many
//! workers as cities) turns only the workers our land does not need into Beastmen
//! ([`Doviello::spell_value`], `production::worker_target`), and they fight as Beastmen
//! (`units::fighters_off_worker_ai`). The cast kept the worker AI, so its Beastmen built nothing
//! and were counted as workers: production trained none, and they sat in our cities. Once they
//! fought, production trained workers at war and the cast took them again (docs/AI_LOG.md,
//! "Unique units"). Gated, against the same build without the two (seeds 9201-9240): share at
//! the end -17 (t -0.20), alive flat; from turn 300, 1.3 more workers (t 2.9), improvements
//! +4-10%, research +8-13%.
//!
//! Tried and not kept (docs/AI_LOG.md): holding Wild Hunt (a wolf for every unit of ours) for the
//! strike, a city in peril or 20 turns of war, with the wolves made army units in the stacks; no
//! Worker -> Beastman conversion unless the worker is about to be captured; a gold reserve for the
//! upgrade spells; a Siege Workshop and Stables for the War Machine and the Bison Rider (never
//! reached in the measured games; generic now).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::{count_ai, worker_target, Choice};
use crate::Ai;

pub struct Doviello;

impl CivPlay for Doviello {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        super::unique_casters(g, c, threat)
    }

    /// Workers made Beastmen only beyond what our land needs (see the module doc).
    fn spell_value(&self, g: &Game, u: UnitId, spell: SpellType, value: i32) -> i32 {
        if matches!(g.d.spells[spell].base.type_.as_str(), "SPELL_UPGRADE_DOVIELLO_WARRIOR" | "SPELL_UPGRADE_DOVIELLO_WARRIOR_SLAVE") {
            let p = g.units[u].owner;
            if count_ai(g, p, UnitAIType::UNITAI_WORKER) <= worker_target(g, p) {
                return 0;
            }
        }
        value
    }
}
