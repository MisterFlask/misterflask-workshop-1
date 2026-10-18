//! Helpers for world spells a civilization holds for a better moment (cast once per game): the
//! end-of-turn spell choice is told not to cast them (`spell_value` 0), and the civilization's
//! `turn` hook casts them through the rules (`unit_can_cast`, so the AI-only gates of the ported
//! Python still apply) when its own condition holds.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::campaign::Phase;
use crate::view::View;

/// The spell type named `name`, if the data has it.
pub fn spell(g: &Game, name: &str) -> Option<SpellType> {
    g.d.ty::<SpellType>(name)
}

/// A unit of ours that can cast `spell` now: the first in unit order (any unit of the
/// civilization casts its world spell).
pub fn caster(g: &Game, p: PlayerId, spell: SpellType) -> Option<UnitId> {
    g.players[p].units.iter().copied().find(|&u| g.units.contains(u) && g.unit_can_cast(u, spell, false))
}

/// Cast `spell` with any unit able to (true if cast).
pub fn cast(g: &mut Game, p: PlayerId, spell: SpellType, why: &str) -> bool {
    let Some(u) = caster(g, p, spell) else { return false };
    crate::trace!(g, p, "world spell {} ({})", g.d.spells[spell].base.type_, why);
    g.unit_cast(u, spell);
    true
}

/// One of our cities is in peril while we are at war with a civ.
pub fn civ_peril(v: &View) -> bool {
    !v.civ_enemies.is_empty() && !v.peril.is_empty()
}

/// The turns a 20-turn effect at normal speed lasts at this game speed (the world spells'
/// Python scales them: quick 14, epic 30, marathon 60).
pub fn speed_turns(g: &Game, normal: i32) -> i32 {
    let d = g.d;
    let s = g.game_speed_type();
    if d.ty::<GameSpeedType>("GAMESPEED_QUICK") == Some(s) {
        normal * 7 / 10
    } else if d.ty::<GameSpeedType>("GAMESPEED_EPIC") == Some(s) {
        normal * 3 / 2
    } else if d.ty::<GameSpeedType>("GAMESPEED_MARATHON") == Some(s) {
        normal * 3
    } else {
        normal
    }
}

/// Turns we have been at war with a civ (the longest of our current civ wars; 0 at peace).
pub fn civ_war_turns(g: &Game, v: &View) -> i32 {
    v.civ_enemies.iter().map(|&e| g.teams[v.team].ai_at_war_counter[e]).max().unwrap_or(0)
}

/// Our war operation against a civ strikes this turn, or steps next to its target (strikes
/// next turn).
pub fn war_op_striking(g: &Game, v: &mut View) -> bool {
    crate::campaign::plan(g, v);
    v.campaign.ops.iter().any(|o| o.civ && o.at_war && matches!(o.phase, Phase::Strike | Phase::StepIn))
}
