//! Spellcasting (item `magic`): what a spell does to the units it reaches, read from the spell
//! data, and what that is worth where the caster stands. FfH's own AI (`casting.rs`, ported term
//! by term) casts damage spells and summons only at the end of the turn when an enemy happens to
//! be in reach, and otherwise the buff with the highest weight: its casters spent their turns on
//! Haste and Regeneration (docs/AI_LOG.md, "Casters through a Mage Guild"). Here a spell is valued
//! by its effect on the units it reaches:
//!
//! - **Before an attack** ([`before_attack`], from `campaign::strike` and `military::assault`):
//!   every caster of ours within two plots of the target casts what helps the attack most, one
//!   cast at a time, the best of all casters first and every value recomputed after each cast:
//!   what weakens the defenders (damage, curses, stolen weapons and blessings), then summons,
//!   which attack the target at once, then blessings on the units about to attack. FfH's AI does
//!   the same in `CvSelectionGroup::groupAttack` ("BETTER AI (Groupattack Spellcasting)", left to
//!   the AI by the rules port).
//! - **At the end of the turn** ([`choose_turn_spell`], from `casting::end_turn`): damage and
//!   curses on the enemies in reach, summons that attack what stands next to us (as FfH's do,
//!   `casting::summons_attack`), lasting blessings on our units that lack them (and short ones by
//!   the defense they add through the enemies' turn), healing, the buildings city spells create;
//!   when nothing is worth casting, FfH's own choice (`casting::end_turn`).
//! - **Movement** ([`caster_target`], from `units::caster`): a caster that knows a spell for a
//!   fight goes with the stack of our operation nearest its target; the others go where their
//!   lasting blessings, city spells or terraforming are worth most, and one that keeps a city
//!   spell's building (`bRequiresCaster`) stays.
//! - **Promotions** ([`promotion_value`], from `units::promotion_value`): a sphere is worth the
//!   spells it opens, and half of those the next promotion of the sphere opens (Fire I for
//!   Fire II's Fireball).
//!
//! It trains no casters: they come from FfH's own sources (see "Casters to train" below).
//!
//! Values are hit points: those a spell takes from enemy units (a change of strength counts as
//! that share of the unit's health), less those it costs ours. FfH's own terms stay for what is
//! not modelled here ([`understood`]: world spells, abilities such as equipment, and spells that
//! live in Python). `FFH_AI_MAGIC_OFF` turns all of this off for some civilizations
//! ([`enabled`]), the measurement switch.

use std::sync::OnceLock;

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::utils::{combat_odds, CombatOddsInput};
use ffh_data::*;

use crate::view::{attack_power, View};

// ----------------------------------------------------------------------------
// Switch and trace
// ----------------------------------------------------------------------------

fn off_table(d: &GameData) -> &'static [bool] {
    static TABLE: OnceLock<Vec<bool>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let off: Vec<String> = std::env::var("FFH_AI_MAGIC_OFF")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().trim_start_matches("CIVILIZATION_").to_ascii_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        d.civilizations
            .keys()
            .map(|c| {
                let name = d.civilizations[c].base.type_.trim_start_matches("CIVILIZATION_");
                off.iter().any(|o| o == "ALL" || o == name)
            })
            .collect()
    })
}

/// False when the player casts as FfH's AI did (`casting.rs` as ported, nothing before attacks,
/// casters moved and promoted as before): the barbarians, and the civilizations listed in
/// `FFH_AI_MAGIC_OFF` (`CIVILIZATION_AMURITES` or `AMURITES`, comma-separated, or `all`). A
/// measurement switch like `FFH_AI_CIV_OFF`: read once, so play stays deterministic, and the run
/// cache keys runs by every `FFH_*` variable.
pub fn enabled(g: &Game, p: PlayerId) -> bool {
    !g.player_is_barbarian(p) && g.player_get_civilization_type(p).is_some_and(|c| !off_table(g.d)[c.idx()])
}

/// Every AI cast goes through here: `FFH_AI_TRACE_CAST=1` prints one line per cast (turn,
/// player, civilization, unit, spell, where, value) to stderr. It only prints.
pub fn cast(g: &mut Game, u: UnitId, spell: SpellType, ctx: &str, value: i32) {
    static ON: OnceLock<bool> = OnceLock::new();
    if *ON.get_or_init(|| std::env::var("FFH_AI_TRACE_CAST").is_ok()) {
        let p = g.units[u].owner;
        let civ = g.player_get_civilization_type(p).map_or("NONE", |c| g.d.civilizations[c].base.type_.as_str());
        eprintln!(
            "[cast t{} p{}] civ={} unit={} spell={} ctx={} value={}",
            g.game_turn(),
            p.0,
            civ.trim_start_matches("CIVILIZATION_"),
            g.d.units[g.units[u].unit_type].base.type_.trim_start_matches("UNIT_"),
            g.d.spells[spell].base.type_.trim_start_matches("SPELL_"),
            ctx,
            value
        );
    }
    g.unit_cast(u, spell);
}

// ----------------------------------------------------------------------------
// What a spell does
// ----------------------------------------------------------------------------

/// Spells whose effect is all in Python, valued here by what that Python does.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Py {
    None,
    /// `spellCrush` / `spellPillarofFire`: 50 damage (limit 75) on every unit of the plot within
    /// two that holds most enemies and no one else.
    Crush,
    PillarOfFire,
    /// `spellDestroyUndead`: 30 holy damage (no limit) on every undead unit within one.
    DestroyUndead,
    /// `spellDomination`: the best enemy unit within one changes sides unless it resists (the AI
    /// may only try units resisting 20% or less).
    Domination,
    /// `spellHeal(caster, 20)`: 20 hit points to every living unit on the plot.
    Heal,
    /// `spellEarthquake`: a quarter of the buildings of every city within one, fortification lost.
    Earthquake,
    /// `spellSpring`, `spellScorch`, `spellVitalize`: the caster's plot gets a better terrain
    /// ([`terraform`]).
    Spring,
    Scorch,
    Vitalize,
}

fn py_kind(d: &GameData, s: SpellType) -> Py {
    static TABLE: OnceLock<Vec<Py>> = OnceLock::new();
    TABLE.get_or_init(|| {
        d.spells
            .keys()
            .map(|s| match d.spells[s].base.type_.as_str() {
                "SPELL_CRUSH" => Py::Crush,
                "SPELL_PILLAR_OF_FIRE" => Py::PillarOfFire,
                "SPELL_DESTROY_UNDEAD" => Py::DestroyUndead,
                "SPELL_DOMINATION" => Py::Domination,
                "SPELL_HEAL" => Py::Heal,
                "SPELL_EARTHQUAKE" => Py::Earthquake,
                "SPELL_SPRING" => Py::Spring,
                "SPELL_SCORCH" => Py::Scorch,
                "SPELL_VITALIZE" => Py::Vitalize,
                _ => Py::None,
            })
            .collect()
    })[s.idx()]
}

/// The spells this module values; the others keep FfH's terms (`casting::ffh_value`): world
/// spells (their timing is `world_spells.rs`), abilities (equipment, crews, cages, upgrades...),
/// and spells whose effect lives only in Python and is not one of [`Py`].
pub fn understood(d: &GameData, s: SpellType) -> bool {
    let i = &d.spells[s];
    if i.global || i.ability {
        return false;
    }
    i.damage != 0
        || i.create_unit_type.is_some()
        || i.add_promotion_type1.is_some()
        || i.remove_promotion_type1.is_some()
        || i.immobile_turns != 0
        || i.dispel
        || i.create_building_type.is_some()
        || py_kind(d, s) != Py::None
}

/// Units of type `t` are summoned for good by some spell (`bPermanentUnitCreate`: Raise
/// Skeleton's skeletons, Battering Rams, Flesh Golems, Tigers, Treants, the Kraken), not trained:
/// a strike spends them right after its collateral attackers (`campaign::strike`).
pub fn raised(d: &GameData, t: UnitType) -> bool {
    static TABLE: OnceLock<Vec<bool>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = vec![false; d.units.len()];
        for s in d.spells.iter() {
            if let (true, Some(ut)) = (s.permanent_unit_create, s.create_unit_type) {
                table[ut.idx()] = true;
            }
        }
        table
    })[t.idx()]
}

fn adds(info: &SpellInfo) -> impl Iterator<Item = PromotionType> {
    [info.add_promotion_type1, info.add_promotion_type2, info.add_promotion_type3].into_iter().flatten()
}

fn removes(info: &SpellInfo) -> impl Iterator<Item = PromotionType> {
    [info.remove_promotion_type1, info.remove_promotion_type2, info.remove_promotion_type3].into_iter().flatten()
}

fn square(g: &Game, x: i32, y: i32, r: i32) -> Vec<PlotIdx> {
    let mut out = Vec::new();
    for i in -r..=r {
        for j in -r..=r {
            if let Some(q) = g.plot_xy(x, y, i, j) {
                out.push(q);
            }
        }
    }
    out
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Side {
    Ours,
    Enemy,
    Neutral,
}

fn side(g: &Game, t: UnitId, team: TeamId) -> Side {
    if g.unit_get_team(t) == team {
        Side::Ours
    } else if g.unit_is_enemy(t, Some(team), None) {
        Side::Enemy
    } else {
        Side::Neutral
    }
}

/// Mean damage a hit of `dmg` (limit `limit`, type `dt`) by `caster` deals `t`. C++ `castDamage`
/// rolls `dmg / 2 + rand(dmg)` and `doDamage` then `rand(n) + rand(n)` with the caster's spell
/// damage bonus in `n` (both about `n - 1` on average), less the target's resistance -- twice
/// its defense strength, twice its level, a quarter of its city's defense, its resistance to
/// the damage type -- and never past the limit.
fn expected_damage(g: &Game, caster: UnitId, t: UnitId, dmg: i32, limit: i32, dt: Option<DamageType>) -> i32 {
    let unit = &g.units[t];
    let mut resist = g.unit_base_combat_str_defense(t) * 2 + unit.level * 2;
    if let Some(c) = g.map.plots[g.unit_plot(t)].plot_city {
        resist += g.city_defense_modifier(c, false) / 4;
    }
    if let Some(dt) = dt {
        resist += g.unit_get_damage_type_resist(t, dt);
    }
    if resist >= 100 {
        return 0;
    }
    let mut n = dmg;
    if dt != Some(DamageType::DAMAGE_PHYSICAL) {
        n += g.units[caster].spell_damage_modify;
    }
    ((n - 1).max(0) * (100 - resist) / 100).min(limit - unit.damage).max(0)
}

/// Strength change, in percent, that promotion `pr` makes to `t` attacking (`attack`) or
/// defending; `city`: the fight is for a city (its attack or defense bonus counts, and a unit
/// ignoring building defense gains those walls). Extra and damage-type strength count against
/// the unit's base strength, a first strike about 10%; a unit that may only defend loses its
/// whole attack.
fn promo_pct(g: &Game, t: UnitId, pr: PromotionType, attack: bool, city: Option<CityId>) -> i32 {
    let info = &g.d.promotions[pr];
    if attack && info.only_defensive {
        return -100;
    }
    let base = if attack { g.unit_base_combat_str(t) } else { g.unit_base_combat_str_defense(t) }.max(1);
    let mut pct = if attack { info.combat_percent } else { info.combat_percent_defense };
    let extra = if attack { info.extra_combat_str } else { info.extra_combat_defense };
    pct += (extra + info.damage_type_combat.iter().sum::<i32>()) * 100 / base;
    if let Some(c) = city {
        pct += if attack { info.city_attack_percent } else { info.city_defense_percent };
        if attack && info.ignore_building_defense {
            pct += g.city_defense_modifier(c, false) - g.city_defense_modifier(c, true);
        }
    }
    pct + info.first_strikes_change * 10 + info.chance_first_strikes_change * 5
}

/// Does the spell's promotion `pr` go to `t` (C++ `castAddPromotion`: units of a unit combat it
/// lists, that lack it)?
fn takes(g: &Game, t: UnitId, pr: PromotionType) -> bool {
    !g.units[t].has_promotion[pr] && g.unit_get_unit_combat_type(t).is_some_and(|uc| g.d.promotions[pr].unit_combat[uc])
}

fn hp(g: &Game, t: UnitId) -> i32 {
    g.unit_curr_hit_points(t).max(0)
}

/// Where a spell is cast, for its value.
#[derive(Copy, Clone)]
enum Ctx<'a> {
    /// Our `attackers` attack plot `tp` this turn.
    Attack { tp: PlotIdx, attackers: &'a [UnitId] },
    /// The end of our turn.
    Turn { v: &'a View },
}

/// Hit points an attacker's strength change of `pct` percent is worth in the strike: its share
/// of a typical fight (about half the defender's health changes hands).
const FIGHT_HP: i32 = 50;

/// What gaining `pr` is worth to our unit `t` at the end of the turn: a lasting promotion its
/// strength gain (defense for a garrison, the better of attack and defense for a field unit, a
/// half for one that combat removes, plus a tenth of its AI weight for what strength does not
/// show), one that expires soon its defense through the enemies' turn (Dance of Blades' first
/// strike; Haste's move is worth nothing then), healing on a hurt unit.
fn ours_worth(g: &Game, v: &View, t: UnitId, pr: PromotionType) -> i32 {
    let info = &g.d.promotions[pr];
    let here = g.unit_plot(t);
    let city = g.map.plots[here].plot_city;
    let garrison = matches!(g.units[t].ai_type, Some(UnitAIType::UNITAI_CITY_DEFENSE) | Some(UnitAIType::UNITAI_CITY_COUNTER));
    let def = promo_pct(g, t, pr, false, city);
    let damage = g.units[t].damage;
    if info.removed_when_healed && damage == 0 {
        return 0;
    }
    let heal = info.friendly_heal_change.max(info.neutral_heal_change).max(info.enemy_heal_change).max(info.same_tile_heal_change);
    let mut w = if heal > 0 { heal.min(damage) } else { 0 };
    if info.expire_chance > 0 {
        // Gone at the start of our next turn with this chance: what counts is the enemies' turn,
        // in full where an enemy we see can strike, a quarter elsewhere (those we do not see).
        let share = if v.danger_at(here) > 0 { 200 } else { 800 };
        w += def * hp(g, t) / share;
        return w;
    }
    let pct = if garrison { def } else { promo_pct(g, t, pr, true, None).max(def) };
    let mut lasting = pct + info.ai_weight / 10;
    if info.removed_by_combat {
        lasting /= 2;
    }
    w + lasting
}

/// The effects of `spell` cast by `caster` on the units it reaches -- damage, promotions given
/// and taken, immobility, dispel -- in hit points for us. `None`: it would hurt a third party (a
/// spell's damage on a neutral's land declares war, C++ `doDamage`).
fn reached_value(g: &Game, caster: UnitId, spell: SpellType, ctx: Ctx) -> Option<i32> {
    let d = g.d;
    let info = &d.spells[spell];
    let team = g.unit_get_team(caster);
    let here = g.unit_plot(caster);
    let (x, y) = g.map.xy(here);
    let mut value = 0;
    let city_at = |q: PlotIdx| g.map.plots[q].plot_city;
    if info.buff_caster_only {
        // The caster alone gains these (C++ `castAddPromotion`), whatever its unit combat.
        for pr in adds(info).filter(|&pr| !g.units[caster].has_promotion[pr]) {
            value += match ctx {
                Ctx::Turn { v } => ours_worth(g, v, caster, pr),
                Ctx::Attack { tp, attackers } if attackers.contains(&caster) => promo_pct(g, caster, pr, true, city_at(tp)) * FIGHT_HP / 100,
                Ctx::Attack { .. } => 0,
            };
        }
    }
    for q in square(g, x, y, info.range) {
        let own_plot = q == here;
        for &t in &g.map.plots[q].units {
            if g.units[t].death_delay || g.unit_is_immune_to_spell(t, caster, spell) {
                continue;
            }
            let s = side(g, t, team);
            // Enemy health counts in full on the target, else at a share: in a strike the other
            // enemies in reach matter less; at the end of the turn, those that can attack us more.
            let weight = match (ctx, s) {
                (Ctx::Attack { tp, .. }, Side::Enemy) => {
                    if q == tp {
                        100
                    } else {
                        40
                    }
                }
                (Ctx::Turn { .. }, Side::Enemy) => {
                    if attack_power(g, t) > 0 {
                        60
                    } else {
                        20
                    }
                }
                _ => 100,
            };
            // The chance the spell lands on an enemy (C++ `isResisted`: rand(100) <= chance).
            let land = if info.resistable && s == Side::Enemy { 100 - g.unit_get_resist_chance(t, caster, spell) } else { 100 };
            let mut tv = 0;
            if info.damage != 0 && !own_plot {
                let dmg = expected_damage(g, caster, t, info.damage, info.damage_limit, info.damage_type);
                match s {
                    Side::Ours => tv -= 2 * dmg,
                    Side::Enemy => tv += dmg * weight / 100,
                    Side::Neutral if dmg > 0 => return None,
                    Side::Neutral => {}
                }
            }
            for pr in adds(info) {
                if info.buff_caster_only || !takes(g, t, pr) {
                    continue;
                }
                let pinfo = &d.promotions[pr];
                match (s, ctx) {
                    (Side::Enemy, Ctx::Attack { tp, .. }) => {
                        // The defenders of the target fight with it.
                        if q == tp {
                            tv -= promo_pct(g, t, pr, false, city_at(q)) * hp(g, t) / 100;
                        }
                    }
                    (Side::Enemy, Ctx::Turn { .. }) => {
                        // An enemy in reach attacks with it (or, made to defend only, not at all).
                        if attack_power(g, t) > 0 {
                            tv -= promo_pct(g, t, pr, true, None) * hp(g, t) * weight / 10000;
                            let moves = g.d.units[g.units[t].unit_type].moves;
                            if pinfo.moves_change < 0 && moves + pinfo.moves_change <= 0 {
                                // Slowed to a standstill until it wears off.
                                tv += 30;
                            }
                        }
                    }
                    (Side::Ours, Ctx::Attack { attackers, tp }) => {
                        if attackers.contains(&t) {
                            tv += promo_pct(g, t, pr, true, city_at(tp)) * FIGHT_HP / 100;
                        }
                    }
                    (Side::Ours, Ctx::Turn { v }) => tv += ours_worth(g, v, t, pr),
                    _ => {}
                }
            }
            for pr in removes(info) {
                if info.buff_caster_only || !g.units[t].has_promotion[pr] {
                    continue;
                }
                match (s, ctx) {
                    (Side::Enemy, Ctx::Attack { tp, .. }) => {
                        if q == tp {
                            tv += promo_pct(g, t, pr, false, city_at(q)) * hp(g, t) / 100;
                        }
                    }
                    (Side::Enemy, Ctx::Turn { .. }) => {
                        if attack_power(g, t) > 0 {
                            tv += promo_pct(g, t, pr, true, None) * hp(g, t) * weight / 10000;
                        }
                    }
                    (Side::Ours, _) => {
                        // A curse lifted (Cure Disease, Haste's end of Slow), or a blessing lost.
                        let pct = promo_pct(g, t, pr, true, None).max(promo_pct(g, t, pr, false, None));
                        tv -= pct * hp(g, t) / 100;
                        if d.promotions[pr].moves_change < 0 {
                            tv += 10;
                        }
                    }
                    _ => {}
                }
            }
            if info.immobile_turns > 0 && !own_plot && g.units[t].immobile_timer == 0 {
                match (s, ctx) {
                    (Side::Enemy, Ctx::Turn { .. }) if attack_power(g, t) > 0 => tv += 25 * info.immobile_turns.min(2),
                    (Side::Enemy, Ctx::Attack { tp, .. }) if q != tp && attack_power(g, t) > 0 => tv += 10,
                    (Side::Ours, _) => tv -= 30,
                    _ => {}
                }
            }
            if info.dispel {
                for pr in d.promotions.keys() {
                    let pinfo = &d.promotions[pr];
                    if !g.units[t].has_promotion[pr] || !pinfo.dispellable {
                        continue;
                    }
                    match s {
                        Side::Enemy if pinfo.ai_weight > 0 => {
                            let attack = matches!(ctx, Ctx::Turn { .. });
                            tv += promo_pct(g, t, pr, attack, None).max(0) * hp(g, t) * weight / 10000 + 5;
                        }
                        Side::Ours if pinfo.ai_weight < 0 => tv -= promo_pct(g, t, pr, true, None).min(0) * hp(g, t) / 100,
                        _ => {}
                    }
                }
            }
            value += tv * land / 100;
        }
    }
    Some(value)
}

/// The value of a Python-only spell ([`Py`]), as [`reached_value`].
fn py_value(g: &Game, caster: UnitId, spell: SpellType, ctx: Ctx) -> Option<i32> {
    let d = g.d;
    let team = g.unit_get_team(caster);
    let here = g.unit_plot(caster);
    let (x, y) = g.map.xy(here);
    let target = match ctx {
        Ctx::Attack { tp, .. } => Some(tp),
        Ctx::Turn { .. } => None,
    };
    match py_kind(d, spell) {
        Py::None => Some(0),
        Py::Crush | Py::PillarOfFire => {
            // The plot within two with most enemies and no one else takes 50 (limit 75).
            let dt = if py_kind(d, spell) == Py::Crush { Some(DamageType::DAMAGE_PHYSICAL) } else { d.ty::<DamageType>("DAMAGE_FIRE") };
            let mut best = 0;
            for q in square(g, x, y, 2) {
                let units = &g.map.plots[q].units;
                if units.is_empty() || units.iter().any(|&t| side(g, t, team) != Side::Enemy) {
                    continue;
                }
                let w = if Some(q) == target || target.is_none() { 100 } else { 40 };
                let v: i32 = units.iter().map(|&t| expected_damage(g, caster, t, 50, 75, dt) * w / 100).sum();
                best = best.max(v);
            }
            Some(best)
        }
        Py::DestroyUndead => {
            let undead = d.t::<PromotionType>("PROMOTION_UNDEAD");
            let holy = d.ty::<DamageType>("DAMAGE_HOLY");
            let mut v = 0;
            for q in square(g, x, y, 1) {
                for &t in &g.map.plots[q].units {
                    if g.units[t].race != Some(undead) {
                        continue;
                    }
                    let dmg = expected_damage(g, caster, t, 30, 100, holy);
                    match side(g, t, team) {
                        Side::Ours => v -= 2 * dmg,
                        Side::Enemy => v += dmg,
                        Side::Neutral if dmg > 0 => return None,
                        Side::Neutral => {}
                    }
                }
            }
            Some(v)
        }
        Py::Domination => {
            // The unit it would take (C++ `spellDomination`'s choice), if it fails to resist.
            let mut best: Option<(i32, UnitId)> = None;
            for q in square(g, x, y, 1) {
                for &t in &g.map.plots[q].units {
                    if !g.unit_is_alive(t) || g.units[t].death_delay || side(g, t, team) != Side::Enemy {
                        continue;
                    }
                    let resist = g.unit_get_resist_chance(t, caster, spell);
                    if resist > 20 {
                        continue;
                    }
                    let value = g.unit_base_combat_str(t) * 10 + (100 - resist);
                    if best.map_or(true, |(b, _)| value > b) {
                        best = Some((value, t));
                    }
                }
            }
            Some(best.map_or(0, |(_, t)| {
                let resist = g.unit_get_resist_chance(t, caster, spell);
                // Theirs lost and ours gained (the caster loses Mind III when it is resisted).
                (hp(g, t) + attack_power(g, t) / 10) * (100 - resist) / 100
            }))
        }
        Py::Heal => {
            let v: i32 = g.map.plots[here]
                .units
                .iter()
                .filter(|&&t| side(g, t, team) == Side::Ours && g.unit_is_alive(t))
                .map(|&t| g.units[t].damage.min(20))
                .sum();
            Some(match ctx {
                Ctx::Attack { attackers, .. } => {
                    g.map.plots[here].units.iter().filter(|&&t| attackers.contains(&t) && g.unit_is_alive(t)).map(|&t| g.units[t].damage.min(20)).sum::<i32>()
                        / 2
                }
                Ctx::Turn { .. } => v / 2,
            })
        }
        Py::Spring | Py::Scorch | Py::Vitalize => Some(match target {
            Some(_) => 0,
            None => terraform_value(g, g.units[caster].owner, spell, here),
        }),
        Py::Earthquake => match target {
            // Walls and fortification of the city we strike.
            Some(tp) if g.map.plots[tp].plot_city.is_some() => {
                let n = g.map.plots[tp].units.iter().filter(|&&t| side(g, t, team) == Side::Enemy).count() as i32;
                Some(15 * n)
            }
            _ => Some(0),
        },
    }
}

/// The terrain a terraforming spell makes of our plot `q` when a unit of `p` casts it there (the
/// Python's change, under its AI requirement: our own land): Spring a desert (not flood plains)
/// into plains, Scorch snow into tundra (not for the Doviello and the Illians, who like snow),
/// Vitalize snow into tundra, tundra and desert into plains, plains into grassland.
fn terraform(g: &Game, p: PlayerId, s: SpellType, q: PlotIdx) -> Option<TerrainType> {
    let d = g.d;
    let plot = &g.map.plots[q];
    if plot.owner != Some(p) || g.plot_is_water(q) {
        return None;
    }
    let t = |name: &str| d.ty::<TerrainType>(name);
    let terrain = plot.terrain_type;
    match py_kind(d, s) {
        Py::Spring if terrain == t("TERRAIN_DESERT") && plot.feature_type != d.ty::<FeatureType>("FEATURE_FLOOD_PLAINS") => t("TERRAIN_PLAINS"),
        Py::Scorch if terrain == t("TERRAIN_SNOW") => {
            let civ = g.player_get_civilization_type(p).map(|c| d.civilizations[c].base.type_.as_str());
            if matches!(civ, Some("CIVILIZATION_DOVIELLO") | Some("CIVILIZATION_ILLIANS")) {
                None
            } else {
                t("TERRAIN_TUNDRA")
            }
        }
        Py::Vitalize => {
            if terrain == t("TERRAIN_SNOW") {
                t("TERRAIN_TUNDRA")
            } else if terrain == t("TERRAIN_TUNDRA") || terrain == t("TERRAIN_DESERT") {
                t("TERRAIN_PLAINS")
            } else if terrain == t("TERRAIN_PLAINS") {
                t("TERRAIN_GRASS")
            } else {
                None
            }
        }
        _ => None,
    }
}

/// What terraforming our plot `q` with `s` is worth: the terrain's yield gain, a turn's worth of
/// it on the scale of `production::building_value` (a quarter of it, as for a city spell), for a
/// plot one of our cities can work (in full if it is worked).
fn terraform_value(g: &Game, p: PlayerId, s: SpellType, q: PlotIdx) -> i32 {
    let Some(to) = terraform(g, p, s, q) else { return 0 };
    let plot = &g.map.plots[q];
    let Some(from) = plot.terrain_type else { return 0 };
    if plot.city_radius_count == 0 {
        return 0;
    }
    let d = g.d;
    let weights = [200, 150, 70];
    let gain: i32 = (0..YieldType::NUM).map(|i| (d.terrains[to].yield_[i] - d.terrains[from].yield_[i]) * weights[i]).sum();
    let gain = gain / 4;
    if g.plot_is_being_worked(q) {
        gain
    } else {
        gain * 3 / 5
    }
}

// ----------------------------------------------------------------------------
// Summons
// ----------------------------------------------------------------------------

/// Hit points a fresh summon of type `ut` of player `p` takes from the defenders of `tp` when it
/// attacks at once (summons cast by the AI attack, C++ `AI_summonAttackMove`): the fight with
/// the defender the rules put up -- its health if the summon wins, else the rounds the summon wins
/// before it falls -- and its collateral damage on the others (C++ `collateralCombat`).
fn summon_strike(g: &Game, p: PlayerId, ut: UnitType, tp: PlotIdx) -> i32 {
    summon_fight(g, p, ut, tp).map_or(0, |(_, v)| v)
}

/// [`summon_strike`] with the summon's odds of winning (of 1000) by the same estimate; `None`
/// when it cannot fight there.
fn summon_fight(g: &Game, p: PlayerId, ut: UnitType, tp: PlotIdx) -> Option<(i32, i32)> {
    let d = g.d;
    let info = &d.units[ut];
    let a = (info.combat + info.damage_type_combat.iter().sum::<i32>()) * 100;
    if a <= 0 || info.only_defensive {
        return None;
    }
    let team = g.players[p].team;
    let defs: Vec<UnitId> = g.map.plots[tp]
        .units
        .iter()
        .copied()
        .filter(|&e| g.unit_is_enemy(e, Some(team), None) && g.unit_can_defend(e, Some(tp)) && !g.unit_is_invisible(e, team, false))
        .collect();
    if defs.is_empty() {
        return None;
    }
    let max_hp = d.defines.max_hit_points.max(1);
    let strength = |e: UnitId| g.unit_max_combat_str(e, Some(tp), None, None);
    let best = *defs.iter().max_by_key(|&&e| (strength(e) as i64 * hp(g, e) as i64, std::cmp::Reverse(e))).unwrap();
    let d_max = strength(best).max(1);
    let d_hp = hp(g, best).max(1);
    let d_str = (d_max * d_hp / max_hp).max(1);
    let d_fp = (d_max + d_str + 1) / 2;
    let k = CombatOddsInput {
        attacker_strength: a,
        attacker_firepower: a,
        defender_strength: d_str,
        defender_firepower: d_fp,
        attacker_hit_points: max_hp,
        defender_hit_points: d_hp,
        defender_max_hit_points: max_hp,
        attacker_combat_limit: info.combat_limit,
        attacker_first_strikes: info.first_strikes,
        attacker_chance_first_strikes: info.chance_first_strikes,
        attacker_immune_to_first_strikes: info.first_strike_immune,
        defender_first_strikes: g.unit_first_strikes(best),
        defender_chance_first_strikes: g.unit_chance_first_strikes(best),
        defender_immune_to_first_strikes: g.unit_immune_to_first_strikes(best),
    };
    let defines = &d.defines;
    let odds = combat_odds(&k, defines.combat_die_sides, defines.combat_damage) as i64;
    // A lost fight: the rounds won before the summon's hit points are gone (as `campaign::fight`).
    let sf = (a + d_fp + 1) / 2;
    let to_att = (defines.combat_damage * (d_fp + sf) / (a + sf)).max(1);
    let to_def = (defines.combat_damage * (a + sf) / (d_fp + sf)).max(1);
    let losses = (max_hp + to_att - 1) / to_att;
    let q = a as i64 * 1000 / (a + d_str) as i64;
    let won = losses as i64 * q / (1000 - q).max(50);
    let mut wound = (won * to_def as i64).min(d_hp as i64);
    if info.combat_limit < max_hp {
        wound = wound.min((d_hp - (max_hp - info.combat_limit)).max(0) as i64);
    }
    let mut v = (odds * d_hp as i64 + (1000 - odds) * wound) / 1000;
    if info.collateral_damage > 0 && defs.len() > 1 {
        let cs = a / 100 * info.collateral_damage / 100;
        let limit = info.collateral_damage_limit * max_hp / 100;
        let mut others: Vec<UnitId> = defs.iter().copied().filter(|&o| o != best).collect();
        others.sort_by_key(|&o| (std::cmp::Reverse(hp(g, o)), o));
        for &o in others.iter().take(info.collateral_damage_max_units.max(0) as usize) {
            let ts = g.unit_base_combat_str(o);
            let s2 = (cs + ts + 1) / 2;
            if ts + s2 <= 0 {
                continue;
            }
            let dmg = defines.collateral_combat_damage * (cs + s2) / (ts + s2);
            let cap = limit.min(limit * (cs + s2) / (ts + s2));
            v += dmg.min(cap - g.units[o].damage).max(0) as i64;
        }
    }
    Some((odds as i32, v as i32))
}

/// Value of the summons of `spell` cast by `caster` (temporary summons attack at once; a lasting
/// summon is an army unit, worth having while there is fighting to do).
fn summon_value(g: &Game, caster: UnitId, spell: SpellType, ctx: Ctx) -> i32 {
    let d = g.d;
    let info = &d.spells[spell];
    let Some(ut) = info.create_unit_type else { return 0 };
    let p = g.units[caster].owner;
    let n = info.create_unit_num * if g.unit_is_twincast(caster) { 2 } else { 1 };
    let uinfo = &d.units[ut];
    let strength = uinfo.combat + uinfo.damage_type_combat.iter().sum::<i32>();
    let here = g.unit_plot(caster);
    let (x, y) = g.map.xy(here);
    match ctx {
        Ctx::Attack { tp, .. } => {
            // Only next to the target (it appears on the caster's plot), and a lasting summon
            // cannot move for two turns.
            let (tx, ty) = g.map.xy(tp);
            if info.permanent_unit_create || g.plot_distance(x, y, tx, ty) != 1 {
                return 0;
            }
            summon_strike(g, p, ut, tp) * n
        }
        Ctx::Turn { v } => {
            if info.permanent_unit_create {
                // A unit for good (it pays support like any other): while at war or with enemies
                // about.
                let near = v.danger_at(here) > 0 || v.at_war();
                return if near { strength * 10 * n } else { 0 };
            }
            // It attacks what `casting::summons_attack` picks: of the plots with enemies next to a
            // plot it reaches with moves left (its own moves, the caster's way of moving), the one
            // where its odds are best, at 5% or better; nothing when none is.
            let moves = (uinfo.moves + g.unit_get_extra_spell_move(caster)) * d.defines.move_denominator;
            let reach = crate::reach::Flood::default().run_moves(g, caster, moves, 0, 0, false);
            let mut seen: Vec<PlotIdx> = Vec::new();
            let mut best: Option<(i32, i32)> = None;
            for r in reach.iter().filter(|r| r.turns == 0) {
                let (rx, ry) = g.map.xy(r.plot);
                for dir in 0..DirectionType::NUM {
                    let Some(q) = g.plot_direction(rx, ry, DirectionType::from_usize(dir)) else { continue };
                    if seen.contains(&q) {
                        continue;
                    }
                    seen.push(q);
                    if g.plot_is_water(q) != (uinfo.domain_type == Some(DomainType::DOMAIN_SEA)) {
                        continue;
                    }
                    let units = &g.map.plots[q].units;
                    if !units.iter().any(|&t| side(g, t, v.team) == Side::Enemy) || units.iter().all(|&t| g.units[t].duration > 0) {
                        continue;
                    }
                    let Some((odds, value)) = summon_fight(g, p, ut, q) else { continue };
                    let score = odds + if g.map.plots[q].plot_city.is_some() { 100 } else { 0 };
                    if odds >= 50 && best.map_or(true, |(s, _)| score > s) {
                        best = Some((score, value));
                    }
                }
            }
            best.map_or(0, |(_, value)| value) * 4 / 5 * n
        }
    }
}

// ----------------------------------------------------------------------------
// Values
// ----------------------------------------------------------------------------

/// What `spell` cast by `caster` is worth just before our `attackers` attack `tp`, in hit points
/// (`i32::MIN / 2` when it would hurt a third party).
pub fn attack_value(g: &Game, caster: UnitId, spell: SpellType, tp: PlotIdx, attackers: &[UnitId]) -> i32 {
    let ctx = Ctx::Attack { tp, attackers };
    let (Some(a), Some(b)) = (reached_value(g, caster, spell, ctx), py_value(g, caster, spell, ctx)) else { return i32::MIN / 2 };
    a + b + summon_value(g, caster, spell, ctx)
}

/// What `spell` cast by `u` at the end of the turn is worth (see the module doc), `None` for a
/// spell FfH's terms value ([`understood`]). Besides the effects: FfH's own terms for gold,
/// conversion, population, sacrifice, features, and the spell's AI weight (FfH's explicit
/// preferences, such as the Balseraphs' puppets).
pub fn turn_value(g: &Game, v: &View, u: UnitId, spell: SpellType) -> Option<i32> {
    let d = g.d;
    if !understood(d, spell) {
        return None;
    }
    let info = &d.spells[spell];
    let ctx = Ctx::Turn { v };
    let (Some(a), Some(b)) = (reached_value(g, u, spell, ctx), py_value(g, u, spell, ctx)) else { return Some(i32::MIN / 2) };
    let mut value = a + b + summon_value(g, u, spell, ctx);
    let here = g.unit_plot(u);
    if let Some(b) = info.create_building_type {
        if let Some(c) = g.map.plots[here].plot_city {
            value += crate::production::building_value(g, c, b, 0) / 4;
        }
    }
    value += crate::casting::ffh_misc_value(g, u, spell);
    Some(value + info.ai_weight)
}

/// The spell `u` casts at the end of the turn: the best by [`turn_value`] (FfH's terms for the
/// spells it leaves alone), after the world spells' timing and the civilization's say.
pub fn choose_turn_spell(g: &Game, v: &View, u: UnitId) -> Option<(SpellType, i32)> {
    let d = g.d;
    let mut best: Option<(SpellType, i32)> = None;
    for s in d.spells.keys() {
        if !g.unit_can_cast(u, s, false) || !crate::world_spells::ready(g, g.units[u].owner, s) {
            continue;
        }
        let value = turn_value(g, v, u, s).unwrap_or_else(|| crate::casting::ffh_value(g, u, s));
        let value = crate::civs::spell_value(g, u, s, value);
        if value > best.map_or(0, |(_, b)| b) {
            best = Some((s, value));
        }
    }
    best
}

// ----------------------------------------------------------------------------
// Casting before an attack
// ----------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Phase {
    /// Damage, curses, dispels: the defenders get weaker.
    Soften,
    /// Summons attack the weakened defenders.
    Summon,
    /// Blessings on the units about to attack.
    Bless,
}

fn phase(d: &GameData, s: SpellType) -> Phase {
    let i = &d.spells[s];
    if i.create_unit_type.is_some() {
        Phase::Summon
    } else if i.damage == 0 && py_kind(d, s) == Py::None && i.add_promotion_type1.is_some_and(|p| d.promotions[p].ai_weight > 0) {
        Phase::Bless
    } else {
        Phase::Soften
    }
}

/// A cast before an attack must be worth this many hit points.
const MIN_ATTACK_VALUE: i32 = 5;

/// Casting before player `v.p`'s units attack `tp` (see the module doc): [`soften`], then
/// [`bless`]. For an attack that is going ahead.
pub fn before_attack(g: &mut Game, v: &View, tp: PlotIdx, attackers: &[UnitId]) {
    soften(g, v, tp, attackers);
    bless(g, v, tp, attackers);
}

/// The first casts before an attack on `tp`: what weakens its defenders, then summons, which
/// attack it at once (the player's summon-suicide mode is on meanwhile, as for FfH's end-of-turn
/// casting: they do not join the caster's stack). Worth casting before deciding whether to
/// attack: the decision weighs the defenders as they are left. `attackers`: our units that may
/// attack.
pub fn soften(g: &mut Game, v: &View, tp: PlotIdx, attackers: &[UnitId]) {
    cast_phases(g, v, tp, attackers, &[Phase::Soften, Phase::Summon]);
}

/// The blessings on `attackers` before they attack `tp` (only once the attack goes ahead: a
/// blessing that wears off by our next turn is lost otherwise).
pub fn bless(g: &mut Game, v: &View, tp: PlotIdx, attackers: &[UnitId]) {
    cast_phases(g, v, tp, attackers, &[Phase::Bless]);
}

/// The casts of `phases` (see [`before_attack`]). A unit casts once a turn, so a caster casts
/// here only what is worth more than what it would cast at the end of the turn (its best value
/// now by [`choose_turn_spell`]: the Balseraphs' Loki keeps his cast for the puppet FfH's weight
/// asks for every turn).
fn cast_phases(g: &mut Game, v: &View, tp: PlotIdx, attackers: &[UnitId], phases: &[Phase]) {
    let p = v.p;
    if !enabled(g, p) || !g.map.plots[tp].units.iter().any(|&t| g.unit_is_enemy(t, Some(g.players[p].team), None)) {
        return;
    }
    let d = g.d;
    let (tx, ty) = g.map.xy(tp);
    let mut casters: Vec<UnitId> = square(g, tx, ty, 2)
        .into_iter()
        .flat_map(|q| g.map.plots[q].units.iter().copied().filter(|&u| g.units[u].owner == p).collect::<Vec<_>>())
        .collect();
    casters.sort();
    casters.dedup();
    // The spells each could cast at all (re-checked before every cast), and what it would cast
    // at the end of the turn instead.
    let mut book: Vec<(UnitId, Vec<SpellType>, i32)> = casters
        .into_iter()
        .filter(|&u| !g.units[u].has_casted && !g.units[u].death_delay && !g.unit_is_cargo(u))
        .map(|u| (u, d.spells.keys().filter(|&s| d.spells[s].allow_ai && !d.spells[s].global && g.unit_can_cast(u, s, false)).collect::<Vec<_>>()))
        .filter(|(_, ss)| !ss.is_empty())
        .map(|(u, ss)| {
            let reserve = choose_turn_spell(g, v, u).map_or(0, |(_, val)| val);
            (u, ss, reserve)
        })
        .collect();
    if book.is_empty() {
        return;
    }
    let was = g.players[p].sum_sui_mode;
    g.players[p].sum_sui_mode = true;
    for &ph in phases {
        for _ in 0..book.len() {
            let mut best: Option<(i32, UnitId, SpellType)> = None;
            for (u, spells, reserve) in &book {
                let u = *u;
                if !g.units.contains(u) || g.units[u].has_casted {
                    continue;
                }
                for &s in spells {
                    if phase(d, s) != ph || !g.unit_can_cast(u, s, false) {
                        continue;
                    }
                    let value = attack_value(g, u, s, tp, attackers);
                    if value >= MIN_ATTACK_VALUE.max(*reserve) && best.map_or(true, |(b, bu, _)| value > b || value == b && u < bu) {
                        best = Some((value, u, s));
                    }
                }
            }
            let Some((value, u, s)) = best else { break };
            let newest = g.players[p].units.iter().copied().max();
            cast(g, u, s, "attack", value);
            if ph == Phase::Summon {
                summons_attack(g, p, newest, tp);
            }
            book.retain(|(c, _, _)| g.units.contains(*c));
        }
        if !g.map.plots[tp].units.iter().any(|&t| g.unit_is_enemy(t, Some(g.players[p].team), None)) {
            break;
        }
    }
    g.players[p].sum_sui_mode = was;
}

/// The summons player `p` just made (unit ids are never reused: those above `newest`) attack
/// `tp` while they can, each out of its group (a summon joins its caster's group).
fn summons_attack(g: &mut Game, p: PlayerId, newest: Option<UnitId>, tp: PlotIdx) {
    let (tx, ty) = g.map.xy(tp);
    let summons: Vec<UnitId> = g.players[p].units.iter().copied().filter(|&n| newest.map_or(true, |m| n > m) && g.units[n].ai_suicide_summon).collect();
    for n in summons {
        for _ in 0..4 {
            if !g.units.contains(n) || g.unit_moves_left(n) <= 0 || !g.plot_is_visible_enemy_unit_for_unit(tp, n) {
                break;
            }
            let (x, y) = (g.units[n].x, g.units[n].y);
            if g.plot_distance(x, y, tx, ty) != 1 || !g.unit_can_move_or_attack_into(n, tp, false) {
                break;
            }
            if g.units[n].group.is_some_and(|grp| g.groups[grp].units.len() > 1) {
                g.unit_join_group(n, None, true);
            }
            let Some(grp) = g.units[n].group else { break };
            let moves = g.unit_moves_left(n);
            crate::units::move_to(g, grp, tp, MOVE_DIRECT_ATTACK);
            if g.units.contains(n) && g.unit_moves_left(n) == moves && !g.units[n].made_attack {
                break;
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Where a caster goes
// ----------------------------------------------------------------------------

/// The spells `u` holds the promotions for and may cast at all (its unit, class, unit combat,
/// civilization and religion fit), world spells and abilities aside.
fn known(g: &Game, u: UnitId) -> Vec<SpellType> {
    let d = g.d;
    let unit = &g.units[u];
    d.spells
        .keys()
        .filter(|&s| {
            let i = &d.spells[s];
            !i.global && !i.ability && [i.promotion_prereq1, i.promotion_prereq2].into_iter().flatten().all(|q| unit.has_promotion[q]) && could_cast(g, u, s)
        })
        .collect()
}

/// Where caster `u` of player `v.p` should go this turn (`units::caster`), and why: with the
/// stack of our operation nearest its target when it knows a spell for a fight and the operation
/// is under way (in the rally city while the force for a war gathers), so it stands next to the
/// city when the strike comes; else where its lasting blessings (on the units there that lack
/// them) or its city spells (a building the city lacks) are worth most for the turns it takes to
/// get there, within 8 turns. `None`: nothing to do there (it guards the most threatened city).
pub fn caster_target(g: &Game, v: &mut View, u: UnitId) -> Option<(PlotIdx, &'static str)> {
    let d = g.d;
    let p = v.p;
    if combat_caster(g, u) {
        for op in &v.campaign.ops {
            if !g.cities.contains(op.city) || g.cities[op.city].owner == p {
                continue;
            }
            if op.phase == crate::campaign::Phase::Gather {
                if op.civ {
                    if let Some(r) = op.rally {
                        return Some((r, "rally"));
                    }
                }
                continue;
            }
            let (tx, ty) = g.map.xy(op.plot);
            let front = op
                .groups
                .iter()
                .copied()
                .filter(|&grp| crate::units::alive(g, grp))
                .filter_map(|grp| g.group_get_head_unit(grp).map(|h| (grp, g.unit_plot(h))))
                .min_by_key(|&(grp, q)| {
                    let (x, y) = g.map.xy(q);
                    (g.plot_distance(x, y, tx, ty), std::cmp::Reverse(g.groups[grp].units.len()), grp)
                });
            if let Some((_, q)) = front {
                return Some((q, "strike"));
            }
        }
    }
    let spells: Vec<SpellType> = known(g, u)
        .into_iter()
        .filter(|&s| {
            let i = &d.spells[s];
            i.create_building_type.is_some()
                || matches!(py_kind(d, s), Py::Spring | Py::Scorch | Py::Vitalize)
                || !i.buff_caster_only && adds(i).any(|pr| d.promotions[pr].ai_weight > 0 && d.promotions[pr].expire_chance == 0 && !d.promotions[pr].removed_when_healed)
        })
        .collect();
    if spells.is_empty() {
        return None;
    }
    let here = g.unit_plot(u);
    // A building only a caster keeps (Inspiration, Wall of Stone, ...) goes when it leaves: the
    // caster that keeps one stays unless twice as much waits elsewhere.
    let keep = upkeep_value(g, u, &spells);
    let mut best: Option<(i32, PlotIdx, &'static str)> = (keep > 0).then_some((keep * 10, here, "keep"));
    let reach = v.flood.run(g, u, 8, false);
    for r in &reach {
        let q = r.plot;
        let units = &g.map.plots[q].units;
        // Our land to terraform (no one else heading there).
        if !v.is_claimed(q) {
            for &s in spells.iter().filter(|&&s| matches!(py_kind(d, s), Py::Spring | Py::Scorch | Py::Vitalize)) {
                let value = terraform_value(g, p, s, q);
                let score = value * 10 / (r.turns + 2);
                if value >= 15 && best.map_or(true, |(b, bq, _)| score > b || score == b && q < bq) {
                    best = Some((score, q, "terraform"));
                }
            }
        }
        if !units.iter().any(|&t| g.units[t].owner == p) {
            continue;
        }
        for &s in &spells {
            let info = &d.spells[s];
            if matches!(py_kind(d, s), Py::Spring | Py::Scorch | Py::Vitalize) {
                continue;
            }
            let mut value = 0;
            let mut why = "bless";
            if let Some(b) = info.create_building_type {
                if let Some(c) = g.map.plots[q].plot_city.filter(|&c| g.cities[c].owner == p && g.city_num_building(c, b) == 0 && !claimed_city_spell(g, u, c, s)) {
                    value += crate::production::building_value(g, c, b, 0) / 4;
                    why = "city spell";
                }
            }
            for pr in adds(info) {
                let pinfo = &d.promotions[pr];
                if pinfo.ai_weight <= 0 || pinfo.expire_chance != 0 || pinfo.removed_when_healed {
                    continue;
                }
                for &t in units {
                    if t != u && g.units[t].owner == p && takes(g, t, pr) && !g.unit_is_immune_to_spell(t, u, s) {
                        value += ours_worth(g, v, t, pr);
                    }
                }
            }
            if value < 15 {
                continue;
            }
            let score = value * 10 / (r.turns + 2);
            if best.map_or(true, |(b, bq, _)| score > b || score == b && q < bq) {
                best = Some((score, q, why));
            }
        }
    }
    if let Some((_, q, "terraform")) = best {
        v.claimed.push(q);
    }
    best.map(|(_, q, why)| (q, why))
}

/// What the buildings `u` alone keeps in our city it stands in are worth: those that need a
/// caster (C++ `CvCity::doTurn` removes a `bRequiresCaster` building when no unit in the city can
/// cast the spell that makes it) made by one of `spells` and that no other unit there could cast.
fn upkeep_value(g: &Game, u: UnitId, spells: &[SpellType]) -> i32 {
    let d = g.d;
    let here = g.unit_plot(u);
    let Some(c) = g.map.plots[here].plot_city.filter(|&c| g.cities[c].owner == g.units[u].owner) else { return 0 };
    let mut value = 0;
    for &s in spells {
        let Some(b) = d.spells[s].create_building_type.filter(|&b| d.buildings[b].requires_caster && g.city_num_building(c, b) > 0) else { continue };
        if !g.map.plots[here].units.iter().any(|&o| o != u && g.unit_can_cast(o, s, true)) {
            value += crate::production::building_value(g, c, b, 0) / 4;
        }
    }
    value
}

/// Is another caster of ours already in city `c` able to cast `s` (it will make the building,
/// or keeps it)?
fn claimed_city_spell(g: &Game, u: UnitId, c: CityId, s: SpellType) -> bool {
    let cp = g.city_plot(c);
    g.map.plots[cp].units.iter().any(|&o| o != u && g.units[o].owner == g.units[u].owner && g.unit_can_cast(o, s, true))
}

// ----------------------------------------------------------------------------
// What a caster can do (promotions)
// ----------------------------------------------------------------------------

/// Rough worth of spell `s` to a caster, on the scale of `units::promotion_value` (Combat I is
/// about 40): a summon by its strength (lasting ones less a bit, more with collateral damage),
/// damage by its size and reach, curses and blessings by the strength they change on the units
/// they reach (blessings on a stack, lasting ones more), Python spells by what they do, a city
/// spell like a good building.
pub fn spell_worth(d: &GameData, s: SpellType) -> i32 {
    let info = &d.spells[s];
    if !info.allow_ai || info.global || info.ability {
        return 0;
    }
    let reach = info.range + 1;
    let mut w = match py_kind(d, s) {
        Py::Crush | Py::PillarOfFire => 100,
        Py::Domination => 150,
        Py::DestroyUndead | Py::Heal => 30,
        Py::Earthquake | Py::Vitalize => 40,
        Py::Spring | Py::Scorch => 20,
        Py::None => 0,
    };
    if let Some(ut) = info.create_unit_type {
        let u = &d.units[ut];
        let strength = u.combat + u.damage_type_combat.iter().sum::<i32>();
        let mut sw = strength * if info.permanent_unit_create { 15 } else { 20 };
        if u.collateral_damage > 0 {
            sw = sw * 3 / 2;
        }
        w += (sw.max(5) * info.create_unit_num.max(1)).min(200);
    }
    w += info.damage * reach;
    for pr in adds(info) {
        let p = &d.promotions[pr];
        if p.ai_weight < 0 {
            let curse = (-p.combat_percent_defense).max(0)
                + (-p.extra_combat_defense).max(0) * 20
                + if p.only_defensive { 40 } else { 0 }
                + if p.moves_change < 0 { 30 } else { 0 };
            w += curse * reach;
        } else {
            let gain = p.combat_percent.max(p.combat_percent_defense)
                + (p.extra_combat_str.max(p.extra_combat_defense) + p.damage_type_combat.iter().sum::<i32>()) * 20
                + p.first_strikes_change * 10;
            let stack = if info.buff_caster_only { 1 } else { 3 };
            let lasting = if p.expire_chance == 0 && !p.removed_by_combat { 3 } else { 2 };
            w += gain * stack * lasting / 2 + p.ai_weight / 10;
        }
    }
    if info.immune_team {
        // Weapons and blessings taken from the enemy (Rust): a unit carries one tier of weapons,
        // so the best of them, at half (not every defender has them).
        let taken = removes(info).map(|pr| (d.promotions[pr].extra_combat_defense * 20 + d.promotions[pr].combat_percent_defense).max(0)).max().unwrap_or(0);
        w += taken * reach / 2;
    }
    w += info.immobile_turns * 20 * reach;
    if info.create_building_type.is_some() {
        w += 40;
    }
    if info.dispel {
        w += 30;
    }
    w
}

/// Can `u` itself ever cast `s` once it has the spell's promotions (its unit, class, unit
/// combat, civilization and religion fit)?
fn could_cast(g: &Game, u: UnitId, s: SpellType) -> bool {
    let info = &g.d.spells[s];
    let unit = &g.units[u];
    info.allow_ai
        && info.unit_prereq.map_or(true, |ut| ut == unit.unit_type)
        && info.unit_class_prereq.map_or(true, |c| c == g.unit_get_unit_class_type(u))
        && info.unit_combat_prereq.map_or(true, |c| Some(c) == g.unit_get_unit_combat_type(u))
        && info.civilization_prereq.map_or(true, |c| c == g.unit_get_civilization_type(u))
        && info.religion_prereq.map_or(true, |r| unit.religion == Some(r))
}

/// The best [`spell_worth`] promotion `pr` opens for `u` (its other promotion held or `pr`).
fn opens(g: &Game, u: UnitId, pr: PromotionType, has: &dyn Fn(PromotionType) -> bool) -> i32 {
    let d = g.d;
    d.spells
        .keys()
        .filter(|&s| {
            let i = &d.spells[s];
            (i.promotion_prereq1 == Some(pr) || i.promotion_prereq2 == Some(pr))
                && [i.promotion_prereq1, i.promotion_prereq2].into_iter().flatten().all(|q| q == pr || has(q))
                && could_cast(g, u, s)
        })
        .map(|s| spell_worth(d, s))
        .max()
        .unwrap_or(0)
}

/// What promotion `pr` is worth to `u` for its spells: the best spell it opens, and half the best
/// one a next promotion of the same line opens (Fire II after Fire I) when `u` has the other
/// promotion that one needs (Channeling II) -- a quarter when it has the channeling below it,
/// since an adept becomes a mage only through an upgrade. 0 for a unit that casts nothing.
pub fn promotion_value(g: &Game, u: UnitId, pr: PromotionType) -> i32 {
    let d = g.d;
    let unit = &g.units[u];
    let has = |q: PromotionType| unit.has_promotion[q];
    let now = opens(g, u, pr, &has);
    let mut later = 0;
    for next in d.promotions.keys() {
        let n = &d.promotions[next];
        if n.prereq_promotion != Some(pr) {
            continue;
        }
        let Some(and) = n.promotion_prereq_and else { continue };
        let factor = if has(and) {
            2
        } else if d.promotions[and].prereq_promotion.map_or(false, |below| has(below)) || channeling_below(d, and).is_some_and(|b| has(b)) {
            4
        } else {
            continue;
        };
        let with = |q: PromotionType| q == pr || has(q);
        later = later.max(opens(g, u, next, &with) / factor);
    }
    now + later
}

/// The channeling promotion below `and` (Channeling I below Channeling II), by name.
fn channeling_below(d: &GameData, and: PromotionType) -> Option<PromotionType> {
    match d.promotions[and].base.type_.as_str() {
        "PROMOTION_CHANNELING2" => d.ty::<PromotionType>("PROMOTION_CHANNELING1"),
        "PROMOTION_CHANNELING3" => d.ty::<PromotionType>("PROMOTION_CHANNELING2"),
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// Casters to train
// ----------------------------------------------------------------------------
//
// Tried, not kept (docs/AI_LOG.md, "Spellcasting"): training casters by what their spells are
// worth in a fight -- one per five field units when an adept could cast such a spell with our
// mana (Enchanted Blade, Rust), and the Mage Guild that opens them in our most productive safe
// city. The Enchantment civilizations that trained them lost wins (3 against 9 over 80 games)
// for no share: casters keep coming from FfH's own sources (heroes, priests, unique units, the
// Catacomb Libralus' guilds, `production` step 10, the Tower of Mastery's mana nodes).

/// Does `u` know a spell that helps in a fight (a summon that attacks, damage, a curse, a
/// blessing for a stack)? Such casters march with the army.
pub fn combat_caster(g: &Game, u: UnitId) -> bool {
    let d = g.d;
    let unit = &g.units[u];
    d.spells.keys().any(|s| {
        let i = &d.spells[s];
        !i.global
            && !i.ability
            && (i.damage > 0
                || (i.create_unit_type.is_some() && !i.permanent_unit_create)
                || matches!(py_kind(d, s), Py::Crush | Py::PillarOfFire | Py::Domination | Py::DestroyUndead)
                || adds(i).any(|p| d.promotions[p].ai_weight != 0 && !i.buff_caster_only))
            && [i.promotion_prereq1, i.promotion_prereq2].into_iter().flatten().all(|q| unit.has_promotion[q])
            && could_cast(g, u, s)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worth(d: &GameData, name: &str) -> i32 {
        spell_worth(d, d.t::<SpellType>(name))
    }

    #[test]
    fn spell_worth_ranks_combat_spells() {
        let d = ffh_data::global();
        if std::env::var("FFH_PRINT_SPELL_WORTH").is_ok() {
            let mut all: Vec<(i32, &str)> = d.spells.keys().map(|s| (spell_worth(d, s), d.spells[s].base.type_.as_str())).filter(|&(w, _)| w > 0).collect();
            all.sort();
            for (w, n) in all {
                eprintln!("{w:5} {n}");
            }
        }
        // A summon that attacks with collateral damage beats a one-turn move bonus; a stack
        // blessing that lasts beats one that does not; a curse that takes weapons is worth a
        // caster.
        assert!(worth(d, "SPELL_FIREBALL") > worth(d, "SPELL_HASTE"));
        assert!(worth(d, "SPELL_ENCHANTED_BLADE") > worth(d, "SPELL_DANCE_OF_BLADES"));
        assert!(worth(d, "SPELL_RUST") >= 60);
        assert!(worth(d, "SPELL_ENCHANTED_BLADE") >= 60);
        // World spells and abilities are not valued here.
        assert_eq!(worth(d, "SPELL_RALLY"), 0);
        assert_eq!(worth(d, "SPELL_TAKE_GOLDEN_HAMMER_UNIT"), 0);
    }
}
