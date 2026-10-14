//! End-of-turn spellcasting (FfH `CvGame::updateMoves`, "BETTER AI (Spellcasting end of
//! turn)"): with the player's summon-suicide mode on, every unit casts its best spell
//! (`CvUnit::chooseSpell`), then buff (`CvUnitAI::AI_BuffCast`) and summon
//! (`CvUnitAI::AI_SummonCast`) spells. The valuation follows `chooseSpell` term by term; the
//! C++ AI's `AI_promotionValue` / `AI_unitValue` / `AI_buildingValue` are replaced by this
//! AI's own estimates.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::production::building_value;
use crate::units::promotion_value;
use crate::view::{type_attack_power, type_defense_power, View};

pub fn end_turn(g: &mut Game, v: &View) {
    let p = v.p;
    // C++ AI_setSummonSuicideMode(true): summons created now attack on their own.
    g.players[p].sum_sui_mode = true;
    for u in g.players[p].units.clone() {
        if !g.units.contains(u) {
            continue;
        }
        if let Some(spell) = choose_spell(g, u) {
            cast_and_act(g, v, u, spell);
        }
        if !g.units.contains(u) || g.units[u].death_delay {
            continue;
        }
        if is_buffer(g, u) {
            buff_cast(g, v, u);
        }
        if !g.units.contains(u) || g.units[u].death_delay {
            continue;
        }
        summon_cast(g, v, u);
    }
    g.players[p].sum_sui_mode = false;
}

/// Cast, then let summons created in suicide mode attack at once (C++ `castCreateUnit` runs
/// their `AI_update` -> `AI_summonAttackMove`: `AI_anyAttack(baseMoves(), 5)`, i.e. any target
/// at odds of 5% or better). Unit ids are never reused, so new units have larger ids.
fn cast_and_act(g: &mut Game, v: &View, u: UnitId, spell: SpellType) {
    let p = v.p;
    let max_before = g.players[p].units.iter().copied().max();
    g.unit_cast(u, spell);
    let summons: Vec<UnitId> = g.players[p]
        .units
        .iter()
        .copied()
        .filter(|&n| max_before.map_or(true, |m| n > m) && g.units[n].ai_suicide_summon)
        .collect();
    for n in summons {
        // Attack while alive with moves left (blitzing summons may attack more than once).
        for _ in 0..4 {
            if !g.units.contains(n) || g.unit_moves_left(n) <= 0 {
                break;
            }
            let Some(grp) = g.units[n].group else { break };
            if !crate::units::attack_adjacent(g, v, grp, n, 50) {
                break;
            }
        }
    }
}

fn unit_value(d: &GameData, ut: UnitType) -> i32 {
    type_attack_power(d, ut) + type_defense_power(d, ut) / 2 + d.units[ut].ai_weight
}

fn enemy_near(g: &Game, u: UnitId, range: i32) -> bool {
    let (x, y) = (g.units[u].x, g.units[u].y);
    for i in -range..=range {
        for j in -range..=range {
            if let Some(q) = g.plot_xy(x, y, i, j) {
                if g.plot_is_visible_enemy_unit_for_unit(q, u) {
                    return true;
                }
            }
        }
    }
    false
}

fn summon_value(g: &Game, u: UnitId, spell: SpellType) -> i32 {
    let d = g.d;
    let info = &d.spells[spell];
    let Some(cu) = info.create_unit_type else { return 0 };
    let move_range = d.units[cu].moves + g.unit_get_extra_spell_move(u);
    if !enemy_near(g, u, move_range) {
        return 0;
    }
    let mut v = d.units[cu].combat + d.units[cu].damage_type_combat.iter().sum::<i32>();
    v *= 100;
    v * info.create_unit_num
}

/// Port of `CvUnit::chooseSpell` (with AI value estimates).
pub fn choose_spell(g: &Game, u: UnitId) -> Option<SpellType> {
    let d = g.d;
    let mut best: Option<SpellType> = None;
    let mut best_value = 0;
    let here = g.unit_plot(u);
    let team = g.unit_get_team(u);
    for spell in d.spells.keys() {
        if !g.unit_can_cast(u, spell, false) {
            continue;
        }
        let info = &d.spells[spell];
        let range = info.range;
        let mut value = 0;
        if info.create_unit_type.is_some() {
            value += summon_value(g, u, spell);
        }
        if info.damage != 0 {
            let (x, y) = (g.units[u].x, g.units[u].y);
            for i in -range..=range {
                for j in -range..=range {
                    let Some(q) = g.plot_xy(x, y, i, j) else { continue };
                    if q == here {
                        continue;
                    }
                    for &t in &g.map.plots[q].units {
                        if g.unit_is_immune_to_spell(t, u, spell) {
                            continue;
                        }
                        let enemy = g.unit_is_enemy(t, Some(team), None);
                        if enemy && g.units[t].damage < info.damage_limit {
                            value += info.damage * 10;
                        }
                        let tteam = g.unit_get_team(t);
                        if tteam == team {
                            value -= info.damage * 20;
                        }
                        if tteam != team && !enemy {
                            value -= 1000;
                        }
                    }
                }
            }
        }
        for promo in [info.add_promotion_type1, info.add_promotion_type2, info.add_promotion_type3].into_iter().flatten() {
            value += promotion_value(g, u, promo);
        }
        for promo in [info.remove_promotion_type1, info.remove_promotion_type2, info.remove_promotion_type3].into_iter().flatten() {
            value -= promotion_value(g, u, promo);
        }
        if let Some(cu) = info.convert_unit_type {
            value += unit_value(d, cu) - unit_value(d, g.units[u].unit_type);
        }
        if let Some(b) = info.create_building_type {
            if let Some(c) = g.map.plots[here].plot_city {
                value += building_value(g, c, b, 0);
            }
        }
        if info.create_feature_type.is_some() {
            value += 10;
        }
        if info.create_improvement_type.is_some() {
            value += 10;
        }
        if info.dispel {
            value += 25 * (range + 1) * (range + 1);
        }
        if info.push {
            value += 20 * (range + 1) * (range + 1);
            if g.map.plots[here].plot_city.is_some() {
                value *= 3;
            }
        }
        if info.change_population != 0 {
            value += 50 * info.change_population;
        }
        if info.cost != 0 {
            value -= 4 * info.cost;
        }
        if info.immobile_turns != 0 {
            value += 20 * info.immobile_turns * (range + 1) * (range + 1);
        }
        if info.sacrifice_caster {
            value -= g.units[u].level * unit_value(d, g.units[u].unit_type);
        }
        if info.resistable {
            value /= 2;
        }
        value += info.ai_weight;
        if value > best_value {
            best_value = value;
            best = Some(spell);
        }
    }
    best
}

/// C++ `CvUnitAI::isBuffer`: can cast a spell adding a beneficial promotion.
fn is_buffer(g: &Game, u: UnitId) -> bool {
    g.d.spells.keys().any(|s| {
        let info = &g.d.spells[s];
        info.add_promotion_type1.is_some_and(|p| g.d.promotions[p].ai_weight > 0) && g.unit_can_cast(u, s, false)
    })
}

/// C++ `CvUnitAI::AI_BuffCast`.
fn buff_cast(g: &mut Game, v: &View, u: UnitId) {
    if g.units[u].has_casted {
        return;
    }
    let d = g.d;
    let mut best: Option<SpellType> = None;
    let mut best_value = 0;
    for s in d.spells.keys() {
        let info = &d.spells[s];
        let Some(p1) = info.add_promotion_type1 else { continue };
        if !g.unit_can_cast(u, s, false) {
            continue;
        }
        let mut value = d.promotions[p1].ai_weight;
        if let Some(p2) = info.add_promotion_type2 {
            value += d.promotions[p2].ai_weight;
        }
        if let Some(p3) = info.add_promotion_type3 {
            value += d.promotions[p3].ai_weight;
        }
        if value > best_value {
            best_value = value;
            best = Some(s);
        }
    }
    if let Some(s) = best {
        cast_and_act(g, v, u, s);
    }
}

/// C++ `CvUnitAI::AI_SummonCast` (only when enemies are within the summon's reach).
fn summon_cast(g: &mut Game, v: &View, u: UnitId) {
    if g.units[u].has_casted {
        return;
    }
    let mut best: Option<SpellType> = None;
    let mut best_value = 0;
    for s in g.d.spells.keys() {
        if g.d.spells[s].create_unit_type.is_none() || !g.unit_can_cast(u, s, false) {
            continue;
        }
        let value = summon_value(g, u, s);
        if value > best_value {
            best_value = value;
            best = Some(s);
        }
    }
    if let Some(s) = best {
        cast_and_act(g, v, u, s);
    }
}
