//! Infernal (joins mid-game when the first team learns Infernal Pact). Fallow (`bIgnoreFood`):
//! their cities produce no food and never grow, so population comes only from a new city's
//! starting 3 and from Manes (`SPELL_ADD_TO_CITY`, +1 population, which FfH's spell value never
//! casts). No civilization-specific play.
//!
//! Tried, not kept (round 5, commit f20b509; docs/AI_LOG.md): Manes in one of our cities with
//! another defender there added themselves, lone Manes walked to the nearest city. Development
//! (the 6 games of seeds 1-40 where the Infernals appear): share 1056 -> 1557 (t 2.21); fresh
//! seeds 5001-5064 (the 60 games where they appear, the logic on against off): population +5.0
//! (t 2.23) but units -6.2 (t -2.77), alive 0.8 -> 0.7 (t -1.66), share -29 (t -0.68). The Manes
//! were most of their army.

use super::CivPlay;

pub struct Infernal;

impl CivPlay for Infernal {}
