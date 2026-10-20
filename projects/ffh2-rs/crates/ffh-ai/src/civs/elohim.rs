//! Elohim: no civilization-specific play. Corlindale (a hero of strength 0, Channeling III) is
//! trained as a hero and moves as a caster (`production::role_ok`, `units::role_of`); before, no
//! Elohim trained him.
//!
//! Tried and not kept (docs/AI_LOG.md): holding Sanctuary (30 turns in which no foreign unit can
//! enter our land; AI gate: two civ wars) until enemy units stand in our land or a city is in
//! peril. The gate opens in few games and the measured change was within noise, slightly
//! negative.

use super::CivPlay;

pub struct Elohim;

impl CivPlay for Elohim {}
