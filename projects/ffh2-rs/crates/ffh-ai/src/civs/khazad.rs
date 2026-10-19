//! Khazad: nothing kept.
//!
//! Tried and measured (docs/AI_LOG.md): their siege engines (Trebuchet, Dwarven Cannon) need a
//! Siege Workshop and their knight (Hornguard) a Stable, specific buildings (`PrereqBuilding`) the
//! generic production never builds. Building the Workshop in the most productive city when the
//! operation wants siege, and a Stable where it opens the Hornguard, got Trebuchets and Cannons
//! built (2.5 and 1.4 a game) but no better results (full games, seeds 1-40: win 28% -> 23%,
//! world share at turn 300 -6, at the end +50, t 0.4). Saving gold for their Dwarven Vaults
//! (`cf_do_turn_khazad`) was tried before: more population, fewer techs, the same score by turn
//! 300 (see docs/AI.md).

use super::CivPlay;

pub struct Khazad;

impl CivPlay for Khazad {}
