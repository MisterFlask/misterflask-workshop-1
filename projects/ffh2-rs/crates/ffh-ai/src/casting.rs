//! End-of-turn spellcasting (FfH `CvGame::updateMoves`, "BETTER AI (Spellcasting end of
//! turn)"): with the player's summon-suicide mode on, every unit casts its best spell
//! (`CvUnit::chooseSpell`), then buff (`CvUnitAI::AI_BuffCast`) and summon
//! (`CvUnitAI::AI_SummonCast`) spells. The valuation follows `chooseSpell` term by term; the
//! C++ AI's `AI_promotionValue` / `AI_unitValue` / `AI_buildingValue` are replaced by this
//! AI's own estimates. A player whose spellcasting is this AI's own (`magic::enabled`) chooses
//! by `magic::choose_turn_spell` instead, which keeps these terms for the spells it does not
//! model; its summons attack as FfH's do ([`summons_attack`]).

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::production::building_value;
use crate::units::promotion_value;
use crate::view::{attack_power, type_attack_power, type_defense_power, View};

pub fn end_turn(g: &mut Game, v: &View) {
    let p = v.p;
    let magic = crate::magic::enabled(g, p);
    // C++ AI_setSummonSuicideMode(true): summons created now attack on their own.
    g.players[p].sum_sui_mode = true;
    for u in g.players[p].units.clone() {
        if !g.units.contains(u) {
            continue;
        }
        if magic {
            // C++ quirk: a unit another unit's spell has just killed (delayed death: Form Wolf
            // Pack's second wolf) still casts, as in FfH's loop below.
            if let Some((spell, value)) = crate::magic::choose_turn_spell(g, v, u) {
                cast_and_act(g, v, u, spell, "turn", value);
                continue;
            }
            // Nothing is worth casting by what it does: FfH's own choice below, a free cast (Haste
            // or a Dance of Blades that blesses no one) that keeps the game FfH's where the values
            // see no difference (a cast rolls for a miscast and ends promotions casting removes).
        }
        if let Some(spell) = choose_spell(g, u) {
            cast_and_act(g, v, u, spell, "turn", 0);
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

/// Cast (`value`: the cast's value, for `FFH_AI_TRACE_CAST`), then let the summons it made attack
/// ([`summons_attack`]).
fn cast_and_act(g: &mut Game, v: &View, u: UnitId, spell: SpellType, ctx: &str, value: i32) {
    let newest = g.players[v.p].units.iter().copied().max();
    crate::magic::cast(g, u, spell, ctx, value);
    summons_attack(g, v, newest);
}

/// Summons created in suicide mode since unit `newest` attack at once (C++ `castCreateUnit` runs
/// their `AI_update` -> `AI_summonAttackMove`: `AI_anyAttack(baseMoves(), 5)`): the target of
/// [`summon_target`], again while they have moves left. Unit ids are never reused, so new units
/// have larger ids. The barbarians' summons attack only next to them, at 5% or better: with the
/// reach of [`summon_target`] they took more cities from the civilizations (docs/AI.md,
/// "Spellcasting").
///
/// Tried, not kept (docs/AI_LOG.md, "Spellcasting"): the summons of a player casting by
/// `magic.rs` attacked the adjacent plot where they took most hit points, at any odds and then at
/// 5% or better; Loki's puppets, cast every turn, fought more and the Balseraphs lost.
pub fn summons_attack(g: &mut Game, v: &View, newest: Option<UnitId>) {
    let p = v.p;
    let barbarian = g.player_is_barbarian(p);
    let summons: Vec<UnitId> = g.players[p]
        .units
        .iter()
        .copied()
        .filter(|&n| newest.map_or(true, |m| n > m) && g.units[n].ai_suicide_summon)
        .collect();
    for n in summons {
        // Attack while alive with moves left (blitzing summons may attack more than once).
        for _ in 0..4 {
            if !g.units.contains(n) || g.unit_moves_left(n) <= 0 {
                break;
            }
            let Some(grp) = g.units[n].group else { break };
            if barbarian {
                if !crate::units::attack_adjacent(g, v, grp, n, 50) {
                    break;
                }
                continue;
            }
            if attack_power(g, n) == 0 || !g.unit_can_attack(n) {
                break;
            }
            let Some(q) = summon_target(g, n) else { break };
            let (x, y) = (g.units[n].x, g.units[n].y);
            let (qx, qy) = g.map.xy(q);
            let flags = if g.plot_distance(x, y, qx, qy) == 1 { MOVE_DIRECT_ATTACK } else { 0 };
            let moves = g.unit_moves_left(n);
            crate::units::move_to(g, grp, q, flags);
            if g.units.contains(n) && g.unit_moves_left(n) == moves {
                break;
            }
        }
    }
}

/// The plot fresh summon `n` attacks (C++ `AI_summonAttackMove`: `AI_anyAttack(baseMoves(), 5)`):
/// of the plots with visible enemies next to a plot it reaches with moves left this turn, the one
/// where its odds are best, at 5% or better (a city counts 10% more, as in
/// `units::attack_adjacent`). A plot holding only temporary summons is left alone, as in FfH:
/// they are gone before they could act again. FfH also walks toward targets up to `baseMoves()`
/// turns away; a summon gone at the start of our next turn only attacks what it reaches now.
fn summon_target(g: &Game, n: UnitId) -> Option<PlotIdx> {
    let owner = g.units[n].owner;
    let reach = crate::reach::Flood::default().run(g, n, 0, false);
    let mut seen: Vec<PlotIdx> = Vec::new();
    let mut best: Option<(PlotIdx, i32)> = None;
    for r in reach.iter().filter(|r| r.turns == 0) {
        let (x, y) = g.map.xy(r.plot);
        for dir in 0..DirectionType::NUM {
            let Some(q) = g.plot_direction(x, y, DirectionType::from_usize(dir)) else { continue };
            if seen.contains(&q) {
                continue;
            }
            seen.push(q);
            if !g.plot_is_visible_enemy_unit_for_unit(q, n) || !g.unit_can_move_or_attack_into(n, q, false) {
                continue;
            }
            if g.map.plots[q].units.iter().all(|&e| g.units[e].duration > 0) {
                continue;
            }
            let Some(def) = g.plot_get_best_defender(q, None, Some(owner), Some(n), false, false, false) else { continue };
            let odds = g.get_combat_odds(n, def);
            let score = odds + if g.map.plots[q].plot_city.is_some() { 100 } else { 0 };
            if odds >= 50 && best.map_or(true, |(_, s)| score > s) {
                best = Some((q, score));
            }
        }
    }
    best.map(|(q, _)| q)
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
    for spell in d.spells.keys() {
        if !g.unit_can_cast(u, spell, false) || !crate::world_spells::ready(g, g.units[u].owner, spell) {
            continue;
        }
        // Civilization-specific values (civs/: e.g. Luchuirp units in our cities leave a Golden
        // Hammer to become an Engineer).
        let value = crate::civs::spell_value(g, u, spell, ffh_value(g, u, spell));
        if value > best_value {
            best_value = value;
            best = Some(spell);
        }
    }
    best
}

/// `chooseSpell`'s value of one castable spell (before the civilization's say).
pub fn ffh_value(g: &Game, u: UnitId, spell: SpellType) -> i32 {
    let d = g.d;
    let here = g.unit_plot(u);
    let team = g.unit_get_team(u);
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
    value + info.ai_weight
}

/// `chooseSpell`'s terms for what a spell costs or gives besides its effect on units (for
/// `magic::turn_value`): a unit conversion, a feature or improvement, a push, population, gold,
/// the caster's sacrifice.
pub fn ffh_misc_value(g: &Game, u: UnitId, spell: SpellType) -> i32 {
    let d = g.d;
    let info = &d.spells[spell];
    let range = info.range;
    let mut value = 0;
    if let Some(cu) = info.convert_unit_type {
        value += unit_value(d, cu) - unit_value(d, g.units[u].unit_type);
    }
    if info.create_feature_type.is_some() {
        value += 10;
    }
    if info.create_improvement_type.is_some() {
        value += 10;
    }
    if info.push {
        value += 20 * (range + 1) * (range + 1);
        if g.map.plots[g.unit_plot(u)].plot_city.is_some() {
            value *= 3;
        }
    }
    if info.change_population != 0 {
        value += 50 * info.change_population;
    }
    if info.cost != 0 {
        value -= 4 * info.cost;
    }
    if info.sacrifice_caster {
        value -= g.units[u].level * unit_value(d, g.units[u].unit_type);
    }
    value
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
        cast_and_act(g, v, u, s, "buff", 0);
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
        cast_and_act(g, v, u, s, "summon", 0);
    }
}
