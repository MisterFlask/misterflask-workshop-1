//! Grigori: their Medics heal the army at war (the owner's rule: civilizations use their unique
//! units, CLAUDE.md "AI changes").
//!
//! **Grigori Medic** (Medicine, needs an Infirmary; strength 4, Medic I-III: heals its plot and
//! the plots around it; upgrades to the Luonnotar): the generic unit choice rates it by strength
//! per hammer and never trains it, so the siege stacks heal at the normal rate. At war with a
//! civ, with an army of 4 attackers or more, one medic per 4 attackers (at most 3) is trained as
//! an army unit (it joins the stacks); the most productive city builds an Infirmary for it if
//! none of our cities has one.
//!
//! First tried in round 5 (development seeds 1-40: 0.9 medics a game, share at the end +31, t 0.3;
//! docs/AI_LOG.md); back since 2026-10-20 under the owner's rule. Ardor (the great-person
//! threshold back to its base) is cast by the generic end-of-turn choice when its AI gate opens at
//! 5 great people, close to the best moment for a civ that makes about ten great people a game.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::{count_ai, Choice};
use crate::Ai;

pub struct Grigori;

impl CivPlay for Grigori {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        if threat > 0 {
            return None;
        }
        medic(g, c)
    }
}

fn medic(g: &Game, c: CityId) -> Option<Choice> {
    let d = g.d;
    let p = g.cities[c].owner;
    let team = g.players[p].team;
    let unit = d.ty::<UnitType>("UNIT_GRIGORI_MEDIC")?;
    if !g.team_is_has_tech(team, d.units[unit].prereq_and_tech) || g.team_get_at_war_count(team, true) == 0 {
        return None;
    }
    let attackers = count_ai(g, p, UnitAIType::UNITAI_ATTACK) + count_ai(g, p, UnitAIType::UNITAI_ATTACK_CITY) + count_ai(g, p, UnitAIType::UNITAI_COLLATERAL);
    let class = d.units[unit].unit_class_type?;
    let medics = g.player_get_unit_class_count_plus_making(p, class);
    if attackers < 4 || medics >= (attackers / 4).min(3) || !crate::economy::can_afford_unit(g, p, true, true) {
        return None;
    }
    if g.city_can_train(c, unit, false, false, false, false) {
        return Some(Choice::Train(unit, UnitAIType::UNITAI_ATTACK));
    }
    // No Infirmary anywhere: the most productive city builds one.
    let inf = d.units[unit].prereq_building?;
    let inf_class = d.buildings[inf].building_class_type?;
    let best = g.players[p].cities.iter().copied().max_by_key(|&o| (g.city_yield_rate(o, YieldType::YIELD_PRODUCTION), std::cmp::Reverse(o)));
    if best != Some(c) || g.player_get_building_class_count_plus_making(p, inf_class) > 0 || !g.city_can_construct(c, inf, false, false, false) {
        return None;
    }
    Some(Choice::Construct(inf))
}
