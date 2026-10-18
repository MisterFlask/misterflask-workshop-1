//! Bannor: no civilization-specific play kept.
//!
//! Crusade (civic, needs a war with a civ; +25% unit production, Demagogs, Rally; but no
//! settlers, workers or diplomacy with enemies, and lost at peace) is adopted by the generic AI
//! in any war, through its `iAIWeight` of 1000 a city. Holding it back to wars we are clearly
//! winning on power ([`super::CivPlay::civic_value`]: ours at least 1.2x the enemies' sum, kept
//! once adopted) halved how often it was taken (15 -> 7 of 40 full games) and changed nothing
//! measurable (share@end +10, t 0.16; see docs/civ-ai-a.md), so it was not kept.

use super::CivPlay;

pub struct Bannor;

impl CivPlay for Bannor {}
