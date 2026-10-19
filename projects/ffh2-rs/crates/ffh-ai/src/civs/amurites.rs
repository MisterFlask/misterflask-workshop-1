//! Amurites: the arcane civilization (both leaders Arcane; the palace gives Fire, Body and
//! Metamagic mana; Wizards are Mages with a free Spellstaff; the research plan beelines Sorcery).
//! No civilization-specific play.
//!
//! Tried, not kept (round 5, commit 5c6fdbe; docs/AI_LOG.md): adepts and mages need a Mage Guild in
//! the training city, which no AI builds (`iAIWeight` 0, no yields), so the Amurites had no casters
//! at all. The empire's most productive unthreatened city built the Mage Guild once Knowledge of
//! the Ether was known (turn ~230) and trained casters up to (cities + 2) / 2. Development seeds
//! 1-40: win 5.0% -> 7.5% (2/1 seeds won by one side only), share at the end +108 (t 1.06); fresh
//! seeds 5001-5064 (the civ with and without its logic): win 4.7% -> 1.6% (0/2), share at the end
//! -25 (t -0.31). The casters come too late to change these games. Arcane Lacuna is never cast: its
//! AI-only gate (7 typed mana nodes on the whole map) is never met (docs/AI.md,
//! "Civilization-specific").
//!
//! The generic spellcasting (`magic.rs`) casts with the casters there are and trains none:
//! training adepts for the Fireball they would cast as mages was tried there and lost (the
//! palace's Fire, Body and Metamagic give an adept nothing for a fight; docs/AI_LOG.md,
//! "Spellcasting").

use super::CivPlay;

pub struct Amurites;

impl CivPlay for Amurites {}
