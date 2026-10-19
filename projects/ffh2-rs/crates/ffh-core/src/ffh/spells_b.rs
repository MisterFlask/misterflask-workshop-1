//! Port of CvSpellInterface.py (part b): every function from `reqPirateCove` to the end of the
//! file (spells P..W, the improvement/feature `atRange*`/`onMove*` callbacks and
//! `voteFundDissidents`). Each Python function becomes a private Rust `fn` (named after it in
//! snake_case, the Python name in its doc comment) registered by its Python name in the
//! `find_*` lookups below (see dispatch.rs).
//!
//! Porting notes:
//! - `CyMap().plot(x, y)` is `g.map.plot_xy(x, y)`; `None` is Python's NULL plot, whose getters
//!   answer defaults (`getNumUnits()` -1, `getImprovementType()` -1, ...), so loops that skip
//!   `None` plots are exact even where the Python has no `isNone()` check.
//! - `for i in range(pPlot.getNumUnits()): pUnit = pPlot.getUnit(i)` takes the count once and
//!   then reads the live list; when the body can move or kill units this is [`plot_unit_at`],
//!   whose `None` is the NULL `CyUnit` Python gets once the list has shrunk.
//! - `PyPlayer(p).getUnitList()` is [`py_unit_list`] (skips dead units, as PyHelpers does),
//!   `getCityList()` is [`py_city_list`].
//! - A Python exception (e.g. `gc.getPlayer(-1)` returning `None`) ends the function; where
//!   the port can reach that case it returns at the same point.
//! - AI-only branches of the requirements (`if pPlayer.isHuman() == False: ...`) are kept: they
//!   are part of `canCast` for AI players.
//! - UI-only Python (messages, sounds, `CyEngine` effects, `setUnitArtStyleType`) is omitted.
//! - `CyGame().getWBMapScript()` is always false (worldbuilder scenarios are out of scope), so
//!   the scenario hooks (`sf.openChest`, `sf.onMovePortal`, ...) never run.

use crate::ffh::dispatch::*;
use crate::ffh::game_utils::py_div;
use crate::ffh::pycall::PyCall;
use crate::game::Game;
use crate::ids::*;
use crate::utils::is_world_unit_class;
use ffh_data::*;

/// `DirectionTypes.DIRECTION_SOUTH` (facing of units created by `initUnit`).
const SOUTH: Option<DirectionType> = Some(DirectionType::DIRECTION_SOUTH);
/// `DirectionTypes.DIRECTION_NORTH`.
const NORTH: Option<DirectionType> = Some(DirectionType::DIRECTION_NORTH);

/// PyResult functions (`spellX`).
pub fn find_spell(name: &str) -> Option<SpellFn> {
    let f: SpellFn = match name {
        "spellMarchOfTheTrees" => spell_march_of_the_trees,
        "spellMotherLode" => spell_mother_lode,
        "spellOpenChest" => spell_open_chest,
        "spellPromoteSettlement" => spell_promote_settlement,
        "spellRagingSeas" => spell_raging_seas,
        "spellRaiseSkeleton" => spell_raise_skeleton,
        "spellRally" => spell_rally,
        "spellReadTheGrimoire" => spell_read_the_grimoire,
        "spellRebuildBarnaxus" => spell_rebuild_barnaxus,
        "spellRecruit" => spell_recruit,
        "spellRecruitMercenary" => spell_recruit_mercenary,
        "spellReleaseFromCage" => spell_release_from_cage,
        "spellReligiousFervor" => spell_religious_fervor,
        "spellRepair" => spell_repair,
        "spellRessurection" => spell_ressurection,
        "spellRessurectionGraveyard" => spell_ressurection_graveyard,
        "spellReveal" => spell_reveal,
        "spellRevelry" => spell_revelry,
        "spellRevelation" => spell_revelation,
        "spellRingofFlames" => spell_ring_of_flames,
        "spellRiverOfBlood" => spell_river_of_blood,
        "spellRoar" => spell_roar,
        "spellRobGrave" => spell_rob_grave,
        "spellSacrificeAltar" => spell_sacrifice_altar,
        "spellSacrificePyre" => spell_sacrifice_pyre,
        "spellSanctify" => spell_sanctify,
        "spellSanctuary" => spell_sanctuary,
        "spellScorch" => spell_scorch,
        "spellSing" => spell_sing,
        "spellSironasTouch" => spell_sironas_touch,
        "spellSlaveTradeBuy" => spell_slave_trade_buy,
        "spellSnowfall" => spell_snowfall,
        "spellSnowfallGreator" => spell_snowfall_greator,
        "spellSpring" => spell_spring,
        "spellStasis" => spell_stasis,
        "spellSteal" => spell_steal,
        "spellTakeEquipmentBuilding" => spell_take_equipment_building,
        "spellTakeEquipmentPromotion" => spell_take_equipment_promotion,
        "spellTakeEquipmentUnit" => spell_take_equipment_unit,
        "spellTaunt" => spell_taunt,
        "spellTeleport" => spell_teleport,
        "spellTeachSpellcasting" => spell_teach_spellcasting,
        "spellTreetopDefence" => spell_treetop_defence,
        "spellTrust" => spell_trust,
        "spellTsunami" => spell_tsunami,
        "spellUnyieldingOrder" => spell_unyielding_order,
        "spellVeilOfNight" => spell_veil_of_night,
        "spellVitalize" => spell_vitalize,
        "spellWarcry" => spell_warcry,
        "spellWildHunt" => spell_wild_hunt,
        "spellWonder" => spell_wonder,
        "spellWorldbreak" => spell_worldbreak,
        _ => return None,
    };
    Some(f)
}

/// PyRequirement functions (`reqX`).
pub fn find_req(name: &str) -> Option<ReqFn> {
    let f: ReqFn = match name {
        "reqPirateCove" => req_pirate_cove,
        "reqMarchOfTheTrees" => req_march_of_the_trees,
        "reqMotherLode" => req_mother_lode,
        "reqPromoteSettlement" => req_promote_settlement,
        "reqRagingSeas" => req_raging_seas,
        "reqRally" => req_rally,
        "reqRebuildBarnaxus" => req_rebuild_barnaxus,
        "reqRecruitMercenary" => req_recruit_mercenary,
        "reqReligiousFervor" => req_religious_fervor,
        "reqRepair" => req_repair,
        "reqRessurection" => req_ressurection,
        "reqReveal" => req_reveal,
        "reqRevelry" => req_revelry,
        "reqRevelation" => req_revelation,
        "reqRiverOfBlood" => req_river_of_blood,
        "reqRoar" => req_roar,
        "reqRobGrave" => req_rob_grave,
        "reqSanctify" => req_sanctify,
        "reqSanctuary" => req_sanctuary,
        "reqSandLion" => req_sand_lion,
        "reqScorch" => req_scorch,
        "reqSironasTouch" => req_sironas_touch,
        "reqSpreadTheCouncilOfEsus" => req_spread_the_council_of_esus,
        "reqSpring" => req_spring,
        "reqSprint" => req_sprint,
        "reqStasis" => req_stasis,
        "reqSteal" => req_steal,
        "reqTakeEquipmentBuilding" => req_take_equipment_building,
        "reqTakeEquipmentPromotion" => req_take_equipment_promotion,
        "reqTakeEquipmentUnit" => req_take_equipment_unit,
        "reqTaunt" => req_taunt,
        "reqTeachSpellcasting" => req_teach_spellcasting,
        "reqTrust" => req_trust,
        "reqUpgradeDovielloWarrior" => req_upgrade_doviello_warrior,
        "reqVeilOfNight" => req_veil_of_night,
        "reqVitalize" => req_vitalize,
        "reqWane" => req_wane,
        "reqWarcry" => req_warcry,
        "reqWhiteout" => req_whiteout,
        "reqWildHunt" => req_wild_hunt,
        "reqWorldbreak" => req_worldbreak,
        _ => return None,
    };
    Some(f)
}

/// Unit-only effects: promotion PyPerTurn (`effectX`), spell PyMiscast (`miscastX`).
/// (None in this half of the file.)
pub fn find_unit_fn(name: &str) -> Option<UnitFn> {
    match name {
        _ => None,
    }
}

/// Plot callbacks: improvement/feature PythonOnMove (`onMoveX`), improvement PythonAtRange (`atRangeX`).
pub fn find_plot_fn(name: &str) -> Option<PlotFn> {
    let f: PlotFn = match name {
        "atRangeGuardian" => at_range_guardian,
        "atRangeJungleAltar" => at_range_jungle_altar,
        "atRangeNecrototem" => at_range_necrototem,
        "onMoveAncientForest" => on_move_ancient_forest,
        "onMoveBlizzard" => on_move_blizzard,
        "onMoveLetumFrigus" => on_move_letum_frigus,
        "onMoveMaelstrom" => on_move_maelstrom,
        "onMoveMirrorOfHeaven" => on_move_mirror_of_heaven,
        "onMovePoolOfTears" => on_move_pool_of_tears,
        "onMoveJungleAltar" => on_move_jungle_altar,
        "onMovePortal" => on_move_portal,
        "onMoveWarningPost" => on_move_warning_post,
        _ => return None,
    };
    Some(f)
}

/// Post-combat callbacks (`postCombatX`). (None in this half of the file.)
pub fn find_combat_fn(name: &str) -> Option<CombatFn> {
    match name {
        _ => None,
    }
}

/// Vote PyResult functions (`voteX`).
pub fn find_vote_fn(name: &str) -> Option<VoteFn> {
    let f: VoteFn = match name {
        "voteFundDissidents" => vote_fund_dissidents,
        _ => return None,
    };
    Some(f)
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

/// `pPlot.getUnit(i)`: the unit now at index `i` of the plot's unit list. `None` is the NULL
/// `CyUnit` Python gets when the list has shrunk below the count taken before the loop (every
/// getter of a NULL `CyUnit` answers a default and every action is a no-op).
fn plot_unit_at(g: &Game, plot: PlotIdx, i: usize) -> Option<UnitId> {
    g.map.plots[plot].units.get(i).copied()
}

/// `PyPlayer(p).getUnitList()`: snapshot of the player's units, without the dead ones
/// (PyHelpers appends a unit only `if not loopUnit.isDead()`).
fn py_unit_list(g: &Game, p: PlayerId) -> Vec<UnitId> {
    g.players[p].units.iter().copied().filter(|&u| !g.unit_is_dead(u)).collect()
}

/// `PyPlayer(p).getCityList()`: snapshot of the player's cities.
fn py_city_list(g: &Game, p: PlayerId) -> Vec<CityId> {
    g.players[p].cities.clone()
}

/// `gc.getCivilizationInfo(pPlayer.getCivilizationType()).getCivilizationUnits(eClass)`.
fn civ_unit(g: &Game, p: PlayerId, class: UnitClassType) -> Option<UnitType> {
    let civ = g.player_get_civilization_type(p).expect("player without a civilization");
    g.d.civ_unit(civ, class)
}

/// Python `lList[i]` for `-len <= i < len` (a negative index counts from the end, so
/// `lList[getSorenRandNum(len, ..) - 1]` maps a roll of 0 to the last entry).
fn py_list_index(i: i32, len: usize) -> usize {
    if i < 0 {
        (len as i32 + i) as usize
    } else {
        i as usize
    }
}

/// `gc.getImprovementInfo(eImprovement).isPermanent()`.
fn is_permanent(g: &Game, improvement: ImprovementType) -> bool {
    g.d.improvements[improvement].permanent
}

/// The equipment unit type named by the literal argument of the `*TakeEquipment*(caster, unit)`
/// callbacks (`gc.getInfoTypeForString(unit)`).
fn equipment_unit(g: &Game, call: &PyCall) -> UnitType {
    let name = call.str_arg(1).expect("TakeEquipment callback without its 'EQUIPMENT_X' argument");
    g.d.t::<UnitType>(name)
}

/// `caster.isHasPromotion(iProm)` for `iProm = getEquipmentPromotion()`, which is -1 for a unit
/// without one (the C++ would read `m_pabHasPromotion[-1]`; treated as false).
fn has_promotion_opt(g: &Game, u: UnitId, promotion: Option<PromotionType>) -> bool {
    promotion.map_or(false, |p| g.units[u].has_promotion[p])
}

// ---------------------------------------------------------------------------------------------
// Spells
// ---------------------------------------------------------------------------------------------

/// `reqPirateCove(caster)`.
fn req_pirate_cove(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let pirate_cove = d.t::<ImprovementType>("IMPROVEMENT_PIRATE_COVE");
    if g.map.plots[plot].owner != Some(g.units[caster].owner) {
        return false;
    }
    if !g.plot_can_have_improvement(plot, pirate_cove, None, false) {
        return false;
    }
    if g.map.plots[plot].improvement_type.is_some() {
        return false;
    }
    // `pPlot.getBonusType(-1)`: NO_TEAM reads the raw bonus.
    if g.map.plots[plot].bonus_type.is_some() {
        return false;
    }
    let pirate_harbor = d.t::<ImprovementType>("IMPROVEMENT_PIRATE_HARBOR");
    let pirate_port = d.t::<ImprovementType>("IMPROVEMENT_PIRATE_PORT");
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    for ix in x - 2..x + 3 {
        for iy in y - 2..y + 3 {
            if let Some(p) = g.map.plot_xy(ix, iy) {
                let improvement = g.map.plots[p].improvement_type;
                if improvement == Some(pirate_cove) {
                    return false;
                }
                if improvement == Some(pirate_harbor) {
                    return false;
                }
                if improvement == Some(pirate_port) {
                    return false;
                }
            }
        }
    }
    true
}

/// `reqMarchOfTheTrees(caster)`. Change from FFH2: its only condition was AI-only (at war with two
/// teams); it is AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_march_of_the_trees(_g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    true
}

/// `spellMarchOfTheTrees(caster)`.
fn spell_march_of_the_trees(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let ancient_forest = d.t::<FeatureType>("FEATURE_FOREST_ANCIENT");
    let forest = d.t::<FeatureType>("FEATURE_FOREST");
    let new_forest = d.t::<FeatureType>("FEATURE_FOREST_NEW");
    let treant = d.t::<UnitType>("UNIT_TREANT");
    let p = g.units[caster].owner;
    for plot in g.map.plot_ids() {
        if g.plot_is_owned(plot) && g.map.plots[plot].owner == Some(p) {
            let feature = g.map.plots[plot].feature_type;
            if feature == Some(forest) || feature == Some(ancient_forest) {
                if !g.plot_is_visible_enemy_unit(plot, p) {
                    let (x, y) = g.map.xy(plot);
                    let new_unit = g.player_init_unit(p, treant, x, y, None, SOUTH);
                    g.units[new_unit].duration = 5;
                    g.plot_set_feature_type(plot, Some(new_forest));
                }
            }
        }
    }
}

/// `reqMotherLode(caster)`. Change from FFH2: its only condition was AI-only (10 mines); it is AI
/// timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_mother_lode(_g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    true
}

/// `spellMotherLode(caster)`.
fn spell_mother_lode(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let gold = g.players[p].improvement_count[g.d.t::<ImprovementType>("IMPROVEMENT_MINE")] * 25;
    g.players[p].gold += gold;
    for plot in g.map.plot_ids() {
        if g.plot_is_owned(plot)
            && g.map.plots[plot].owner == Some(p)
            && !g.plot_is_water(plot)
            && !g.plot_is_peak(plot)
            && !g.plot_is_hills(plot)
        {
            if g.soren_rand_num(100, "Mother Lode") < 10 {
                g.plot_set_plot_type(plot, PlotType::PLOT_HILLS, true);
            }
        }
    }
}

/// `spellOpenChest(caster)`.
fn spell_open_chest(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let p = g.units[caster].owner;
    // `if CyGame().getWBMapScript(): bValid = sf.openChest(caster, pPlot)`: never a scenario.
    if g.soren_rand_num(100, "Explore Lair") < 25 {
        const TRAPS: [&str; 3] = ["POISON", "FIRE", "SPORES"];
        let roll = g.soren_rand_num(TRAPS.len() as i32, "Pick Trap");
        let trap = TRAPS[py_list_index(roll - 1, TRAPS.len())];
        if trap == "POISON" {
            g.unit_set_has_promotion(caster, d.t::<PromotionType>("PROMOTION_POISONED"), true);
            g.unit_do_damage(caster, 25, 90, None, Some(d.t::<DamageType>("DAMAGE_POISON")), false);
        }
        if trap == "FIRE" {
            g.unit_do_damage(caster, 50, 90, None, Some(d.t::<DamageType>("DAMAGE_FIRE")), false);
        }
        if trap == "SPORES" {
            g.unit_change_immobile_timer(caster, 3);
        }
    }
    const GOODIES: [&str; 7] =
        ["EMPTY", "HIGH_GOLD", "ITEM_HEALING_SALVE", "ITEM_JADE_TORC", "ITEM_ROD_OF_WINDS", "ITEM_TIMOR_MASK", "TECH"];
    let roll = g.soren_rand_num(GOODIES.len() as i32, "Pick Goody");
    // 'EMPTY' only shows a message. (The Python also tests 'ITEM_POTION_OF_INVISIBILITY' and
    // 'ITEM_POTION_OF_RESTORATION', which are not in the list.)
    let goody = match GOODIES[py_list_index(roll - 1, GOODIES.len())] {
        "HIGH_GOLD" => Some(d.t::<GoodyType>("GOODY_EXPLORE_LAIR_HIGH_GOLD")),
        "ITEM_HEALING_SALVE" => Some(d.t::<GoodyType>("GOODY_EXPLORE_LAIR_ITEM_HEALING_SALVE")),
        "ITEM_JADE_TORC" => Some(d.t::<GoodyType>("GOODY_EXPLORE_LAIR_ITEM_JADE_TORC")),
        "ITEM_ROD_OF_WINDS" => Some(d.t::<GoodyType>("GOODY_EXPLORE_LAIR_ITEM_ROD_OF_WINDS")),
        "ITEM_TIMOR_MASK" => Some(d.t::<GoodyType>("GOODY_EXPLORE_LAIR_ITEM_TIMOR_MASK")),
        "TECH" => Some(d.t::<GoodyType>("GOODY_GRAVE_TECH")),
        _ => None,
    };
    if let Some(goody) = goody {
        g.player_receive_goody(p, plot, goody, Some(caster));
    }
    // The last treasure on the plot (of any owner) is consumed. The spell requires one in the
    // stack (UnitInStackPrereq); without one the Python raises on `(-1).kill`.
    let treasure_type = d.t::<UnitType>("EQUIPMENT_TREASURE");
    let mut treasure = None;
    for &u in &g.map.plots[plot].units {
        if g.units[u].unit_type == treasure_type {
            treasure = Some(u);
        }
    }
    if let Some(treasure) = treasure {
        // `pTreasure.kill(True, 0)`: player 0 is reported as the killer.
        g.unit_kill(treasure, true, Some(PlayerId(0)));
    }
}

/// `reqPromoteSettlement(caster)`.
fn req_promote_settlement(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let plot = g.unit_plot(caster);
    // A NULL CyCity answers isSettlement() False.
    if !g.map.plots[plot].plot_city.map_or(false, |c| g.cities[c].settlement) {
        return false;
    }
    let p = g.units[caster].owner;
    if g.players[p].num_cities() - g.player_get_num_settlements(p) >= g.players[p].max_cities {
        return false;
    }
    true
}

/// `spellPromoteSettlement(caster)`.
fn spell_promote_settlement(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let plot = g.unit_plot(caster);
    // bInCityOnly: there is a city (a NULL CyCity would ignore both calls).
    if let Some(city) = g.map.plots[plot].plot_city {
        g.cities[city].settlement = false;
        g.city_set_plot_radius(city, 3);
    }
}

/// `reqRagingSeas(caster)`. Change from FFH2: its only condition was AI-only (at war with a player
/// whose fleet is worth more than 10); it is AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_raging_seas(_g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    true
}

/// `spellRagingSeas(caster)`.
fn spell_raging_seas(g: &mut Game, _caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let cold = d.t::<DamageType>("DAMAGE_COLD");
    let flames = d.t::<FeatureType>("FEATURE_FLAMES");
    let lanun = d.t::<CivilizationType>("CIVILIZATION_LANUN");
    let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
    for plot in g.map.plot_ids() {
        if g.plot_is_adjacent_to_water(plot) {
            let n = g.map.plots[plot].units.len();
            for i in 0..n {
                let Some(u) = plot_unit_at(g, plot, i) else { continue };
                if g.unit_get_civilization_type(u) != lanun {
                    g.unit_do_damage(u, 75, 100, None, Some(cold), false);
                }
            }
            if g.map.plots[plot].improvement_type.is_some() {
                if g.map.plots[plot].feature_type == Some(flames) {
                    g.plot_set_feature_type(plot, None);
                }
                if g.map.plots[plot].improvement_type == Some(smoke) {
                    g.plot_set_improvement_type(plot, None);
                } else if g.plot_is_owned(plot) {
                    // Removing the flames can remove an improvement that requires a feature;
                    // `gc.getImprovementInfo(-1)` is then None and the Python raises.
                    let Some(improvement) = g.map.plots[plot].improvement_type else { return };
                    if !is_permanent(g, improvement) {
                        let owner = g.map.plots[plot].owner.expect("owned plot");
                        if g.player_get_civilization_type(owner) != Some(lanun) {
                            if g.soren_rand_num(100, "Raging Seas") <= 25 {
                                g.plot_set_improvement_type(plot, None);
                            }
                        }
                    }
                }
            }
            // (EFFECT_SPRING on plots visible to a human: graphics.)
        }
    }
}

/// `spellRaiseSkeleton(caster)`.
fn spell_raise_skeleton(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    if g.map.plots[plot].improvement_type == Some(d.t::<ImprovementType>("IMPROVEMENT_GRAVEYARD")) {
        g.plot_set_improvement_type(plot, None);
        // Each cast creates a skeleton and re-enters this function (the graveyard is gone now).
        g.unit_cast(caster, d.t::<SpellType>("SPELL_RAISE_SKELETON"));
        g.unit_cast(caster, d.t::<SpellType>("SPELL_RAISE_SKELETON"));
    }
}

/// `reqRally(caster)`. Change from FFH2: its AI-only branch (5 cities) is AI timing, in ffh-ai
/// `world_spells` (docs/PORTING.md).
fn req_rally(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let p = g.units[caster].owner;
    if g.players[p].civics[d.t::<CivicOptionType>("CIVICOPTION_CULTURAL_VALUES")] != Some(d.t::<CivicType>("CIVIC_CRUSADE")) {
        return false;
    }
    true
}

/// `spellRally(caster)`.
fn spell_rally(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let owner = g.units[caster].owner;
    let demagog = d.t::<UnitType>("UNIT_DEMAGOG");
    let town = d.t::<ImprovementType>("IMPROVEMENT_TOWN");
    let village = d.t::<ImprovementType>("IMPROVEMENT_VILLAGE");
    for plot in g.map.plot_ids() {
        if g.map.plots[plot].owner == Some(owner) {
            if !g.plot_is_visible_enemy_unit(plot, owner) {
                let (x, y) = g.map.xy(plot);
                if g.map.plots[plot].plot_city.is_some() {
                    g.player_init_unit(owner, demagog, x, y, None, SOUTH);
                }
                if g.map.plots[plot].improvement_type == Some(town) {
                    g.plot_set_improvement_type(plot, Some(village));
                    g.player_init_unit(owner, demagog, x, y, None, SOUTH);
                }
            }
        }
    }
}

/// `spellReadTheGrimoire(caster)`.
fn spell_read_the_grimoire(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let rnd = g.soren_rand_num(100, "Read the Grimoire");
    if rnd < 20 {
        g.unit_cast(caster, d.t::<SpellType>("SPELL_SUMMON_SPECTRE"));
    }
    if (20..40).contains(&rnd) {
        g.unit_set_has_promotion(caster, d.t::<PromotionType>("PROMOTION_UNHOLY_TAINT"), true);
    }
    if (40..60).contains(&rnd) {
        g.unit_cast(caster, d.t::<SpellType>("SPELL_WITHER"));
    }
    if (60..70).contains(&rnd) {
        g.unit_cast(caster, d.t::<SpellType>("SPELL_SUMMON_PIT_BEAST"));
    }
    if (70..80).contains(&rnd) {
        g.unit_cast(caster, d.t::<SpellType>("SPELL_BURNING_BLOOD"));
    }
    if (80..90).contains(&rnd) {
        g.unit_set_has_promotion(caster, d.t::<PromotionType>("PROMOTION_DEMON"), true);
    }
    if rnd >= 90 {
        g.unit_kill(caster, true, None);
    }
}

/// `reqRebuildBarnaxus(caster)`.
fn req_rebuild_barnaxus(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let city_owner = g.map.plots[plot].plot_city.map(|c| g.cities[c].owner);
    if g.units[caster].unit_type == d.t::<UnitType>("UNIT_BARNAXUS") {
        return false;
    }
    // bInCityOnly; without a city `gc.getPlayer(-1)` is None and the Python raises (False).
    let Some(city_owner) = city_owner else { return false };
    if g.player_get_civilization_type(city_owner) != Some(d.t::<CivilizationType>("CIVILIZATION_LUCHUIRP")) {
        return false;
    }
    true
}

/// `spellRebuildBarnaxus(caster)`.
fn spell_rebuild_barnaxus(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let Some(city) = g.map.plots[plot].plot_city else { return }; // bInCityOnly
    let p = g.units[caster].owner;
    let city_player = g.cities[city].owner;
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    let new_unit = g.player_init_unit(city_player, d.t::<UnitType>("UNIT_BARNAXUS"), x, y, None, NORTH);
    g.unit_set_damage(new_unit, 75, Some(p));
    g.unit_finish_moves(new_unit);
    // CyCity::applyBuildEffects forwards to CvCity::addProductionExperience (not to
    // CvCity::applyBuildEffects).
    g.city_add_production_experience(city, new_unit, false);
    if g.player_get_civilization_type(p) != Some(d.t::<CivilizationType>("CIVILIZATION_LUCHUIRP")) {
        // `pCityPlayer.AI_changeAttitudeExtra(iPlayer, 2)`
        g.players[city_player].ai_attitude_extra[p] += 2;
    }
}

/// `spellRecruit(caster)`.
fn spell_recruit(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let Some(city) = g.map.plots[plot].plot_city else { return }; // bInCityOnly
    let p = g.units[caster].owner;
    let team = g.players[p].team;
    let mut loops = py_div(g.cities[city].population, 3) + 1;
    if g.city_num_building(city, d.t::<BuildingType>("BUILDING_TEMPLE_OF_THE_ORDER")) == 1 {
        loops *= 2;
    }
    let has_tech = |g: &Game, tech: TechType| g.team_is_has_tech(team, Some(tech));
    for _ in 0..loops {
        let rnd = g.soren_rand_num(60, "Bob");
        let mut unit_class: Option<UnitClassType> = None;
        if rnd <= 10 {
            unit_class = Some(d.t::<UnitClassType>("UNITCLASS_SCOUT"));
            if has_tech(g, d.t::<TechType>("TECH_HUNTING")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_HUNTER"));
            }
            if has_tech(g, d.t::<TechType>("TECH_ANIMAL_HANDLING")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_RANGER"));
            }
        } else if rnd <= 20 {
            unit_class = Some(d.t::<UnitClassType>("UNITCLASS_SCOUT"));
            if has_tech(g, d.t::<TechType>("TECH_HUNTING")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_HUNTER"));
            }
            if has_tech(g, d.t::<TechType>("TECH_POISONS")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_ASSASSIN"));
            }
        } else if rnd <= 30 {
            unit_class = Some(d.t::<UnitClassType>("UNITCLASS_SCOUT"));
            if has_tech(g, d.t::<TechType>("TECH_HORSEBACK_RIDING")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_HORSEMAN"));
            }
            if has_tech(g, d.t::<TechType>("TECH_STIRRUPS")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_HORSE_ARCHER"));
            }
        } else if rnd <= 40 {
            unit_class = Some(d.t::<UnitClassType>("UNITCLASS_SCOUT"));
            if has_tech(g, d.t::<TechType>("TECH_HORSEBACK_RIDING")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_HORSEMAN"));
            }
            if has_tech(g, d.t::<TechType>("TECH_TRADE")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_CHARIOT"));
            }
        } else if rnd <= 50 {
            unit_class = Some(d.t::<UnitClassType>("UNITCLASS_WARRIOR"));
            if has_tech(g, d.t::<TechType>("TECH_BRONZE_WORKING")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_AXEMAN"));
            }
            if has_tech(g, d.t::<TechType>("TECH_IRON_WORKING")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_CHAMPION"));
            }
        } else if rnd <= 60 {
            unit_class = Some(d.t::<UnitClassType>("UNITCLASS_WARRIOR"));
            if has_tech(g, d.t::<TechType>("TECH_ARCHERY")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_ARCHER"));
            }
            if has_tech(g, d.t::<TechType>("TECH_BOWYERS")) {
                unit_class = Some(d.t::<UnitClassType>("UNITCLASS_LONGBOWMAN"));
            }
        }
        if let Some(class) = unit_class {
            if let Some(unit) = civ_unit(g, p, class) {
                let (x, y) = (g.units[caster].x, g.units[caster].y);
                g.player_init_unit(p, unit, x, y, None, NORTH);
            }
        }
    }
    g.unit_set_has_promotion(caster, d.t::<PromotionType>("PROMOTION_RECRUITER"), false);
    if g.units[caster].unit_type != d.t::<UnitType>("UNIT_DONAL") {
        g.unit_kill(caster, true, None);
    }
}

/// `reqRecruitMercenary(caster)`.
fn req_recruit_mercenary(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let p = g.units[caster].owner;
    if !g.players[p].human {
        if g.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_KHAZAD")) {
            return false;
        }
        let (x, y) = (g.units[caster].x, g.units[caster].y);
        let team = g.players[p].team;
        for ix in x - 1..x + 2 {
            for iy in y - 1..y + 2 {
                let Some(plot) = g.map.plot_xy(ix, iy) else { continue };
                for &u in &g.map.plots[plot].units {
                    let team2 = g.players[g.units[u].owner].team;
                    if g.teams[team].at_war[team2] {
                        return true;
                    }
                }
            }
        }
        return false;
    }
    true
}

/// `spellRecruitMercenary(caster)`.
fn spell_recruit_mercenary(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    // A civilization without a mercenary would make the C++ `initUnit(NO_UNIT)` fail.
    let Some(unit) = civ_unit(g, p, d.t::<UnitClassType>("UNITCLASS_MERCENARY")) else { return };
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    let new_unit = g.player_init_unit(p, unit, x, y, None, NORTH);
    g.unit_finish_moves(new_unit);
    g.units[new_unit].has_casted = true;
    if g.units[caster].unit_type == d.t::<UnitType>("UNIT_MAGNADINE") {
        g.unit_set_has_promotion(new_unit, d.t::<PromotionType>("PROMOTION_LOYALTY"), true);
    }
}

/// `spellReleaseFromCage(caster)`.
fn spell_release_from_cage(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let plot = g.unit_plot(caster);
    g.plot_set_improvement_type(plot, None);
}

/// `reqReligiousFervor(caster)`. Change from FFH2: its AI-only branch (5 cities with the state
/// religion) is AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_religious_fervor(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    g.player_get_state_religion(g.units[caster].owner).is_some()
}

/// `spellReligiousFervor(caster)`.
fn spell_religious_fervor(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    // The requirement guarantees a state religion.
    let Some(religion) = g.player_get_state_religion(p) else { return };
    let mut unit = None;
    if religion == d.t::<ReligionType>("RELIGION_FELLOWSHIP_OF_LEAVES") {
        unit = Some(d.t::<UnitType>("UNIT_PRIEST_OF_LEAVES"));
    }
    if religion == d.t::<ReligionType>("RELIGION_RUNES_OF_KILMORPH") {
        unit = Some(d.t::<UnitType>("UNIT_PRIEST_OF_KILMORPH"));
    }
    if religion == d.t::<ReligionType>("RELIGION_THE_EMPYREAN") {
        unit = Some(d.t::<UnitType>("UNIT_PRIEST_OF_THE_EMPYREAN"));
    }
    if religion == d.t::<ReligionType>("RELIGION_THE_ORDER") {
        unit = Some(d.t::<UnitType>("UNIT_PRIEST_OF_THE_ORDER"));
    }
    if religion == d.t::<ReligionType>("RELIGION_OCTOPUS_OVERLORDS") {
        unit = Some(d.t::<UnitType>("UNIT_PRIEST_OF_THE_OVERLORDS"));
    }
    if religion == d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL") {
        unit = Some(d.t::<UnitType>("UNIT_PRIEST_OF_THE_VEIL"));
    }
    if religion == d.t::<ReligionType>("RELIGION_COUNCIL_OF_ESUS") {
        unit = Some(d.t::<UnitType>("UNIT_ASSASSIN"));
    }
    let count = py_city_list(g, p).into_iter().filter(|&c| g.cities[c].has_religion[religion]).count() as i32;
    // Any other religion leaves `iUnit` unbound: NameError at the first initUnit.
    let Some(unit) = unit else { return };
    for city in py_city_list(g, p) {
        let (x, y) = (g.cities[city].x, g.cities[city].y);
        let new_unit = g.player_init_unit(p, unit, x, y, None, SOUTH);
        g.unit_change_experience(new_unit, count, -1, false, false, false);
        g.units[new_unit].religion = Some(religion);
    }
}

/// `iSiege`, `iNaval`, `iGolem` of `reqRepair`/`spellRepair`.
struct RepairTargets {
    siege: UnitCombatType,
    naval: UnitCombatType,
    golem: PromotionType,
}

impl RepairTargets {
    fn new(d: &GameData) -> RepairTargets {
        RepairTargets {
            siege: d.t::<UnitCombatType>("UNITCOMBAT_SIEGE"),
            naval: d.t::<UnitCombatType>("UNITCOMBAT_NAVAL"),
            golem: d.t::<PromotionType>("PROMOTION_GOLEM"),
        }
    }

    /// Siege or naval units and golems.
    fn matches(&self, g: &Game, u: UnitId) -> bool {
        let unit_combat = g.unit_get_unit_combat_type(u);
        unit_combat == Some(self.siege) || unit_combat == Some(self.naval) || g.units[u].has_promotion[self.golem]
    }
}

/// `reqRepair(caster)`.
fn req_repair(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let targets = RepairTargets::new(g.d);
    let plot = g.unit_plot(caster);
    for &u in &g.map.plots[plot].units {
        if targets.matches(g, u) {
            if g.units[u].damage > 0 {
                return true;
            }
        }
    }
    false
}

/// `spellRepair(caster, amount)` (XML: `spellRepair(pCaster,20)`).
fn spell_repair(g: &mut Game, caster: UnitId, call: &PyCall) {
    let amount = call.int_arg(1).expect("spellRepair(pCaster, amount)");
    let targets = RepairTargets::new(g.d);
    let plot = g.unit_plot(caster);
    let n = g.map.plots[plot].units.len();
    for i in 0..n {
        let Some(u) = plot_unit_at(g, plot, i) else { continue };
        if targets.matches(g, u) {
            // `pUnit.changeDamage(-amount, 0)`: player 0.
            g.unit_change_damage(u, -amount, Some(PlayerId(0)));
        }
    }
}

/// `reqRessurection(caster)`.
fn req_ressurection(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    let Some(hero) = g.cf_get_hero(p) else { return false };
    if !g.is_unit_class_maxed_out(hero, 0) {
        return false;
    }
    for p2 in g.player_ids() {
        if g.players[p2].unit_class_count[hero] > 0 {
            return false;
        }
    }
    let spell = g.d.t::<SpellType>("SPELL_RESSURECTION");
    for u in py_unit_list(g, p) {
        if g.units[u].delayed_spell == Some(spell) {
            return false;
        }
    }
    true
}

/// `spellRessurection(caster)`.
fn spell_ressurection(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let p = g.units[caster].owner;
    // The requirement guarantees a hero class.
    let Some(hero) = g.cf_get_hero(p) else { return };
    let unit = civ_unit(g, p, hero);
    let barn = d.t::<UnitType>("EQUIPMENT_PIECES_OF_BARNAXUS");
    let barn_prom = d.t::<PromotionType>("PROMOTION_PIECES_OF_BARNAXUS");
    if unit == Some(d.t::<UnitType>("UNIT_BARNAXUS")) {
        for p2 in g.player_ids() {
            if g.players[p2].alive {
                for u in py_unit_list(g, p2) {
                    // (A kill takes the victim's cargo with it at once.)
                    if !g.units.contains(u) {
                        continue;
                    }
                    if g.units[u].has_promotion[barn_prom] {
                        g.unit_set_has_promotion(u, barn_prom, false);
                    }
                    if g.units[u].unit_type == barn {
                        g.unit_kill(u, true, None);
                    }
                }
            }
        }
    }
    // A civilization without the hero's unit would make the C++ `initUnit(NO_UNIT)` fail.
    let Some(unit) = unit else { return };
    let (x, y) = g.map.xy(plot);
    let new_unit = g.player_init_unit(p, unit, x, y, None, NORTH);
    for promotion in d.promotions.keys() {
        if g.units[new_unit].has_promotion[promotion] {
            if d.promotions[promotion].equipment {
                g.unit_set_has_promotion(new_unit, promotion, false);
            }
        }
    }
}

/// `spellRessurectionGraveyard(caster)`.
fn spell_ressurection_graveyard(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let p = g.units[caster].owner;
    let unit = civ_unit(g, p, d.t::<UnitClassType>("UNITCLASS_CHAMPION")).unwrap_or_else(|| d.t::<UnitType>("UNIT_CHAMPION"));
    let (x, y) = g.map.xy(plot);
    let new_unit = g.player_init_unit(p, unit, x, y, None, SOUTH);
    let experience = g.soren_rand_num(30, "Ressurection Graveyard");
    g.unit_change_experience(new_unit, experience, -1, false, false, false);
    g.plot_set_improvement_type(plot, None);
}

/// `reqReveal(caster)`.
fn req_reveal(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    if g.units[caster].ignore_hide {
        return false;
    }
    let plot = g.unit_plot(caster);
    if g.map.plots[plot].owner != Some(g.units[caster].owner) {
        return false;
    }
    true
}

/// `spellReveal(caster)`.
fn spell_reveal(g: &mut Game, caster: UnitId, _call: &PyCall) {
    g.units[caster].ignore_hide = true;
}

/// `reqRevelry(caster)`. Change from FFH2: its AI-only branch (25 population) is AI timing, in
/// ffh-ai `world_spells` (docs/PORTING.md).
fn req_revelry(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if g.player_is_golden_age(p) {
        return false;
    }
    true
}

/// `spellRevelry(caster)`.
fn spell_revelry(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let turns = g.golden_age_length() * 2;
    g.player_change_golden_age_turns(p, turns);
}

/// The promotions `reqRevelation`/`spellRevelation` look for: hidden, hidden nationality,
/// invisible, illusion.
fn revelation_promotions(d: &GameData) -> [PromotionType; 4] {
    [
        d.t::<PromotionType>("PROMOTION_HIDDEN"),
        d.t::<PromotionType>("PROMOTION_HIDDEN_NATIONALITY"),
        d.t::<PromotionType>("PROMOTION_INVISIBLE"),
        d.t::<PromotionType>("PROMOTION_ILLUSION"),
    ]
}

/// `reqRevelation(caster)`.
fn req_revelation(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    let team = g.players[p].team;
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    let [hidden, hidden_nationality, invisible, illusion] = revelation_promotions(g.d);
    for ix in x - 3..x + 4 {
        for iy in y - 3..y + 4 {
            let Some(plot) = g.map.plot_xy(ix, iy) else { continue };
            for &u in &g.map.plots[plot].units {
                if g.unit_get_team(u) != team {
                    let has = &g.units[u].has_promotion;
                    if has[hidden] {
                        return true;
                    }
                    if has[hidden_nationality] {
                        return true;
                    }
                    if has[invisible] {
                        return true;
                    }
                    if has[illusion] {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// `spellRevelation(caster)`.
fn spell_revelation(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let team = g.players[p].team;
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    let [hidden, hidden_nationality, invisible, illusion] = revelation_promotions(g.d);
    for ix in x - 3..x + 4 {
        for iy in y - 3..y + 4 {
            let Some(plot) = g.map.plot_xy(ix, iy) else { continue };
            let n = g.map.plots[plot].units.len();
            for i in 0..n {
                let Some(u) = plot_unit_at(g, plot, i) else { continue };
                if g.unit_get_team(u) != team {
                    if g.units[u].has_promotion[hidden] {
                        g.unit_set_has_promotion(u, hidden, false);
                    }
                    if g.units[u].has_promotion[hidden_nationality] {
                        g.unit_set_has_promotion(u, hidden_nationality, false);
                    }
                    if g.units[u].has_promotion[invisible] {
                        g.unit_set_has_promotion(u, invisible, false);
                    }
                    if g.units[u].has_promotion[illusion] {
                        g.unit_kill(u, true, Some(p));
                    }
                }
            }
        }
    }
}

/// `spellRingofFlames(caster)`.
fn spell_ring_of_flames(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let forest = d.t::<FeatureType>("FEATURE_FOREST");
    let jungle = d.t::<FeatureType>("FEATURE_JUNGLE");
    let new_forest = d.t::<FeatureType>("FEATURE_FOREST_NEW");
    let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    for ix in x - 1..x + 2 {
        for iy in y - 1..y + 2 {
            if !(ix == x && iy == y) {
                // (No isNone() check: a NULL plot has neither improvement nor feature.)
                let Some(plot) = g.map.plot_xy(ix, iy) else { continue };
                let mut valid = true;
                if let Some(improvement) = g.map.plots[plot].improvement_type {
                    if is_permanent(g, improvement) {
                        valid = false;
                    }
                }
                if valid {
                    let feature = g.map.plots[plot].feature_type;
                    if feature == Some(forest) || feature == Some(jungle) || feature == Some(new_forest) {
                        if g.soren_rand_num(100, "Flames Spread") <= d.defines.flames_spread_chance {
                            g.plot_set_improvement_type(plot, Some(smoke));
                        }
                    }
                }
            }
        }
    }
}

/// `reqRiverOfBlood(caster)`. Change from FFH2: its AI-only branch (5 cities) is AI timing, in
/// ffh-ai `world_spells` (docs/PORTING.md).
fn req_river_of_blood(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if g.players[p].num_cities() == 0 {
        return false;
    }
    true
}

/// `spellRiverOfBlood(caster)`.
fn spell_river_of_blood(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let owner = g.units[caster].owner;
    for p in g.player_ids() {
        if g.players[p].alive {
            if p != owner {
                for city in py_city_list(g, p) {
                    if g.cities[city].population > 2 {
                        g.city_change_population(city, -2);
                    }
                }
            }
            if p == owner {
                for city in py_city_list(g, p) {
                    g.city_change_population(city, 2);
                }
            }
        }
    }
}

/// `reqRoar(caster)`.
fn req_roar(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    let team = g.players[p].team;
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    for ix in x - 1..x + 2 {
        for iy in y - 1..y + 2 {
            let Some(plot) = g.map.plot_xy(ix, iy) else { continue };
            for &u in &g.map.plots[plot].units {
                if !g.unit_is_immune_to_fear(u) {
                    let team2 = g.players[g.units[u].owner].team;
                    if g.teams[team].at_war[team2] {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// `spellRoar(caster)`.
fn spell_roar(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let team = g.players[p].team;
    let plot = g.unit_plot(caster);
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    for ix in x - 1..x + 2 {
        for iy in y - 1..y + 2 {
            let Some(loop_plot) = g.map.plot_xy(ix, iy) else { continue };
            // Backwards, so units that flee do not shift the ones still to visit.
            let n = g.map.plots[loop_plot].units.len();
            for i in (0..n).rev() {
                // NULL unit (a fleeing transport took cargo listed before it along):
                // `gc.getPlayer(-1).getTeam()` raises and the spell ends.
                let Some(u) = plot_unit_at(g, loop_plot, i) else { return };
                let team2 = g.players[g.units[u].owner].team;
                if g.teams[team].at_war[team2] {
                    // (Messages when the unit flees.)
                    g.cf_do_fear(u, plot, caster, true);
                }
            }
        }
    }
}

/// `reqRobGrave(caster)`.
fn req_rob_grave(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if !g.players[p].human {
        if g.players[p].alignment == Some(g.d.t::<AlignmentType>("ALIGNMENT_GOOD")) {
            return false;
        }
    }
    true
}

/// `spellRobGrave(caster)`.
fn spell_rob_grave(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    g.change_global_counter(1);
    let plot = g.unit_plot(caster);
    g.plot_set_improvement_type(plot, None);
    let p = g.units[caster].owner;
    let mut goodies = vec!["LOW_GOLD", "HIGH_GOLD", "SPAWN_SKELETONS", "SPAWN_SPECTRE"];
    if g.player_can_receive_goody(p, plot, d.t::<GoodyType>("GOODY_GRAVE_TECH"), Some(caster)) {
        goodies.push("TECH");
    }
    let roll = g.soren_rand_num(goodies.len() as i32, "Pick Goody");
    let goody = match goodies[py_list_index(roll - 1, goodies.len())] {
        "LOW_GOLD" => d.t::<GoodyType>("GOODY_GRAVE_LOW_GOLD"),
        "HIGH_GOLD" => d.t::<GoodyType>("GOODY_GRAVE_HIGH_GOLD"),
        "TECH" => d.t::<GoodyType>("GOODY_GRAVE_TECH"),
        "SPAWN_SKELETONS" => d.t::<GoodyType>("GOODY_GRAVE_SKELETONS"),
        _ => d.t::<GoodyType>("GOODY_GRAVE_SPECTRE"),
    };
    g.player_receive_goody(p, plot, goody, Some(caster));
}

/// `spellSacrificeAltar(caster)`.
fn spell_sacrifice_altar(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let team = g.players[p].team;
    let tech = g.player_get_current_research(p);
    let level = g.units[caster].level;
    let num = 10 + level * level;
    // C++ quirk: with nothing being researched `CvTeam::changeResearchProgress(NO_TECH, ..)`
    // writes `m_paiResearchProgress[-1]` (undefined behaviour, no rules effect); skipped here.
    if let Some(tech) = tech {
        g.team_change_research_progress(team, tech, num, p);
    }
}

/// `spellSacrificePyre(caster)`.
fn spell_sacrifice_pyre(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    g.unit_cast(caster, d.t::<SpellType>("SPELL_RING_OF_FLAMES"));
    if g.unit_is_immortal(caster) {
        g.unit_change_immortal(caster, -1);
    }
    let mut count = 1;
    if is_world_unit_class(d, g.unit_get_unit_class_type(caster)) {
        count = 7;
    }
    for _ in 0..count {
        let (x, y) = (g.units[caster].x, g.units[caster].y);
        g.player_init_unit(p, d.t::<UnitType>("UNIT_FIRE_ELEMENTAL"), x, y, None, SOUTH);
    }
}

/// `reqSanctify(caster)`.
fn req_sanctify(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let caster_plot = g.unit_plot(caster);
    // Python's `pPlot`: the caster's plot, reassigned by the loops below.
    let mut plot = Some(caster_plot);
    let mut valid = false;
    if g.map.plots[caster_plot].improvement_type == Some(d.t::<ImprovementType>("IMPROVEMENT_CITY_RUINS")) {
        return true;
    }
    if g.map.plots[caster_plot].improvement_type == Some(d.t::<ImprovementType>("IMPROVEMENT_GRAVEYARD")) {
        return true;
    }
    let (x, y) = g.map.xy(caster_plot);
    if g.is_option(GameOptionType::GAMEOPTION_NO_PLOT_COUNTER) {
        let broken_lands = d.t::<TerrainType>("TERRAIN_BROKEN_LANDS");
        let burning_sands = d.t::<TerrainType>("TERRAIN_BURNING_SANDS");
        let fields_of_perdition = d.t::<TerrainType>("TERRAIN_FIELDS_OF_PERDITION");
        let shallows = d.t::<TerrainType>("TERRAIN_SHALLOWS");
        for ix in x - 1..x + 2 {
            for iy in y - 1..y + 2 {
                plot = g.map.plot_xy(ix, iy);
                if let Some(p) = plot {
                    let terrain = g.map.plots[p].terrain_type;
                    if terrain == Some(broken_lands)
                        || terrain == Some(burning_sands)
                        || terrain == Some(fields_of_perdition)
                        || terrain == Some(shallows)
                    {
                        valid = true;
                    }
                }
            }
        }
    } else {
        for ix in x - 1..x + 2 {
            for iy in y - 1..y + 2 {
                plot = g.map.plot_xy(ix, iy);
                if let Some(p) = plot {
                    if g.map.plots[p].plot_counter > 0 {
                        valid = true;
                    }
                }
            }
        }
    }
    if !valid {
        return false;
    }
    let p = g.units[caster].owner;
    if !g.players[p].human {
        // Python quirk: `pPlot` is the last plot of the loops above, (x+1, y+1), not the
        // caster's plot; a NULL plot's getOwner() is -1.
        if Some(p) != plot.and_then(|lp| g.map.plots[lp].owner) {
            return false;
        }
        if g.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_INFERNAL")) {
            return false;
        }
    }
    true
}

/// `spellSanctify(caster)`.
fn spell_sanctify(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    if g.map.plots[plot].improvement_type == Some(d.t::<ImprovementType>("IMPROVEMENT_CITY_RUINS")) {
        g.plot_set_improvement_type(plot, None);
        g.change_global_counter(-1);
    }
    if g.map.plots[plot].improvement_type == Some(d.t::<ImprovementType>("IMPROVEMENT_GRAVEYARD")) {
        g.plot_set_improvement_type(plot, None);
        g.change_global_counter(-1);
        let p = g.units[caster].owner;
        let (x, y) = g.map.xy(plot);
        g.player_init_unit(p, d.t::<UnitType>("UNIT_EINHERJAR"), x, y, None, SOUTH);
    }
    let (x, y) = g.map.xy(plot);
    if g.is_option(GameOptionType::GAMEOPTION_NO_PLOT_COUNTER) {
        let broken_lands = d.t::<TerrainType>("TERRAIN_BROKEN_LANDS");
        let burning_sands = d.t::<TerrainType>("TERRAIN_BURNING_SANDS");
        let desert = d.t::<TerrainType>("TERRAIN_DESERT");
        let fields_of_perdition = d.t::<TerrainType>("TERRAIN_FIELDS_OF_PERDITION");
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        let marsh = d.t::<TerrainType>("TERRAIN_MARSH");
        let plains = d.t::<TerrainType>("TERRAIN_PLAINS");
        let shallows = d.t::<TerrainType>("TERRAIN_SHALLOWS");
        for ix in x - 1..x + 2 {
            for iy in y - 1..y + 2 {
                let Some(p) = g.map.plot_xy(ix, iy) else { continue };
                let terrain = g.map.plots[p].terrain_type;
                if terrain == Some(broken_lands) {
                    g.plot_set_terrain_type(p, grass, true);
                }
                if terrain == Some(burning_sands) {
                    g.plot_set_terrain_type(p, desert, true);
                }
                if terrain == Some(fields_of_perdition) {
                    g.plot_set_terrain_type(p, plains, true);
                }
                if terrain == Some(shallows) {
                    g.plot_set_terrain_type(p, marsh, true);
                }
            }
        }
    } else {
        for ix in x - 1..x + 2 {
            for iy in y - 1..y + 2 {
                // (No isNone() check: a NULL plot's getPlotCounter() is -1.)
                let Some(p) = g.map.plot_xy(ix, iy) else { continue };
                if g.map.plots[p].plot_counter > 0 {
                    let counter = g.map.plots[p].plot_counter;
                    g.plot_change_plot_counter(p, counter * -1);
                }
            }
        }
    }
}

/// `reqSanctuary(caster)`. Change from FFH2: its AI-only branch (at war with two teams) is AI
/// timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_sanctuary(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if g.players[p].num_cities() == 0 {
        return false;
    }
    true
}

/// `spellSanctuary(caster)`.
fn spell_sanctuary(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    let team = g.unit_get_team(caster);
    // `changeSanctuaryTimer(30)`: its other effects (the Great Wall art) are graphics.
    g.players[p].sanctuary_timer += 30;
    for plot in g.map.plot_ids() {
        if g.plot_is_owned(plot) && g.map.plots[plot].owner == Some(p) {
            // `range(pPlot.getNumUnits(), -1, -1)` starts one past the end (a NULL unit).
            let n = g.map.plots[plot].units.len();
            for i in (0..=n).rev() {
                let Some(u) = plot_unit_at(g, plot, i) else { continue };
                if g.unit_get_team(u) != team {
                    g.unit_jump_to_nearest_valid_plot(u);
                }
            }
        }
    }
}

/// `reqSandLion(caster)`.
fn req_sand_lion(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let terrain = g.map.plots[plot].terrain_type;
    if terrain != Some(d.t::<TerrainType>("TERRAIN_DESERT")) && terrain != Some(d.t::<TerrainType>("TERRAIN_BURNING_SANDS")) {
        return false;
    }
    true
}

/// `reqScorch(caster)`.
fn req_scorch(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let p = g.units[caster].owner;
    let terrain = g.map.plots[plot].terrain_type;
    if terrain == Some(d.t::<TerrainType>("TERRAIN_PLAINS")) || terrain == Some(d.t::<TerrainType>("TERRAIN_FIELDS_OF_PERDITION")) {
        if !g.players[p].human {
            if Some(p) == g.map.plots[plot].owner {
                return false;
            }
        }
        return true;
    }
    if terrain == Some(d.t::<TerrainType>("TERRAIN_SNOW")) {
        if !g.players[p].human {
            if Some(p) != g.map.plots[plot].owner {
                return false;
            }
            if g.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_DOVIELLO")) {
                return false;
            }
            if g.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_ILLIANS")) {
                return false;
            }
        }
        return true;
    }
    false
}

/// `spellScorch(caster)`.
fn spell_scorch(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    if g.map.plots[plot].terrain_type == Some(d.t::<TerrainType>("TERRAIN_PLAINS")) {
        g.plot_set_terrain_type(plot, d.t::<TerrainType>("TERRAIN_DESERT"), true);
    }
    if g.map.plots[plot].terrain_type == Some(d.t::<TerrainType>("TERRAIN_FIELDS_OF_PERDITION")) {
        g.plot_set_terrain_type(plot, d.t::<TerrainType>("TERRAIN_BURNING_SANDS"), true);
    }
    if g.map.plots[plot].terrain_type == Some(d.t::<TerrainType>("TERRAIN_SNOW")) {
        g.plot_set_terrain_type(plot, d.t::<TerrainType>("TERRAIN_TUNDRA"), true);
    }
    if let Some(plot_owner) = g.map.plots[plot].owner {
        let p = g.units[caster].owner;
        g.cf_start_war(p, plot_owner, WarPlanType::WARPLAN_TOTAL);
    }
}

/// `spellSing(caster)`: picks one of four songs to play (sound only); the draw is kept.
fn spell_sing(g: &mut Game, _caster: UnitId, _call: &PyCall) {
    g.soren_rand_num(100, "Sing");
}

/// `reqSironasTouch(caster)`.
fn req_sironas_touch(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if !g.players[p].feat_accomplished[FeatType::FEAT_HEAL_UNIT_PER_TURN] {
        return false;
    }
    if g.units[caster].damage == 0 {
        return false;
    }
    if !g.players[p].human {
        if g.units[caster].damage < 15 {
            return false;
        }
    }
    true
}

/// `spellSironasTouch(caster)`.
fn spell_sironas_touch(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    g.players[p].feat_accomplished[FeatType::FEAT_HEAL_UNIT_PER_TURN] = false;
    // `caster.changeDamage(-15, 0)`: player 0.
    g.unit_change_damage(caster, -15, Some(PlayerId(0)));
}

/// `spellSlaveTradeBuy(caster)`.
fn spell_slave_trade_buy(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    let plot = g.unit_plot(caster);
    let (x, y) = g.map.xy(plot);
    let new_unit = g.player_init_unit(p, d.t::<UnitType>("UNIT_SLAVE"), x, y, None, SOUTH);
    let mut promotion = None;
    let rnd = g.soren_rand_num(100, "Slave Trade Buy");
    if (60..70).contains(&rnd) {
        promotion = Some(d.t::<PromotionType>("PROMOTION_DWARF"));
    }
    if (70..80).contains(&rnd) {
        promotion = Some(d.t::<PromotionType>("PROMOTION_ELF"));
    }
    if rnd >= 80 {
        promotion = Some(d.t::<PromotionType>("PROMOTION_ORC"));
    }
    if let Some(promotion) = promotion {
        g.unit_set_has_promotion(new_unit, promotion, true);
    }
}

/// Body of `spellSnowfall` / `spellSnowfallGreator`: temporary snow for `rand(rand_num) +
/// rand_base` turns on the land plots within `radius`, putting out flames and smoke.
fn snowfall(g: &mut Game, caster: UnitId, radius: i32, rand_num: i32, rand_base: i32) {
    let d = g.d;
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    let flames = d.t::<FeatureType>("FEATURE_FLAMES");
    let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
    let snow = d.t::<TerrainType>("TERRAIN_SNOW");
    for ix in x - radius..x + radius + 1 {
        for iy in y - radius..y + radius + 1 {
            let Some(plot) = g.map.plot_xy(ix, iy) else { continue };
            if !g.plot_is_water(plot) {
                if g.map.plots[plot].terrain_type != Some(snow) {
                    let rnd = g.soren_rand_num(rand_num, "Snowfall") + rand_base;
                    g.plot_set_temp_terrain_type(plot, snow, rnd);
                    if g.map.plots[plot].feature_type == Some(flames) {
                        g.plot_set_feature_type(plot, None);
                    }
                    if g.map.plots[plot].improvement_type == Some(smoke) {
                        g.plot_set_improvement_type(plot, None);
                    }
                }
            }
        }
    }
}

/// `spellSnowfall(caster)`.
fn spell_snowfall(g: &mut Game, caster: UnitId, _call: &PyCall) {
    snowfall(g, caster, 1, 6, 3);
}

/// `spellSnowfallGreator(caster)`.
fn spell_snowfall_greator(g: &mut Game, caster: UnitId, _call: &PyCall) {
    snowfall(g, caster, 2, 12, 6);
}

/// `reqSpreadTheCouncilOfEsus(caster)`.
fn req_spread_the_council_of_esus(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let esus = g.d.t::<ReligionType>("RELIGION_COUNCIL_OF_ESUS");
    let p = g.units[caster].owner;
    let plot = g.unit_plot(caster);
    // A NULL CyCity answers isHasReligion() False.
    if g.map.plots[plot].plot_city.map_or(false, |c| g.cities[c].has_religion[esus]) {
        return false;
    }
    if !g.players[p].human {
        if g.player_get_state_religion(p) != Some(esus) {
            return false;
        }
    }
    true
}

/// `reqSpring(caster)`.
fn req_spring(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let p = g.units[caster].owner;
    let flames = d.t::<FeatureType>("FEATURE_FLAMES");
    let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
    let mut has_flames = false;
    let (x, y) = g.map.xy(plot);
    for ix in x - 1..x + 2 {
        for iy in y - 1..y + 2 {
            let Some(plot2) = g.map.plot_xy(ix, iy) else { continue };
            if g.map.plots[plot2].feature_type == Some(flames) || g.map.plots[plot2].improvement_type == Some(smoke) {
                has_flames = true;
            }
        }
    }
    if !has_flames {
        if g.map.plots[plot].terrain_type != Some(d.t::<TerrainType>("TERRAIN_DESERT")) {
            return false;
        }
        if g.map.plots[plot].feature_type == Some(d.t::<FeatureType>("FEATURE_FLOOD_PLAINS")) {
            return false;
        }
        if !g.players[p].human {
            if Some(p) != g.map.plots[plot].owner {
                return false;
            }
        }
    }
    if !g.players[p].human {
        if g.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_INFERNAL")) {
            return false;
        }
    }
    true
}

/// `spellSpring(caster)`.
fn spell_spring(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    if g.map.plots[plot].terrain_type == Some(d.t::<TerrainType>("TERRAIN_DESERT"))
        && g.map.plots[plot].feature_type != Some(d.t::<FeatureType>("FEATURE_FLOOD_PLAINS"))
    {
        g.plot_set_terrain_type(plot, d.t::<TerrainType>("TERRAIN_PLAINS"), true);
        if g.map.plots[plot].feature_type == Some(d.t::<FeatureType>("FEATURE_SCRUB")) {
            g.plot_set_feature_type(plot, None);
        }
    }
    let flames = d.t::<FeatureType>("FEATURE_FLAMES");
    let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
    let (x, y) = g.map.xy(plot);
    for ix in x - 1..x + 2 {
        for iy in y - 1..y + 2 {
            // (No isNone() check: a NULL plot has neither feature nor improvement.)
            let Some(plot2) = g.map.plot_xy(ix, iy) else { continue };
            if g.map.plots[plot2].feature_type == Some(flames) {
                g.plot_set_feature_type(plot2, None);
            }
            if g.map.plots[plot2].improvement_type == Some(smoke) {
                g.plot_set_improvement_type(plot2, None);
            }
        }
    }
}

/// `reqSprint(caster)`.
fn req_sprint(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    if g.units[caster].has_promotion[g.d.t::<PromotionType>("PROMOTION_FATIGUED")] {
        return false;
    }
    true
}

/// `reqStasis(caster)`. Change from FFH2: its only condition was AI-only (5 cities); it is AI
/// timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_stasis(_g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    true
}

/// `spellStasis(caster)`.
fn spell_stasis(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let p = g.units[caster].owner;
    let mut delay = 20;
    let team = g.players[p].team;
    if g.game_speed_type() == d.t::<GameSpeedType>("GAMESPEED_QUICK") {
        delay = 14;
    }
    if g.game_speed_type() == d.t::<GameSpeedType>("GAMESPEED_EPIC") {
        delay = 30;
    }
    if g.game_speed_type() == d.t::<GameSpeedType>("GAMESPEED_MARATHON") {
        delay = 60;
    }
    for p2 in g.player_ids() {
        if g.players[p2].alive {
            if g.players[p2].team != team {
                g.players[p2].disable_production += delay;
                g.players[p2].disable_research += delay;
            }
        }
    }
}

/// `reqSteal(caster)`.
fn req_steal(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let team = g.unit_get_team(caster);
    let plot = g.unit_plot(caster);
    for &u in &g.map.plots[plot].units {
        if g.unit_get_team(u) != team {
            for promotion in d.promotions.keys() {
                if g.units[u].has_promotion[promotion] {
                    if d.promotions[promotion].equipment {
                        return true;
                    }
                }
            }
        }
    }
    if let Some(city) = g.map.plots[plot].plot_city {
        if g.city_team(city) != team {
            for building in d.buildings.keys() {
                if g.cities[city].num_real_building[building] > 0 {
                    if d.buildings[building].equipment {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// `spellSteal(caster)`.
fn spell_steal(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let team = g.unit_get_team(caster);
    let plot = g.unit_plot(caster);
    // Python's loop variable `pUnit` after the loop: the unit at the last index (or NULL).
    let mut last_unit = None;
    let n = g.map.plots[plot].units.len();
    for i in 0..n {
        let unit = plot_unit_at(g, plot, i);
        last_unit = unit;
        let Some(u) = unit else { continue };
        if g.unit_get_team(u) != team {
            for promotion in d.promotions.keys() {
                if g.units[u].has_promotion[promotion] {
                    if d.promotions[promotion].equipment {
                        if g.soren_rand_num(100, "Steal") <= 20 {
                            let (p, p2) = (g.units[caster].owner, g.units[u].owner);
                            g.cf_start_war(p, p2, WarPlanType::WARPLAN_TOTAL);
                        } else {
                            g.unit_set_has_promotion(caster, promotion, true);
                            g.unit_set_has_promotion(u, promotion, false);
                        }
                    }
                }
            }
        }
    }
    if let Some(city) = g.map.plots[plot].plot_city {
        if g.city_team(city) != team {
            for building in d.buildings.keys() {
                if g.cities[city].num_real_building[building] > 0 {
                    if d.buildings[building].equipment {
                        if g.soren_rand_num(100, "Steal") <= 20 {
                            // Python quirk: war on the owner of `pUnit`, the last unit of the loop
                            // above, not on the city owner. A NULL unit's owner is -1 and
                            // `gc.getPlayer(-1).getTeam()` raises, ending the spell.
                            let Some(last_owner) = last_unit.and_then(|u| g.units.get(u)).map(|u| u.owner) else { return };
                            let p = g.units[caster].owner;
                            g.cf_start_war(p, last_owner, WarPlanType::WARPLAN_TOTAL);
                        } else {
                            for unit in d.units.keys() {
                                if d.units[unit].buildings[building] {
                                    g.city_set_num_real_building(city, building, 0);
                                    if let Some(promotion) = d.units[unit].equipment_promotion {
                                        g.unit_set_has_promotion(caster, promotion, true);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Shared start of `reqTakeEquipmentBuilding/Promotion/Unit(caster, unit)`: `None` where the
/// Python returns False, else the equipment unit type and its equipment promotion.
fn take_equipment_checks(g: &Game, caster: UnitId, call: &PyCall) -> Option<(UnitType, Option<PromotionType>)> {
    let d = g.d;
    if g.unit_get_unit_combat_type(caster) == Some(d.t::<UnitCombatType>("UNITCOMBAT_NAVAL")) {
        return None;
    }
    if g.unit_get_unit_combat_type(caster) == Some(d.t::<UnitCombatType>("UNITCOMBAT_SIEGE")) {
        return None;
    }
    if g.unit_get_special_unit_type(caster) == Some(d.t::<SpecialUnitType>("SPECIALUNIT_SPELL")) {
        return None;
    }
    if g.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_ILLUSION")] {
        return None;
    }
    let unit = equipment_unit(g, call);
    let promotion = d.units[unit].equipment_promotion;
    if has_promotion_opt(g, caster, promotion) {
        return None;
    }
    Some((unit, promotion))
}

/// `reqTakeEquipmentBuilding(caster, unit)`.
fn req_take_equipment_building(g: &Game, caster: UnitId, call: &PyCall) -> bool {
    take_equipment_checks(g, caster, call).is_some()
}

/// `spellTakeEquipmentBuilding(caster, unit)`.
fn spell_take_equipment_building(g: &mut Game, caster: UnitId, call: &PyCall) {
    let d = g.d;
    let unit = equipment_unit(g, call);
    let plot = g.unit_plot(caster);
    for building in d.buildings.keys() {
        if d.units[unit].buildings[building] {
            // bInCityOnly: there is a city (a NULL CyCity would ignore the call).
            if let Some(city) = g.map.plots[plot].plot_city {
                g.city_set_num_real_building(city, building, 0);
            }
        }
    }
}

/// `reqTakeEquipmentPromotion(caster, unit)`.
fn req_take_equipment_promotion(g: &Game, caster: UnitId, call: &PyCall) -> bool {
    let d = g.d;
    let Some((_, promotion)) = take_equipment_checks(g, caster, call) else { return false };
    let p = g.units[caster].owner;
    let plot = g.unit_plot(caster);
    let mut holder = None;
    for &u in &g.map.plots[plot].units {
        if g.units[u].owner == p && has_promotion_opt(g, u, promotion) {
            holder = Some(u);
        }
    }
    let Some(holder) = holder else { return false };
    if g.units[holder].has_casted {
        return false;
    }
    if g.units[holder].unit_type == d.t::<UnitType>("UNIT_BARNAXUS") {
        if promotion == Some(d.t::<PromotionType>("PROMOTION_PIECES_OF_BARNAXUS")) {
            return false;
        }
    }
    if !g.players[p].human {
        if g.unit_base_combat_str(caster) - 2 <= g.unit_base_combat_str(holder) {
            return false;
        }
        // (`promotion` is set: the holder has it.)
        if promotion.map_or(false, |pr| d.units[g.units[holder].unit_type].free_promotions[pr]) {
            return false;
        }
    }
    true
}

/// `spellTakeEquipmentPromotion(caster, unit)`.
fn spell_take_equipment_promotion(g: &mut Game, caster: UnitId, call: &PyCall) {
    let unit = equipment_unit(g, call);
    let promotion = g.d.units[unit].equipment_promotion;
    let p = g.units[caster].owner;
    let plot = g.unit_plot(caster);
    let mut holder = None;
    for &u in &g.map.plots[plot].units {
        // The Python also tests `pUnit != caster`, which compares two distinct CyUnit wrappers
        // and is always true.
        if g.units[u].owner == p && has_promotion_opt(g, u, promotion) {
            holder = Some(u);
        }
    }
    if let (Some(holder), Some(promotion)) = (holder, promotion) {
        g.unit_set_has_promotion(holder, promotion, false);
        g.unit_set_has_promotion(caster, promotion, true);
    }
}

/// The last unit of the equipment's type owned by `p` on `plot` and not about to die (the
/// holder `reqTakeEquipmentUnit`/`spellTakeEquipmentUnit` look for).
fn equipment_unit_holder(g: &Game, p: PlayerId, plot: PlotIdx, unit: UnitType) -> Option<UnitId> {
    let mut holder = None;
    for &u in &g.map.plots[plot].units {
        if g.units[u].owner == p && g.units[u].unit_type == unit {
            if !g.units[u].death_delay {
                holder = Some(u);
            }
        }
    }
    holder
}

/// `reqTakeEquipmentUnit(caster, unit)`.
fn req_take_equipment_unit(g: &Game, caster: UnitId, call: &PyCall) -> bool {
    let Some((unit, _)) = take_equipment_checks(g, caster, call) else { return false };
    let p = g.units[caster].owner;
    let plot = g.unit_plot(caster);
    equipment_unit_holder(g, p, plot, unit).is_some()
}

/// `spellTakeEquipmentUnit(caster, unit)`.
fn spell_take_equipment_unit(g: &mut Game, caster: UnitId, call: &PyCall) {
    let unit = equipment_unit(g, call);
    let p = g.units[caster].owner;
    let plot = g.unit_plot(caster);
    if let Some(holder) = equipment_unit_holder(g, p, plot, unit) {
        g.unit_kill(holder, true, None);
    }
}

/// `reqTaunt(caster)`.
fn req_taunt(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    let p = g.units[caster].owner;
    let team = g.players[p].team;
    let mut valid = false;
    for ix in x - 1..x + 2 {
        for iy in y - 1..y + 2 {
            let Some(loop_plot) = g.map.plot_xy(ix, iy) else { continue };
            for &u in &g.map.plots[loop_plot].units {
                if g.teams[team].at_war[g.unit_get_team(u)] {
                    if g.unit_is_alive(u) {
                        if g.unit_can_attack(u) {
                            if g.unit_get_domain_type(caster) == g.unit_get_domain_type(u) {
                                valid = true;
                            }
                        }
                    }
                }
            }
        }
    }
    valid
}

/// `spellTaunt(caster)`: adjacent enemies that fail to resist attack the caster's plot.
fn spell_taunt(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let spell = g.d.t::<SpellType>("SPELL_TAUNT");
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    let p = g.units[caster].owner;
    let plot = g.unit_plot(caster);
    let team = g.players[p].team;
    for ix in x - 1..x + 2 {
        for iy in y - 1..y + 2 {
            let Some(loop_plot) = g.map.plot_xy(ix, iy) else { continue };
            let n = g.map.plots[loop_plot].units.len();
            for i in 0..n {
                let Some(u) = plot_unit_at(g, loop_plot, i) else { continue };
                if g.teams[team].at_war[g.unit_get_team(u)] {
                    if g.unit_is_alive(u) {
                        if g.unit_can_attack(u) {
                            // Deviation: once an attack has killed the caster the C++ would read
                            // the deleted unit; the spell ends instead.
                            if !g.units.contains(caster) {
                                return;
                            }
                            if g.unit_get_domain_type(caster) == g.unit_get_domain_type(u) {
                                if !g.unit_is_resisted(u, caster, spell) {
                                    g.unit_attack(u, plot);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `spellTeleport(caster, loc)` (XML: `spellTeleport(pCaster,'Capital')`; `loc` is unused).
fn spell_teleport(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    // Without a capital the NULL CyCity answers getX()/getY() -1 and setXY goes there.
    let (x, y) = match g.player_get_capital_city(p) {
        Some(city) => (g.cities[city].x, g.cities[city].y),
        None => (-1, -1),
    };
    g.unit_set_xy(caster, x, y, false);
}

/// The first-level sphere promotions `reqTeachSpellcasting`/`spellTeachSpellcasting` look at,
/// in Python order.
const SPHERE1_PROMOTIONS: [&str; 18] = [
    "PROMOTION_AIR1",
    "PROMOTION_BODY1",
    "PROMOTION_CHAOS1",
    "PROMOTION_DEATH1",
    "PROMOTION_EARTH1",
    "PROMOTION_ENCHANTMENT1",
    "PROMOTION_ENTROPY1",
    "PROMOTION_FIRE1",
    "PROMOTION_ICE1",
    "PROMOTION_LAW1",
    "PROMOTION_LIFE1",
    "PROMOTION_METAMAGIC1",
    "PROMOTION_MIND1",
    "PROMOTION_NATURE1",
    "PROMOTION_SHADOW1",
    "PROMOTION_SPIRIT1",
    "PROMOTION_SUN1",
    "PROMOTION_WATER1",
];

/// `lList` of the teach-spellcasting functions: the caster's first-level sphere promotions.
fn teachable_promotions(g: &Game, caster: UnitId) -> Vec<PromotionType> {
    SPHERE1_PROMOTIONS
        .iter()
        .map(|name| g.d.t::<PromotionType>(name))
        .filter(|&promotion| g.units[caster].has_promotion[promotion])
        .collect()
}

/// `iAnimal`, `iBird` of the teach-spellcasting functions.
struct Learners {
    animal: UnitCombatType,
    bird: SpecialUnitType,
}

impl Learners {
    fn new(d: &GameData) -> Learners {
        Learners {
            animal: d.t::<UnitCombatType>("UNITCOMBAT_ANIMAL"),
            bird: d.t::<SpecialUnitType>("SPECIALUNIT_BIRD"),
        }
    }

    /// Whether `u` can learn spheres: owned by `p`, alive, neither an animal nor a bird.
    fn matches(&self, g: &Game, u: UnitId, p: PlayerId) -> bool {
        g.units[u].owner == p
            && g.unit_is_alive(u)
            && g.unit_get_unit_combat_type(u) != Some(self.animal)
            && g.unit_get_special_unit_type(u) != Some(self.bird)
    }
}

/// `reqTeachSpellcasting(caster)`.
fn req_teach_spellcasting(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let learners = Learners::new(g.d);
    let list = teachable_promotions(g, caster);
    if !list.is_empty() {
        let plot = g.unit_plot(caster);
        let p = g.units[caster].owner;
        for &u in &g.map.plots[plot].units {
            if learners.matches(g, u, p) {
                for &promotion in &list {
                    if !g.units[u].has_promotion[promotion] {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// `spellTeachSpellcasting(caster)`.
fn spell_teach_spellcasting(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let learners = Learners::new(g.d);
    let channeling1 = g.d.t::<PromotionType>("PROMOTION_CHANNELING1");
    let list = teachable_promotions(g, caster);
    let plot = g.unit_plot(caster);
    let p = g.units[caster].owner;
    let n = g.map.plots[plot].units.len();
    for i in 0..n {
        let Some(u) = plot_unit_at(g, plot, i) else { continue };
        if learners.matches(g, u, p) {
            for &promotion in &list {
                if !g.units[u].has_promotion[promotion] {
                    g.unit_set_has_promotion(u, promotion, true);
                    if !g.units[u].has_promotion[channeling1] {
                        g.unit_set_has_promotion(u, channeling1, true);
                    }
                }
            }
        }
    }
}

/// `spellTreetopDefence(caster)`.
fn spell_treetop_defence(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let plot = g.unit_plot(caster);
    let n = g.map.plots[plot].units.len();
    for i in 0..n {
        let Some(u) = plot_unit_at(g, plot, i) else { continue };
        if g.unit_get_team(u) == g.unit_get_team(caster) {
            g.unit_set_fortify_turns(u, 5);
        }
    }
}

/// `reqTrust(caster)`.
fn req_trust(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if g.players[p].feat_accomplished[FeatType::FEAT_TRUST] {
        return false;
    }
    if g.player_is_barbarian(p) {
        return false;
    }
    true
}

/// `spellTrust(caster)` (its `pCity = pPlayer.getCapitalCity()` is unused).
fn spell_trust(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let p = g.units[caster].owner;
    g.players[p].feat_accomplished[FeatType::FEAT_TRUST] = true;
}

/// `spellTsunami(caster)`.
fn spell_tsunami(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let cold = d.t::<DamageType>("DAMAGE_COLD");
    let (x, y) = (g.units[caster].x, g.units[caster].y);
    for ix in x - 2..x + 3 {
        for iy in y - 2..y + 3 {
            // (No isNone() check: a NULL plot is not adjacent to water.)
            let Some(plot) = g.map.plot_xy(ix, iy) else { continue };
            if g.plot_is_adjacent_to_water(plot) {
                if x != ix || y != iy {
                    let n = g.map.plots[plot].units.len();
                    for i in 0..n {
                        let Some(u) = plot_unit_at(g, plot, i) else { continue };
                        g.unit_do_damage(u, 30, 75, Some(caster), Some(cold), true);
                    }
                    if let Some(improvement) = g.map.plots[plot].improvement_type {
                        if !is_permanent(g, improvement) {
                            if g.soren_rand_num(100, "Tsunami") <= 25 {
                                g.plot_set_improvement_type(plot, None);
                            }
                        }
                    }
                    // (EFFECT_SPRING: graphics.)
                }
            }
        }
    }
}

/// `spellUnyieldingOrder(caster)`.
fn spell_unyielding_order(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let plot = g.unit_plot(caster);
    // bInCityOnly: there is a city (a NULL CyCity would ignore both calls).
    if let Some(city) = g.map.plots[plot].plot_city {
        g.city_set_occupation_timer(city, 0);
        g.city_change_hurry_anger_timer(city, -9);
    }
}

/// `reqUpgradeDovielloWarrior(caster)`.
fn req_upgrade_doviello_warrior(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let p = g.units[caster].owner;
    if !g.players[p].human {
        let team = g.players[p].team;
        if g.team_get_at_war_count(team, true) == 0 {
            return false;
        }
        if g.players[p].num_cities() > g.players[p].unit_class_count[g.d.t::<UnitClassType>("UNITCLASS_WORKER")] {
            return false;
        }
    }
    true
}

/// `reqVeilOfNight(caster)`. Change from FFH2: its only condition was AI-only (25 units and at
/// peace); it is AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_veil_of_night(_g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    true
}

/// `spellVeilOfNight(caster)`.
fn spell_veil_of_night(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let hidden_nationality = g.d.t::<PromotionType>("PROMOTION_HIDDEN_NATIONALITY");
    let p = g.units[caster].owner;
    for u in py_unit_list(g, p) {
        if g.unit_base_combat_str(u) > 0 {
            g.unit_set_has_promotion(u, hidden_nationality, true);
        }
    }
}

/// `reqVitalize(caster)`.
fn req_vitalize(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let plot = g.unit_plot(caster);
    if g.map.plots[plot].owner != Some(g.units[caster].owner) {
        return false;
    }
    if g.plot_is_water(plot) {
        return false;
    }
    let terrain = g.map.plots[plot].terrain_type;
    if terrain == Some(d.t::<TerrainType>("TERRAIN_GRASS")) {
        return false;
    }
    if terrain == Some(d.t::<TerrainType>("TERRAIN_BURNING_SANDS")) {
        return false;
    }
    if terrain == Some(d.t::<TerrainType>("TERRAIN_BROKEN_LANDS")) {
        return false;
    }
    if terrain == Some(d.t::<TerrainType>("TERRAIN_FIELDS_OF_PERDITION")) {
        return false;
    }
    if terrain == Some(d.t::<TerrainType>("TERRAIN_MARSH")) {
        return false;
    }
    true
}

/// `spellVitalize(caster)`.
fn spell_vitalize(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let plot = g.unit_plot(caster);
    let terrain = g.map.plots[plot].terrain_type;
    if terrain == Some(d.t::<TerrainType>("TERRAIN_SNOW")) {
        g.plot_set_terrain_type(plot, d.t::<TerrainType>("TERRAIN_TUNDRA"), true);
    } else if terrain == Some(d.t::<TerrainType>("TERRAIN_TUNDRA")) {
        g.plot_set_terrain_type(plot, d.t::<TerrainType>("TERRAIN_PLAINS"), true);
    } else if terrain == Some(d.t::<TerrainType>("TERRAIN_DESERT")) {
        g.plot_set_terrain_type(plot, d.t::<TerrainType>("TERRAIN_PLAINS"), true);
        if g.map.plots[plot].feature_type == Some(d.t::<FeatureType>("FEATURE_SCRUB")) {
            g.plot_set_feature_type(plot, None);
        }
    } else if terrain == Some(d.t::<TerrainType>("TERRAIN_PLAINS")) {
        g.plot_set_terrain_type(plot, d.t::<TerrainType>("TERRAIN_GRASS"), true);
    }
}

/// `reqWane(caster)`.
fn req_wane(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    if g.unit_get_unit_combat_type(caster) == Some(d.t::<UnitCombatType>("UNITCOMBAT_ANIMAL")) {
        return false;
    }
    if g.unit_get_unit_combat_type(caster) == Some(d.t::<UnitCombatType>("UNITCOMBAT_BEAST")) {
        return false;
    }
    if g.unit_is_immortal(caster) {
        return false;
    }
    let p = g.units[caster].owner;
    if g.players[p].unit_class_count[d.t::<UnitClassType>("UNITCLASS_SHADE")] >= 4 {
        return false;
    }
    true
}

/// `reqWarcry(caster)`. Change from FFH2: its only condition was AI-only (25 units and at war); it
/// is AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_warcry(_g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    true
}

/// `spellWarcry(caster)`.
fn spell_warcry(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let warcry = g.d.t::<PromotionType>("PROMOTION_WARCRY");
    let p = g.units[caster].owner;
    for u in py_unit_list(g, p) {
        if g.unit_get_unit_combat_type(u).is_some() {
            g.unit_set_has_promotion(u, warcry, true);
        }
    }
}

/// `reqWhiteout(caster)`.
fn req_whiteout(g: &Game, caster: UnitId, _call: &PyCall) -> bool {
    let d = g.d;
    let plot = g.unit_plot(caster);
    if g.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_HIDDEN")] {
        return false;
    }
    if g.map.plots[plot].terrain_type != Some(d.t::<TerrainType>("TERRAIN_SNOW")) {
        return false;
    }
    true
}

/// `reqWildHunt(caster)`. Change from FFH2: its only condition was AI-only (at war and 20 units);
/// it is AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_wild_hunt(_g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    true
}

/// `spellWildHunt(caster)`.
fn spell_wild_hunt(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let wolf = g.d.t::<UnitType>("UNIT_WOLF");
    let p = g.units[caster].owner;
    for u in py_unit_list(g, p) {
        if g.unit_base_combat_str(u) > 0 {
            let (x, y) = (g.units[u].x, g.units[u].y);
            let new_unit = g.player_init_unit(p, wolf, x, y, None, SOUTH);
            if g.unit_base_combat_str(u) > 3 {
                let i = py_div(g.unit_base_combat_str(u) - 2, 2);
                // `newUnit.setBaseCombatStr(2 + i)`
                g.units[new_unit].base_combat = 2 + i;
            }
        }
    }
}

/// `spellWonder(caster)`: 3-5 random effects.
fn spell_wonder(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let count = g.soren_rand_num(3, "Wonder") + 3;
    let p = g.units[caster].owner;
    // `pPlot` and `bCity` are taken once, before any effect (Escape can move the caster).
    let plot = g.unit_plot(caster);
    let city = g.map.plots[plot].plot_city.is_some();
    let (px, py) = g.map.xy(plot);
    let cast = |g: &mut Game, name: &str| g.unit_cast(caster, d.t::<SpellType>(name));
    for _ in 0..count {
        let rnd = g.soren_rand_num(66, "Wonder");
        let mut unit: Option<UnitType> = None;
        match rnd {
            0 => cast(g, "SPELL_BLAZE"),
            1 => cast(g, "SPELL_BLESS"),
            2 => cast(g, "SPELL_BLINDING_LIGHT"),
            3 => cast(g, "SPELL_BLOOM"),
            4 => cast(g, "SPELL_BLUR"),
            5 => cast(g, "SPELL_CHARM_PERSON"),
            6 => cast(g, "SPELL_CONTAGION"),
            7 => cast(g, "SPELL_COURAGE"),
            8 => cast(g, "SPELL_CRUSH"),
            9 => cast(g, "SPELL_DESTROY_UNDEAD"),
            10 => cast(g, "SPELL_DISPEL_MAGIC"),
            11 => cast(g, "SPELL_EARTHQUAKE"),
            12 => cast(g, "SPELL_ENCHANTED_BLADE"),
            13 => {
                // (EFFECT_SPELL1 and sound: graphics.)
                for ix in px - 1..px + 2 {
                    for iy in py - 1..py + 2 {
                        if let Some(loop_plot) = g.map.plot_xy(ix, iy) {
                            g.plot_change_plot_counter(loop_plot, 100);
                        }
                    }
                }
            }
            14 => cast(g, "SPELL_ENTANGLE"),
            15 => {
                if g.units[caster].owner != g.barbarian_player() {
                    cast(g, "SPELL_ESCAPE");
                }
            }
            16 => cast(g, "SPELL_FIREBALL"),
            17 => cast(g, "SPELL_FLAMING_ARROWS"),
            18 => cast(g, "SPELL_FLOATING_EYE"),
            19 => cast(g, "SPELL_HASTE"),
            20 => cast(g, "SPELL_HASTURS_RAZOR"),
            21 => cast(g, "SPELL_HEAL"),
            22 => cast(g, "SPELL_HIDE"),
            23 => cast(g, "SPELL_LOYALTY"),
            24 => cast(g, "SPELL_MAELSTROM"),
            25 => cast(g, "SPELL_MORALE"),
            26 => cast(g, "SPELL_MUTATION"),
            27 => cast(g, "SPELL_PILLAR_OF_FIRE"),
            28 => cast(g, "SPELL_POISONED_BLADE"),
            29 => cast(g, "SPELL_REVELATION"),
            30 => cast(g, "SPELL_RING_OF_FLAMES"),
            31 => cast(g, "SPELL_RUST"),
            32 => cast(g, "SPELL_SANCTIFY"),
            33 => cast(g, "SPELL_SCORCH"),
            34 => cast(g, "SPELL_SHADOWWALK"),
            35 => cast(g, "SPELL_SPORES"),
            36 => cast(g, "SPELL_SPRING"),
            37 => cast(g, "SPELL_STONESKIN"),
            38 => cast(g, "SPELL_SUMMON_AIR_ELEMENTAL"),
            39 => cast(g, "SPELL_SUMMON_AUREALIS"),
            40 => cast(g, "SPELL_SUMMON_BALOR"),
            41 => cast(g, "SPELL_SUMMON_DJINN"),
            42 => cast(g, "SPELL_SUMMON_EARTH_ELEMENTAL"),
            43 => cast(g, "SPELL_SUMMON_EINHERJAR"),
            44 => cast(g, "SPELL_SUMMON_FIRE_ELEMENTAL"),
            45 => unit = Some(d.t::<UnitType>("UNIT_KRAKEN")),
            46 => cast(g, "SPELL_SUMMON_MISTFORM"),
            47 => cast(g, "SPELL_SUMMON_PIT_BEAST"),
            48 => cast(g, "SPELL_SUMMON_SAND_LION"),
            49 => cast(g, "SPELL_SUMMON_SPECTRE"),
            50 => unit = Some(d.t::<UnitType>("UNIT_TIGER")),
            51 => unit = Some(d.t::<UnitType>("UNIT_TREANT")),
            52 => cast(g, "SPELL_SUMMON_WATER_ELEMENTAL"),
            53 => cast(g, "SPELL_SUMMON_WRAITH"),
            54 => cast(g, "SPELL_TSUNAMI"),
            55 => cast(g, "SPELL_VALOR"),
            56 => cast(g, "SPELL_VITALIZE"),
            57 => cast(g, "SPELL_WITHER"),
            58 | 59 => {
                if !city {
                    let mut valid = true;
                    if let Some(improvement) = g.map.plots[plot].improvement_type {
                        if is_permanent(g, improvement) {
                            valid = false;
                        }
                    }
                    if valid {
                        let improvement = if rnd == 58 {
                            d.t::<ImprovementType>("IMPROVEMENT_PENGUINS")
                        } else {
                            d.t::<ImprovementType>("IMPROVEMENT_MUSHROOMS")
                        };
                        g.plot_set_improvement_type(plot, Some(improvement));
                    }
                }
            }
            60 => {
                for promotion in d.promotions.keys() {
                    if g.units[caster].has_promotion[promotion] {
                        if d.promotions[promotion].race {
                            g.unit_set_has_promotion(caster, promotion, false);
                        }
                    }
                }
                // (`setUnitArtStyleType(UNIT_ARTSTYLE_BABOON)`: graphics.)
                if g.players[p].human {
                    let trophy = "TROPHY_FEAT_BABOON";
                    if !g.is_has_trophy(trophy) {
                        g.change_trophy_value(trophy, 1);
                    }
                }
            }
            61 => {
                // (EFFECT_SPELL1 and sound: graphics.)
                for ix in px - 2..px + 3 {
                    for iy in py - 2..py + 3 {
                        if let Some(loop_plot) = g.map.plot_xy(ix, iy) {
                            g.plot_change_plot_counter(loop_plot, -100);
                        }
                    }
                }
            }
            62 => unit = Some(d.t::<UnitType>("UNIT_SPIDERKIN")),
            63 => cast(g, "SPELL_SLOW"),
            64 => cast(g, "SPELL_SUMMON_ICE_ELEMENTAL"),
            65 => cast(g, "SPELL_SNOWFALL"),
            _ => {}
        }
        if let Some(unit) = unit {
            let new_unit = g.player_init_unit(p, unit, px, py, None, SOUTH);
            if g.players[p].traits[d.t::<TraitType>("TRAIT_SUMMONER")] {
                g.units[new_unit].duration = 2;
            } else {
                g.units[new_unit].duration = 1;
            }
        }
    }
}

/// `reqWorldbreak(caster)`. Change from FFH2: its AI-only branch (an Armageddon counter of 50) is
/// AI timing, in ffh-ai `world_spells` (docs/PORTING.md).
fn req_worldbreak(g: &Game, _caster: UnitId, _call: &PyCall) -> bool {
    if g.get_global_counter() == 0 {
        return false;
    }
    true
}

/// `spellWorldbreak(caster)`.
fn spell_worldbreak(g: &mut Game, caster: UnitId, _call: &PyCall) {
    let d = g.d;
    let counter = g.get_global_counter();
    let fire = d.t::<DamageType>("DAMAGE_FIRE");
    let forest = d.t::<FeatureType>("FEATURE_FOREST");
    let jungle = d.t::<FeatureType>("FEATURE_JUNGLE");
    let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
    for plot in g.map.plot_ids() {
        let mut valid = true;
        if g.plot_is_owned(plot) {
            if g.map.plots[plot].owner == Some(g.units[caster].owner) {
                valid = false;
            }
        }
        if valid {
            if let Some(city) = g.map.plots[plot].plot_city {
                if g.soren_rand_num(100, "Worldbreak") <= py_div(counter, 4) {
                    g.cf_do_city_fire(city);
                }
                let n = g.map.plots[plot].units.len();
                for i in 0..n {
                    let Some(u) = plot_unit_at(g, plot, i) else { continue };
                    g.unit_do_damage(u, counter, 100, Some(caster), Some(fire), false);
                }
                // (EFFECT_PILLAR_OF_FIRE: graphics.)
            }
            let feature = g.map.plots[plot].feature_type;
            if feature == Some(forest) || feature == Some(jungle) {
                if g.map.plots[plot].improvement_type.is_none() {
                    if g.soren_rand_num(100, "Flames Spread") <= py_div(counter, 4) {
                        g.plot_set_improvement_type(plot, Some(smoke));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Improvement / feature callbacks: `(pCaster, pPlot)`
// ---------------------------------------------------------------------------------------------

/// `atRangeGuardian(pCaster, pPlot)`: three barbarian gargoyles guard the improvement.
fn at_range_guardian(g: &mut Game, caster: UnitId, plot: PlotIdx, _call: &PyCall) {
    if g.map.plots[plot].units.is_empty() {
        // "fixes a problem if units spawn next to the gargoyle"
        if g.start_turn + 20 < g.game_turn() {
            let p = g.units[caster].owner;
            let barbarian = g.barbarian_player();
            if p != barbarian {
                let gargoyle = g.d.t::<UnitType>("UNIT_GARGOYLE");
                let (x, y) = g.map.xy(plot);
                for _ in 0..3 {
                    let new_unit = g.player_init_unit(barbarian, gargoyle, x, y, None, SOUTH);
                    g.unit_ai_set_unit_ai_type(new_unit, UnitAIType::UNITAI_ANIMAL);
                }
                g.map.plots[plot].python_active = false;
            }
        }
    }
}

/// `atRangeJungleAltar(pCaster, pPlot)`: worldbuilder scenarios only (`sf.atRangeJungleAltar`).
fn at_range_jungle_altar(_g: &mut Game, _caster: UnitId, _plot: PlotIdx, _call: &PyCall) {}

/// `atRangeNecrototem(pCaster, pPlot)`: the unit may flee in fear.
fn at_range_necrototem(g: &mut Game, caster: UnitId, plot: PlotIdx, _call: &PyCall) {
    // `cf.doFear(pCaster, pPlot, -1, False)`: doFear reads its caster argument only when the
    // fear is resistable, so the victim stands in for the -1. (Message on success.)
    g.cf_do_fear(caster, plot, caster, false);
}

/// `onMoveAncientForest(pCaster, pPlot)`: treants may rise against enemies of a Fellowship
/// owner.
fn on_move_ancient_forest(g: &mut Game, caster: UnitId, plot: PlotIdx, _call: &PyCall) {
    let d = g.d;
    if g.plot_is_owned(plot) {
        if g.map.plots[plot].units.len() == 1 {
            if !g.unit_is_flying(caster) {
                let mut chance = d.defines.treant_spawn_chance;
                if g.plot_is_being_worked(plot) {
                    let city = g.map.plots[plot].working_city;
                    // (A NULL CyCity's getNumBuilding() is -1.)
                    if city.map_or(false, |c| g.city_num_building(c, d.t::<BuildingType>("BUILDING_TEMPLE_OF_LEAVES")) > 0) {
                        chance *= 3;
                    }
                }
                if g.soren_rand_num(100, "Treant Spawn Chance") <= chance {
                    let p = g.units[caster].owner;
                    let p2 = g.map.plots[plot].owner.expect("owned plot");
                    let team = g.players[p].team;
                    let team2 = g.players[p2].team;
                    if g.teams[team].at_war[team2]
                        && g.player_get_state_religion(p2) == Some(d.t::<ReligionType>("RELIGION_FELLOWSHIP_OF_LEAVES"))
                    {
                        let n = g.map.plots[plot].units.len();
                        for i in 0..n {
                            let Some(u2) = plot_unit_at(g, plot, i) else { continue };
                            // `cf.findClearPlot(p2Unit, -1)`: the plot argument is only read
                            // when no unit is given.
                            if let Some(plot2) = g.cf_find_clear_plot(Some(u2), plot) {
                                let (x2, y2) = g.map.xy(plot2);
                                g.unit_set_xy(u2, x2, y2, false);
                                g.unit_finish_moves(u2);
                            }
                        }
                        if g.map.plots[plot].units.is_empty() {
                            let (x, y) = g.map.xy(plot);
                            let new_unit = g.player_init_unit(p2, d.t::<UnitType>("UNIT_TREANT"), x, y, None, SOUTH);
                            g.units[new_unit].duration = 3;
                        }
                    }
                }
            }
        }
    }
}

/// `onMoveBlizzard(pCaster, pPlot)`.
fn on_move_blizzard(g: &mut Game, caster: UnitId, _plot: PlotIdx, _call: &PyCall) {
    let d = g.d;
    if !g.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_WINTERBORN")] {
        g.unit_do_damage(caster, 10, 50, Some(caster), Some(d.t::<DamageType>("DAMAGE_COLD")), false);
    }
}

/// `onMoveLetumFrigus(pCaster, pPlot)`: fires the Letum Frigus event for human players only
/// (as in the Python; the event's choice then goes through the event system).
fn on_move_letum_frigus(g: &mut Game, caster: UnitId, plot: PlotIdx, _call: &PyCall) {
    let p = g.units[caster].owner;
    if g.players[p].human {
        let trigger = g.d.t::<EventTriggerType>("EVENTTRIGGER_LETUM_FRIGUS");
        let (x, y) = (g.units[caster].x, g.units[caster].y);
        g.player_init_triggered_data(p, trigger, true, None, x, y, Some(p), None, None, None, None, None);
        g.map.plots[plot].python_active = false;
    }
}

/// `onMoveMaelstrom(pCaster, pPlot)`: the unit sinks or is thrown to a random ocean plot.
fn on_move_maelstrom(g: &mut Game, caster: UnitId, _plot: PlotIdx, _call: &PyCall) {
    if g.soren_rand_num(100, "Maelstrom") <= 25 {
        g.unit_kill(caster, true, None);
    } else {
        let ocean = g.d.t::<TerrainType>("TERRAIN_OCEAN");
        let mut best_value = 0;
        let mut best_plot = None;
        for target_plot in g.map.plot_ids() {
            if g.map.plots[target_plot].terrain_type == Some(ocean) {
                let mut value = g.soren_rand_num(1000, "Maelstrom");
                if !g.plot_is_owned(target_plot) {
                    value += 1000;
                }
                if value > best_value {
                    best_value = value;
                    best_plot = Some(target_plot);
                }
            }
        }
        if let Some(best_plot) = best_plot {
            let (x, y) = g.map.xy(best_plot);
            g.unit_set_xy(caster, x, y, false);
            g.unit_set_damage(caster, 25, None);
        }
    }
}

/// `onMoveMirrorOfHeaven(pCaster, pPlot)`: worldbuilder scenarios only.
fn on_move_mirror_of_heaven(_g: &mut Game, _caster: UnitId, _plot: PlotIdx, _call: &PyCall) {}

/// `onMovePoolOfTears(pCaster, pPlot)`: cures disease, plague, poison and withering.
fn on_move_pool_of_tears(g: &mut Game, caster: UnitId, _plot: PlotIdx, _call: &PyCall) {
    let d = g.d;
    let diseased = d.t::<PromotionType>("PROMOTION_DISEASED");
    if g.units[caster].has_promotion[diseased] {
        g.unit_set_has_promotion(caster, diseased, false);
    }
    let plagued = d.t::<PromotionType>("PROMOTION_PLAGUED");
    if g.units[caster].has_promotion[plagued] {
        g.unit_set_has_promotion(caster, plagued, false);
    }
    let poisoned = d.t::<PromotionType>("PROMOTION_POISONED");
    if g.units[caster].has_promotion[poisoned] {
        g.unit_set_has_promotion(caster, poisoned, false);
    }
    let withered = d.t::<PromotionType>("PROMOTION_WITHERED");
    if g.units[caster].has_promotion[withered] {
        g.unit_set_has_promotion(caster, withered, false);
    }
}

/// `onMoveJungleAltar(pCaster, pPlot)`: worldbuilder scenarios only.
fn on_move_jungle_altar(_g: &mut Game, _caster: UnitId, _plot: PlotIdx, _call: &PyCall) {}

/// `onMovePortal(pCaster, pPlot)`: worldbuilder scenarios only.
fn on_move_portal(_g: &mut Game, _caster: UnitId, _plot: PlotIdx, _call: &PyCall) {}

/// `onMoveWarningPost(pCaster, pPlot)`: worldbuilder scenarios only.
fn on_move_warning_post(_g: &mut Game, _caster: UnitId, _plot: PlotIdx, _call: &PyCall) {}

// ---------------------------------------------------------------------------------------------
// Votes
// ---------------------------------------------------------------------------------------------

/// `voteFundDissidents()`: hurry anger in about half the cities of every Overcouncil member.
fn vote_fund_dissidents(g: &mut Game, _vote: VoteType, _data: i32, _call: &PyCall) {
    let overcouncil = g.d.t::<VoteSourceType>("DIPLOVOTE_OVERCOUNCIL");
    for p in g.player_ids() {
        if g.players[p].alive {
            if g.player_is_full_member(p, overcouncil) {
                for city in py_city_list(g, p) {
                    if g.soren_rand_num(100, "Fund Dissidents") < 50 {
                        let change = 1 + g.soren_rand_num(3, "Fund Dissidents");
                        g.city_change_hurry_anger_timer(city, change);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Registry class of an info id type named in this file.
    fn class_of(ty: &str) -> &'static str {
        match ty {
            "AlignmentType" => AlignmentType::CLASS,
            "BuildingType" => BuildingType::CLASS,
            "CivicOptionType" => CivicOptionType::CLASS,
            "CivicType" => CivicType::CLASS,
            "CivilizationType" => CivilizationType::CLASS,
            "DamageType" => DamageType::CLASS,
            "EventTriggerType" => EventTriggerType::CLASS,
            "FeatureType" => FeatureType::CLASS,
            "GameSpeedType" => GameSpeedType::CLASS,
            "GoodyType" => GoodyType::CLASS,
            "ImprovementType" => ImprovementType::CLASS,
            "PromotionType" => PromotionType::CLASS,
            "ReligionType" => ReligionType::CLASS,
            "SpecialUnitType" => SpecialUnitType::CLASS,
            "SpellType" => SpellType::CLASS,
            "TechType" => TechType::CLASS,
            "TerrainType" => TerrainType::CLASS,
            "TraitType" => TraitType::CLASS,
            "UnitClassType" => UnitClassType::CLASS,
            "UnitCombatType" => UnitCombatType::CLASS,
            "UnitType" => UnitType::CLASS,
            "VoteSourceType" => VoteSourceType::CLASS,
            _ => panic!("add {ty} to class_of"),
        }
    }

    /// Every type name this file looks up exists in the game data under the expected class:
    /// each typed lookup `d.t` of a literal name, the `spellWonder` spells (`cast(g, ..)`) and
    /// the sphere promotions.
    #[test]
    fn type_names_resolve() {
        let d = ffh_data::global();
        let src = include_str!("spells_b.rs");
        let mut checked = 0;
        let lookup = concat!("t::", "<");
        let mut rest = src;
        while let Some(i) = rest.find(lookup) {
            rest = &rest[i + lookup.len()..];
            let close = rest.find('>').unwrap();
            let ty = &rest[..close];
            let Some(args) = rest[close + 1..].strip_prefix("(\"") else { continue };
            let name = &args[..args.find('"').unwrap()];
            assert_eq!(d.reg.entry(name).map(|e| e.class), Some(class_of(ty)), "{ty} {name}");
            checked += 1;
        }
        let wonder = concat!("cast(g", ", \"");
        let mut rest = src;
        while let Some(i) = rest.find(wonder) {
            rest = &rest[i + wonder.len()..];
            let name = &rest[..rest.find('"').unwrap()];
            assert!(d.ty::<SpellType>(name).is_some(), "SpellType {name}");
            checked += 1;
        }
        for name in SPHERE1_PROMOTIONS {
            assert!(d.ty::<PromotionType>(name).is_some(), "PromotionType {name}");
        }
        assert!(checked > 200, "only {checked} names found");
    }

    /// The literal arguments of the take-equipment spells in the XML name equipment units.
    #[test]
    fn take_equipment_arguments_resolve() {
        let d = ffh_data::global();
        let mut checked = 0;
        for spell in d.spells.iter() {
            for src in [&spell.py_result, &spell.py_requirement] {
                let Some(call) = PyCall::parse(src) else { continue };
                if call.name.contains("TakeEquipment") {
                    let name = call.str_arg(1).expect("equipment argument");
                    let unit = d.ty::<UnitType>(name).unwrap_or_else(|| panic!("UnitType {name}"));
                    if call.name.contains("Promotion") {
                        assert!(d.units[unit].equipment_promotion.is_some(), "{name} has no equipment promotion");
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 0);
    }

    /// A human takes Syliven's Perfect Lyre out of its city and drops it again, twice: each drop
    /// (`CreateBuildingType`) counts the world wonder as created again, past its
    /// `iMaxGlobalInstances` of 1, as in C++ (the take spell's Python does not lower the count).
    /// Production checks of the class then must not trip the C++ "created <= max" assertion,
    /// which the checked build did (ffh-play soak; docs/PLAY_DESIGN.md P2).
    #[test]
    fn equipment_wonder_drop_counts_it_created_again() {
        let d = ffh_data::global();
        let mut g = Game::new(d, crate::setup::GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 5));
        g.autorun = true;
        g.game_init();
        let (p, other) = (PlayerId(0), PlayerId(1));
        g.setup.players[0].slot_status = crate::setup::SlotStatus::Taken;
        g.setup.players[0].human = true;
        g.player_update_human(p);

        let start = |g: &Game, p: PlayerId| {
            let u = g.players[p].units[0];
            (g.units[u].x, g.units[u].y)
        };
        let (x, y) = start(&g, p);
        let city = g.player_init_city(p, x, y, true, true);
        let (ox, oy) = start(&g, other);
        let other_city = g.player_init_city(other, ox, oy, true, true);
        let caster = g.player_init_unit(p, d.t::<UnitType>("UNIT_WARRIOR"), x, y, None, SOUTH);

        let lyre = d.t::<BuildingType>("BUILDING_SYLIVENS_PERFECT_LYRE");
        let class = d.buildings[lyre].building_class_type.unwrap();
        assert_eq!(d.building_classes[class].max_global_instances, 1);
        let take = d.t::<SpellType>("SPELL_TAKE_SYLIVENS_PERFECT_LYRE_BUILDING");
        let drop = d.t::<SpellType>("SPELL_DROP_SYLIVENS_PERFECT_LYRE");
        let carried = d.t::<PromotionType>("PROMOTION_SYLIVENS_PERFECT_LYRE");

        // The city completes the wonder.
        g.city_set_num_real_building(city, lyre, 1);
        assert_eq!(g.building_class_created_count[class], 1);
        assert!(g.is_building_class_maxed_out(class, 0));

        for created in 2..=3 {
            g.units[caster].has_casted = false;
            assert!(g.unit_can_cast(caster, take, false));
            g.unit_cast(caster, take);
            assert!(g.units[caster].has_promotion[carried]);
            assert_eq!(g.cities[city].num_real_building[lyre], 0);
            // Taking it does not free the class.
            assert_eq!(g.building_class_created_count[class], created - 1);
            assert!(!g.city_can_construct(other_city, lyre, false, false, false));

            g.units[caster].has_casted = false;
            assert!(g.unit_can_cast(caster, drop, false));
            g.unit_cast(caster, drop);
            assert!(!g.units[caster].has_promotion[carried]);
            assert_eq!(g.cities[city].num_real_building[lyre], 1);
            assert_eq!(g.building_class_created_count[class], created);
        }

        // C++ quirk: created 3 times against a maximum of 1. The class stays maxed out, and
        // the production checks run without the C++ assertion.
        assert!(g.is_building_class_maxed_out(class, 0));
        assert!(!g.city_can_construct(city, lyre, false, false, false));
        assert!(!g.city_can_construct(other_city, lyre, false, false, false));

        // The take spell has no bAllowAI: an AI player cannot cast it.
        g.units[caster].has_casted = false;
        g.setup.players[0].slot_status = crate::setup::SlotStatus::Computer;
        g.setup.players[0].human = false;
        g.player_update_human(p);
        assert!(!g.unit_can_cast(caster, take, false));
    }

    #[test]
    fn python_negative_list_index() {
        assert_eq!(py_list_index(-1, 3), 2);
        assert_eq!(py_list_index(0, 3), 0);
        assert_eq!(py_list_index(1, 3), 1);
    }

    #[test]
    fn every_function_is_registered() {
        let req = [
            "reqPirateCove", "reqMarchOfTheTrees", "reqMotherLode", "reqPromoteSettlement", "reqRagingSeas",
            "reqRally", "reqRebuildBarnaxus", "reqRecruitMercenary", "reqReligiousFervor", "reqRepair",
            "reqRessurection", "reqReveal", "reqRevelry", "reqRevelation", "reqRiverOfBlood", "reqRoar",
            "reqRobGrave", "reqSanctify", "reqSanctuary", "reqSandLion", "reqScorch", "reqSironasTouch",
            "reqSpreadTheCouncilOfEsus", "reqSpring", "reqSprint", "reqStasis", "reqSteal",
            "reqTakeEquipmentBuilding", "reqTakeEquipmentPromotion", "reqTakeEquipmentUnit", "reqTaunt",
            "reqTeachSpellcasting", "reqTrust", "reqUpgradeDovielloWarrior", "reqVeilOfNight", "reqVitalize",
            "reqWane", "reqWarcry", "reqWhiteout", "reqWildHunt", "reqWorldbreak",
        ];
        for name in req {
            assert!(find_req(name).is_some(), "{name}");
        }
        let spell = [
            "spellMarchOfTheTrees", "spellMotherLode", "spellOpenChest", "spellPromoteSettlement",
            "spellRagingSeas", "spellRaiseSkeleton", "spellRally", "spellReadTheGrimoire", "spellRebuildBarnaxus",
            "spellRecruit", "spellRecruitMercenary", "spellReleaseFromCage", "spellReligiousFervor", "spellRepair",
            "spellRessurection", "spellRessurectionGraveyard", "spellReveal", "spellRevelry", "spellRevelation",
            "spellRingofFlames", "spellRiverOfBlood", "spellRoar", "spellRobGrave", "spellSacrificeAltar",
            "spellSacrificePyre", "spellSanctify", "spellSanctuary", "spellScorch", "spellSing",
            "spellSironasTouch", "spellSlaveTradeBuy", "spellSnowfall", "spellSnowfallGreator", "spellSpring",
            "spellStasis", "spellSteal", "spellTakeEquipmentBuilding", "spellTakeEquipmentPromotion",
            "spellTakeEquipmentUnit", "spellTaunt", "spellTeleport", "spellTeachSpellcasting",
            "spellTreetopDefence", "spellTrust", "spellTsunami", "spellUnyieldingOrder", "spellVeilOfNight",
            "spellVitalize", "spellWarcry", "spellWildHunt", "spellWonder", "spellWorldbreak",
        ];
        for name in spell {
            assert!(find_spell(name).is_some(), "{name}");
        }
        let plot = [
            "atRangeGuardian", "atRangeJungleAltar", "atRangeNecrototem", "onMoveAncientForest", "onMoveBlizzard",
            "onMoveLetumFrigus", "onMoveMaelstrom", "onMoveMirrorOfHeaven", "onMovePoolOfTears",
            "onMoveJungleAltar", "onMovePortal", "onMoveWarningPost",
        ];
        for name in plot {
            assert!(find_plot_fn(name).is_some(), "{name}");
        }
        assert!(find_vote_fn("voteFundDissidents").is_some());
        assert_eq!(req.len() + spell.len() + plot.len() + 1, 106);
    }
}
