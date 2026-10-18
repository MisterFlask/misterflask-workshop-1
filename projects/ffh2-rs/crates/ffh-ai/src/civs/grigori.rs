//! Grigori: no civilization-specific play. Tried and not kept (docs/civ-ai-b.md): training
//! Grigori Medics (Medic I-III, Infirmary) for the army at war. They were built and joined the
//! stacks, without a measurable effect. Ardor (the great-person threshold back to its base) is
//! cast by the generic end-of-turn choice when its AI gate opens at 5 great people, close to the
//! best moment for a civ that makes about ten great people a game.

use super::CivPlay;

pub struct Grigori;

impl CivPlay for Grigori {}
