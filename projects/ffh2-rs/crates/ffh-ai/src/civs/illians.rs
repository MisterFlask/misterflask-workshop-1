//! Illians. No civilization-specific play.
//!
//! Tried, not kept (round 5, commit 76c1ffd; docs/AI_LOG.md): the ritual projects The White Hand
//! (Philosophy, 400 hammers: three Priests of Winter) and Stir from Slumber (Drifa), which the
//! generic AI reaches only when no building has value left, built by the most productive city as
//! soon as it can. Development seeds 1-40: win 12.5% -> 17.5% (2/0 seeds won by one side only),
//! share at the end +269 (t 1.90); fresh seeds 5001-5064 (the civ with and without its logic): win
//! 15.6% -> 14.1% (2/3), share at the end -40 (t -0.56). Also tried: holding Stasis (20 turns
//! without production or research for every other player) for a war, a city in peril or a rival
//! near victory -- it lost share up to turn 400 (share@200 -48, t -1.4): the freeze is worth most
//! early, so it stays with the generic cast at its gate (5 cities).

use super::CivPlay;

pub struct Illians;

impl CivPlay for Illians {}
