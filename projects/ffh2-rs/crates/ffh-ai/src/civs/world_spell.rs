//! World spells held for a moment of the AI's choosing (group-d civilizations): the civilization's
//! `spell_value` returns 0 for the spell, so the end-of-turn spell choice never casts it, and its
//! `turn` hook casts it through [`cast`] when the moment comes (the rules' requirement and the
//! AI's default timing, `world_spells::ready`, must both allow it).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

/// Casts world spell `name` with one of the player's units that can (a city defender first, so
/// the army keeps its turn); true if cast.
pub fn cast(g: &mut Game, p: PlayerId, name: &str) -> bool {
    let Some(spell) = g.d.ty::<SpellType>(name) else { return false };
    if g.players[p].feat_accomplished[FeatType::FEAT_GLOBAL_SPELL] || !crate::world_spells::ready(g, p, spell) {
        return false;
    }
    let units: Vec<UnitId> = g.players[p].units.clone();
    let caster = units
        .iter()
        .copied()
        .filter(|&u| g.unit_can_cast(u, spell, false))
        .min_by_key(|&u| (g.units[u].ai_type != Some(UnitAIType::UNITAI_CITY_DEFENSE), u));
    let Some(u) = caster else { return false };
    crate::trace!(g, p, "world spell {} by {}", name, g.d.units[g.units[u].unit_type].base.type_);
    g.unit_cast(u, spell);
    true
}

/// True if `spell` is world spell `name`.
pub fn is(g: &Game, spell: SpellType, name: &str) -> bool {
    g.d.ty::<SpellType>(name) == Some(spell)
}

/// At war with a civilization (not the barbarians), or preparing a war.
pub fn war_or_preparing(g: &Game, p: PlayerId) -> bool {
    let team = g.players[p].team;
    let barb = g.barbarian_team();
    g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t))
        || crate::diplomacy::preparing_target(g, team).is_some()
}
