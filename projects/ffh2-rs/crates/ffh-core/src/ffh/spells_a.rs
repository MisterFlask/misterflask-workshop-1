//! Port of CvSpellInterface.py, part a: Python lines 1-1843 -- the module's own
//! `findClearPlot`, the `postCombat*` unit callbacks and the spell callbacks from
//! `reqAddToFleshGolem` through `spellPillarofFire` (part b, `spells_b.rs`, continues with
//! `reqPirateCove`). The dispatchers at the top of the Python module (`cast`, `canCast`,
//! `effect`, `atRange`, `onMove`, `onMoveFeature`, `vote`, `miscast`, `postCombatLost`,
//! `postCombatWon`) are `spells.rs` + `dispatch.rs`. Each Python function is a private Rust
//! `fn` (its doc comment names the Python function) registered by its Python name in the
//! `find_*` lookups below.
//!
//! Porting notes:
//! - UI-only Python is omitted (`CyInterface().addMessage`, `CyEngine().triggerEffect`).
//! - `isHuman()` branches are ported as written (most of them restrict when an AI may cast).
//! - `for i in range(pPlot.getNumUnits()): pUnit = pPlot.getUnit(i)` reads the plot's *live*
//!   unit list by index, with the count fixed when the loop starts. Where the loop body can
//!   change that list (damage that kills a transport -- its cargo dies at once -- or makes an
//!   immortal unit rebirth, war declarations that bump units, units created on the plot) the
//!   port does the same (`plot_unit`); past the end of the list Python gets the NULL
//!   `CyUnit`, whose methods return defaults, and each such loop reproduces what that does.
//! - `PyPlayer(p).getUnitList()` is a snapshot of the player's units that are not dead
//!   (PyHelpers skips `isDead()` units); a unit deleted while such a loop runs is skipped
//!   (Python would call a dangling `CyUnit`).
//! - `CyMap().plot(x, y)` off the map is the NULL `CyPlot` (`None` here): `getNumUnits()` is
//!   -1, `getFeatureType()`/`getImprovementType()`/`getOwner()` are -1, `isOwned()`/`isCity()`
//!   are false.
//! - `CvInitCore::reassignPlayerAdvanced` (Impersonate Leader) has no ffh-core method yet; it
//!   is ported here as `reassign_player_advanced`.

use crate::ffh::dispatch::*;
use crate::ffh::game_utils::py_div;
use crate::ffh::pycall::PyCall;
use crate::game::Game;
use crate::ids::*;
use crate::utils::is_world_unit_class;
use ffh_data::*;

// ---------------------------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------------------------

/// PyResult functions (`spellX`).
pub fn find_spell(name: &str) -> Option<SpellFn> {
    let f: SpellFn = match name {
        "spellAddToFleshGolem" => spell_add_to_flesh_golem,
        "spellAddToWolfPack" => spell_add_to_wolf_pack,
        "spellArcaneLacuna" => spell_arcane_lacuna,
        "spellArdor" => spell_ardor,
        "spellArenaBattle" => spell_arena_battle,
        "spellCallBlizzard" => spell_call_blizzard,
        "spellCallForm" => spell_call_form,
        "spellCallOfTheGrave" => spell_call_of_the_grave,
        "spellCommanderJoin" => spell_commander_join,
        "spellCommanderJoinDecius" => spell_commander_join_decius,
        "spellCommanderSplit" => spell_commander_split,
        "spellConvertCityBasium" => spell_convert_city_basium,
        "spellConvertCityRantine" => spell_convert_city_rantine,
        "spellCreateBatteringRam" => spell_create_battering_ram,
        "spellCreateDenBear" => spell_create_den_bear,
        "spellCreateDenLion" => spell_create_den_lion,
        "spellCrush" => spell_crush,
        "spellDestroyUndead" => spell_destroy_undead,
        "spellDispelMagic" => spell_dispel_magic,
        "spellDisrupt" => spell_disrupt,
        "spellDivineRetribution" => spell_divine_retribution,
        "spellDomination" => spell_domination,
        "spellEarthquake" => spell_earthquake,
        "spellEnterPortal" => spell_enter_portal,
        "spellEntertain" => spell_entertain,
        "spellExploreLair" => spell_explore_lair,
        "spellExploreLairEpic" => spell_explore_lair_epic,
        "spellFeast" => spell_feast,
        "spellFeed" => spell_feed,
        "spellForTheHorde" => spell_for_the_horde,
        "spellFormWolfPack" => spell_form_wolf_pack,
        "spellGiftsOfNantosuelta" => spell_gifts_of_nantosuelta,
        "spellGiftVampirism" => spell_gift_vampirism,
        "spellGiveHammerToCraftsman" => spell_give_hammer_to_craftsman,
        "spellHastursRazor" => spell_hasturs_razor,
        "spellHeal" => spell_heal,
        "spellHealingSalve" => spell_healing_salve,
        "spellHeraldsCall" => spell_heralds_call,
        "spellHireArcher" => spell_hire_archer,
        "spellHireChariot" => spell_hire_chariot,
        "spellHireGoblin" => spell_hire_goblin,
        "spellHireUnits" => spell_hire_units,
        "spellHireWolfRider" => spell_hire_wolf_rider,
        "spellHyboremsWhisper" => spell_hyborems_whisper,
        "spellImpersonateLeader" => spell_impersonate_leader,
        "spellInquisition" => spell_inquisition,
        "spellIntoTheMist" => spell_into_the_mist,
        "spellIraUnleashed" => spell_ira_unleashed,
        "spellKidnap" => spell_kidnap,
        "spellLegends" => spell_legends,
        "spellMezmerizeAnimal" => spell_mezmerize_animal,
        "spellMirror" => spell_mirror,
        "spellPeace" => spell_peace,
        "spellPeaceSevenPines" => spell_peace_seven_pines,
        "spellPillarofFire" => spell_pillar_of_fire,
        _ => return None,
    };
    Some(f)
}

/// PyRequirement functions (`reqX`).
pub fn find_req(name: &str) -> Option<ReqFn> {
    let f: ReqFn = match name {
        "reqAddToFleshGolem" => req_add_to_flesh_golem,
        "reqAddToFreakShowHuman" => req_add_to_freak_show_human,
        "reqAddToWolfPack" => req_add_to_wolf_pack,
        "reqArcaneLacuna" => req_arcane_lacuna,
        "reqArdor" => req_ardor,
        "reqArenaBattle" => req_arena_battle,
        "reqCallBlizzard" => req_call_blizzard,
        "reqCallForm" => req_call_form,
        "reqCallOfTheGrave" => req_call_of_the_grave,
        "reqCommanderJoin" => req_commander_join,
        "reqConvertCityBasium" => req_convert_city_basium,
        "reqConvertCityRantine" => req_convert_city_rantine,
        "reqCreateDenBear" => req_create_den_bear,
        "reqCreateDenLion" => req_create_den_lion,
        "reqCrewBuccaneers" => req_crew_buccaneers,
        "reqCrewLongshoremen" => req_crew_longshoremen,
        "reqCrewNormalCrew" => req_crew_normal_crew,
        "reqCrewSkeletonCrew" => req_crew_skeleton_crew,
        "reqCrownOfBrillance" => req_crown_of_brillance,
        "reqCrush" => req_crush,
        "reqDeclareNationality" => req_declare_nationality,
        "reqDestroyUndead" => req_destroy_undead,
        "reqDispelMagic" => req_dispel_magic,
        "reqDisrupt" => req_disrupt,
        "reqDivineRetribution" => req_divine_retribution,
        "reqDomination" => req_domination,
        "reqEarthquake" => req_earthquake,
        "reqEscape" => req_escape,
        "reqExploreLair" => req_explore_lair,
        "reqFeast" => req_feast,
        "reqFeed" => req_feed,
        "reqForTheHorde" => req_for_the_horde,
        "reqFormWolfPack" => req_form_wolf_pack,
        "reqGiftsOfNantosuelta" => req_gifts_of_nantosuelta,
        "reqGiftVampirism" => req_gift_vampirism,
        "reqHastursRazor" => req_hasturs_razor,
        "reqHeal" => req_heal,
        "reqHealingSalve" => req_healing_salve,
        "reqHellfire" => req_hellfire,
        "reqHeraldsCall" => req_heralds_call,
        "reqHide" => req_hide,
        "reqHireScorpionClan" => req_hire_scorpion_clan,
        "reqHyboremsWhisper" => req_hyborems_whisper,
        "reqImpersonateLeader" => req_impersonate_leader,
        "reqInquisition" => req_inquisition,
        "reqIntoTheMist" => req_into_the_mist,
        "reqIraUnleashed" => req_ira_unleashed,
        "reqKidnap" => req_kidnap,
        "reqLegends" => req_legends,
        "reqLichdom" => req_lichdom,
        "reqMask" => req_mask,
        "reqMezmerizeAnimal" => req_mezmerize_animal,
        "reqMirror" => req_mirror,
        "reqPeace" => req_peace,
        "reqPeaceSevenPines" => req_peace_seven_pines,
        "reqPillarofFire" => req_pillar_of_fire,
        _ => return None,
    };
    Some(f)
}

/// Unit-only effects: promotion PyPerTurn (`effectX`), spell PyMiscast (`miscastX`).
pub fn find_unit_fn(name: &str) -> Option<UnitFn> {
    let f: UnitFn = match name {
        "effectCrownOfBrillance" => effect_crown_of_brillance,
        _ => return None,
    };
    Some(f)
}

/// Plot callbacks: improvement/feature PythonOnMove (`onMoveX`), improvement PythonAtRange
/// (`atRangeX`). None are defined in this part of the module.
pub fn find_plot_fn(name: &str) -> Option<PlotFn> {
    match name {
        _ => None,
    }
}

/// Post-combat callbacks (`postCombatX`).
pub fn find_combat_fn(name: &str) -> Option<CombatFn> {
    let f: CombatFn = match name {
        "postCombatConsumePaladin" => post_combat_consume_paladin,
        "postCombatDonal" => post_combat_donal,
        "postCombatExplode" => post_combat_explode,
        "postCombatHeal50" => post_combat_heal50,
        "postCombatIra" => post_combat_ira,
        "postCombatMimic" => post_combat_mimic,
        "postCombatAcheron" => post_combat_acheron,
        "postCombatArs" => post_combat_ars,
        "postCombatAuricAscendedLost" => post_combat_auric_ascended_lost,
        "postCombatAuricAscendedWon" => post_combat_auric_ascended_won,
        "postCombatBasium" => post_combat_basium,
        "postCombatBrigitHeld" => post_combat_brigit_held,
        "postCombatBuboes" => post_combat_buboes,
        "postCombatHyborem" => post_combat_hyborem,
        "postCombatLeviathan" => post_combat_leviathan,
        "postCombatOrthus" => post_combat_orthus,
        "postCombatStephanos" => post_combat_stephanos,
        "postCombatTreant" => post_combat_treant,
        "postCombatYersinia" => post_combat_yersinia,
        "postCombatReduceCombat1" => post_combat_reduce_combat1,
        "postCombatReducePopulation" => post_combat_reduce_population,
        "postCombatLostSailorsDirge" => post_combat_lost_sailors_dirge,
        "postCombatSplit" => post_combat_split,
        "postCombatWolfRider" => post_combat_wolf_rider,
        _ => return None,
    };
    Some(f)
}

/// Vote PyResult functions (`voteX`). None are defined in this part of the module.
pub fn find_vote_fn(name: &str) -> Option<VoteFn> {
    match name {
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Helpers (Python idioms)
// ---------------------------------------------------------------------------------------------

/// `pUnit.getX()`, `pUnit.getY()`.
fn xy(g: &Game, u: UnitId) -> (i32, i32) {
    (g.units[u].x, g.units[u].y)
}

/// `for iiX in range(x - r, x + r + 1): for iiY in range(y - r, y + r + 1): pPlot =
/// CyMap().plot(iiX, iiY)`: `(iiX, iiY, plot)` in that order (x outer, y inner); `None` is
/// the NULL plot (off the map).
fn square(g: &Game, x: i32, y: i32, r: i32) -> Vec<(i32, i32, Option<PlotIdx>)> {
    let mut v = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
    for xx in x - r..x + r + 1 {
        for yy in y - r..y + r + 1 {
            v.push((xx, yy, g.map.plot_xy(xx, yy)));
        }
    }
    v
}

/// `pPlot.getUnit(i)`: the unit at index `i` of the plot's current unit list, or `None` (the
/// NULL `CyUnit`) past its end.
fn plot_unit(g: &Game, p: PlotIdx, i: usize) -> Option<UnitId> {
    g.map.plots[p].units.get(i).copied()
}

/// `pPlot.getNumUnits()` (of a real plot).
fn num_units(g: &Game, p: PlotIdx) -> usize {
    g.map.plots[p].units.len()
}

/// `caster.plot().getPlotCity()`. Every spell below that uses it is `bInCityOnly` in XML (C++
/// `canCast` tests that before the Python runs), so `None` (the NULL `CyCity`) cannot happen
/// there; requirements still answer what Python would.
fn caster_city(g: &Game, caster: UnitId) -> Option<CityId> {
    g.map.plots[g.unit_plot(caster)].plot_city
}

/// `PyPlayer(p).getUnitList()` (PyHelpers): snapshot of the player's units, skipping dead ones.
fn py_unit_list(g: &Game, p: PlayerId) -> Vec<UnitId> {
    g.players[p].units.iter().copied().filter(|&u| !g.unit_is_dead(u)).collect()
}

/// `PyPlayer(p).getCityList()` + `pyCity.GetCy()`: the player's cities, in list order.
fn py_city_list(g: &Game, p: PlayerId) -> Vec<CityId> {
    g.players[p].cities.clone()
}

/// `gc.getPlayer(p).getUnit(iID)`: the unit if `p` owns it (C++ unit ids are per player);
/// `None` is the NULL `CyUnit`.
fn player_get_unit(g: &Game, p: PlayerId, id: Option<UnitId>) -> Option<UnitId> {
    let id = id?;
    match g.units.get(id) {
        Some(unit) if unit.owner == p => Some(id),
        _ => None,
    }
}

/// `pPlayer.initUnit(eUnit, iX, iY, UnitAITypes.NO_UNITAI, DirectionTypes.DIRECTION_SOUTH)`.
fn init_unit(g: &mut Game, p: PlayerId, unit: UnitType, x: i32, y: i32) -> UnitId {
    g.player_init_unit(p, unit, x, y, None, Some(DirectionType::DIRECTION_SOUTH))
}

/// `if pPlayer.isHuman(): t = ...; if not CyGame().isHasTrophy(t): CyGame().changeTrophyValue(t, 1)`.
fn trophy_if_human(g: &mut Game, p: PlayerId, t: &str) {
    if g.players[p].human && !g.is_has_trophy(t) {
        g.change_trophy_value(t, 1);
    }
}

/// `for iTrait in range(gc.getNumTraitInfos()): [if iTrait != keep:] if pPlayer.hasTrait(iTrait):
/// pPlayer.setHasTrait(iTrait, False)`.
fn remove_traits(g: &mut Game, p: PlayerId, keep: Option<TraitType>) {
    for t in g.d.traits.keys() {
        if Some(t) != keep && g.players[p].traits[t] {
            g.player_set_has_trait(p, t, false);
        }
    }
}

/// `eTeam.isAtWar(iTeam)` for the caster's team (`gc.getTeam(pPlayer.getTeam())`).
fn caster_at_war(g: &Game, caster: UnitId, team: TeamId) -> bool {
    let my_team = g.players[g.units[caster].owner].team;
    g.teams[my_team].at_war[team]
}

/// `CyGame().reassignPlayerAdvanced(eOldID, eNewID, iTimer)` = C++
/// `CvInitCore::reassignPlayerAdvanced` (CvInitCore.cpp:426-497), which has no ffh-core method
/// yet. Both must be civ slots. The two slots swap who plays them: the setup's handicap, slot
/// status and human flag (C++ also swaps e-mail, SMTP host, slot claim, playable/minor flags,
/// net id, ready flag and checksums, which have no equivalent here) and the controllers (the
/// port's form of the slot claim / net id). Player options move from the old to the new slot
/// (the old slot's are cleared); `setActivePlayer` is UI. Both players then update their team
/// and human flag, and with a timer the new slot becomes a temporary player of `old`.
fn reassign_player_advanced(g: &mut Game, old: PlayerId, new: PlayerId, timer: i32) {
    crate::player::turn::reassign_player_advanced(g, old, new, timer);
}

// ---------------------------------------------------------------------------------------------
// Module helper
// ---------------------------------------------------------------------------------------------

/// `findClearPlot(pUnit, plot)` (CvSpellInterface.py's own copy; CustomFunctions has another
/// with a different RNG label). `unit = None` is Python's `pUnit == -1`. The best-scoring plot
/// (later plots win ties), or `None` (Python -1).
fn find_clear_plot(g: &mut Game, unit: Option<UnitId>, plot: PlotIdx) -> Option<PlotIdx> {
    let mut best_plot = None;
    let mut best_value = 0;
    let Some(u) = unit else {
        let (x, y) = (g.map.plots[plot].x, g.map.plots[plot].y);
        let water = g.plot_is_water(plot);
        for (_, _, pl) in square(g, x, y, 1) {
            // NULL plot: getNumUnits() is -1, so it never scores.
            let Some(pl) = pl else { continue };
            let mut current = 0;
            if num_units(g, pl) == 0
                && g.plot_is_water(pl) == water
                && !g.plot_is_peak(pl)
                && g.map.plots[pl].plot_city.is_none()
            {
                current += 5;
            }
            if current >= 1 {
                current += g.soren_rand_num(5, "findClearPlot");
                if current >= best_value {
                    best_plot = Some(pl);
                    best_value = current;
                }
            }
        }
        return best_plot;
    };
    let (x, y) = xy(g, u);
    let owner = g.units[u].owner;
    for (xx, yy, pl) in square(g, x, y, 1) {
        // NULL plot: getNumUnits() is -1 and there is no city, so it never scores.
        let Some(pl) = pl else { continue };
        let mut current = 0;
        if num_units(g, pl) == 0 && g.unit_can_move_or_attack_into(u, pl, false) {
            current += 5;
        }
        for i in 0..num_units(g, pl) {
            let other = g.map.plots[pl].units[i];
            if g.units[other].owner == owner && g.unit_can_move_or_attack_into(u, pl, false) {
                current += 15;
            }
        }
        if let Some(c) = g.map.plots[pl].plot_city {
            if g.cities[c].owner == owner {
                current += 50;
            }
        }
        if x == xx && y == yy {
            current = 0;
        }
        if current >= 1 {
            current += g.soren_rand_num(5, "findClearPlot");
            if current >= best_value {
                best_plot = Some(pl);
                best_value = current;
            }
        }
    }
    best_plot
}

// ---------------------------------------------------------------------------------------------
// postCombat* (pCaster = the unit whose type names the callback, pOpponent = the other one)
// ---------------------------------------------------------------------------------------------

/// `postCombatConsumePaladin(pCaster, pOpponent)`.
fn post_combat_consume_paladin(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    if g.unit_get_unit_class_type(opponent) == g.d.t::<UnitClassType>("UNITCLASS_PALADIN") {
        let owner = g.units[caster].owner;
        g.unit_set_damage(caster, 0, Some(owner));
    }
}

/// `postCombatDonal(pCaster, pOpponent)`.
fn post_combat_donal(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let d = g.d;
    let opp = &g.units[opponent];
    if opp.has_promotion[d.t::<PromotionType>("PROMOTION_DEMON")] || opp.has_promotion[d.t::<PromotionType>("PROMOTION_UNDEAD")] {
        g.unit_set_has_promotion(caster, d.t::<PromotionType>("PROMOTION_RECRUITER"), true);
    }
}

/// `postCombatExplode(pCaster, pOpponent)`.
fn post_combat_explode(g: &mut Game, caster: UnitId, _opponent: UnitId, _call: &PyCall) {
    let d = g.d;
    let forest = d.t::<FeatureType>("FEATURE_FOREST");
    let jungle = d.t::<FeatureType>("FEATURE_JUNGLE");
    let forest_new = d.t::<FeatureType>("FEATURE_FOREST_NEW");
    let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
    let fire = d.t::<DamageType>("DAMAGE_FIRE");
    let (x, y) = xy(g, caster);
    for (xx, yy, pl) in square(g, x, y, 1) {
        if xx == x && yy == y {
            continue;
        }
        let Some(pl) = pl else { continue };
        let feature = g.map.plots[pl].feature_type;
        if feature == Some(forest) || feature == Some(jungle) || feature == Some(forest_new) {
            if g.soren_rand_num(100, "Flames Spread") <= d.defines.flames_spread_chance {
                let mut valid = true;
                if let Some(imp) = g.map.plots[pl].improvement_type {
                    if d.improvements[imp].permanent {
                        valid = false;
                    }
                }
                if valid {
                    g.plot_set_improvement_type(pl, Some(smoke));
                }
            }
        }
        for i in 0..num_units(g, pl) {
            // NULL unit: doDamage is a no-op.
            if let Some(u) = plot_unit(g, pl, i) {
                g.unit_do_damage(u, 10, 100, Some(caster), Some(fire), false);
            }
        }
    }
}

/// `postCombatHeal50(pCaster, pOpponent)`.
fn post_combat_heal50(g: &mut Game, caster: UnitId, _opponent: UnitId, _call: &PyCall) {
    let damage = g.units[caster].damage;
    if damage > 0 {
        let owner = g.units[caster].owner;
        g.unit_set_damage(caster, py_div(damage, 2), Some(owner));
    }
}

/// `postCombatIra(pCaster, pOpponent)`.
fn post_combat_ira(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    if g.unit_is_alive(opponent) && g.unit_base_combat_str(caster) < 32 {
        // setBaseCombatStr(baseCombatStr() - 3): the getter includes damage-type strength.
        g.units[caster].base_combat = g.unit_base_combat_str(caster) - 3;
        g.units[caster].base_combat_defense = g.unit_base_combat_str_defense(caster) - 3;
    }
}

/// `postCombatMimic(pCaster, pOpponent)`.
fn post_combat_mimic(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let d = g.d;
    let excluded = [
        d.t::<PromotionType>("PROMOTION_CHANNELING3"),
        d.t::<PromotionType>("PROMOTION_DIVINE"),
        d.t::<PromotionType>("PROMOTION_BRONZE_WEAPONS"),
        d.t::<PromotionType>("PROMOTION_IRON_WEAPONS"),
        d.t::<PromotionType>("PROMOTION_MITHRIL_WEAPONS"),
        d.t::<PromotionType>("PROMOTION_GREAT_COMMANDER"),
    ];
    let mut list_proms = Vec::new();
    let mut count = 0;
    for prom in d.promotions.keys() {
        if g.units[caster].has_promotion[prom] {
            count += 1;
        } else if g.units[opponent].has_promotion[prom]
            && !d.promotions[prom].equipment
            && !excluded.contains(&prom)
            && !d.promotions[prom].race
        {
            list_proms.push(prom);
        }
    }
    if !list_proms.is_empty() {
        count += 1;
        let rnd = g.soren_rand_num(list_proms.len() as i32, "Mimic");
        g.unit_set_has_promotion(caster, list_proms[rnd as usize], true);
    }
    if count >= 20 {
        let owner = g.units[caster].owner;
        trophy_if_human(g, owner, "TROPHY_FEAT_MIMIC_20");
    }
}

/// `postCombatAcheron(pCaster, pOpponent)`.
fn post_combat_acheron(g: &mut Game, _caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let p = g.units[opponent].owner;
    trophy_if_human(g, p, "TROPHY_DEFEATED_ACHERON");
}

/// `postCombatArs(pCaster, pOpponent)`.
fn post_combat_ars(g: &mut Game, _caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let p = g.units[opponent].owner;
    trophy_if_human(g, p, "TROPHY_DEFEATED_ARS");
}

/// `postCombatAuricAscendedLost(pCaster, pOpponent)`.
fn post_combat_auric_ascended_lost(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    remove_traits(g, p, None);
    if g.units[opponent].has_promotion[g.d.t::<PromotionType>("PROMOTION_GODSLAYER")] {
        let opp_owner = g.units[opponent].owner;
        trophy_if_human(g, opp_owner, "TROPHY_FEAT_GODSLAYER");
    }
}

/// `postCombatAuricAscendedWon(pCaster, pOpponent)`.
fn post_combat_auric_ascended_won(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    if g.units[opponent].has_promotion[g.d.t::<PromotionType>("PROMOTION_GODSLAYER")] {
        let p = g.units[caster].owner;
        remove_traits(g, p, None);
        let opp_owner = g.units[opponent].owner;
        g.unit_kill(caster, true, Some(opp_owner));
        trophy_if_human(g, opp_owner, "TROPHY_FEAT_GODSLAYER");
    }
}

/// `postCombatBasium(pCaster, pOpponent)`.
fn post_combat_basium(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    if !g.unit_is_immortal(caster) {
        let p = g.units[caster].owner;
        remove_traits(g, p, None);
        let opp_owner = g.units[opponent].owner;
        trophy_if_human(g, opp_owner, "TROPHY_DEFEATED_BASIUM");
    }
}

/// `postCombatBrigitHeld(pCaster, pOpponent)`.
fn post_combat_brigit_held(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let pl = g.unit_plot(caster);
    if g.map.plots[pl].improvement_type == Some(g.d.t::<ImprovementType>("IMPROVEMENT_RING_OF_CARCER")) {
        g.plot_set_improvement_type(pl, None);
    }
    let p = g.units[opponent].owner;
    trophy_if_human(g, p, "TROPHY_FEAT_RESCUE_BRIGIT");
}

/// `postCombatBuboes(pCaster, pOpponent)`.
fn post_combat_buboes(g: &mut Game, _caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let p = g.units[opponent].owner;
    trophy_if_human(g, p, "TROPHY_DEFEATED_BUBOES");
}

/// `postCombatHyborem(pCaster, pOpponent)`.
fn post_combat_hyborem(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    if !g.unit_is_immortal(caster) {
        let p = g.units[caster].owner;
        let fallow = g.d.t::<TraitType>("TRAIT_FALLOW");
        remove_traits(g, p, Some(fallow));
        let opp_owner = g.units[opponent].owner;
        trophy_if_human(g, opp_owner, "TROPHY_DEFEATED_HYBOREM");
    }
}

/// `postCombatLeviathan(pCaster, pOpponent)`.
fn post_combat_leviathan(g: &mut Game, _caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let p = g.units[opponent].owner;
    trophy_if_human(g, p, "TROPHY_DEFEATED_LEVIATHAN");
}

/// `postCombatOrthus(pCaster, pOpponent)`.
fn post_combat_orthus(g: &mut Game, _caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let p = g.units[opponent].owner;
    trophy_if_human(g, p, "TROPHY_DEFEATED_ORTHUS");
}

/// `postCombatStephanos(pCaster, pOpponent)`.
fn post_combat_stephanos(g: &mut Game, _caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let p = g.units[opponent].owner;
    trophy_if_human(g, p, "TROPHY_DEFEATED_STEPHANOS");
}

/// `postCombatTreant(pCaster, pOpponent)`.
fn post_combat_treant(g: &mut Game, caster: UnitId, _opponent: UnitId, _call: &PyCall) {
    let pl = g.unit_plot(caster);
    let forest_new = g.d.t::<FeatureType>("FEATURE_FOREST_NEW");
    if g.map.plots[pl].feature_type.is_none() && g.plot_can_have_feature(pl, Some(forest_new)) {
        g.plot_set_feature_type(pl, Some(forest_new));
    }
}

/// `postCombatYersinia(pCaster, pOpponent)`.
fn post_combat_yersinia(g: &mut Game, _caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let p = g.units[opponent].owner;
    trophy_if_human(g, p, "TROPHY_DEFEATED_YERSINIA");
}

/// `postCombatReduceCombat1(pCaster, pOpponent)`.
fn post_combat_reduce_combat1(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    if g.unit_is_alive(opponent) && g.unit_base_combat_str(caster) > 5 {
        // setBaseCombatStr(baseCombatStr() - 5): the getter includes damage-type strength.
        g.units[caster].base_combat = g.unit_base_combat_str(caster) - 5;
        g.units[caster].base_combat_defense = g.unit_base_combat_str_defense(caster) - 5;
    }
}

/// `postCombatReducePopulation(pCaster, pOpponent)`.
fn post_combat_reduce_population(g: &mut Game, _caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let pl = g.unit_plot(opponent);
    if let Some(c) = g.map.plots[pl].plot_city {
        if g.cities[c].population > 1 {
            g.city_change_population(c, -1);
        }
    }
}

/// `postCombatLostSailorsDirge(pCaster, pOpponent)`.
fn post_combat_lost_sailors_dirge(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let p = g.units[opponent].owner;
    let trigger = g.d.t::<EventTriggerType>("EVENTTRIGGER_SAILORS_DIRGE_DEFEATED");
    let (x, y) = xy(g, caster);
    g.player_init_triggered_data(p, trigger, true, None, x, y, Some(p), None, None, None, None, None);
}

/// `postCombatSplit(pCaster, pOpponent)`.
fn post_combat_split(g: &mut Game, caster: UnitId, _opponent: UnitId, _call: &PyCall) {
    let d = g.d;
    let weak = d.t::<PromotionType>("PROMOTION_WEAK");
    let strong = d.t::<PromotionType>("PROMOTION_STRONG");
    if !g.units[caster].has_promotion[weak] {
        let p = g.units[caster].owner;
        let unit_type = g.units[caster].unit_type;
        let (x, y) = xy(g, caster);
        let new1 = init_unit(g, p, unit_type, x, y);
        let new2 = init_unit(g, p, unit_type, x, y);
        g.unit_set_has_promotion(new1, strong, false);
        g.unit_set_has_promotion(new2, strong, false);
        g.unit_set_damage(new1, 25, None);
        g.unit_set_damage(new2, 25, None);
        if !g.units[caster].has_promotion[strong] {
            g.unit_set_has_promotion(new1, weak, true);
            g.unit_set_has_promotion(new2, weak, true);
        }
        g.units[new1].duration = g.units[caster].duration;
        g.units[new2].duration = g.units[caster].duration;
    }
}

/// `postCombatWolfRider(pCaster, pOpponent)`.
fn post_combat_wolf_rider(g: &mut Game, caster: UnitId, opponent: UnitId, _call: &PyCall) {
    let d = g.d;
    let opp_type = g.units[opponent].unit_type;
    if opp_type == d.t::<UnitType>("UNIT_WOLF") || opp_type == d.t::<UnitType>("UNIT_WOLF_PACK") {
        let p = g.units[caster].owner;
        let (x, y) = xy(g, caster);
        let new_unit = init_unit(g, p, d.t::<UnitType>("UNIT_WOLF_RIDER"), x, y);
        g.unit_convert(new_unit, caster);
    }
}

// ---------------------------------------------------------------------------------------------
// Spells (req* = PyRequirement, spell* = PyResult, effect* = promotion PyPerTurn)
// ---------------------------------------------------------------------------------------------

/// `reqAddToFleshGolem(caster)`.
fn req_add_to_flesh_golem(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    if g.units[caster].has_promotion[g.d.t::<PromotionType>("PROMOTION_IMMORTAL")] {
        return false;
    }
    if g.unit_is_immune_to_magic(caster) {
        return false;
    }
    true
}

/// `spellAddToFleshGolem(caster)`.
fn spell_add_to_flesh_golem(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    let pl = g.unit_plot(caster);
    // 'PROMOTION_CHANNELING' is not a type in 0.41: Python gets -1, which matches no promotion.
    let channeling = d.ty::<PromotionType>("PROMOTION_CHANNELING");
    let channeling2 = d.t::<PromotionType>("PROMOTION_CHANNELING2");
    let channeling3 = d.t::<PromotionType>("PROMOTION_CHANNELING3");
    let divine = d.t::<PromotionType>("PROMOTION_DIVINE");
    let flesh_golem_class = d.t::<UnitClassType>("UNITCLASS_FLESH_GOLEM");
    let mut golem = None;
    for &u in &g.map.plots[pl].units {
        if p == g.units[u].owner && g.unit_get_unit_class_type(u) == flesh_golem_class {
            golem = Some(u);
        }
    }
    let Some(golem) = golem else { return };
    if g.unit_base_combat_str(caster) > g.unit_base_combat_str(golem) {
        g.units[golem].base_combat = g.unit_base_combat_str(golem) + 1;
    }
    if g.unit_base_combat_str_defense(caster) > g.unit_base_combat_str_defense(golem) {
        g.units[golem].base_combat_defense = g.unit_base_combat_str_defense(golem) + 1;
    }
    for prom in d.promotions.keys() {
        if g.units[caster].has_promotion[prom] {
            let info = &d.promotions[prom];
            if Some(prom) != channeling
                && prom != channeling2
                && prom != channeling3
                && prom != divine
                && !info.race
                && info.bonus_prereq.is_none()
                && info.promotion_prereq_and != Some(channeling2)
                && info.promotion_prereq_and != Some(channeling3)
                && !info.equipment
            {
                g.unit_set_has_promotion(golem, prom, true);
            }
        }
    }
    let bronze = d.t::<PromotionType>("PROMOTION_BRONZE_WEAPONS");
    let iron = d.t::<PromotionType>("PROMOTION_IRON_WEAPONS");
    if g.units[golem].has_promotion[d.t::<PromotionType>("PROMOTION_MITHRIL_WEAPONS")] {
        g.unit_set_has_promotion(golem, iron, false);
        g.unit_set_has_promotion(golem, bronze, false);
    }
    if g.units[golem].has_promotion[iron] {
        g.unit_set_has_promotion(golem, bronze, false);
    }
    if g.unit_base_combat_str(golem) >= 15 {
        trophy_if_human(g, p, "TROPHY_FEAT_FLESH_GOLEM_15");
    }
}

/// `reqAddToFreakShowHuman(caster)`.
fn req_add_to_freak_show_human(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    g.units[caster].race.is_none()
}

/// `reqAddToWolfPack(caster)`.
fn req_add_to_wolf_pack(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let pl = g.unit_plot(caster);
    let wolf_pack = d.t::<UnitType>("UNIT_WOLF_PACK");
    let empower5 = d.t::<PromotionType>("PROMOTION_EMPOWER5");
    for &u in &g.map.plots[pl].units {
        if g.units[u].unit_type == wolf_pack && g.units[u].owner == g.units[caster].owner && !g.units[u].has_promotion[empower5] {
            return true;
        }
    }
    false
}

/// `spellAddToWolfPack(caster)`.
fn spell_add_to_wolf_pack(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let pl = g.unit_plot(caster);
    let wolf_pack = d.t::<UnitType>("UNIT_WOLF_PACK");
    // Checked from EMPOWER5 down to EMPOWER1: the lowest missing one wins.
    let empower = [
        d.t::<PromotionType>("PROMOTION_EMPOWER5"),
        d.t::<PromotionType>("PROMOTION_EMPOWER4"),
        d.t::<PromotionType>("PROMOTION_EMPOWER3"),
        d.t::<PromotionType>("PROMOTION_EMPOWER2"),
        d.t::<PromotionType>("PROMOTION_EMPOWER1"),
    ];
    let mut valid = true;
    for i in 0..num_units(g, pl) {
        if !valid {
            continue;
        }
        // NULL unit: getUnitType() is -1.
        let Some(u) = plot_unit(g, pl, i) else { continue };
        if g.units[u].unit_type == wolf_pack && g.units[u].owner == g.units[caster].owner {
            let mut prom = None;
            for e in empower {
                if !g.units[u].has_promotion[e] {
                    prom = Some(e);
                }
            }
            if let Some(prom) = prom {
                g.unit_set_has_promotion(u, prom, true);
                valid = false;
            }
        }
    }
}

/// `manaTypes` of `reqArcaneLacuna` / `spellArcaneLacuna`.
const MANA_TYPES: [&str; 17] = [
    "BONUS_MANA_AIR",
    "BONUS_MANA_BODY",
    "BONUS_MANA_CHAOS",
    "BONUS_MANA_DEATH",
    "BONUS_MANA_EARTH",
    "BONUS_MANA_ENCHANTMENT",
    "BONUS_MANA_ENTROPY",
    "BONUS_MANA_FIRE",
    "BONUS_MANA_LAW",
    "BONUS_MANA_LIFE",
    "BONUS_MANA_METAMAGIC",
    "BONUS_MANA_MIND",
    "BONUS_MANA_NATURE",
    "BONUS_MANA_SHADOW",
    "BONUS_MANA_SPIRIT",
    "BONUS_MANA_SUN",
    "BONUS_MANA_WATER",
];

/// `for szBonus in manaTypes: iCount += CyMap().getNumBonuses(gc.getInfoTypeForString(szBonus))`.
fn count_mana_bonuses(g: &Game) -> i32 {
    MANA_TYPES.iter().map(|name| g.map.num_bonus[g.d.t::<BonusType>(name)]).sum()
}

/// `reqArcaneLacuna(caster)`. Change from FFH2: its AI-only branch (7 typed mana bonuses on the
/// map) is AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_arcane_lacuna(g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    let count = count_mana_bonuses(g);
    if count == 0 {
        return false;
    }
    true
}

/// `spellArcaneLacuna(caster)`.
fn spell_arcane_lacuna(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let adept = d.t::<UnitCombatType>("UNITCOMBAT_ADEPT");
    let p = g.units[caster].owner;
    let count = count_mana_bonuses(g);
    for u in py_unit_list(g, p) {
        if g.units.contains(u) && g.unit_get_unit_combat_type(u) == Some(adept) {
            g.unit_change_experience(u, count, -1, false, false, false);
        }
    }
    let speed = g.game_speed_type();
    let mut delay = 20;
    if speed == d.t::<GameSpeedType>("GAMESPEED_QUICK") {
        delay = 14;
    }
    if speed == d.t::<GameSpeedType>("GAMESPEED_EPIC") {
        delay = 30;
    }
    if speed == d.t::<GameSpeedType>("GAMESPEED_MARATHON") {
        delay = 60;
    }
    let team = g.players[p].team;
    for p2 in g.player_ids() {
        if g.players[p2].alive && g.players[p2].team != team {
            // CvPlayer::changeDisableSpellcasting (plain member update).
            g.players[p2].disable_spellcasting += delay;
        }
    }
}

/// `reqArdor(caster)`. Change from FFH2: its AI-only branch (5 great people) is AI timing, in
/// ffh-ai `world_spells` (docs/PORTING.md).
fn req_ardor(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let player = &g.players[g.units[caster].owner];
    if player.great_people_created == 0 {
        return false;
    }
    true
}

/// `spellArdor(caster)`.
fn spell_ardor(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    g.players[p].great_people_created = 0;
    g.players[p].great_people_threshold_modifier = 0;
}

/// `reqArenaBattle(caster)`.
fn req_arena_battle(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let combat = g.unit_get_unit_combat_type(caster);
    if combat == Some(d.t::<UnitCombatType>("UNITCOMBAT_MELEE")) {
        return true;
    }
    if combat == Some(d.t::<UnitCombatType>("UNITCOMBAT_RECON")) {
        return true;
    }
    if g.unit_get_unit_class_type(caster) == d.t::<UnitClassType>("UNITCLASS_SLAVE") {
        return true;
    }
    false
}

/// `spellArenaBattle(caster)`.
fn spell_arena_battle(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    if let Some(c) = caster_city(g, caster) {
        g.city_change_happiness_timer(c, 3);
    }
    if g.soren_rand_num(100, "Arena Battle") < 50 {
        let xp = g.soren_rand_num(6, "Arena Battle") + 1;
        g.unit_change_experience(caster, xp, -1, false, false, false);
        let p = g.units[caster].owner;
        g.unit_set_damage(caster, 25, Some(p));
        if g.unit_get_unit_class_type(caster) == d.t::<UnitClassType>("UNITCLASS_SLAVE") {
            let (x, y) = xy(g, caster);
            let new_unit = init_unit(g, p, d.t::<UnitType>("UNIT_WARRIOR"), x, y);
            g.unit_convert(new_unit, caster);
        }
    } else {
        g.unit_kill(caster, true, None);
    }
}

/// `reqCallBlizzard(caster)`.
fn req_call_blizzard(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let blizzard = d.t::<FeatureType>("FEATURE_BLIZZARD");
    let pl = g.unit_plot(caster);
    if g.map.plots[pl].feature_type == Some(blizzard) {
        return false;
    }
    if g.map.plots[pl].feature_type == Some(d.t::<FeatureType>("FEATURE_FOREST_ANCIENT")) {
        return false;
    }
    let (x, y) = xy(g, caster);
    for (_, _, p) in square(g, x, y, 1) {
        // NULL plot: getFeatureType() is -1.
        if p.is_some_and(|p| g.map.plots[p].feature_type == Some(blizzard)) {
            return true;
        }
    }
    false
}

/// `spellCallBlizzard(caster)`.
fn spell_call_blizzard(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let blizzard = d.t::<FeatureType>("FEATURE_BLIZZARD");
    let tundra = d.t::<TerrainType>("TERRAIN_TUNDRA");
    let (x, y) = xy(g, caster);
    let mut bliz_plot = None;
    for (_, _, p) in square(g, x, y, 1) {
        if let Some(p) = p {
            if g.map.plots[p].feature_type == Some(blizzard) {
                bliz_plot = Some(p);
            }
        }
    }
    if let Some(p) = bliz_plot {
        g.plot_set_feature_type(p, None);
    }
    let pl = g.unit_plot(caster);
    g.plot_set_feature_type(pl, Some(blizzard));
    if g.map.plots[pl].terrain_type == Some(d.t::<TerrainType>("TERRAIN_GRASS")) {
        g.plot_set_terrain_type(pl, tundra, true);
    }
    if g.map.plots[pl].terrain_type == Some(d.t::<TerrainType>("TERRAIN_PLAINS")) {
        g.plot_set_terrain_type(pl, tundra, true);
    }
    if g.map.plots[pl].terrain_type == Some(d.t::<TerrainType>("TERRAIN_DESERT")) {
        g.plot_set_terrain_type(pl, tundra, true);
    }
}

/// `reqCallForm(caster)`.
fn req_call_form(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    if g.units[caster].summoner.is_none() {
        return false;
    }
    let p = g.units[caster].owner;
    let pl = g.unit_plot(caster);
    // NULL unit (summoner gone): canMoveInto is false.
    match player_get_unit(g, p, g.units[caster].summoner) {
        Some(u) => g.unit_can_move_into(u, pl, false, false, false),
        None => false,
    }
}

/// `spellCallForm(caster)`.
fn spell_call_form(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let pl = g.unit_plot(caster);
    if let Some(u) = player_get_unit(g, p, g.units[caster].summoner) {
        let (x, y) = (g.map.plots[pl].x, g.map.plots[pl].y);
        g.unit_set_xy(u, x, y, false);
    }
}

/// `reqCallOfTheGrave(caster)`.
fn req_call_of_the_grave(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let (x, y) = xy(g, caster);
    let graveyard = d.t::<ImprovementType>("IMPROVEMENT_GRAVEYARD");
    let undead = d.t::<PromotionType>("PROMOTION_UNDEAD");
    for (_, _, pl) in square(g, x, y, 2) {
        let Some(pl) = pl else { continue };
        for &u in &g.map.plots[pl].units {
            let team2 = g.players[g.units[u].owner].team;
            if caster_at_war(g, caster, team2) {
                return true;
            }
            if g.units[u].race == Some(undead) && g.units[u].damage != 0 {
                return true;
            }
            if g.map.plots[pl].improvement_type == Some(graveyard) {
                return true;
            }
        }
    }
    false
}

/// `spellCallOfTheGrave(caster)`.
fn spell_call_of_the_grave(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    let (x, y) = xy(g, caster);
    let graveyard = d.t::<ImprovementType>("IMPROVEMENT_GRAVEYARD");
    let undead = d.t::<PromotionType>("PROMOTION_UNDEAD");
    let death = d.t::<DamageType>("DAMAGE_DEATH");
    let wraith = d.t::<UnitType>("UNIT_WRAITH");
    for (_, _, pl) in square(g, x, y, 2) {
        let Some(pl) = pl else { continue };
        for i in 0..num_units(g, pl) {
            // NULL unit: gc.getPlayer(-1) is None, so `.getTeam()` raises and the spell stops.
            let Some(u) = plot_unit(g, pl, i) else { return };
            let team2 = g.players[g.units[u].owner].team;
            if caster_at_war(g, caster, team2) {
                g.unit_do_damage(u, 40, 100, Some(caster), Some(death), true);
            } else if g.units[u].race == Some(undead) && g.units[u].damage != 0 {
                g.unit_set_damage(u, 0, Some(p));
            }
            if g.map.plots[pl].improvement_type == Some(graveyard) {
                g.plot_set_improvement_type(pl, None);
                let (px, py) = (g.map.plots[pl].x, g.map.plots[pl].y);
                for _ in 0..3 {
                    init_unit(g, p, wraith, px, py);
                }
            }
        }
        // `if bValid: CyEngine().triggerEffect(...)` is graphics only.
    }
}

/// The last unit of class `UNITCLASS_COMMANDER` on the caster's plot owned by the caster's
/// owner (`pCommander` of `reqCommanderJoin` / `spellCommanderJoin`).
fn find_commander(g: &Game, caster: UnitId) -> Option<UnitId> {
    let commander_class = g.d.t::<UnitClassType>("UNITCLASS_COMMANDER");
    let pl = g.unit_plot(caster);
    let mut commander = None;
    for &u in &g.map.plots[pl].units {
        if g.units[caster].owner == g.units[u].owner && g.unit_get_unit_class_type(u) == commander_class {
            commander = Some(u);
        }
    }
    commander
}

/// `reqCommanderJoin(caster)`.
fn req_commander_join(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if g.units[caster].has_promotion[g.d.t::<PromotionType>("PROMOTION_GREAT_COMMANDER")] {
        return false;
    }
    let Some(commander) = find_commander(g, caster) else { return false };
    if g.units[commander].has_casted {
        return false;
    }
    if !g.players[p].human && g.unit_base_combat_str(caster) <= 5 {
        return false;
    }
    true
}

/// `spellCommanderJoin(caster)`.
fn spell_commander_join(g: &mut Game, caster: UnitId, _call: &PyCall) {
    if let Some(commander) = find_commander(g, caster) {
        g.unit_set_has_promotion(commander, g.d.t::<PromotionType>("PROMOTION_GOLEM"), true);
        g.unit_kill(commander, false, None);
    }
}

/// `spellCommanderJoinDecius(caster)`.
fn spell_commander_join_decius(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let decius = g.d.t::<UnitType>("UNIT_DECIUS");
    let pl = g.unit_plot(caster);
    let mut commander = None;
    for &u in &g.map.plots[pl].units {
        if g.units[caster].owner == g.units[u].owner && g.units[u].unit_type == decius {
            commander = Some(u);
        }
    }
    if let Some(commander) = commander {
        // The scenario counter remembers Decius' unit type index.
        g.units[caster].scenario_counter = decius.idx() as i32;
        g.unit_set_has_promotion(commander, g.d.t::<PromotionType>("PROMOTION_GOLEM"), true);
        g.unit_kill(commander, false, None);
    }
}

/// `spellCommanderSplit(caster)`.
fn spell_commander_split(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    let mut commander = d.t::<UnitType>("UNIT_COMMANDER");
    let decius = d.t::<UnitType>("UNIT_DECIUS");
    if g.units[caster].scenario_counter == decius.idx() as i32 {
        commander = decius;
        g.units[caster].scenario_counter = -1;
    }
    let (x, y) = xy(g, caster);
    init_unit(g, p, commander, x, y);
}

/// `reqConvertCityBasium(caster)`.
fn req_convert_city_basium(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    // NULL city: getOwner() is -1, never the caster's owner.
    match caster_city(g, caster) {
        Some(c) => g.cities[c].owner != g.units[caster].owner,
        None => true,
    }
}

/// `spellConvertCityBasium(caster)`.
fn spell_convert_city_basium(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let Some(c) = caster_city(g, caster) else { return };
    let p = g.units[caster].owner;
    g.player_acquire_city(p, c, false, false, true);
    // acquireCity replaces the city: Python fetches it again from the plot.
    if let Some(c) = caster_city(g, caster) {
        g.city_change_culture(c, p, 300, true, true);
    }
}

/// `reqConvertCityRantine(caster)`.
fn req_convert_city_rantine(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let barbarian = g.barbarian_player();
    // NULL city: getOwner() is -1, which is not the barbarian player.
    let Some(c) = caster_city(g, caster) else { return false };
    if g.cities[c].owner == g.units[caster].owner {
        return false;
    }
    if g.cities[c].owner != barbarian {
        return false;
    }
    let pl = g.unit_plot(caster);
    for &u in &g.map.plots[pl].units {
        if g.units[u].owner == barbarian && g.unit_base_combat_str(u) > g.unit_base_combat_str(caster) {
            return false;
        }
    }
    true
}

/// `spellConvertCityRantine(caster)`.
fn spell_convert_city_rantine(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let Some(c) = caster_city(g, caster) else { return };
    let p = g.units[caster].owner;
    g.player_acquire_city(p, c, false, false, true);
}

/// `spellCreateBatteringRam(caster)`.
fn spell_create_battering_ram(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let pl = g.unit_plot(caster);
    g.plot_set_feature_type(pl, None);
}

/// Body of `reqCreateDenBear` / `reqCreateDenLion` (identical but for the improvement).
fn req_create_den(g: &Game, caster: UnitId, den: &str) -> bool {
    if g.is_option(GameOptionType::GAMEOPTION_NO_LAIRS) {
        return false;
    }
    let improvement = g.d.t::<ImprovementType>(den);
    if g.units[caster].damage > 0 {
        return false;
    }
    let pl = g.unit_plot(caster);
    if g.plot_is_owned(pl) {
        return false;
    }
    if num_units(g, pl) != 1 {
        return false;
    }
    if g.map.plots[pl].improvement_type.is_some() {
        return false;
    }
    // Python quirk: passes the caster's *player* id as the team (C++ reads it as TeamTypes;
    // player and team slots have the same count, so the index is valid).
    let team = TeamId(g.units[caster].owner.0);
    if !g.plot_can_have_improvement(pl, improvement, Some(team), true) {
        return false;
    }
    let (x, y) = (g.map.plots[pl].x, g.map.plots[pl].y);
    for (_, _, p2) in square(g, x, y, 4) {
        if p2.is_some_and(|p2| g.map.plots[p2].improvement_type == Some(improvement)) {
            return false;
        }
    }
    true
}

/// Body of `spellCreateDenBear` / `spellCreateDenLion` (identical but for the types).
fn spell_create_den(g: &mut Game, caster: UnitId, den: &str, animal: &str) {
    let d = g.d;
    let pl = g.unit_plot(caster);
    let improvement = d.t::<ImprovementType>(den);
    let unit = d.t::<UnitType>(animal);
    g.plot_set_improvement_type(pl, Some(improvement));
    if let Some(p2) = find_clear_plot(g, None, pl) {
        let (x2, y2) = (g.map.plots[p2].x, g.map.plots[p2].y);
        g.unit_set_xy(caster, x2, y2, false);
        let barbarian = g.barbarian_player();
        let (x, y) = (g.map.plots[pl].x, g.map.plots[pl].y);
        let new_unit = init_unit(g, barbarian, unit, x, y);
        g.unit_convert(new_unit, caster);
    }
}

/// `reqCreateDenBear(caster)`.
fn req_create_den_bear(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    req_create_den(g, caster, "IMPROVEMENT_BEAR_DEN")
}

/// `spellCreateDenBear(caster)`.
fn spell_create_den_bear(g: &mut Game, caster: UnitId, _call: &PyCall) {
    spell_create_den(g, caster, "IMPROVEMENT_BEAR_DEN", "UNIT_BEAR");
}

/// `reqCreateDenLion(caster)`.
fn req_create_den_lion(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    req_create_den(g, caster, "IMPROVEMENT_LION_DEN")
}

/// `spellCreateDenLion(caster)`.
fn spell_create_den_lion(g: &mut Game, caster: UnitId, _call: &PyCall) {
    spell_create_den(g, caster, "IMPROVEMENT_LION_DEN", "UNIT_LION");
}

/// `reqCrewBuccaneers(caster)`.
fn req_crew_buccaneers(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    if g.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_BUCCANEERS")] {
        return false;
    }
    // maxMoves() is in MOVE_DENOMINATOR units, so this only excludes immobile units.
    if g.unit_max_moves(caster) <= 1 {
        return false;
    }
    if g.unit_base_combat_str(caster) < 1 {
        return false;
    }
    if !g.players[g.units[caster].owner].human {
        let unit = g.units[caster].unit_type;
        if unit != d.t::<UnitType>("UNIT_FRIGATE") && unit != d.t::<UnitType>("UNIT_MAN_O_WAR") {
            return false;
        }
    }
    true
}

/// `reqCrewLongshoremen(caster)`.
fn req_crew_longshoremen(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    if g.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_LONGSHOREMEN")] {
        return false;
    }
    if g.unit_cargo_space_available(caster, None, Some(DomainType::DOMAIN_LAND)) == 0 {
        return false;
    }
    if g.unit_base_combat_str(caster) < 1 {
        return false;
    }
    if !g.players[g.units[caster].owner].human {
        let unit = g.units[caster].unit_type;
        if unit != d.t::<UnitType>("UNIT_GALLEY") && unit != d.t::<UnitType>("UNIT_CARAVEL") {
            return false;
        }
    }
    true
}

/// `reqCrewNormalCrew(caster)`.
fn req_crew_normal_crew(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let has = |name: &str| g.units[caster].has_promotion[d.t::<PromotionType>(name)];
    if !has("PROMOTION_BUCCANEERS") && !has("PROMOTION_SKELETON_CREW") && !has("PROMOTION_LONGSHOREMEN") {
        return false;
    }
    if !g.players[g.units[caster].owner].human {
        return false;
    }
    true
}

/// `reqCrewSkeletonCrew(caster)`.
fn req_crew_skeleton_crew(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    if g.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_SKELETON_CREW")] {
        return false;
    }
    if g.unit_base_combat_str(caster) < 1 {
        return false;
    }
    if !g.players[g.units[caster].owner].human {
        let unit = g.units[caster].unit_type;
        if unit != d.t::<UnitType>("UNIT_QUEEN_OF_THE_LINE")
            && unit != d.t::<UnitType>("UNIT_TRIREME")
            && unit != d.t::<UnitType>("UNIT_GALLEON")
        {
            return false;
        }
    }
    true
}

/// `effectCrownOfBrillance(caster)` (promotion PyPerTurn).
fn effect_crown_of_brillance(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let spell = g.d.t::<SpellType>("SPELL_CROWN_OF_BRILLANCE");
    g.unit_cast(caster, spell);
}

/// `reqCrownOfBrillance(caster)`.
fn req_crown_of_brillance(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    !g.units[caster].has_promotion[g.d.t::<PromotionType>("PROMOTION_CROWN_OF_BRILLANCE")]
}

/// Body of `reqCrush` / `reqPillarofFire` (identical): some plot within 2 has units, all of
/// them at war with the caster's team.
fn req_enemy_only_plot_within_2(g: &Game, caster: UnitId) -> bool {
    let (x, y) = xy(g, caster);
    for (_, _, pl) in square(g, x, y, 2) {
        let Some(pl) = pl else { continue };
        let mut enemy = false;
        let mut neutral = false;
        for &u in &g.map.plots[pl].units {
            if caster_at_war(g, caster, g.unit_get_team(u)) {
                enemy = true;
            } else {
                neutral = true;
            }
        }
        if enemy && !neutral {
            return true;
        }
    }
    false
}

/// `reqCrush(caster)`.
fn req_crush(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    req_enemy_only_plot_within_2(g, caster)
}

/// `spellCrush(caster)`.
fn spell_crush(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let physical = g.d.t::<DamageType>("DAMAGE_PHYSICAL");
    let (x, y) = xy(g, caster);
    let mut best_value = 0;
    let mut best_plot = None;
    for (_, _, pl) in square(g, x, y, 2) {
        // NULL plot: no units, value 0.
        let Some(pl) = pl else { continue };
        let mut neutral = false;
        let mut value = 0;
        for &u in &g.map.plots[pl].units {
            if caster_at_war(g, caster, g.unit_get_team(u)) {
                value += 10;
            } else {
                neutral = true;
            }
        }
        if value > best_value && !neutral {
            best_value = value;
            best_plot = Some(pl);
        }
    }
    if let Some(pl) = best_plot {
        for i in 0..num_units(g, pl) {
            // NULL unit: doDamage is a no-op.
            if let Some(u) = plot_unit(g, pl, i) {
                g.unit_do_damage(u, 50, 75, Some(caster), Some(physical), true);
            }
        }
        // CyEngine().triggerEffect(EFFECT_CRUSH) is graphics only.
    }
}

/// `reqDeclareNationality(caster)`.
fn req_declare_nationality(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let p = g.units[caster].owner;
    if !g.players[p].human {
        let ai = g.units[caster].ai_type;
        if ai == Some(UnitAIType::UNITAI_PIRATE_SEA) || ai == Some(UnitAIType::UNITAI_ANIMAL) {
            return false;
        }
        if g.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_HIDDEN")]
            || g.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_INVISIBLE")]
        {
            return false;
        }
        if g.player_is_barbarian(p) {
            return false;
        }
    }
    true
}

/// `reqDestroyUndead(caster)`.
fn req_destroy_undead(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let undead = g.d.t::<PromotionType>("PROMOTION_UNDEAD");
    let p = g.units[caster].owner;
    let team = g.players[p].team;
    let (x, y) = xy(g, caster);
    for (_, _, pl) in square(g, x, y, 1) {
        let Some(pl) = pl else { continue };
        for &u in &g.map.plots[pl].units {
            if g.units[u].race == Some(undead) {
                if g.players[p].human {
                    return true;
                }
                let team2 = g.players[g.units[u].owner].team;
                if g.teams[team2].at_war[team] {
                    return true;
                }
            }
        }
    }
    false
}

/// `spellDestroyUndead(caster)`.
fn spell_destroy_undead(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let undead = g.d.t::<PromotionType>("PROMOTION_UNDEAD");
    let holy = g.d.t::<DamageType>("DAMAGE_HOLY");
    let (x, y) = xy(g, caster);
    for (_, _, pl) in square(g, x, y, 1) {
        let Some(pl) = pl else { continue };
        for i in 0..num_units(g, pl) {
            // NULL unit: getRace() is -1.
            let Some(u) = plot_unit(g, pl, i) else { continue };
            if g.units[u].race == Some(undead) {
                g.unit_do_damage(u, 30, 100, Some(caster), Some(holy), true);
            }
        }
    }
}

/// `pPlot.getBonusType(-1)` is mana and its improvement (if any) is not permanent: the plot
/// part shared by `reqDispelMagic` / `spellDispelMagic`. `Some(has_improvement)` when it holds.
fn dispellable_mana(g: &Game, pl: PlotIdx) -> Option<bool> {
    let d = g.d;
    let bonus = g.map.plots[pl].bonus_type?;
    if d.bonuses[bonus].bonus_class_type != Some(d.t::<BonusClassType>("BONUSCLASS_MANA")) {
        return None;
    }
    match g.map.plots[pl].improvement_type {
        None => Some(false),
        Some(imp) if !d.improvements[imp].permanent => Some(true),
        Some(_) => None,
    }
}

/// `reqDispelMagic(caster)`.
fn req_dispel_magic(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    if g.unit_can_dispel(caster, g.d.t::<SpellType>("SPELL_DISPEL_MAGIC")) {
        return true;
    }
    dispellable_mana(g, g.unit_plot(caster)).is_some()
}

/// `spellDispelMagic(caster)`.
fn spell_dispel_magic(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let pl = g.unit_plot(caster);
    let mana = g.d.t::<BonusType>("BONUS_MANA");
    match dispellable_mana(g, pl) {
        Some(false) => g.plot_set_bonus_type(pl, Some(mana)),
        Some(true) => {
            g.plot_set_bonus_type(pl, Some(mana));
            g.plot_set_improvement_type(pl, None);
        }
        None => {}
    }
}

/// `reqDisrupt(caster)`.
fn req_disrupt(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    // NULL city: getTeam() is NO_TEAM, never the caster's team.
    match caster_city(g, caster) {
        Some(c) => g.unit_get_team(caster) != g.city_team(c),
        None => true,
    }
}

/// `spellDisrupt(caster)`.
fn spell_disrupt(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let Some(c) = caster_city(g, caster) else { return };
    let p = g.units[caster].owner;
    let p2 = g.cities[c].owner;
    g.city_change_hurry_anger_timer(c, 2);
    let rnd = g.soren_rand_num(3, "Disrupt");
    if rnd != 0 {
        g.city_change_culture(c, p2, -rnd, true, true);
    }
    if g.city_culture(c, p2) < 1 {
        g.player_acquire_city(p, c, false, false, true);
        g.players[p2].ai_attitude_extra[p] -= 4;
    }
}

/// `reqDivineRetribution(caster)`. Change from FFH2: its only condition was AI-only (at war with
/// two teams); it is AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_divine_retribution(_g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    true
}

/// `spellDivineRetribution(caster)`.
fn spell_divine_retribution(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let demon = g.d.t::<PromotionType>("PROMOTION_DEMON");
    let undead = g.d.t::<PromotionType>("PROMOTION_UNDEAD");
    let holy = g.d.t::<DamageType>("DAMAGE_HOLY");
    for p in g.player_ids() {
        if g.players[p].alive {
            for u in py_unit_list(g, p) {
                if !g.units.contains(u) {
                    continue;
                }
                let race = g.units[u].race;
                if race == Some(demon) || race == Some(undead) {
                    g.unit_do_damage(u, 50, 100, Some(caster), Some(holy), false);
                }
            }
        }
    }
}

/// `reqDomination(caster)`.
fn req_domination(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    let resist_max = if g.players[p].human { 95 } else { 20 };
    let spell = g.d.t::<SpellType>("SPELL_DOMINATION");
    let (x, y) = xy(g, caster);
    for (_, _, pl) in square(g, x, y, 1) {
        let Some(pl) = pl else { continue };
        for &u in &g.map.plots[pl].units {
            if g.unit_is_alive(u)
                && !g.units[u].death_delay
                && caster_at_war(g, caster, g.unit_get_team(u))
                && g.unit_get_resist_chance(u, caster, spell) <= resist_max
            {
                return true;
            }
        }
    }
    false
}

/// `spellDomination(caster)`.
fn spell_domination(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let spell = d.t::<SpellType>("SPELL_DOMINATION");
    let p = g.units[caster].owner;
    let resist_max = if g.players[p].human { 95 } else { 20 };
    let mut best_value = 0;
    let mut best_unit = None;
    let (x, y) = xy(g, caster);
    for (_, _, pl) in square(g, x, y, 1) {
        let Some(pl) = pl else { continue };
        for &u in &g.map.plots[pl].units {
            if g.unit_is_alive(u) && !g.units[u].death_delay && caster_at_war(g, caster, g.unit_get_team(u)) {
                let resist = g.unit_get_resist_chance(u, caster, spell);
                if resist <= resist_max {
                    let value = g.unit_base_combat_str(u) * 10 + (100 - resist);
                    if value > best_value {
                        best_value = value;
                        best_unit = Some(u);
                    }
                }
            }
        }
    }
    let Some(best) = best_unit else { return };
    let pl = g.unit_plot(caster);
    if !g.unit_is_resisted(best, caster, spell) {
        let (px, py) = (g.map.plots[pl].x, g.map.plots[pl].y);
        let unit_type = g.units[best].unit_type;
        let new_unit = init_unit(g, p, unit_type, px, py);
        g.unit_convert(new_unit, best);
        g.unit_change_immobile_timer(new_unit, 1);
    } else {
        g.unit_set_has_promotion(caster, d.t::<PromotionType>("PROMOTION_MIND3"), false);
    }
}

/// `reqEarthquake(caster)`.
fn req_earthquake(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if !g.players[p].human {
        let team = g.players[p].team;
        let (x, y) = xy(g, caster);
        for (_, _, pl) in square(g, x, y, 1) {
            // NULL plot: isOwned() is false.
            let Some(owner2) = pl.and_then(|pl| g.map.plots[pl].owner) else { continue };
            let team2 = g.players[owner2].team;
            if !g.teams[team2].at_war[team] {
                return false;
            }
        }
    }
    true
}

/// `spellEarthquake(caster)`.
fn spell_earthquake(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    let (x, y) = xy(g, caster);
    for (_, _, pl) in square(g, x, y, 1) {
        let Some(pl) = pl else { continue };
        if g.map.plots[pl].plot_city.is_some() || g.map.plots[pl].improvement_type.is_some() {
            if let Some(owner2) = g.map.plots[pl].owner {
                g.cf_start_war(p, owner2, WarPlanType::WARPLAN_TOTAL);
            }
        }
        if let Some(c) = g.map.plots[pl].plot_city {
            for b in d.buildings.keys() {
                let rnd = g.soren_rand_num(100, "Bob");
                if d.buildings[b].conquest_probability != 100 && rnd <= 25 {
                    g.city_set_num_real_building(c, b, 0);
                }
            }
        }
        for i in 0..num_units(g, pl) {
            // NULL unit: isFlying() is false and setFortifyTurns is a no-op.
            if let Some(u) = plot_unit(g, pl, i) {
                if !g.unit_is_flying(u) {
                    g.unit_set_fortify_turns(u, 0);
                }
            }
        }
        let rnd = g.soren_rand_num(100, "Bob");
        if rnd <= 25 {
            if let Some(imp) = g.map.plots[pl].improvement_type {
                if !d.improvements[imp].permanent {
                    g.plot_set_improvement_type(pl, None);
                }
            }
        }
    }
}

/// `spellEnterPortal(caster)`.
fn spell_enter_portal(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let pl = g.unit_plot(caster);
    let (x, y) = (g.map.plots[pl].portal_exit_x, g.map.plots[pl].portal_exit_y);
    // Python looks the exit plot up but tests `pPlot.isNone()` -- the caster's own plot, which
    // is never None -- so the caster always moves.
    g.unit_set_xy(caster, x, y, false);
}

/// `spellEntertain(caster)`.
fn spell_entertain(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let Some(c) = caster_city(g, caster) else { return };
    let p = g.units[caster].owner;
    let p2 = g.cities[c].owner;
    if p != p2 {
        // CvPlayer::changeGold only adds UI side effects to the member update.
        let gold = py_div(g.cities[c].population, 2) + 1;
        g.players[p].gold += gold;
        g.players[p2].gold -= gold;
    }
    g.city_change_happiness_timer(c, 2);
}

/// `reqEscape(caster)`.
fn req_escape(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if p == g.barbarian_player() {
        return false;
    }
    if !g.players[p].human && g.units[caster].damage >= 50 {
        return false;
    }
    true
}

/// `reqExploreLair(caster)`.
fn req_explore_lair(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    if g.unit_is_only_defensive(caster) {
        return false;
    }
    if g.unit_get_unit_combat_type(caster) == Some(d.t::<UnitCombatType>("UNITCOMBAT_SIEGE")) {
        return false;
    }
    if g.unit_is_barbarian(caster) {
        return false;
    }
    if g.units[caster].duration > 0 {
        return false;
    }
    let special = g.unit_get_special_unit_type(caster);
    if special == Some(d.t::<SpecialUnitType>("SPECIALUNIT_SPELL")) {
        return false;
    }
    if special == Some(d.t::<SpecialUnitType>("SPECIALUNIT_BIRD")) {
        return false;
    }
    let pl = g.unit_plot(caster);
    if !g.plot_is_water(pl) {
        let barbarian_team = g.players[g.barbarian_player()].team;
        if !caster_at_war(g, caster, barbarian_team) {
            return false;
        }
    }
    true
}

/// Tail of `spellExploreLair` / `spellExploreLairEpic`: maybe destroy the lair, end the moves,
/// one experience point.
fn explore_lair_finish(g: &mut Game, caster: UnitId, pl: PlotIdx, destroy_lair: i32) {
    if destroy_lair > g.soren_rand_num(100, "Explore Lair") {
        g.plot_set_improvement_type(pl, None);
    }
    g.unit_finish_moves(caster);
    g.unit_change_experience(caster, 1, -1, false, false, false);
}

/// `spellExploreLair(caster)`.
fn spell_explore_lair(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let pl = g.unit_plot(caster);
    let rnd = g.soren_rand_num(100, "Explore Lair") + g.units[caster].level;
    let mut destroy_lair = 0;
    if rnd < 14 {
        destroy_lair = g.cf_explore_lair_big_bad(caster);
    }
    if (14..44).contains(&rnd) {
        destroy_lair = g.cf_explore_lair_bad(caster);
    }
    if (44..74).contains(&rnd) {
        destroy_lair = g.cf_explore_lair_neutral(caster);
    }
    if (74..94).contains(&rnd) {
        destroy_lair = g.cf_explore_lair_good(caster);
    }
    if rnd >= 94 {
        destroy_lair = g.cf_explore_lair_big_good(caster);
    }
    explore_lair_finish(g, caster, pl, destroy_lair);
}

/// `spellExploreLairEpic(caster)`.
fn spell_explore_lair_epic(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let pl = g.unit_plot(caster);
    let rnd = g.soren_rand_num(100, "Explore Lair") + g.units[caster].level;
    let mut destroy_lair = 0;
    if rnd < 54 {
        destroy_lair = g.cf_explore_lair_big_bad(caster);
    }
    if rnd >= 54 {
        destroy_lair = g.cf_explore_lair_big_good(caster);
    }
    explore_lair_finish(g, caster, pl, destroy_lair);
}

/// `reqFeast(caster)`.
fn req_feast(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    // NULL city: getPopulation() is -1.
    match caster_city(g, caster) {
        Some(c) => g.cities[c].population >= 3,
        None => false,
    }
}

/// `spellFeast(caster)`.
fn spell_feast(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let Some(c) = caster_city(g, caster) else { return };
    let xp = g.cities[c].population - 2;
    g.unit_change_experience(caster, xp, -1, false, false, false);
    g.city_change_hurry_anger_timer(c, 3);
}

/// `reqFeed(caster)`.
fn req_feed(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let damage = g.units[caster].damage;
    if damage == 0 {
        return false;
    }
    if !g.players[g.units[caster].owner].human && damage < 20 {
        return false;
    }
    true
}

/// `spellFeed(caster)`.
fn spell_feed(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let damage = g.units[caster].damage - 20;
    g.unit_set_damage(caster, damage, Some(p));
    let bloodpet = g.d.t::<UnitType>("UNIT_BLOODPET");
    let pl = g.unit_plot(caster);
    let mut victim: Option<UnitId> = None;
    for &u in &g.map.plots[pl].units {
        if g.units[u].owner == p && g.units[u].unit_type == bloodpet {
            if victim.is_none_or(|v| g.units[v].level > g.units[u].level) {
                victim = Some(u);
            }
        }
    }
    if let Some(v) = victim {
        // Python passes player 0 as the killer.
        g.unit_kill(v, true, Some(PlayerId(0)));
    }
}

/// `reqForTheHorde(caster)`. Change from FFH2: its AI-only branch (the barbarians have 60 units) is
/// AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_for_the_horde(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let barbarian = g.barbarian_player();
    if caster_at_war(g, caster, g.players[barbarian].team) {
        return false;
    }
    if g.players[barbarian].num_units() == 0 {
        return false;
    }
    true
}

/// `spellForTheHorde(caster)`.
fn spell_for_the_horde(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let hero = g.d.t::<PromotionType>("PROMOTION_HERO");
    let orc = g.d.t::<PromotionType>("PROMOTION_ORC");
    let p = g.units[caster].owner;
    let barbarian = g.barbarian_player();
    for u in py_unit_list(g, barbarian) {
        if !g.units.contains(u) {
            continue;
        }
        if g.units[u].race == Some(orc) && !g.units[u].has_promotion[hero] && g.soren_rand_num(100, "Bob") < 50 {
            let pl = g.unit_plot(u);
            // range(pPlot.getNumUnits(), -1, -1): the first index is one past the end.
            for i in (0..=num_units(g, pl)).rev() {
                // NULL unit: isHiddenNationality() is false.
                let Some(lu) = plot_unit(g, pl, i) else { continue };
                if g.unit_is_hidden_nationality(lu) {
                    // cf.findClearPlot(pLoopUnit, -1): the plot argument is unused with a unit.
                    if let Some(np) = g.cf_find_clear_plot(Some(lu), pl) {
                        let (nx, ny) = (g.map.plots[np].x, g.map.plots[np].y);
                        g.unit_set_xy(lu, nx, ny, false);
                    }
                }
            }
            let unit_type = g.units[u].unit_type;
            let (x, y) = xy(g, u);
            let new_unit = init_unit(g, p, unit_type, x, y);
            g.unit_convert(new_unit, u);
        }
    }
}

/// `reqFormWolfPack(caster)`.
fn req_form_wolf_pack(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let wolf = g.d.t::<UnitType>("UNIT_WOLF");
    let pl = g.unit_plot(caster);
    let count = g.map.plots[pl]
        .units
        .iter()
        .filter(|&&u| g.units[u].unit_type == wolf && g.units[u].owner == g.units[caster].owner)
        .count();
    count >= 2
}

/// `spellFormWolfPack(caster)`.
fn spell_form_wolf_pack(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let wolf = d.t::<UnitType>("UNIT_WOLF");
    let pl = g.unit_plot(caster);
    let mut wolf2 = None;
    for &u in &g.map.plots[pl].units {
        if g.units[u].unit_type == wolf && g.units[u].owner == g.units[caster].owner && u != caster {
            wolf2 = Some(u);
        }
    }
    if let Some(wolf2) = wolf2 {
        let p = g.units[caster].owner;
        let (x, y) = xy(g, caster);
        let new_unit = init_unit(g, p, d.t::<UnitType>("UNIT_WOLF_PACK"), x, y);
        let xp = g.units[caster].experience + g.units[wolf2].experience;
        g.unit_set_experience(new_unit, xp, -1);
        // Python passes player 0 as the killer.
        g.unit_kill(caster, true, Some(PlayerId(0)));
        g.unit_kill(wolf2, true, Some(PlayerId(0)));
    }
}

/// `reqGiftsOfNantosuelta(caster)`. Change from FFH2: its AI-only branch (5 cities) is AI timing,
/// in ffh-ai `world_spells` (docs/PORTING.md).
fn req_gifts_of_nantosuelta(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let player = &g.players[g.units[caster].owner];
    if player.num_cities() == 0 {
        return false;
    }
    true
}

/// `spellGiftsOfNantosuelta(caster)`.
fn spell_gifts_of_nantosuelta(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let golden_hammer = g.d.t::<UnitType>("EQUIPMENT_GOLDEN_HAMMER");
    for c in py_city_list(g, p) {
        let (x, y) = (g.cities[c].x, g.cities[c].y);
        init_unit(g, p, golden_hammer, x, y);
    }
}

/// The unit qualifies for `reqGiftVampirism` / `spellGiftVampirism`: (level >= 6, Moroi at
/// level >= 4) for an alive unit of the caster's owner without Vampire.
fn vampirism_candidate(g: &Game, caster: UnitId, u: UnitId) -> Option<(bool, bool)> {
    let d = g.d;
    let unit = &g.units[u];
    if g.unit_is_alive(u) && unit.owner == g.units[caster].owner && !unit.has_promotion[d.t::<PromotionType>("PROMOTION_VAMPIRE")] {
        Some((unit.level >= 6, unit.level >= 4 && unit.unit_type == d.t::<UnitType>("UNIT_MOROI")))
    } else {
        None
    }
}

/// `reqGiftVampirism(caster)`.
fn req_gift_vampirism(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let pl = g.unit_plot(caster);
    for &u in &g.map.plots[pl].units {
        if let Some((level6, moroi4)) = vampirism_candidate(g, caster, u) {
            if level6 || moroi4 {
                return true;
            }
        }
    }
    false
}

/// `spellGiftVampirism(caster)`.
fn spell_gift_vampirism(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let vampire = g.d.t::<PromotionType>("PROMOTION_VAMPIRE");
    let pl = g.unit_plot(caster);
    for i in 0..num_units(g, pl) {
        // NULL unit: isAlive() is false.
        let Some(u) = plot_unit(g, pl, i) else { continue };
        if let Some((level6, moroi4)) = vampirism_candidate(g, caster, u) {
            if level6 {
                g.unit_set_has_promotion(u, vampire, true);
            }
            if moroi4 {
                g.unit_set_has_promotion(u, vampire, true);
            }
        }
    }
}

/// `spellGiveHammerToCraftsman(caster)`.
fn spell_give_hammer_to_craftsman(g: &mut Game, caster: UnitId, _call: &PyCall) {
    if let Some(c) = caster_city(g, caster) {
        g.city_change_free_specialist_count(c, g.d.t::<SpecialistType>("SPECIALIST_ENGINEER"), 1);
    }
}

/// `reqHastursRazor(caster)`.
fn req_hasturs_razor(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    let (x, y) = xy(g, caster);
    for (_, _, pl) in square(g, x, y, 1) {
        let Some(pl) = pl else { continue };
        for &u in &g.map.plots[pl].units {
            if g.units[u].damage > 0 {
                if g.players[p].human {
                    return true;
                }
                if g.units[u].owner == p {
                    return true;
                }
            }
        }
    }
    false
}

/// `spellHastursRazor(caster)`.
fn spell_hasturs_razor(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let (x, y) = xy(g, caster);
    let plots = square(g, x, y, 1);
    let mut list_damage = Vec::new();
    for &(_, _, pl) in &plots {
        if let Some(pl) = pl {
            for &u in &g.map.plots[pl].units {
                list_damage.push(g.units[u].damage);
            }
        }
    }
    for &(_, _, pl) in &plots {
        let Some(pl) = pl else { continue };
        for i in 0..num_units(g, pl) {
            let rnd = list_damage[g.soren_rand_num(list_damage.len() as i32, "Hasturs Razor") as usize];
            // NULL unit: getDamage() is -1 (never equal) and setDamage is a no-op.
            if let Some(u) = plot_unit(g, pl, i) {
                if rnd != g.units[u].damage {
                    g.unit_set_damage(u, rnd, Some(p));
                }
            }
        }
    }
}

/// `reqHeal(caster)`.
fn req_heal(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let poisoned = g.d.t::<PromotionType>("PROMOTION_POISONED");
    let pl = g.unit_plot(caster);
    for &u in &g.map.plots[pl].units {
        if g.unit_is_alive(u) && g.units[u].damage > 0 {
            return true;
        }
        if g.units[u].has_promotion[poisoned] {
            return true;
        }
    }
    false
}

/// `spellHeal(caster, amount)` (XML: `spellHeal(pCaster,20)`).
fn spell_heal(g: &mut Game, caster: UnitId, call: &PyCall) {
    // Without the amount argument Python raises TypeError before doing anything.
    let Some(amount) = call.int_arg(1) else { return };
    let poisoned = g.d.t::<PromotionType>("PROMOTION_POISONED");
    let pl = g.unit_plot(caster);
    for i in 0..num_units(g, pl) {
        // NULL unit: every call is a no-op.
        let Some(u) = plot_unit(g, pl, i) else { continue };
        g.unit_set_has_promotion(u, poisoned, false);
        if g.unit_is_alive(u) {
            // Python: "player doesn't matter - it won't kill" (passes player 0).
            g.unit_change_damage(u, -amount, Some(PlayerId(0)));
        }
    }
}

/// `reqHealingSalve(caster)`.
fn req_healing_salve(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let damage = g.units[caster].damage;
    if damage == 0 {
        return false;
    }
    if !g.players[g.units[caster].owner].human && damage < 25 {
        return false;
    }
    true
}

/// `spellHealingSalve(caster)`.
fn spell_healing_salve(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    g.unit_set_damage(caster, 0, Some(p));
    for name in ["PROMOTION_DISEASED", "PROMOTION_PLAGUED", "PROMOTION_POISONED", "PROMOTION_WITHERED"] {
        g.unit_set_has_promotion(caster, d.t::<PromotionType>(name), false);
    }
}

/// `reqHellfire(caster)`.
fn req_hellfire(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let p = g.units[caster].owner;
    if g.player_get_civilization_type(p) != Some(d.t::<CivilizationType>("CIVILIZATION_INFERNAL")) {
        return false;
    }
    let pl = g.unit_plot(caster);
    if g.map.plots[pl].plot_city.is_some() {
        return false;
    }
    if g.plot_is_water(pl) {
        return false;
    }
    if g.map.plots[pl].improvement_type.is_some() {
        return false;
    }
    let hellfire = d.t::<ImprovementType>("IMPROVEMENT_HELLFIRE");
    let (x, y) = (g.map.plots[pl].x, g.map.plots[pl].y);
    for (_, _, p2) in square(g, x, y, 2) {
        if p2.is_some_and(|p2| g.map.plots[p2].improvement_type == Some(hellfire)) {
            return false;
        }
    }
    if !g.players[p].human && g.plot_is_owned(pl) && g.map.plots[pl].owner == Some(p) {
        return false;
    }
    true
}

/// `reqHeraldsCall(caster)`.
fn req_heralds_call(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let pl = g.unit_plot(caster);
    g.map.plots[pl].units.iter().any(|&u| g.unit_is_alive(u) && g.units[u].owner == g.units[caster].owner)
}

/// `spellHeraldsCall(caster)`.
fn spell_heralds_call(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let proms = [
        d.t::<PromotionType>("PROMOTION_VALOR"),
        d.t::<PromotionType>("PROMOTION_BLESSED"),
        d.t::<PromotionType>("PROMOTION_COURAGE"),
        d.t::<PromotionType>("PROMOTION_LOYALTY"),
    ];
    let pl = g.unit_plot(caster);
    for i in 0..num_units(g, pl) {
        // NULL unit: isAlive() is false.
        let Some(u) = plot_unit(g, pl, i) else { continue };
        if g.unit_is_alive(u) && g.units[u].owner == g.units[caster].owner {
            for prom in proms {
                g.unit_set_has_promotion(u, prom, true);
            }
            g.units[u].duration = 1;
        }
    }
}

/// `reqHide(caster)`.
fn req_hide(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    if g.units[caster].made_attack {
        return false;
    }
    if g.units[caster].has_promotion[g.d.t::<PromotionType>("PROMOTION_HIDDEN")] {
        return false;
    }
    true
}

/// `reqHireScorpionClan(caster)`.
fn req_hire_scorpion_clan(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let barbarian_team = g.players[g.barbarian_player()].team;
    !caster_at_war(g, caster, barbarian_team)
}

/// Body of the `spellHire*` unit spells: one `unit` at the caster.
fn hire_unit(g: &mut Game, caster: UnitId, unit: &str) {
    let p = g.units[caster].owner;
    let (x, y) = xy(g, caster);
    let unit = g.d.t::<UnitType>(unit);
    init_unit(g, p, unit, x, y);
}

/// `spellHireArcher(caster)`.
fn spell_hire_archer(g: &mut Game, caster: UnitId, _call: &PyCall) {
    hire_unit(g, caster, "UNIT_ARCHER_SCORPION_CLAN");
}

/// `spellHireChariot(caster)`.
fn spell_hire_chariot(g: &mut Game, caster: UnitId, _call: &PyCall) {
    hire_unit(g, caster, "UNIT_CHARIOT_SCORPION_CLAN");
}

/// `spellHireGoblin(caster)`.
fn spell_hire_goblin(g: &mut Game, caster: UnitId, _call: &PyCall) {
    hire_unit(g, caster, "UNIT_GOBLIN_SCORPION_CLAN");
}

/// `spellHireUnits(caster)`.
fn spell_hire_units(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let trigger = g.d.t::<EventTriggerType>("EVENTTRIGGER_MAGNADINE_HIRE_UNITS");
    // NULL city: getID(), getX(), getY() are -1.
    let city = caster_city(g, caster);
    let (x, y) = city.map_or((-1, -1), |c| (g.cities[c].x, g.cities[c].y));
    g.player_init_triggered_data(p, trigger, true, city, x, y, Some(p), None, None, None, None, None);
}

/// `spellHireWolfRider(caster)`.
fn spell_hire_wolf_rider(g: &mut Game, caster: UnitId, _call: &PyCall) {
    hire_unit(g, caster, "UNIT_WOLF_RIDER_SCORPION_CLAN");
}

/// `reqHyboremsWhisper(caster)`.
fn req_hyborems_whisper(g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    if g.is_network_multi_player() {
        return false;
    }
    g.cf_get_ashen_veil_city(3).is_some()
}

/// `spellHyboremsWhisper(caster)`.
fn spell_hyborems_whisper(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let trigger = g.d.t::<EventTriggerType>("EVENTTRIGGER_HYBOREMS_WHISPER");
    let (x, y) = xy(g, caster);
    g.player_init_triggered_data(p, trigger, true, None, x, y, Some(p), None, None, None, None, None);
}

/// `reqImpersonateLeader(caster)`.
fn req_impersonate_leader(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    // NULL city: gc.getPlayer(-1) is None, so `.isHuman()` raises: canCast reads that as false.
    let Some(c) = caster_city(g, caster) else { return false };
    let owner = g.cities[c].owner;
    if owner == g.units[caster].owner {
        return false;
    }
    if g.players[owner].human {
        return false;
    }
    true
}

/// `spellImpersonateLeader(caster)`.
fn spell_impersonate_leader(g: &mut Game, caster: UnitId, _call: &PyCall) {
    // NULL city: the new player is -1 and reassignPlayerAdvanced does nothing (after the rand).
    let new_player = caster_city(g, caster).map(|c| g.cities[c].owner);
    let old_player = g.units[caster].owner;
    let timer = 5 + g.soren_rand_num(10, "Impersonate Leader");
    if let Some(new_player) = new_player {
        reassign_player_advanced(g, old_player, new_player, timer);
    }
}

/// `reqInquisition(caster)`.
fn req_inquisition(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    // NULL city: Python returns False or raises on gc.getPlayer(-1) (canCast reads false).
    let Some(c) = caster_city(g, caster) else { return false };
    let state_belief = g.player_get_state_religion(p);
    if state_belief.is_none() && p != g.cities[c].owner {
        return false;
    }
    if state_belief != g.player_get_state_religion(g.cities[c].owner) {
        return false;
    }
    for target in g.d.religions.keys() {
        if state_belief != Some(target) && g.cities[c].has_religion[target] && !g.city_is_holy_city(c, target) {
            return true;
        }
    }
    false
}

/// `spellInquisition(caster)`.
fn spell_inquisition(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    // NULL city: gc.getPlayer(-1) is None, so Python raises before the rand.
    let Some(c) = caster_city(g, caster) else { return };
    let state_belief = g.player_get_state_religion(g.cities[c].owner);
    let mut rnd = g.soren_rand_num(4, "Bob");
    if state_belief == Some(d.t::<ReligionType>("RELIGION_THE_ORDER")) {
        rnd -= 1;
    }
    for target in d.religions.keys() {
        if state_belief != Some(target) && g.cities[c].has_religion[target] && !g.city_is_holy_city(c, target) {
            g.city_set_has_religion(c, target, false, true, true);
            rnd += 1;
            for b in d.buildings.keys() {
                if d.buildings[b].prereq_religion == Some(target) {
                    g.city_set_num_real_building(c, b, 0);
                }
            }
        }
    }
    if rnd >= 1 {
        g.city_change_hurry_anger_timer(c, rnd);
    }
}

/// `reqIntoTheMist(caster)`. Change from FFH2: its only condition was AI-only (40 units); it is AI
/// timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_into_the_mist(_g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    true
}

/// `spellIntoTheMist(caster)`.
fn spell_into_the_mist(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let hidden = g.d.t::<PromotionType>("PROMOTION_HIDDEN");
    let p = g.units[caster].owner;
    for u in py_unit_list(g, p) {
        if g.units.contains(u) {
            g.unit_set_has_promotion(u, hidden, true);
        }
    }
}

/// `reqIraUnleashed(caster)`.
fn req_ira_unleashed(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    g.players[p].unit_class_count[g.d.t::<UnitClassType>("UNITCLASS_IRA")] < 4
}

/// `spellIraUnleashed(caster)`.
fn spell_ira_unleashed(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    let count = 4 - g.players[p].unit_class_count[d.t::<UnitClassType>("UNITCLASS_IRA")];
    let ira = d.t::<UnitType>("UNIT_IRA");
    let (x, y) = xy(g, caster);
    for _ in 0..count {
        init_unit(g, p, ira, x, y);
    }
}

/// The great-person specialists `reqKidnap` / `spellKidnap` look at, in Python order, with the
/// unit `spellKidnap` creates for each.
const KIDNAP_TARGETS: [(&str, &str); 5] = [
    ("SPECIALIST_GREAT_PRIEST", "UNIT_PROPHET"),
    ("SPECIALIST_GREAT_ARTIST", "UNIT_ARTIST"),
    ("SPECIALIST_GREAT_MERCHANT", "UNIT_MERCHANT"),
    ("SPECIALIST_GREAT_ENGINEER", "UNIT_ENGINEER"),
    ("SPECIALIST_GREAT_SCIENTIST", "UNIT_SCIENTIST"),
];

/// `pCity.getFreeSpecialistCount(eSpecialist)`; the NULL city answers -1.
fn free_specialist_count(g: &Game, city: Option<CityId>, specialist: SpecialistType) -> i32 {
    city.map_or(-1, |c| g.cities[c].free_specialist_count[specialist])
}

/// `reqKidnap(caster)`.
fn req_kidnap(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let city = caster_city(g, caster);
    // NULL city: getTeam() is NO_TEAM and every count is -1 (so the sum is not 0).
    if let Some(c) = city {
        if g.city_team(c) == g.unit_get_team(caster) {
            return false;
        }
    }
    let mut i = 0;
    for (specialist, _) in KIDNAP_TARGETS {
        i += free_specialist_count(g, city, g.d.t::<SpecialistType>(specialist));
    }
    i != 0
}

/// `spellKidnap(caster)`.
fn spell_kidnap(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let city = caster_city(g, caster);
    // iUnit/iSpec: the last specialist present wins; unbound (None) when there is none.
    let mut target = None;
    for (specialist, unit) in KIDNAP_TARGETS {
        let specialist = d.t::<SpecialistType>(specialist);
        if free_specialist_count(g, city, specialist) > 0 {
            target = Some((d.t::<UnitType>(unit), specialist));
        }
    }
    let mut chance = g.unit_base_combat_str(caster) * 8;
    if chance > 95 {
        chance = 95;
    }
    let p = g.units[caster].owner;
    if g.soren_rand_num(100, "Kidnap") <= chance {
        // Unbound iUnit: Python raises NameError here.
        let Some((unit, specialist)) = target else { return };
        let (x, y) = xy(g, caster);
        init_unit(g, p, unit, x, y);
        if let Some(c) = city {
            g.city_change_free_specialist_count(c, specialist, -1);
        }
    } else {
        if g.soren_rand_num(100, "Kidnap") <= 50 {
            // NULL capital: getX()/getY() are -1.
            let (cx, cy) = g.player_get_capital_city(p).map_or((-1, -1), |c| (g.cities[c].x, g.cities[c].y));
            g.unit_set_xy(caster, cx, cy, false);
        } else {
            // Python passes player 0 as the killer.
            g.unit_kill(caster, true, Some(PlayerId(0)));
        }
        // NULL city: startWar(.., -1, ..) raises on gc.getPlayer(-1).
        if let Some(c) = city {
            let city_owner = g.cities[c].owner;
            g.cf_start_war(p, city_owner, WarPlanType::WARPLAN_TOTAL);
        }
    }
}

/// `reqLegends(caster)`. Change from FFH2: its AI-only branch (5 cities) is AI timing, in ffh-ai
/// `world_spells` (docs/PORTING.md).
fn req_legends(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let player = &g.players[g.units[caster].owner];
    if player.num_cities() == 0 {
        return false;
    }
    true
}

/// `spellLegends(caster)`.
fn spell_legends(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    for c in py_city_list(g, p) {
        g.city_change_culture(c, p, 300, true, true);
    }
}

/// `reqLichdom(caster)`.
fn req_lichdom(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let unit_class = g.unit_get_unit_class_type(caster);
    if unit_class == d.t::<UnitClassType>("UNITCLASS_FLESH_GOLEM") {
        return false;
    }
    if g.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_PUPPET")] {
        return false;
    }
    if is_world_unit_class(d, unit_class) {
        return false;
    }
    let p = g.units[caster].owner;
    if g.players[p].unit_class_count[d.t::<UnitClassType>("UNITCLASS_LICH")] >= 4 {
        return false;
    }
    true
}

/// `reqMask(caster)`.
fn req_mask(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    if g.unit_is_hidden_nationality(caster) {
        return false;
    }
    if g.unit_has_cargo(caster) {
        return false;
    }
    if g.unit_is_cargo(caster) {
        return false;
    }
    // pGroup.isNone(): no selection group.
    if let Some(group) = g.units[caster].group.and_then(|grp| g.groups.get(grp)) {
        if group.units.len() > 1 {
            return false;
        }
    }
    true
}

/// `reqMezmerizeAnimal(caster)`.
fn req_mezmerize_animal(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let animal = g.d.t::<UnitCombatType>("UNITCOMBAT_ANIMAL");
    let (x, y) = xy(g, caster);
    for (_, _, pl) in square(g, x, y, 1) {
        let Some(pl) = pl else { continue };
        for &u in &g.map.plots[pl].units {
            if g.unit_get_unit_combat_type(u) == Some(animal) && caster_at_war(g, caster, g.unit_get_team(u)) {
                return true;
            }
        }
    }
    false
}

/// `spellMezmerizeAnimal(caster)`.
fn spell_mezmerize_animal(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let animal = g.d.t::<UnitCombatType>("UNITCOMBAT_ANIMAL");
    let spell = g.d.t::<SpellType>("SPELL_MEZMERIZE_ANIMAL");
    let p = g.units[caster].owner;
    let (x, y) = xy(g, caster);
    for (_, _, pl) in square(g, x, y, 1) {
        let Some(pl) = pl else { continue };
        for i in 0..num_units(g, pl) {
            // NULL unit: getUnitCombatType() is -1.
            let Some(u) = plot_unit(g, pl, i) else { continue };
            if g.unit_get_unit_combat_type(u) == Some(animal)
                && caster_at_war(g, caster, g.unit_get_team(u))
                && !g.units[u].death_delay
                && !g.unit_is_resisted(u, caster, spell)
            {
                let unit_type = g.units[u].unit_type;
                let (cx, cy) = xy(g, caster);
                let new_unit = init_unit(g, p, unit_type, cx, cy);
                g.unit_convert(new_unit, u);
            }
        }
    }
}

/// `reqMirror(caster)`.
fn req_mirror(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    let pl = g.unit_plot(caster);
    if g.unit_is_immune_to_magic(caster) {
        return false;
    }
    if g.plot_is_visible_enemy_unit(pl, p) {
        return false;
    }
    true
}

/// `spellMirror(caster)`.
fn spell_mirror(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    let unit_type = g.units[caster].unit_type;
    let (x, y) = xy(g, caster);
    let new_unit = init_unit(g, p, unit_type, x, y);
    for prom in d.promotions.keys() {
        let has = g.units[caster].has_promotion[prom];
        g.unit_set_has_promotion(new_unit, prom, has);
        if d.promotions[prom].equipment {
            g.unit_set_has_promotion(new_unit, prom, false);
        }
    }
    g.unit_set_has_promotion(new_unit, d.t::<PromotionType>("PROMOTION_ILLUSION"), true);
    g.unit_set_has_promotion(new_unit, d.t::<PromotionType>("PROMOTION_IMMORTAL"), false);
    if g.unit_is_immortal(new_unit) {
        g.unit_change_immortal(new_unit, -1);
    }
    let damage = g.units[caster].damage;
    g.unit_set_damage(new_unit, damage, Some(p));
    let level = g.units[caster].level;
    g.unit_set_level(new_unit, level);
    let experience = g.units[caster].experience;
    g.unit_set_experience(new_unit, experience, -1);
    g.units[new_unit].has_casted = true;
    g.units[new_unit].duration = 1;
}

/// `reqPeace(caster)`.
fn req_peace(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    if g.is_option(GameOptionType::GAMEOPTION_NO_CHANGING_WAR_PEACE) {
        return false;
    }
    let team = g.players[g.units[caster].owner].team;
    g.team_get_at_war_count(team, true) != 0
}

/// Body shared by `spellPeace` / `spellPeaceSevenPines`: peace with every alive non-barbarian
/// player at war, then the Armageddon counter loses half of its percentage.
fn make_peace_with_all(g: &mut Game, caster: UnitId) {
    let owner = g.units[caster].owner;
    let team = g.players[owner].team;
    let barbarian = g.barbarian_player();
    for p2 in g.player_ids() {
        if g.players[p2].alive && p2 != owner && p2 != barbarian {
            let team2 = g.players[p2].team;
            if g.teams[team].at_war[team2] {
                g.team_make_peace(team, team2, true);
            }
        }
    }
    // getGlobalCounter() is a percentage; changeGlobalCounter adds raw points.
    let counter = g.get_global_counter();
    g.change_global_counter(-py_div(counter, 2));
}

/// `spellPeace(caster)`.
fn spell_peace(g: &mut Game, caster: UnitId, _call: &PyCall) {
    make_peace_with_all(g, caster);
}

/// `reqPeaceSevenPines(caster)`.
fn req_peace_seven_pines(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let pl = g.unit_plot(caster);
    if !g.map.plots[pl].python_active {
        return false;
    }
    let team = g.players[g.units[caster].owner].team;
    g.team_get_at_war_count(team, true) != 0
}

/// `spellPeaceSevenPines(caster)`.
fn spell_peace_seven_pines(g: &mut Game, caster: UnitId, _call: &PyCall) {
    make_peace_with_all(g, caster);
    let pl = g.unit_plot(caster);
    g.map.plots[pl].python_active = false;
}

/// `reqPillarofFire(caster)`.
fn req_pillar_of_fire(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    req_enemy_only_plot_within_2(g, caster)
}

/// `spellPillarofFire(caster)`.
fn spell_pillar_of_fire(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let fire = d.t::<DamageType>("DAMAGE_FIRE");
    let (x, y) = xy(g, caster);
    let mut best_value = 0;
    let mut best_plot = None;
    // Python's loop variable `pPlot` after the scan: the last plot visited, (x + 2, y + 2).
    let mut last_plot = None;
    for (_, _, pl) in square(g, x, y, 2) {
        last_plot = pl;
        // NULL plot: no units, value 0.
        let Some(pl) = pl else { continue };
        let mut neutral = false;
        let mut value = 0;
        for &u in &g.map.plots[pl].units {
            if caster_at_war(g, caster, g.unit_get_team(u)) {
                value += 5 * g.unit_base_combat_str(u);
            } else {
                neutral = true;
            }
        }
        if value > best_value && !neutral {
            best_value = value;
            best_plot = Some(pl);
        }
    }
    let Some(best) = best_plot else { return };
    for i in 0..num_units(g, best) {
        // NULL unit: doDamage is a no-op.
        if let Some(u) = plot_unit(g, best, i) {
            g.unit_do_damage(u, 50, 75, Some(caster), Some(fire), true);
        }
    }
    // Python quirk: the forest/jungle test uses `pPlot` (the last plot scanned), not pBestPlot.
    if let Some(pl) = last_plot {
        let feature = g.map.plots[pl].feature_type;
        if feature == Some(d.t::<FeatureType>("FEATURE_FOREST")) || feature == Some(d.t::<FeatureType>("FEATURE_JUNGLE")) {
            let mut valid = true;
            if let Some(imp) = g.map.plots[pl].improvement_type {
                if d.improvements[imp].permanent {
                    valid = false;
                }
            }
            if valid && g.soren_rand_num(100, "Flames Spread") <= d.defines.flames_spread_chance {
                g.plot_set_improvement_type(pl, Some(d.t::<ImprovementType>("IMPROVEMENT_SMOKE")));
            }
        }
    }
    // CyEngine().triggerEffect(EFFECT_PILLAR_OF_FIRE) is graphics only.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::{GameSetup, SlotStatus};

    /// Every hard-coded type name in this file (`d.t` with a string literal, and the name
    /// tables) exists in the data: `t` panics on unknown names, and most of this code only
    /// runs when a spell is cast.
    #[test]
    fn hard_coded_type_names_exist() {
        let d = ffh_data::global();
        let src = include_str!("spells_a.rs");
        let mut checked = 0;
        let mut rest = src;
        let marker = concat!("t::", "<");
        while let Some(i) = rest.find(marker) {
            rest = &rest[i + marker.len()..];
            let Some(close) = rest.find('>') else { break };
            let ty = &rest[..close];
            if ty.is_empty() || !ty.chars().all(|c| c.is_ascii_alphanumeric()) {
                continue;
            }
            let Some(lit) = rest[close + 1..].strip_prefix("(\"") else { continue };
            let name = &lit[..lit.find('"').unwrap()];
            let found = match ty {
                "BonusClassType" => d.ty::<BonusClassType>(name).is_some(),
                "BonusType" => d.ty::<BonusType>(name).is_some(),
                "CivilizationType" => d.ty::<CivilizationType>(name).is_some(),
                "DamageType" => d.ty::<DamageType>(name).is_some(),
                "EventTriggerType" => d.ty::<EventTriggerType>(name).is_some(),
                "FeatureType" => d.ty::<FeatureType>(name).is_some(),
                "GameSpeedType" => d.ty::<GameSpeedType>(name).is_some(),
                "ImprovementType" => d.ty::<ImprovementType>(name).is_some(),
                "PromotionType" => d.ty::<PromotionType>(name).is_some(),
                "ReligionType" => d.ty::<ReligionType>(name).is_some(),
                "SpecialUnitType" => d.ty::<SpecialUnitType>(name).is_some(),
                "SpecialistType" => d.ty::<SpecialistType>(name).is_some(),
                "SpellType" => d.ty::<SpellType>(name).is_some(),
                "TerrainType" => d.ty::<TerrainType>(name).is_some(),
                "TraitType" => d.ty::<TraitType>(name).is_some(),
                "UnitClassType" => d.ty::<UnitClassType>(name).is_some(),
                "UnitCombatType" => d.ty::<UnitCombatType>(name).is_some(),
                "UnitType" => d.ty::<UnitType>(name).is_some(),
                other => panic!("type {other} not handled by this test ({name})"),
            };
            assert!(found, "unknown {ty} {name}");
            checked += 1;
        }
        assert!(checked > 100, "only {checked} names found");
        for name in MANA_TYPES {
            assert!(d.ty::<BonusType>(name).is_some(), "{name}");
        }
        for (specialist, unit) in KIDNAP_TARGETS {
            assert!(d.ty::<SpecialistType>(specialist).is_some(), "{specialist}");
            assert!(d.ty::<UnitType>(unit).is_some(), "{unit}");
        }
        for (den, animal) in [("IMPROVEMENT_BEAR_DEN", "UNIT_BEAR"), ("IMPROVEMENT_LION_DEN", "UNIT_LION")] {
            assert!(d.ty::<ImprovementType>(den).is_some(), "{den}");
            assert!(d.ty::<UnitType>(animal).is_some(), "{animal}");
        }
        for unit in ["UNIT_ARCHER_SCORPION_CLAN", "UNIT_CHARIOT_SCORPION_CLAN", "UNIT_GOBLIN_SCORPION_CLAN", "UNIT_WOLF_RIDER_SCORPION_CLAN"] {
            assert!(d.ty::<UnitType>(unit).is_some(), "{unit}");
        }
        for name in ["PROMOTION_DISEASED", "PROMOTION_PLAGUED", "PROMOTION_POISONED", "PROMOTION_WITHERED"] {
            assert!(d.ty::<PromotionType>(name).is_some(), "{name}");
        }
        // The one name Python looks up that 0.41 does not define.
        assert!(d.ty::<PromotionType>("PROMOTION_CHANNELING").is_none());
    }

    /// Every callback name this module registers is used by the XML in the matching slot.
    #[test]
    fn registered_names_resolve() {
        for name in ["spellHeal", "spellPillarofFire", "spellAddToFleshGolem"] {
            assert!(find_spell(name).is_some(), "{name}");
        }
        for name in ["reqPillarofFire", "reqAddToFreakShowHuman", "reqHyboremsWhisper"] {
            assert!(find_req(name).is_some(), "{name}");
        }
        assert!(find_unit_fn("effectCrownOfBrillance").is_some());
        assert!(find_combat_fn("postCombatWolfRider").is_some());
        assert!(find_spell("reqPirateCove").is_none() && find_req("reqPirateCove").is_none());
        assert!(find_plot_fn("spellHeal").is_none() && find_vote_fn("spellHeal").is_none());
    }

    #[test]
    fn square_is_x_outer_y_inner_with_null_plots() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 1));
        let init = crate::map::MapInitData { grid_w: 4, grid_h: 3, wrap_x: true, ..Default::default() };
        g.map = crate::map::Map::new(d, &init, g.num_players(), g.num_teams());
        let sq = square(&g, 0, 0, 1);
        let coords: Vec<(i32, i32)> = sq.iter().map(|&(x, y, _)| (x, y)).collect();
        assert_eq!(coords, vec![(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 0), (0, 1), (1, -1), (1, 0), (1, 1)]);
        // x wraps, y = -1 is off the map (the NULL plot).
        assert_eq!(sq[0].2, None);
        assert_eq!(sq[1].2, g.map.plot_xy(3, 0));
        assert_eq!(sq[4].2, g.map.plot_xy(0, 0));
        assert!(sq[4].2.is_some());
    }

    #[test]
    fn reassign_player_advanced_swaps_slots() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 3, "WORLDSIZE_DUEL", 1));
        let (old, new) = (PlayerId(0), PlayerId(1));
        g.setup.players[0].human = true;
        g.setup.players[0].slot_status = SlotStatus::Taken;
        g.setup.players[0].handicap = d.ty::<HandicapType>("HANDICAP_DEITY");
        for p in g.player_ids() {
            // Game::new holds placeholder players; size what the test reads (player_init does).
            g.players[p].options = IdxVec::from_elem(false, PlayerOptionType::NUM);
            g.player_update_human(p);
        }
        let opt = PlayerOptionType::PLAYEROPTION_WAIT_END_TURN;
        g.players[old].options[opt] = true;
        let (c_old, c_new) = (g.controllers[old].clone(), g.controllers[new].clone());
        reassign_player_advanced(&mut g, old, new, 7);
        assert!(g.players[new].human && !g.players[old].human);
        assert_eq!(g.setup.players[1].slot_status, SlotStatus::Taken);
        assert_eq!(g.setup.players[0].slot_status, SlotStatus::Computer);
        assert_eq!(g.setup.players[1].handicap, d.ty::<HandicapType>("HANDICAP_DEITY"));
        assert!(g.players[new].options[opt] && !g.players[old].options[opt]);
        assert!(std::sync::Arc::ptr_eq(&g.controllers[new], &c_old));
        assert!(std::sync::Arc::ptr_eq(&g.controllers[old], &c_new));
        assert_eq!(g.players[new].temp_player_timer, 7);
        assert_eq!(g.players[new].real_player, Some(old));
        // Back with no timer (CvPlayer::doTurn when the timer runs out): no new temp player.
        reassign_player_advanced(&mut g, new, old, -1);
        assert!(g.players[old].human && !g.players[new].human);
        assert_eq!(g.players[old].temp_player_timer, 0);
        // The barbarian slot is never reassigned.
        let barbarian = g.barbarian_player();
        reassign_player_advanced(&mut g, old, barbarian, 3);
        assert!(g.players[old].human);
        assert_eq!(g.players[barbarian].temp_player_timer, 0);
    }
}
