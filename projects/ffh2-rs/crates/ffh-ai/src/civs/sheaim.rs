//! Sheaim: both leaders are Summoner (Tebryn also Arcane), the palace gives Death, Chaos and Fire
//! mana, and the Training Yard is blocked (no tier-3 melee). No civilization-specific play.
//!
//! Tried, not kept (round 5, commit 4f00cf1; docs/AI_LOG.md): adepts need a Mage Guild in the
//! training city, which no AI builds (value about 0), so no Sheaim ever had a caster. Once
//! Knowledge of the Ether was known the most productive safe city built a Mage Guild (one per four
//! cities); at war, or preparing one, guild cities trained adepts up to one per two cities, taking
//! Death first (Raise Skeleton), then Fire. Development seeds 1-40: win 2.5% -> 2.5% (1/1 seeds won
//! by one side only), share at the end +95 (t 0.79); fresh seeds 5001-5064 (the civ with and
//! without its logic): win 0.0% -> 1.6% (1/0), share at turn 400 -144 (t -2.17), at the end -88
//! (t -0.97). The guilds (2.0 a game) and adepts come around turn 250 and cost more than they
//! bring.
//! Also tried: the Planar Gate valued by the creatures its city can call (0.1 a game, no effect).

use super::CivPlay;

pub struct Sheaim;

impl CivPlay for Sheaim {}
