//! Hippus. No civilization-specific play.
//!
//! Tried, not kept (round 5, commit ebb1427; docs/AI_LOG.md): Warcry (world spell: every unit with
//! a unit combat gets +1 strength, +1 defense, +1 move and Blitz, about 20 turns; AI gate: at war
//! with a civ, 25+ units) is cast by the generic choice on the declaration turn, while the war
//! operation still gathers. Held until the operation steps next to its target or strikes, a city is
//! in peril, or 20 turns of war: development seeds 1-40 win 15.0% -> 22.5% (4/1 seeds won by one
//! side only), share at turn 300 +91 (t 2.10); fresh seeds 5001-5064 (the civ with and without its
//! logic) win 14.1% -> 10.9% (1/3), share at turn 300 -1 (t -0.06), at the end -6 (t -0.07). Also
//! tried: Blitz units attacking again in a campaign strike (flat).

use super::CivPlay;

pub struct Hippus;

impl CivPlay for Hippus {}
