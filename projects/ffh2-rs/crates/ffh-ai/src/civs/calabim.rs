//! Calabim. No civilization-specific play.
//!
//! Tried, not kept (round 5, commit efb8d23; docs/AI_LOG.md), together: (1) the rules put every
//! Vampire on `UNITAI_FEASTING` when it is created (`ev_unit_created`), a unit AI type this AI has
//! no logic for, so vampires stay out of stacks, siege forces and the attacker count -- the `turn`
//! hook put them back on `UNITAI_ATTACK`; (2) Feast (FfH's estimate -50, never cast) cast in cities
//! above their happy cap by units below 30 experience, and Gift Vampirism (Python only, value 0)
//! valued as the Vampire promotion for each level 6+ unit on the plot; (3) River of Blood held
//! until a civ war or until our cities have room to keep the citizens it gives. Development seeds
//! 1-40: win 7.7% -> 7.7% (2/2), share at the end +109 (t 0.59); fresh seeds 5001-5064 (the civ
//! with and without its logic): win 10.9% -> 4.7% (2/6 seeds won by one side only), share at the
//! end -77 (t -0.61). Vampires are rare (they need Feudalism and a Governor's Manor: 0.1 a game),
//! so (1) and (2) are seldom reached; the vampire AI type stays a known gap (docs/AI.md).

use super::CivPlay;

pub struct Calabim;

impl CivPlay for Calabim {}
