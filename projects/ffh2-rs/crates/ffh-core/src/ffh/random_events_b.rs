//! Port of CvRandomEventInterface.py (part b): every trigger/event callback from
//! `canTriggerDustbowlCont` (Python line 3304) to the end of the module, plus the module
//! helpers they use (`getSaltpeterNumExtraPlots`, `getNumPartisanUnits`, `getGreedUnit`,
//! `getHostileTakeoverListResources`). Each Python function becomes a private Rust `fn` (its doc
//! comment names the Python function) registered by its Python name in the `find_*` lookup of
//! the XML slot it fills (see dispatch.rs).
//!
//! Most of this range is stock BTS code that the FFH2 XML no longer references; the callbacks
//! FFH2 does use are `canTriggerDustbowlCont`, `canTriggerChampion(Unit)`/`applyChampion`,
//! `canTriggerAntelope`/`doAntelope2`, `canTriggerWhaleOfAThing`, `doAncientOlympics2`,
//! `canTriggerHeroicGesture`/`doHeroicGesture2`, `canTriggerGreatMediator`/`doGreatMediator2`,
//! `doAncientTexts2`, `canTriggerLiteracy`, `canTriggerEsteemedPlaywright`,
//! `canTriggerExperiencedCaptain`, `doGreatBeast3` and `canTriggerControversialPhilosopherCity`.
//! The unreferenced ones are registered in the slot their Python signature belongs to. The two
//! event callbacks the XML names but no Python defines (`applyHeresy2`, `applyTowerShield`) are
//! registered here as no-ops (see [`missing_python_callback`]).
//!
//! Porting notes:
//! - Trigger `PythonCanDo` functions get the triggered data being tested; the Python edits it
//!   through `player.getEventTriggered(kTriggeredData.iId)`, i.e. the same object, so the port
//!   writes `data`. Where the Python reads `kTriggeredData` (its copy) after such a write, the
//!   port reads the value before writing. Event can-do / expire hooks are read-only (`&Game`,
//!   `&EventTriggeredData`); the one BTS callback that writes there, `canApplyMasterBlacksmithDone1`
//!   (it stores the plot it found), is matched by a new search in `applyMasterBlacksmithDone1`.
//! - Many type names here are BTS types FFH2 does not define; `CvUtil.findInfoTypeNum` /
//!   `gc.getInfoTypeForString` then give -1 (`d.ty` -> `None`). Comparisons with -1 are ported
//!   as `Option` comparisons (e.g. a building's `getFreeBonus() == -1` holds for buildings without
//!   one). Where the C++ would index an array with -1 (undefined behaviour: `getBuildingClassCount`,
//!   `getUnitClassCount`, `getNumAvailableBonuses`, `getProjectCount`, `isHasBuilding`,
//!   `isHasReligion`, `getCivilizationUnits`, ...) the port answers 0 / false / -1, and actions
//!   with such a type are skipped.
//! - `gc.getPlayer(p).getCity(id)` / `getUnit(id)` return a wrapper of NULL when the player has
//!   no such object (`isNone()`; getters answer -1/false, actions are no-ops); here they are
//!   `None` ([`player_city`], [`player_unit`]). C++ ids are per player, Rust ids are global, so
//!   "the player has it" is "it exists and the player owns it".
//! - A Python exception (`None.getX()`, `gc.getBuildingInfo(-1).getBuildingClassType()`, ...)
//!   ends the function and the C++ caller's `lResult` stays uninitialized; where the port can
//!   reach that case, can-do / expire checks answer false and actions stop there.
//! - `list.sort()` on `(distance, CyCity)` / `(distance, CyPlot)` tuples breaks distance ties by
//!   comparing the wrapper objects (memory addresses); the port keeps list order for ties.
//! - UI-only Python (messages, popups, the wonder movie) is omitted. The human-only popups of
//!   `doHeroicGesture2` / `doGreatMediator2` (whose answers go to `heroicGesture2Callback` /
//!   `greatMediator2Callback`) are replaced by their AI branch, which equals the "yes" answer
//!   (for `doGreatMediator2` the popup's "yes" makes the other team call `makePeace`).
//! - AI valuations: `getGreedUnit` needs `CvPlayerAI::AI_unitValue(.., UNITAI_ATTACK, area)`,
//!   ported here ([`ai_unit_value_attack`], AI strategy flags taken as off); `applyEarthDay2`'s
//!   `AI_civicValue` is omitted (`// AI:`), so its candidates are ordered by player id.

use std::cmp::max;

use crate::ffh::dispatch::*;
use crate::ffh::game_utils::py_div;
use crate::game::Game;
use crate::ids::*;
use crate::structs::{EventTriggeredData, TradeData};
use ffh_data::*;

// ---------------------------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------------------------

/// Trigger PythonCanDo (`canTriggerX(argsList=[triggerData])`).
pub fn find_trigger_can_do(name: &str) -> Option<TriggerCanDoFn> {
    let f: TriggerCanDoFn = match name {
        "canTriggerDustbowlCont" => can_trigger_dustbowl_cont,
        "canTriggerChampion" => can_trigger_champion,
        "canTriggerElectricCompany" => can_trigger_electric_company,
        "canTriggerGoldRush" => can_trigger_gold_rush,
        "canTriggerInfluenza" => can_trigger_influenza,
        "canTriggerSoloFlight" => can_trigger_solo_flight,
        "canTriggerAntelope" => can_trigger_antelope,
        "canTriggerWhaleOfAThing" => can_trigger_whale_of_a_thing,
        "canTriggerHiyoSilver" => can_trigger_hiyo_silver,
        "canTriggerWiningMonks" => can_trigger_wining_monks,
        "canTriggerIndependentFilms" => can_trigger_independent_films,
        "canTriggerAncientOlympics" => can_trigger_ancient_olympics,
        "canTriggerModernOlympics" => can_trigger_modern_olympics,
        "canTriggerInterstate" => can_trigger_interstate,
        "canTriggerHeroicGesture" => can_trigger_heroic_gesture,
        "canTriggerGreatMediator" => can_trigger_great_mediator,
        "canTriggerImpactCrater" => can_trigger_impact_crater,
        "canTriggerTheHuns" => can_trigger_the_huns,
        "canTriggerTheVandals" => can_trigger_the_vandals,
        "canTriggerTheGoths" => can_trigger_the_goths,
        "canTriggerThePhilistines" => can_trigger_the_philistines,
        "canTriggerTheVedicAryans" => can_trigger_the_vedic_aryans,
        "canTriggerSecurityTax" => can_trigger_security_tax,
        "canTriggerLiteracy" => can_trigger_literacy,
        "canTriggerTea" => can_trigger_tea,
        "canTriggerHorseWhispering" => can_trigger_horse_whispering,
        "canTriggerHorseWhisperingDone" => can_trigger_horse_whispering_done,
        "canTriggerHarbormaster" => can_trigger_harbormaster,
        "canTriggerHarbormasterDone" => can_trigger_harbormaster_done,
        "canTriggerClassicLiterature" => can_trigger_classic_literature,
        "canTriggerClassicLiteratureDone" => can_trigger_classic_literature_done,
        "canTriggerMasterBlacksmith" => can_trigger_master_blacksmith,
        "canTriggerMasterBlacksmithDone" => can_trigger_master_blacksmith_done,
        "canTriggerBestDefense" => can_trigger_best_defense,
        "canTriggerBestDefenseDone" => can_trigger_best_defense_done,
        "canTriggerSportsLeague" => can_trigger_sports_league,
        "canTriggerSportsLeagueDone" => can_trigger_sports_league_done,
        "canTriggerCrusade" => can_trigger_crusade,
        "canTriggerCrusadeDone" => can_trigger_crusade_done,
        "canTriggerEsteemedPlaywright" => can_trigger_esteemed_playwright,
        "canTriggerHighWarlord" => can_trigger_high_warlord,
        "canTriggerExperiencedCaptain" => can_trigger_experienced_captain,
        "canTriggerGreed" => can_trigger_greed,
        "canTriggerGreedDone" => can_trigger_greed_done,
        "canTriggerWarChariots" => can_trigger_war_chariots,
        "canTriggerWarChariotsDone" => can_trigger_war_chariots_done,
        "canTriggerEliteSwordsDone" => can_trigger_elite_swords_done,
        "canTriggerWarships" => can_trigger_warships,
        "canTriggerWarshipsDone" => can_trigger_warships_done,
        "canTriggerGunsButterDone" => can_trigger_guns_butter_done,
        "canTriggerNobleKnights" => can_trigger_noble_knights,
        "canTriggerNobleKnightsDone" => can_trigger_noble_knights_done,
        "canTriggerOverwhelm" => can_trigger_overwhelm,
        "canTriggerOverwhelmDone" => can_trigger_overwhelm_done,
        "canTriggerCorporateExpansion" => can_trigger_corporate_expansion,
        "canTriggerCorporateExpansionDone" => can_trigger_corporate_expansion_done,
        "canTriggerHostileTakeover" => can_trigger_hostile_takeover,
        "canTriggerHostileTakeoverDone" => can_trigger_hostile_takeover_done,
        "canDoCometFragment" => can_do_comet_fragment,
        "canTriggerNuclearProtest" => can_trigger_nuclear_protest,
        "canTriggerToxcatl" => can_trigger_toxcatl,
        "canTriggerDissidentPriest" => can_trigger_dissident_priest,
        "canTriggerRogueStation" => can_trigger_rogue_station,
        "canTriggerAntiMonarchists" => can_trigger_anti_monarchists,
        "canTriggerImpeachment" => can_trigger_impeachment,
        _ => return None,
    };
    Some(f)
}

/// Trigger PythonCanDoCity (`canTriggerXCity(eTrigger, ePlayer, iCity)`).
pub fn find_trigger_city(name: &str) -> Option<TriggerCityFn> {
    let f: TriggerCityFn = match name {
        "canTriggerImmigrantCity" => can_trigger_immigrant_city,
        "canTriggerControversialPhilosopherCity" => can_trigger_controversial_philosopher_city,
        "canTriggerPreachingResearcherCity" => can_trigger_preaching_researcher_city,
        "canTriggerDissidentPriestCity" => can_trigger_dissident_priest_city,
        "canTriggerImpeachmentCity" => can_trigger_impeachment_city,
        _ => return None,
    };
    Some(f)
}

/// Trigger PythonCanDoUnit (`canTriggerXUnit(eTrigger, ePlayer, iUnit)`).
pub fn find_trigger_unit(name: &str) -> Option<TriggerUnitFn> {
    let f: TriggerUnitFn = match name {
        "canTriggerChampionUnit" => can_trigger_champion_unit,
        _ => return None,
    };
    Some(f)
}

/// Trigger PythonCallback (`doX(triggerData, ePlayer)`). None in this part of the module.
pub fn find_trigger_callback(name: &str) -> Option<TriggerCallbackFn> {
    match name {
        _ => None,
    }
}

/// Event PythonCanDo (`canApplyX(eEvent, triggerData)`).
pub fn find_event_can_do(name: &str) -> Option<EventCanDoFn> {
    let f: EventCanDoFn = match name {
        "canApplySaltpeter" => can_apply_saltpeter,
        "canApplyEarthDay2" => can_apply_earth_day2,
        "canApplyFreedomConcert2" => can_apply_freedom_concert2,
        "canApplyClassicLiteratureDone2" => can_apply_classic_literature_done2,
        "canApplyClassicLiteratureDone3" => can_apply_classic_literature_done3,
        "canApplyMasterBlacksmithDone1" => can_apply_master_blacksmith_done1,
        "canApplyMasterBlacksmithDone3" => can_apply_master_blacksmith_done3,
        "canApplyBestDefenseDone3" => can_apply_best_defense_done3,
        "canApplySportsLeagueDone3" => can_apply_sports_league_done3,
        "canApplyCrusadeDone1" => can_apply_crusade_done1,
        "canApplyCrusadeDone2" => can_apply_crusade_done2,
        "canApplyCrusadeDone3" => can_apply_crusade_done3,
        "canApplyPartisans1" => can_apply_partisans1,
        "canApplyPartisans2" => can_apply_partisans2,
        "canApplyEliteSwordsDone2" => can_apply_elite_swords_done2,
        "canApplyWarshipsDone2" => can_apply_warships_done2,
        "canApplyGunsButterDone2" => can_apply_guns_butter_done2,
        "canApplyGunsButterDone3" => can_apply_guns_butter_done3,
        "canApplyNobleKnightsDone2" => can_apply_noble_knights_done2,
        "canApplyOverwhelmDone3" => can_apply_overwhelm_done3,
        "canDoSpyDiscovered3" => can_do_spy_discovered3,
        _ => return None,
    };
    Some(f)
}

/// Event PythonCallback (`applyX(eEvent, triggerData)`).
pub fn find_event_callback(name: &str) -> Option<EventCallbackFn> {
    let f: EventCallbackFn = match name {
        "applySaltpeter" => apply_saltpeter,
        "applyGreatDepression" => apply_great_depression,
        "applyChampion" => apply_champion,
        "applyInfluenza2" => apply_influenza2,
        "applySoloFlight" => apply_solo_flight,
        "doAntelope2" => do_antelope2,
        "doWiningMonks2" => do_wining_monks2,
        "doIndependentFilms" => do_independent_films,
        "doAncientOlympics2" => do_ancient_olympics2,
        "applyModernOlympics" => apply_modern_olympics,
        "applyInterstate" => apply_interstate,
        "applyEarthDay2" => apply_earth_day2,
        "applyFreedomConcert2" => apply_freedom_concert2,
        "doHeroicGesture2" => do_heroic_gesture2,
        "doGreatMediator2" => do_great_mediator2,
        "doAncientTexts2" => do_ancient_texts2,
        "doImpactCrater2" => do_impact_crater2,
        "applyTheHuns1" => apply_the_huns1,
        "applyTheVandals1" => apply_the_vandals1,
        "applyTheGoths1" => apply_the_goths1,
        "applyThePhilistines1" => apply_the_philistines1,
        "applyTheVedicAryans1" => apply_the_vedic_aryans1,
        "applyHorseWhisperingDone1" => apply_horse_whispering_done1,
        "applyClassicLiteratureDone2" => apply_classic_literature_done2,
        "applyClassicLiteratureDone3" => apply_classic_literature_done3,
        "applyMasterBlacksmithDone1" => apply_master_blacksmith_done1,
        "applyBestDefenseDone2" => apply_best_defense_done2,
        "applyCrusadeDone1" => apply_crusade_done1,
        "applyCrusadeDone2" => apply_crusade_done2,
        "applyCrusadeDone3" => apply_crusade_done3,
        "applySecretKnowledge2" => apply_secret_knowledge2,
        "applyPartisans1" => apply_partisans1,
        "applyPartisans2" => apply_partisans2,
        "applyGreedDone1" => apply_greed_done1,
        "applyOverwhelmDone3" => apply_overwhelm_done3,
        "applyCorporateExpansionDone1" => apply_corporate_expansion_done1,
        "applyHostileTakeoverDone1" => apply_hostile_takeover_done1,
        "doGreatBeast3" => do_great_beast3,
        "doSpyDiscovered3" => do_spy_discovered3,
        "doNuclearProtest1" => do_nuclear_protest1,
        // Named by the XML but defined nowhere in the FFH2 Python (see `missing_python_callback`).
        "applyHeresy2" => missing_python_callback,
        "applyTowerShield" => missing_python_callback,
        _ => return None,
    };
    Some(f)
}

/// Event PythonExpireCheck (`expireX(eEvent, triggerData)`).
pub fn find_event_expire(name: &str) -> Option<EventExpireFn> {
    let f: EventExpireFn = match name {
        "expireMasterBlacksmith1" => expire_master_blacksmith1,
        "expireCrusade1" => expire_crusade1,
        "expireGreed1" => expire_greed1,
        "expireCorporateExpansion1" => expire_corporate_expansion1,
        "expireHostileTakeover1" => expire_hostile_takeover1,
        _ => return None,
    };
    Some(f)
}

// ---------------------------------------------------------------------------------------------
// Helpers (Python idioms)
// ---------------------------------------------------------------------------------------------

/// `DirectionTypes.DIRECTION_SOUTH` (facing of units created by `initUnit`).
const SOUTH: Option<DirectionType> = Some(DirectionType::DIRECTION_SOUTH);

/// `pPlayer.initUnit(eUnit, iX, iY, eUnitAI, DirectionTypes.DIRECTION_SOUTH)`.
fn init_unit(g: &mut Game, p: PlayerId, unit: UnitType, x: i32, y: i32, unit_ai: UnitAIType) {
    g.player_init_unit(p, unit, x, y, Some(unit_ai), SOUTH);
}

/// `gc.getPlayer(p).getCity(iID)`: the city if `p` has it; `None` is the NULL `CyCity`.
fn player_city(g: &Game, p: PlayerId, id: Option<CityId>) -> Option<CityId> {
    let id = id?;
    match g.cities.get(id) {
        Some(city) if city.owner == p => Some(id),
        _ => None,
    }
}

/// `gc.getPlayer(p).getUnit(iID)`: the unit if `p` has it; `None` is the NULL `CyUnit`.
fn player_unit(g: &Game, p: PlayerId, id: Option<UnitId>) -> Option<UnitId> {
    let id = id?;
    match g.units.get(id) {
        Some(unit) if unit.owner == p => Some(id),
        _ => None,
    }
}

/// `city.getX()`, `city.getY()` (-1 for the NULL city).
fn city_xy(g: &Game, city: Option<CityId>) -> (i32, i32) {
    city.map_or((-1, -1), |c| (g.cities[c].x, g.cities[c].y))
}

/// `plot.getX()`, `plot.getY()` (-1 for the NULL plot).
fn plot_coords(g: &Game, plot: Option<PlotIdx>) -> (i32, i32) {
    plot.map_or((-1, -1), |pl| g.map.xy(pl))
}

/// `city.getOwner()` (-1 for the NULL city).
fn city_owner(g: &Game, city: Option<CityId>) -> Option<PlayerId> {
    city.map(|c| g.cities[c].owner)
}

/// `city.isHasReligion(eReligion)`: false for the NULL city; for NO_RELIGION the C++ would
/// index its array at -1 (taken as false).
fn city_has_religion(g: &Game, city: Option<CityId>, religion: Option<ReligionType>) -> bool {
    match (city, religion) {
        (Some(c), Some(r)) => g.cities[c].has_religion[r],
        _ => false,
    }
}

/// `city.isHasBuilding(eBuilding)` (`getNumBuilding(e) > 0`): false for the NULL city; for a
/// building FFH2 does not define (-1) the C++ would index its arrays at -1 (taken as false).
fn has_building(g: &Game, city: Option<CityId>, building: Option<BuildingType>) -> bool {
    match (city, building) {
        (Some(c), Some(b)) => g.city_num_building(c, b) > 0,
        _ => false,
    }
}

/// `gc.getGame().getHolyCity(eReligion)`; `None` is the NULL `CyCity` (for NO_RELIGION the C++
/// would index its array at -1: taken as no holy city).
fn holy_city(g: &Game, religion: Option<ReligionType>) -> Option<CityId> {
    religion.and_then(|r| g.get_holy_city(r))
}

/// `gc.getCivilizationInfo(player.getCivilizationType()).getCivilizationUnits(eClass)`; `None`
/// is -1 (also for a unit class FFH2 does not define, which the C++ would index at -1).
fn civ_unit(g: &Game, p: PlayerId, class: Option<UnitClassType>) -> Option<UnitType> {
    let civ = g.player_get_civilization_type(p)?;
    g.d.civ_unit(civ, class?)
}

/// `player.getBuildingClassCount(eClass)` (0 for a class FFH2 does not define).
fn building_class_count(g: &Game, p: PlayerId, class: Option<BuildingClassType>) -> i32 {
    class.map_or(0, |bc| g.players[p].building_class_count[bc])
}

/// `player.getUnitClassCount(eClass)` (0 for a class FFH2 does not define).
fn unit_class_count(g: &Game, p: PlayerId, class: Option<UnitClassType>) -> i32 {
    class.map_or(0, |uc| g.players[p].unit_class_count[uc])
}

/// `player.getNumAvailableBonuses(eBonus)` (0 for a bonus FFH2 does not define).
fn num_available_bonuses(g: &Game, p: PlayerId, bonus: Option<BonusType>) -> i32 {
    bonus.map_or(0, |b| g.player_get_num_available_bonuses(p, b))
}

/// `player.isCivic(eCivic)`. C++ compares the civic of every civic option with `eCivic`, so for
/// -1 (a civic FFH2 does not define) it is true when some civic option has no civic.
fn is_civic(g: &Game, p: PlayerId, civic: Option<CivicType>) -> bool {
    match civic {
        Some(c) => g.player_is_civic(p, c),
        None => g.players[p].civics.iter().any(|c| c.is_none()),
    }
}

/// `gc.getWorldInfo(gc.getMap().getWorldSize()).getDefaultPlayers()`.
fn default_players(g: &Game) -> i32 {
    g.d.worlds[g.setup.world_size].default_players
}

/// `map.getWorldSize() == CvUtil.findInfoTypeNum(gc.getWorldInfo, .., name)`.
fn world_size_is(g: &Game, name: &str) -> bool {
    Some(g.setup.world_size) == g.d.ty::<WorldSizeType>(name)
}

/// `gc.getGame().isOption(GameOptionTypes.GAMEOPTION_ONE_CITY_CHALLENGE) and
/// gc.getPlayer(p).isHuman()`.
fn one_city_challenge_human(g: &Game, p: PlayerId) -> bool {
    g.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && g.players[p].human
}

/// `trigger.getPrereqEvent(0)` (`None` for a trigger without prerequisite events, where the C++
/// would read past the end of its vector).
fn prereq_event0(g: &Game, trigger: EventTriggerType) -> Option<EventType> {
    g.d.event_triggers[trigger].prereq_events.first().copied()
}

/// `kOrigTriggeredData = player.getEventOccured(trigger.getPrereqEvent(0))` (a copy; `None` is
/// Python's `None`).
fn orig_triggered_data(g: &Game, p: PlayerId, trigger: EventTriggerType) -> Option<EventTriggeredData> {
    let event = prereq_event0(g, trigger)?;
    g.player_get_event_occured(p, event).cloned()
}

/// Python `list.sort()` of `(distance, CyCity | CyPlot)` tuples: ascending distance; ties keep
/// list order (Python compares the wrapper objects by memory address).
fn sort_by_distance<T>(list: &mut [(i32, T)]) {
    list.sort_by_key(|entry| entry.0);
}

/// The "every civ that has met us likes us more" loop of `applySoloFlight`,
/// `applyModernOlympics`, `doAncientTexts2` and `applyBestDefenseDone2`:
/// `loopPlayer.AI_changeAttitudeExtra(ePlayer, change)` for every other living civ player whose
/// team has met `p`'s team.
fn change_attitude_of_contacts(g: &mut Game, p: PlayerId, change: i32) {
    let team = g.players[p].team;
    for other in g.civ_player_ids() {
        if g.players[other].alive && other != p {
            let other_team = g.players[other].team;
            if g.teams[other_team].has_met[team] {
                g.players[other].ai_attitude_extra[p] += change;
            }
        }
    }
}

/// `plot = gc.getMap().plot(iPlotX, iPlotY); if not plot.isNone(): plot.setImprovementType(e)`.
fn set_trigger_plot_improvement(g: &mut Game, data: &EventTriggeredData, improvement: Option<ImprovementType>) {
    if let Some(plot) = g.map.plot_xy(data.plot_x, data.plot_y) {
        g.plot_set_improvement_type(plot, improvement);
    }
}

/// The body shared by `canTriggerAntelope`, `canTriggerWhaleOfAThing` and
/// `canTriggerHiyoSilver`: the player has at most five happiness bonuses and not `bonus`, and the
/// trigger plot can have `bonus`.
fn can_trigger_new_happy_bonus(g: &Game, data: &EventTriggeredData, bonus: Option<BonusType>) -> bool {
    let p = data.player;
    let mut happy_bonuses = 0;
    for b in g.d.bonuses.keys() {
        let num = g.player_get_num_available_bonuses(p, b);
        if num > 0 {
            if g.d.bonuses[b].happiness > 0 {
                happy_bonuses += 1;
                if happy_bonuses > 5 {
                    return false;
                }
            }
            if Some(b) == bonus {
                return false;
            }
        }
    }

    // NULL plot: canHaveBonus() is false.
    match g.map.plot_xy(data.plot_x, data.plot_y) {
        Some(plot) => g.plot_can_have_bonus(plot, bonus, false),
        None => false,
    }
}

/// The "eligible plot" test of the barbarian invasion events (The Huns, The Vandals, ...):
/// unowned passable land in an area where `p` has a city, next to `p`'s land.
fn is_barbarian_invasion_plot(g: &Game, plot: PlotIdx, p: PlayerId) -> bool {
    let pl = &g.map.plots[plot];
    pl.owner.is_none()
        && !g.plot_is_water(plot)
        && !g.plot_is_impassable(plot)
        // (a real plot always has an area; a NULL CyArea would answer -1)
        && pl.area.map_or(false, |a| g.map.areas[a].cities_per_player[p] > 0)
        && g.plot_is_adjacent_player(plot, p, true)
}

/// The body shared by `canTriggerTheHuns`, `canTriggerTheVandals`, `canTriggerTheGoths`,
/// `canTriggerThePhilistines` and `canTriggerTheVedicAryans`.
fn can_trigger_barbarian_invasion(g: &Game, data: &EventTriggeredData, techs: [&str; 2], counter_unit_class: &str) -> bool {
    let p = data.player;

    // If Barbarians are disabled in this game, this event will not occur.
    if g.is_option(GameOptionType::GAMEOPTION_NO_BARBARIANS) {
        return false;
    }

    // At least one civ on the board must know each of the two techs (`isHasTech(-1)` is true).
    for tech_name in techs {
        let tech = g.d.ty::<TechType>(tech_name);
        let mut found_valid = false;
        for other in g.civ_player_ids() {
            if g.players[other].alive && g.team_is_has_tech(g.players[other].team, tech) {
                found_valid = true;
                break;
            }
        }
        if !found_valid {
            return false;
        }
    }

    // Can we build the counter unit?
    let Some(counter_unit) = civ_unit(g, p, g.d.ty::<UnitClassType>(counter_unit_class)) else {
        return false;
    };
    let mut found = false;
    for &c in &g.players[p].cities {
        if g.city_can_train(c, counter_unit, false, false, false, false) {
            found = true;
            break;
        }
    }
    if !found {
        return false;
    }

    // Find an eligible plot
    for plot in g.map.plot_ids() {
        if is_barbarian_invasion_plot(g, plot, p) {
            return true;
        }
    }
    false
}

/// The body shared by `applyTheHuns1`, `applyTheVandals1`, `applyTheGoths1`,
/// `applyThePhilistines1` and `applyTheVedicAryans1`: 1-6 barbarian `unit_name`s (by world size)
/// on a random eligible plot.
fn apply_barbarian_invasion(g: &mut Game, data: &EventTriggeredData, rand_label: &str, unit_name: &str) {
    let p = data.player;

    let mut list_plots = Vec::new();
    for plot in g.map.plot_ids() {
        if is_barbarian_invasion_plot(g, plot, p) {
            list_plots.push(plot);
        }
    }

    if list_plots.is_empty() {
        return;
    }

    let plot = list_plots[g.soren_rand_num(list_plots.len() as i32, rand_label) as usize];

    let num_units = if world_size_is(g, "WORLDSIZE_DUEL") {
        1
    } else if world_size_is(g, "WORLDSIZE_TINY") {
        2
    } else if world_size_is(g, "WORLDSIZE_SMALL") {
        3
    } else if world_size_is(g, "WORLDSIZE_STANDARD") {
        4
    } else if world_size_is(g, "WORLDSIZE_LARGE") {
        5
    } else {
        6
    };

    // UNIT_SPEARMAN (The Philistines) is not an FFH2 unit: C++ initUnit(NO_UNIT) would crash.
    let Some(unit_type) = g.d.ty::<UnitType>(unit_name) else { return };

    let barb_player = g.barbarian_player();
    let (x, y) = g.map.xy(plot);
    for _ in 0..num_units {
        init_unit(g, barb_player, unit_type, x, y, UnitAIType::UNITAI_ATTACK_CITY_LEMMING);
    }
}

/// The plot-counting loop of `canTriggerHarbormaster`, `canTriggerWarships` and
/// `canTriggerOverwhelm`: true once the water plots seen (in index order) reach `percent`% of
/// the map.
fn water_reaches_percent(g: &Game, percent: i32) -> bool {
    let num_plots = g.map.num_plots() as i32;
    let mut num_water = 0;
    for plot in g.map.plot_ids() {
        if g.plot_is_water(plot) {
            num_water += 1;
        }
        if 100 * num_water >= percent * num_plots {
            return true;
        }
    }
    false
}

/// `if player.getCivilizationType() == gc.getInfoTypeForString(name): return true; return false`
/// (`canTriggerToxcatl` & co; none of these civilizations exists in FFH2).
fn is_civilization(g: &Game, p: PlayerId, name: &str) -> bool {
    g.player_get_civilization_type(p) == g.d.ty::<CivilizationType>(name)
}

// ---------------------------------------------------------------------------------------------
// DUSTBOWL
// ---------------------------------------------------------------------------------------------

/// `canTriggerDustbowlCont(argsList)`: moves the trigger to the player's plains farm nearest to
/// the first dust bowl; without one, forgets the first event and fails.
fn can_trigger_dustbowl_cont(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let prereq_event = prereq_event0(g, data.trigger);

    let Some(orig) = orig_triggered_data(g, p, data.trigger) else {
        return false;
    };

    let farm_type = g.d.ty::<ImprovementType>("IMPROVEMENT_FARM");
    let plains_type = g.d.ty::<TerrainType>("TERRAIN_PLAINS");

    let mut best_value = g.map.grid_width + g.map.grid_height;
    let mut best_plot = None;
    for plot in g.map.plot_ids() {
        let pl = &g.map.plots[plot];
        if pl.owner == Some(p) && pl.improvement_type == farm_type && pl.terrain_type == plains_type {
            let value = g.plot_distance(orig.plot_x, orig.plot_y, pl.x, pl.y);
            if value < best_value {
                best_value = value;
                best_plot = Some(plot);
            }
        }
    }

    match best_plot {
        Some(plot) => {
            let (x, y) = g.map.xy(plot);
            data.plot_x = x;
            data.plot_y = y;
        }
        None => {
            // (orig is Some, so the prerequisite event exists)
            if let Some(event) = prereq_event {
                g.player_reset_event_occured(p, event, true);
            }
            return false;
        }
    }

    true
}

// ---------------------------------------------------------------------------------------------
// SALTPETER
// ---------------------------------------------------------------------------------------------

/// `getSaltpeterNumExtraPlots()`: 1-4 plots by world size index.
fn get_saltpeter_num_extra_plots(g: &Game) -> i32 {
    let world_size = g.setup.world_size.idx() as i32;
    if world_size <= 1 {
        1
    } else if world_size <= 3 {
        2
    } else if world_size <= 4 {
        3
    } else {
        4
    }
}

/// The plots `canApplySaltpeter` counts and `applySaltpeter` improves: the player's forested
/// hills other than the trigger plot, with their distance to it.
fn saltpeter_plots(g: &Game, data: &EventTriggeredData) -> Vec<(i32, PlotIdx)> {
    let p = data.player;
    let forest = g.d.ty::<FeatureType>("FEATURE_FOREST");
    let mut list_plots = Vec::new();
    for plot in g.map.plot_ids() {
        let pl = &g.map.plots[plot];
        if pl.owner == Some(p) && pl.feature_type == forest && g.plot_is_hills(plot) {
            let distance = g.plot_distance(data.plot_x, data.plot_y, pl.x, pl.y);
            if distance > 0 {
                list_plots.push((distance, plot));
            }
        }
    }
    list_plots
}

/// `canApplySaltpeter(argsList)`. (Its `if (plot == None)` never holds: `CyMap().plot()`
/// returns a `CyPlot`.)
fn can_apply_saltpeter(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let num_plots = saltpeter_plots(g, data).len() as i32;
    num_plots >= get_saltpeter_num_extra_plots(g)
}

/// `applySaltpeter(argsList)`: +1 commerce on the nearest forested hills.
fn apply_saltpeter(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let mut list_plots = saltpeter_plots(g, data);
    sort_by_distance(&mut list_plots);

    let mut count = get_saltpeter_num_extra_plots(g);
    for &(_, plot) in &list_plots {
        if count == 0 {
            break;
        }
        count -= 1;
        let (x, y) = g.map.xy(plot);
        g.set_plot_extra_yield(x, y, YieldType::YIELD_COMMERCE, 1);
    }
}

// ---------------------------------------------------------------------------------------------
// GREAT DEPRESSION
// ---------------------------------------------------------------------------------------------

/// `applyGreatDepression(argsList)`: every living civ loses a quarter of its gold (Python
/// `-gold/4` floors, e.g. -10/4 = -3). The popup to the other players is UI.
fn apply_great_depression(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    for other in g.civ_player_ids() {
        if g.players[other].alive {
            let gold = g.players[other].gold;
            g.players[other].gold += py_div(-gold, 4);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// CHAMPION
// ---------------------------------------------------------------------------------------------

/// `canTriggerChampion(argsList)`: only at peace.
fn can_trigger_champion(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let team = g.players[data.player].team;
    if g.team_get_at_war_count(team, true) > 0 {
        return false;
    }
    true
}

/// `canTriggerChampionUnit(argsList)`: an undamaged unit with 3+ experience that is not a hero.
fn can_trigger_champion_unit(g: &Game, _trigger: EventTriggerType, p: PlayerId, u: UnitId) -> bool {
    let Some(u) = player_unit(g, p, Some(u)) else {
        return false;
    };
    let unit = &g.units[u];

    if unit.damage > 0 {
        return false;
    }

    if unit.experience < 3 {
        return false;
    }

    // FfH: PROMOTION_HERO replaces BTS PROMOTION_LEADERSHIP.
    let leadership = g.d.t::<PromotionType>("PROMOTION_HERO");
    if unit.has_promotion[leadership] {
        return false;
    }

    true
}

/// `applyChampion(argsList)`: the unit becomes a hero.
fn apply_champion(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let unit = player_unit(g, data.player, data.unit);
    let leadership = g.d.t::<PromotionType>("PROMOTION_HERO");
    // NULL unit: setHasPromotion is a no-op.
    if let Some(u) = unit {
        g.unit_set_has_promotion(u, leadership, true);
    }
}

// ---------------------------------------------------------------------------------------------
// ELECTRIC COMPANY / GOLD RUSH / INFLUENZA
// ---------------------------------------------------------------------------------------------

/// `canTriggerElectricCompany(argsList)`: no angry citizens anywhere.
fn can_trigger_electric_company(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    for &c in &g.players[p].cities {
        if g.city_angry_population(c, 0) > 0 {
            return false;
        }
    }
    true
}

/// `canTriggerGoldRush(argsList)`. ERA_INDUSTRIAL is not an FFH2 era (-1), so it never triggers.
fn can_trigger_gold_rush(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let industrial = g.d.ty::<EraType>("ERA_INDUSTRIAL");
    if Some(g.players[data.player].current_era) != industrial {
        return false;
    }
    true
}

/// `canTriggerInfluenza(argsList)`. (`getCurrentEra() <= -1` never holds in FFH2.)
fn can_trigger_influenza(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let team = g.players[p].team;

    let industrial = g.d.ty::<EraType>("ERA_INDUSTRIAL").map_or(-1, |e| e.idx() as i32);
    if g.players[p].current_era.idx() as i32 <= industrial {
        return false;
    }

    let medicine = g.d.ty::<TechType>("TECH_MEDICINE");
    if g.team_is_has_tech(team, medicine) {
        return false;
    }

    true
}

/// `applyInfluenza2(argsList)`: 2-4 of the player's other cities above size 2, nearest to the
/// event city first, lose 2 population.
fn apply_influenza2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let event_city = player_city(g, p, data.city);

    let mut num_cities = 2 + g.soren_rand_num(3, "Influenza event number of cities");

    let (ex, ey) = city_xy(g, event_city);
    let mut list_cities = Vec::new();
    for &c in &g.players[p].cities {
        let city = &g.cities[c];
        if city.population > 2 {
            let distance = g.plot_distance(ex, ey, city.x, city.y);
            if distance > 0 {
                list_cities.push((distance, c));
            }
        }
    }

    sort_by_distance(&mut list_cities);

    if num_cities > list_cities.len() as i32 {
        num_cities = list_cities.len() as i32;
    }

    for &(_, c) in list_cities.iter().take(num_cities as usize) {
        g.city_change_population(c, -2);
    }
}

// ---------------------------------------------------------------------------------------------
// SOLO FLIGHT
// ---------------------------------------------------------------------------------------------

/// `canTriggerSoloFlight(argsList)`: enough land areas for the world size; single player only.
fn can_trigger_solo_flight(g: &mut Game, _data: &mut EventTriggeredData) -> bool {
    let min_landmass = if world_size_is(g, "WORLDSIZE_DUEL") {
        3
    } else if world_size_is(g, "WORLDSIZE_TINY") {
        4
    } else if world_size_is(g, "WORLDSIZE_SMALL") {
        6
    } else if world_size_is(g, "WORLDSIZE_STANDARD") {
        8
    } else if world_size_is(g, "WORLDSIZE_LARGE") {
        10
    } else {
        12
    };

    if g.map.get_num_land_areas() < min_landmass {
        return false;
    }

    if g.is_game_multi_player() {
        return false;
    }

    true
}

/// `applySoloFlight(argsList)`: +1 attitude from every civ that has met us.
fn apply_solo_flight(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    change_attitude_of_contacts(g, data.player, 1);
}

// ---------------------------------------------------------------------------------------------
// ANTELOPE / WHALE OF A THING / HIYO SILVER / WINING MONKS / INDEPENDENT FILMS
// ---------------------------------------------------------------------------------------------

/// `canTriggerAntelope(argsList)`.
fn can_trigger_antelope(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let deer = g.d.ty::<BonusType>("BONUS_DEER");
    can_trigger_new_happy_bonus(g, data, deer)
}

/// `doAntelope2(argsList)`: a camp on the plot ("camps are not normally allowed unless there is
/// already deer").
fn do_antelope2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let camp = g.d.ty::<ImprovementType>("IMPROVEMENT_CAMP");
    set_trigger_plot_improvement(g, data, camp);
}

/// `canTriggerWhaleOfAThing(argsList)`.
fn can_trigger_whale_of_a_thing(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let whale = g.d.ty::<BonusType>("BONUS_WHALE");
    can_trigger_new_happy_bonus(g, data, whale)
}

/// `canTriggerHiyoSilver(argsList)`. BONUS_SILVER is not an FFH2 bonus (-1).
fn can_trigger_hiyo_silver(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let silver = g.d.ty::<BonusType>("BONUS_SILVER");
    can_trigger_new_happy_bonus(g, data, silver)
}

/// `canTriggerWiningMonks(argsList)`: no wine yet.
fn can_trigger_wining_monks(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let wine = g.d.ty::<BonusType>("BONUS_WINE");
    if num_available_bonuses(g, data.player, wine) > 0 {
        return false;
    }
    true
}

/// `doWiningMonks2(argsList)`: a winery on the plot.
fn do_wining_monks2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let winery = g.d.ty::<ImprovementType>("IMPROVEMENT_WINERY");
    set_trigger_plot_improvement(g, data, winery);
}

/// `canTriggerIndependentFilms(argsList)`: no building giving BONUS_MOVIES. That bonus is not in
/// FFH2 (-1), so this tests the buildings without a free bonus: any of those built fails.
fn can_trigger_independent_films(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let movies = g.d.ty::<BonusType>("BONUS_MOVIES");
    for b in g.d.buildings.keys() {
        if g.d.buildings[b].free_bonus == movies && g.player_count_num_buildings(data.player, b) > 0 {
            return false;
        }
    }
    true
}

/// `doIndependentFilms(argsList)`: a free BONUS_MOVIES in the city (not an FFH2 bonus: the C++
/// `changeFreeBonus(-1)` would index at -1, skipped).
fn do_independent_films(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let city = player_city(g, data.player, data.city);
    let bonus = g.d.ty::<BonusType>("BONUS_MOVIES");
    if let (Some(c), Some(b)) = (city, bonus) {
        g.city_change_free_bonus(c, b, 1);
    }
}

// ---------------------------------------------------------------------------------------------
// ANCIENT OLYMPICS / MODERN OLYMPICS / INTERSTATE
// ---------------------------------------------------------------------------------------------

/// `canTriggerAncientOlympics(argsList)`. None of the three religions is in FFH2 (-1), so this
/// fails exactly for players without a state religion.
fn can_trigger_ancient_olympics(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let state_religion = g.player_get_state_religion(data.player);

    if state_religion == g.d.ty::<ReligionType>("RELIGION_JUDAISM") {
        return false;
    }

    if state_religion == g.d.ty::<ReligionType>("RELIGION_CHRISTIANITY") {
        return false;
    }

    if state_religion == g.d.ty::<ReligionType>("RELIGION_ISLAM") {
        return false;
    }

    true
}

/// `doAncientOlympics2(argsList)`: every other living major civ with land next to ours gets a
/// "good event" memory of us.
fn do_ancient_olympics2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    for j in g.civ_player_ids() {
        if j != p && g.players[j].alive && !g.player_is_minor_civ(j) {
            for plot in g.map.plot_ids() {
                if !g.plot_is_water(plot) && g.map.plots[plot].owner == Some(p) && g.plot_is_adjacent_player(plot, j, true) {
                    g.players[j].ai_memory_count[p][MemoryType::MEMORY_EVENT_GOOD_TO_US] += 1;
                    break;
                }
            }
        }
    }
}

/// `canTriggerModernOlympics(argsList)`: continues at the first event's city and plot.
fn can_trigger_modern_olympics(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let Some(orig) = orig_triggered_data(g, data.player, data.trigger) else {
        return false;
    };

    data.city = orig.city;
    data.plot_x = orig.plot_x;
    data.plot_y = orig.plot_y;

    true
}

/// `applyModernOlympics(argsList)`: +1 attitude from every civ that has met us.
fn apply_modern_olympics(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    change_attitude_of_contacts(g, data.player, 1);
}

/// `canTriggerInterstate(argsList)`. CIVIC_EMANCIPATION is not in FFH2 (`isCivic(-1)`).
fn can_trigger_interstate(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if !is_civic(g, data.player, g.d.ty::<CivicType>("CIVIC_EMANCIPATION")) {
        return false;
    }
    true
}

/// `applyInterstate(argsList)`: roads cost the team less movement (`changeRouteChange(road, -5)`).
fn apply_interstate(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let team = g.players[data.player].team;
    if let Some(road) = g.d.ty::<RouteType>("ROUTE_ROAD") {
        g.teams[team].route_change[road] += -5;
    }
}

// ---------------------------------------------------------------------------------------------
// EARTH DAY / FREEDOM CONCERT
// ---------------------------------------------------------------------------------------------

/// The test `canApplyEarthDay2` / `applyEarthDay2` apply to each civ player `other`: living, not
/// us, not human, met us, and willing to adopt `civic` on our request (`canTradeItem` +
/// `NO_DENIAL`).
fn is_earth_day_candidate(g: &Game, p: PlayerId, other: PlayerId, civic: Option<CivicType>) -> bool {
    let op = &g.players[other];
    if op.alive && other != p && !op.human && g.teams[op.team].has_met[g.players[p].team] {
        let trade_data = TradeData {
            item_type: TradeableItem::TRADE_CIVIC,
            data: civic.map_or(-1, |c| c.idx() as i32),
            offering: false,
            hidden: false,
        };
        if g.player_can_trade_item(other, p, &trade_data, false) && g.player_get_trade_denial(other, p, &trade_data).is_none() {
            return true;
        }
    }
    false
}

/// `canApplyEarthDay2(argsList)`. (CIVIC_ENVIRONMENTALISM is not in FFH2: the trade item's data
/// is -1.)
fn can_apply_earth_day2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let civic = g.d.ty::<CivicType>("CIVIC_ENVIRONMENTALISM");
    for other in g.civ_player_ids() {
        if is_earth_day_candidate(g, data.player, other, civic) {
            return true;
        }
    }
    false
}

/// `applyEarthDay2(argsList)`: the candidate AIs adopt the civic.
fn apply_earth_day2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civic = g.d.ty::<CivicType>("CIVIC_ENVIRONMENTALISM");
    let Some(civic_option) = g.d.ty::<CivicOptionType>("CIVICOPTION_ECONOMY") else { return };

    // AI: AI_civicValue(iCivic) -- the Python sorts by `-AI_civicValue`; omitted (0), so the
    // candidates stay in player order.
    let mut list_players: Vec<(i32, PlayerId)> = Vec::new();
    for other in g.civ_player_ids() {
        if is_earth_day_candidate(g, data.player, other, civic) {
            list_players.push((0, other));
        }
    }

    list_players.sort();

    // C++ quirk (Python): more than 3 candidates keeps only the first 2.
    if list_players.len() > 3 {
        list_players.truncate(2);
    }

    for (_, other) in list_players {
        g.player_set_civics(other, civic_option, civic);
    }
}

/// `canApplyFreedomConcert2(argsList)`: some religion of the event city is missing from one of
/// our cities that has another religion.
fn can_apply_freedom_concert2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let p = data.player;
    let event_city = player_city(g, p, data.city);
    let d = g.d;

    for religion in d.religions.keys() {
        if city_has_religion(g, event_city, Some(religion)) {
            for &c in &g.players[p].cities {
                if !g.cities[c].has_religion[religion] {
                    for j_religion in d.religions.keys() {
                        if g.cities[c].has_religion[j_religion] {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// `applyFreedomConcert2(argsList)`: each religion of the event city spreads to the nearest of
/// our cities that lacks it but has some religion.
fn apply_freedom_concert2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let event_city = player_city(g, p, data.city);
    let d = g.d;

    for religion in d.religions.keys() {
        if city_has_religion(g, event_city, Some(religion)) {
            let mut best_city = None;
            let mut best_distance = 0;
            let (ex, ey) = city_xy(g, event_city);
            for &c in &g.players[p].cities {
                let city = &g.cities[c];
                if !city.has_religion[religion] {
                    let valid = d.religions.keys().any(|j_religion| city.has_religion[j_religion]);

                    if valid {
                        let distance = g.plot_distance(ex, ey, city.x, city.y);

                        if distance < best_distance || best_city.is_none() {
                            best_city = Some(c);
                            best_distance = distance;
                        }
                    }
                }
            }

            if let Some(c) = best_city {
                g.city_set_has_religion(c, religion, true, true, true);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// HEROIC GESTURE / GREAT MEDIATOR / ANCIENT TEXTS
// ---------------------------------------------------------------------------------------------

/// `canTriggerHeroicGesture(argsList)`: at war with the other player, both sides having had
/// success. (No other player: `gc.getPlayer(-1)` is None and the Python raises.)
fn can_trigger_heroic_gesture(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let Some(dest_player) = data.other_player else {
        return false;
    };
    let team = g.players[data.player].team;
    let dest_team = g.players[dest_player].team;

    if !g.team_can_change_war_peace(dest_team, team, false) {
        return false;
    }

    if g.teams[dest_team].ai_war_success[team] <= 0 {
        return false;
    }

    if g.teams[team].ai_war_success[dest_team] <= 0 {
        return false;
    }

    true
}

/// `doHeroicGesture2(argsList)`: peace with the other player and +1 attitude both ways. A human
/// other player is asked by popup in the Python (`heroicGesture2Callback`, whose "yes" does the
/// same); here everyone takes the AI branch.
fn do_heroic_gesture2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let Some(dest_player) = data.other_player else { return };

    g.player_force_peace(dest_player, p);
    g.players[dest_player].ai_attitude_extra[p] += 1;
    g.players[p].ai_attitude_extra[dest_player] += 1;
}

/// `canTriggerGreatMediator(argsList)`: at war with the other player for 10+ turns.
fn can_trigger_great_mediator(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let Some(dest_player) = data.other_player else {
        return false;
    };
    let team = g.players[data.player].team;
    let dest_team = g.players[dest_player].team;

    if !g.team_can_change_war_peace(team, dest_team, false) {
        return false;
    }

    if g.teams[team].ai_at_war_counter[dest_team] < 10 {
        return false;
    }

    true
}

/// `doGreatMediator2(argsList)`: peace with the other player and +1 attitude both ways (AI
/// branch for everyone; the Python asks a human other player by popup, `greatMediator2Callback`).
fn do_great_mediator2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let Some(dest_player) = data.other_player else { return };

    let team = g.players[p].team;
    let dest_team = g.players[dest_player].team;
    g.team_make_peace(team, dest_team, true);
    g.players[dest_player].ai_attitude_extra[p] += 1;
    g.players[p].ai_attitude_extra[dest_player] += 1;
}

/// `doAncientTexts2(argsList)`: +1 attitude from every civ that has met us.
fn do_ancient_texts2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    change_attitude_of_contacts(g, data.player, 1);
}

// ---------------------------------------------------------------------------------------------
// IMPACT CRATER
// ---------------------------------------------------------------------------------------------

/// `canTriggerImpactCrater(argsList)`. BONUS_URANIUM is not in FFH2; `canHaveBonus(NO_BONUS)` is
/// true for a real plot.
fn can_trigger_impact_crater(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let uranium = g.d.ty::<BonusType>("BONUS_URANIUM");
    if num_available_bonuses(g, data.player, uranium) > 0 {
        return false;
    }

    match g.map.plot_xy(data.plot_x, data.plot_y) {
        Some(plot) if g.plot_can_have_bonus(plot, uranium, false) => true,
        _ => false,
    }
}

/// `doImpactCrater2(argsList)`: a mine on the plot.
fn do_impact_crater2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let mine = g.d.ty::<ImprovementType>("IMPROVEMENT_MINE");
    set_trigger_plot_improvement(g, data, mine);
}

// ---------------------------------------------------------------------------------------------
// BARBARIAN INVASIONS (THE HUNS, THE VANDALS, THE GOTHS, THE PHILISTINES, THE VEDIC ARYANS)
// ---------------------------------------------------------------------------------------------

/// `canTriggerTheHuns(argsList)`.
fn can_trigger_the_huns(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_barbarian_invasion(g, data, ["TECH_HORSEBACK_RIDING", "TECH_IRON_WORKING"], "UNITCLASS_SPEARMAN")
}

/// `applyTheHuns1(argsList)`.
fn apply_the_huns1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    apply_barbarian_invasion(g, data, "Hun event location", "UNIT_HORSE_ARCHER");
}

/// `canTriggerTheVandals(argsList)`.
fn can_trigger_the_vandals(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_barbarian_invasion(g, data, ["TECH_METAL_CASTING", "TECH_IRON_WORKING"], "UNITCLASS_AXEMAN")
}

/// `applyTheVandals1(argsList)`.
fn apply_the_vandals1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    apply_barbarian_invasion(g, data, "Vandal event location", "UNIT_SWORDSMAN");
}

/// `canTriggerTheGoths(argsList)`.
fn can_trigger_the_goths(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_barbarian_invasion(g, data, ["TECH_MATHEMATICS", "TECH_IRON_WORKING"], "UNITCLASS_CHARIOT")
}

/// `applyTheGoths1(argsList)`.
fn apply_the_goths1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    apply_barbarian_invasion(g, data, "Goth event location", "UNIT_AXEMAN");
}

/// `canTriggerThePhilistines(argsList)`.
fn can_trigger_the_philistines(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_barbarian_invasion(g, data, ["TECH_MONOTHEISM", "TECH_BRONZE_WORKING"], "UNITCLASS_AXEMAN")
}

/// `applyThePhilistines1(argsList)`.
fn apply_the_philistines1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    apply_barbarian_invasion(g, data, "Philistine event location", "UNIT_SPEARMAN");
}

/// `canTriggerTheVedicAryans(argsList)`.
fn can_trigger_the_vedic_aryans(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_barbarian_invasion(g, data, ["TECH_POLYTHEISM", "TECH_ARCHERY"], "UNITCLASS_ARCHER")
}

/// `applyTheVedicAryans1(argsList)`.
fn apply_the_vedic_aryans1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    apply_barbarian_invasion(g, data, "Vedic Aryan event location", "UNIT_ARCHER");
}

// ---------------------------------------------------------------------------------------------
// SECURITY TAX / LITERACY / TEA
// ---------------------------------------------------------------------------------------------

/// `canTriggerSecurityTax(argsList)`: walls in every city.
fn can_trigger_security_tax(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let walls = g.d.ty::<BuildingClassType>("BUILDINGCLASS_WALLS");
    if g.players[p].num_cities() > building_class_count(g, p, walls) {
        return false;
    }
    true
}

/// `canTriggerLiteracy(argsList)`: a library in every city.
fn can_trigger_literacy(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let library = g.d.ty::<BuildingClassType>("BUILDINGCLASS_LIBRARY");
    if g.players[p].num_cities() > building_class_count(g, p, library) {
        return false;
    }
    true
}

/// `canTriggerTea(argsList)`: not in Mercantilism and able to trade with someone.
fn can_trigger_tea(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    if is_civic(g, p, g.d.ty::<CivicType>("CIVIC_MERCANTILISM")) {
        return false;
    }

    let mut can_trade = false;
    for other in g.civ_player_ids() {
        if g.player_can_have_trade_routes_with(p, other) {
            can_trade = true;
            break;
        }
    }

    if !can_trade {
        return false;
    }

    true
}

// ---------------------------------------------------------------------------------------------
// HORSE WHISPERING / HARBORMASTER / CLASSIC LITERATURE (quests)
// ---------------------------------------------------------------------------------------------

/// `canTriggerHorseWhispering(argsList)`: not for a human in One City Challenge.
fn can_trigger_horse_whispering(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if one_city_challenge_human(g, data.player) {
        return false;
    }
    true
}

/// `canTriggerHorseWhisperingDone(argsList)`: as many stables as the world size's default
/// player count.
fn can_trigger_horse_whispering_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let stable = g.d.ty::<BuildingClassType>("BUILDINGCLASS_STABLE");
    if default_players(g) > building_class_count(g, data.player, stable) {
        return false;
    }
    true
}

/// `applyHorseWhisperingDone1(argsList)`: free horse archers on the plot.
fn apply_horse_whispering_done1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let plot = g.map.plot_xy(data.plot_x, data.plot_y);
    let num_units = default_players(g);
    let unit_type = civ_unit(g, p, g.d.ty::<UnitClassType>("UNITCLASS_HORSE_ARCHER"));

    if let Some(unit_type) = unit_type {
        let (x, y) = plot_coords(g, plot);
        for _ in 0..num_units {
            init_unit(g, p, unit_type, x, y, UnitAIType::UNITAI_ATTACK);
        }
    }
}

/// `canTriggerHarbormaster(argsList)`: not for a human in One City Challenge; 40%+ water.
fn can_trigger_harbormaster(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if one_city_challenge_human(g, data.player) {
        return false;
    }
    water_reaches_percent(g, 40)
}

/// `canTriggerHarbormasterDone(argsList)`: enough harbors and caravels.
fn can_trigger_harbormaster_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let harbor = g.d.ty::<BuildingClassType>("BUILDINGCLASS_HARBOR");
    let harbors_required = default_players(g);
    if harbors_required > building_class_count(g, p, harbor) {
        return false;
    }

    let caravel = g.d.ty::<UnitClassType>("UNITCLASS_CARAVEL");
    let caravels_required = py_div(harbors_required, 2) + 1;
    if caravels_required > unit_class_count(g, p, caravel) {
        return false;
    }

    true
}

/// `canTriggerClassicLiterature(argsList)`: not for a human in One City Challenge.
fn can_trigger_classic_literature(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if one_city_challenge_human(g, data.player) {
        return false;
    }
    true
}

/// `canTriggerClassicLiteratureDone(argsList)`: enough libraries.
fn can_trigger_classic_literature_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let library = g.d.ty::<BuildingClassType>("BUILDINGCLASS_LIBRARY");
    let buildings_required = default_players(g);
    if buildings_required > building_class_count(g, data.player, library) {
        return false;
    }
    true
}

/// `gc.getTechInfo(iTech).getEra() == iEraAncient and player.canResearch(iTech, false)`
/// (`canApplyClassicLiteratureDone2`, `applyClassicLiteratureDone2`).
fn is_classic_literature_tech(g: &Game, p: PlayerId, tech: TechType, era_ancient: Option<EraType>) -> bool {
    g.d.techs[tech].era == era_ancient && g.player_can_research(p, tech, false)
}

/// `canApplyClassicLiteratureDone2(argsList)`: an ancient tech we can research.
fn can_apply_classic_literature_done2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let era_ancient = g.d.ty::<EraType>("ERA_ANCIENT");
    for tech in g.d.techs.keys() {
        if is_classic_literature_tech(g, data.player, tech, era_ancient) {
            return true;
        }
    }
    false
}

/// `applyClassicLiteratureDone2(argsList)`: a random ancient tech.
fn apply_classic_literature_done2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let era_ancient = g.d.ty::<EraType>("ERA_ANCIENT");
    let list_techs: Vec<TechType> = g.d.techs.keys().filter(|&tech| is_classic_literature_tech(g, p, tech, era_ancient)).collect();

    if !list_techs.is_empty() {
        let tech = list_techs[g.soren_rand_num(list_techs.len() as i32, "Classic Literature Event Tech selection") as usize];
        let team = g.players[p].team;
        g.team_set_has_tech(team, tech, true, Some(p), true, true);
    }
}

/// `canApplyClassicLiteratureDone3(argsList)`: we have the Great Library.
fn can_apply_classic_literature_done3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let great_library = g.d.ty::<BuildingType>("BUILDING_GREAT_LIBRARY");
    g.players[data.player].cities.iter().any(|&c| has_building(g, Some(c), great_library))
}

/// `applyClassicLiteratureDone3(argsList)`: a free scientist in the Great Library's city.
fn apply_classic_literature_done3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let specialist = g.d.ty::<SpecialistType>("SPECIALIST_SCIENTIST");
    let great_library = g.d.ty::<BuildingType>("BUILDING_GREAT_LIBRARY");

    for c in g.players[p].cities.clone() {
        if has_building(g, Some(c), great_library) {
            if let Some(specialist) = specialist {
                g.city_change_free_specialist_count(c, specialist, 1);
            }
            return;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// MASTER BLACKSMITH
// ---------------------------------------------------------------------------------------------

/// `canTriggerMasterBlacksmith(argsList)`: not for a human in One City Challenge.
fn can_trigger_master_blacksmith(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if one_city_challenge_human(g, data.player) {
        return false;
    }
    true
}

/// `expireMasterBlacksmith1(argsList)`: the quest city is gone (or no longer ours).
fn expire_master_blacksmith1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    player_city(g, data.player, data.city).is_none()
}

/// `canTriggerMasterBlacksmithDone(argsList)`: enough forges and the quest city still ours.
fn can_trigger_master_blacksmith_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let forge = g.d.ty::<BuildingClassType>("BUILDINGCLASS_FORGE");
    let buildings_required = default_players(g);
    if buildings_required > building_class_count(g, p, forge) {
        return false;
    }

    // (No first event: `kOrigTriggeredData.iCityId` raises in the Python.)
    let Some(orig) = orig_triggered_data(g, p, data.trigger) else {
        return false;
    };

    if player_city(g, p, orig.city).is_none() {
        return false;
    }

    data.city = orig.city;

    true
}

/// The search of `canApplyMasterBlacksmithDone1`: our plot that can have copper nearest to the
/// event city (the first in index order on ties). `player.getCity()` returns a `CyCity`, so its
/// `city == None` test never holds; a NULL city's coordinates are -1.
fn master_blacksmith_plot(g: &Game, data: &EventTriggeredData) -> Option<PlotIdx> {
    let p = data.player;
    let bonus = g.d.ty::<BonusType>("BONUS_COPPER");
    let city = player_city(g, p, data.city);
    let (cx, cy) = city_xy(g, city);

    let mut best_value = g.map.grid_width + g.map.grid_height;
    let mut best_plot = None;
    for plot in g.map.plot_ids() {
        if g.map.plots[plot].owner == Some(p) && g.plot_can_have_bonus(plot, bonus, false) {
            let (x, y) = g.map.xy(plot);
            let value = g.plot_distance(cx, cy, x, y);
            if value < best_value {
                best_value = value;
                best_plot = Some(plot);
            }
        }
    }
    best_plot
}

/// `canApplyMasterBlacksmithDone1(argsList)`: some plot of ours can have copper. The Python also
/// stores that plot in the triggered data for `applyMasterBlacksmithDone1`; the event can-do
/// hook is read-only, so the apply callback searches again.
fn can_apply_master_blacksmith_done1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    master_blacksmith_plot(g, data).is_some()
}

/// `applyMasterBlacksmithDone1(argsList)`: copper on the plot chosen by
/// `canApplyMasterBlacksmithDone1` (which C++ `applyEvent` evaluates just before this callback;
/// searched again here, falling back to the trigger plot).
fn apply_master_blacksmith_done1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let plot = master_blacksmith_plot(g, data).or_else(|| g.map.plot_xy(data.plot_x, data.plot_y));
    let bonus = g.d.ty::<BonusType>("BONUS_COPPER");
    // NULL plot: setBonusType is a no-op.
    if let Some(plot) = plot {
        g.plot_set_bonus_type(plot, bonus);
    }
}

/// `canApplyMasterBlacksmithDone3(argsList)`: we have a state religion.
fn can_apply_master_blacksmith_done3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    g.player_get_state_religion(data.player).is_some()
}

// ---------------------------------------------------------------------------------------------
// THE BEST DEFENSE / NATIONAL SPORTS LEAGUE
// ---------------------------------------------------------------------------------------------

/// `canTriggerBestDefense(argsList)`: not for a human in One City Challenge.
fn can_trigger_best_defense(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if one_city_challenge_human(g, data.player) {
        return false;
    }
    true
}

/// `canTriggerBestDefenseDone(argsList)`: enough castles (not an FFH2 building class).
fn can_trigger_best_defense_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let castle = g.d.ty::<BuildingClassType>("BUILDINGCLASS_CASTLE");
    let buildings_required = default_players(g);
    if buildings_required > building_class_count(g, data.player, castle) {
        return false;
    }
    true
}

/// `applyBestDefenseDone2(argsList)`: +3 attitude from every civ that has met us.
fn apply_best_defense_done2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    change_attitude_of_contacts(g, data.player, 3);
}

/// `canApplyBestDefenseDone3(argsList)`: we have the Great Wall (not an FFH2 building).
fn can_apply_best_defense_done3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let great_wall = g.d.ty::<BuildingType>("BUILDING_GREAT_WALL");
    g.players[data.player].cities.iter().any(|&c| has_building(g, Some(c), great_wall))
}

/// `canTriggerSportsLeague(argsList)`: not for a human in One City Challenge.
fn can_trigger_sports_league(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if one_city_challenge_human(g, data.player) {
        return false;
    }
    true
}

/// `canTriggerSportsLeagueDone(argsList)`: enough colosseums (not an FFH2 building class).
fn can_trigger_sports_league_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let colosseum = g.d.ty::<BuildingClassType>("BUILDINGCLASS_COLOSSEUM");
    let buildings_required = default_players(g);
    if buildings_required > building_class_count(g, data.player, colosseum) {
        return false;
    }
    true
}

/// `canApplySportsLeagueDone3(argsList)`: we have the Statue of Zeus (not an FFH2 building).
fn can_apply_sports_league_done3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let zeus = g.d.ty::<BuildingType>("BUILDING_STATUE_OF_ZEUS");
    g.players[data.player].cities.iter().any(|&c| has_building(g, Some(c), zeus))
}

// ---------------------------------------------------------------------------------------------
// CRUSADE
// ---------------------------------------------------------------------------------------------

/// `canTriggerCrusade(argsList)`: the other player owns the holy city of the trigger's
/// religion; remembers that city.
fn can_trigger_crusade(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let holy = holy_city(g, data.religion);

    if one_city_challenge_human(g, data.player) {
        return false;
    }

    if city_owner(g, holy) != data.other_player {
        return false;
    }

    // holyCity.getID() (-1 for the NULL city)
    data.other_player_city = holy;

    true
}

/// `expireCrusade1(argsList)`: the crusade is over unless we still fight the holy city's owner
/// in our state religion's name (taking the city ourselves never expires it).
fn expire_crusade1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let p = data.player;
    let holy_owner = city_owner(g, holy_city(g, data.religion));

    if holy_owner == Some(p) {
        return false;
    }

    if g.player_get_state_religion(p) != data.religion {
        return true;
    }

    if holy_owner != data.other_player {
        return true;
    }

    // (No other player: `otherPlayer.getTeam()` raises in the Python.)
    let Some(other) = data.other_player else {
        return false;
    };
    if !g.teams[g.players[p].team].at_war[g.players[other].team] {
        return true;
    }

    false
}

/// `canTriggerCrusadeDone(argsList)`: we took the holy city; carries over the other player and
/// religion, and picks the religion's holy-city building (its shrine).
fn can_trigger_crusade_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    // (No first event: `kOrigTriggeredData.eReligion` raises in the Python.)
    let Some(orig) = orig_triggered_data(g, p, data.trigger) else {
        return false;
    };
    let holy = holy_city(g, orig.religion);

    if city_owner(g, holy) != Some(p) {
        return false;
    }

    data.city = holy;
    data.other_player = orig.other_player;
    data.religion = orig.religion;

    for b in g.d.buildings.keys() {
        if g.d.buildings[b].holy_city == orig.religion {
            data.building = Some(b);
            break;
        }
    }

    true
}

/// `canApplyCrusadeDone1(argsList)`: the holy city has a conscript unit.
fn can_apply_crusade_done1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let holy = holy_city(g, data.religion);
    // NULL city: getConscriptUnit() is -1.
    holy.and_then(|c| g.city_conscript_unit(c)).is_some()
}

/// `applyCrusadeDone1(argsList)`: free conscript units in the holy city.
fn apply_crusade_done1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let holy = holy_city(g, data.religion);
    let unit_type = holy.and_then(|c| g.city_conscript_unit(c));
    let num_units = py_div(default_players(g), 2) + 1;

    if let Some(unit_type) = unit_type {
        let (x, y) = city_xy(g, holy);
        for _ in 0..num_units {
            init_unit(g, p, unit_type, x, y, UnitAIType::UNITAI_CITY_DEFENSE);
        }
    }
}

/// `canApplyCrusadeDone2(argsList)`: the holy city lacks the building picked by
/// `canTriggerCrusadeDone`.
fn can_apply_crusade_done2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let holy = holy_city(g, data.religion);

    if data.building.is_none() || has_building(g, holy, data.building) {
        return false;
    }

    true
}

/// `applyCrusadeDone2(argsList)`: the holy city gets that building (the wonder movie popup is UI).
fn apply_crusade_done2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let holy = holy_city(g, data.religion);
    // NULL city: no-op; NO_BUILDING (excluded by canApplyCrusadeDone2) would index C++ arrays at -1.
    if let (Some(c), Some(b)) = (holy, data.building) {
        g.city_set_num_real_building(c, b, 1);
    }
}

/// `canApplyCrusadeDone3(argsList)`: some city in the world lacks the religion. (The Python also
/// reads the default player count, unused.)
fn can_apply_crusade_done3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    // countReligionLevels(NO_RELIGION) would index C++ arrays at -1; taken as 0.
    let religion_levels = data.religion.map_or(0, |r| g.count_religion_levels(r));
    if g.num_cities == religion_levels {
        return false;
    }

    true
}

/// `applyCrusadeDone3(argsList)`: the religion spreads to the cities without it nearest to the
/// holy city (as many as the world size's default player count).
fn apply_crusade_done3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let holy = holy_city(g, data.religion);
    let (hx, hy) = city_xy(g, holy);

    let mut list_cities = Vec::new();
    for other in g.civ_player_ids() {
        if g.players[other].alive {
            for &c in &g.players[other].cities {
                if !city_has_religion(g, Some(c), data.religion) {
                    let distance = g.plot_distance(hx, hy, g.cities[c].x, g.cities[c].y);
                    list_cities.push((distance, c));
                }
            }
        }
    }

    sort_by_distance(&mut list_cities);

    let num_cities = default_players(g).min(list_cities.len() as i32);

    for &(_, c) in list_cities.iter().take(max(num_cities, 0) as usize) {
        // setHasReligion(NO_RELIGION) would index C++ arrays at -1; skipped.
        if let Some(religion) = data.religion {
            g.city_set_has_religion(c, religion, true, true, true);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// ESTEEMED PLAYWRIGHT / SECRET KNOWLEDGE / HIGH WARLORD / EXPERIENCED CAPTAIN
// ---------------------------------------------------------------------------------------------

/// `canTriggerEsteemedPlaywright(argsList)`: not under Slavery.
fn can_trigger_esteemed_playwright(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    // If source civ is operating this Civic, disallow the event to trigger.
    if is_civic(g, data.player, g.d.ty::<CivicType>("CIVIC_SLAVERY")) {
        return false;
    }
    true
}

/// `applySecretKnowledge2(argsList)`: +4 culture from the triggered building class in the city.
fn apply_secret_knowledge2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let city = player_city(g, data.player, data.city);
    // (NO_BUILDING: `gc.getBuildingInfo(-1)` is None and the Python raises.)
    let Some(building) = data.building else { return };
    let Some(building_class) = g.d.buildings[building].building_class_type else { return };
    // NULL city: setBuildingCommerceChange is a no-op.
    if let Some(c) = city {
        g.city_set_building_commerce_change(c, building_class, CommerceType::COMMERCE_CULTURE, 4);
    }
}

/// `canTriggerHighWarlord(argsList)`. CIVIC_EMANCIPATION is not in FFH2 (`isCivic(-1)`).
fn can_trigger_high_warlord(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    // If source civ is operating this Civic, disallow the event to trigger.
    if is_civic(g, data.player, g.d.ty::<CivicType>("CIVIC_EMANCIPATION")) {
        return false;
    }
    true
}

/// `canTriggerExperiencedCaptain(argsList)`: the unit has 7+ experience.
fn can_trigger_experienced_captain(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let Some(u) = player_unit(g, data.player, data.unit) else {
        return false;
    };

    if g.units[u].experience < 7 {
        return false;
    }

    true
}

// ---------------------------------------------------------------------------------------------
// PARTISANS
// ---------------------------------------------------------------------------------------------

/// `getNumPartisanUnits(plot, iPlayer)`: the highest culture level whose game-speed threshold the
/// player's culture on the plot reaches (0 if none; a NULL plot's culture is -1).
fn get_num_partisan_units(g: &Game, plot: Option<PlotIdx>, p: PlayerId) -> i32 {
    let culture = plot.map_or(-1, |pl| g.map.plots[pl].culture[p]);
    let num_levels = g.d.culture_levels.len();
    let game_speed = g.game_speed_type();
    for i in 0..num_levels {
        let level = num_levels - i - 1;
        if culture >= g.d.culture_levels[CultureLevelType::from_usize(level)].speed_threshold[game_speed] {
            return level as i32;
        }
    }
    0
}

/// `canApplyPartisans1(argsList)`: partisans to raise and a free land plot around the trigger
/// plot (the plot itself included).
fn can_apply_partisans1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let p = data.player;
    let plot = g.map.plot_xy(data.plot_x, data.plot_y);

    if get_num_partisan_units(g, plot, p) <= 0 {
        return false;
    }

    for i in 0..3 {
        for j in 0..3 {
            if let Some(loop_plot) = g.map.plot_xy(data.plot_x + i - 1, data.plot_y + j - 1) {
                if !(g.plot_is_visible_enemy_unit(loop_plot, p)
                    || g.plot_is_water(loop_plot)
                    || g.plot_is_impassable(loop_plot)
                    || g.map.plots[loop_plot].plot_city.is_some())
                {
                    return true;
                }
            }
        }
    }
    false
}

/// `applyPartisans1(argsList)`: the capital's conscript unit, once per partisan, on random free
/// land plots around the trigger plot.
fn apply_partisans1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let capital = g.player_get_capital_city(p);
    let plot = g.map.plot_xy(data.plot_x, data.plot_y);

    let Some(capital) = capital else { return };

    let num_units = get_num_partisan_units(g, plot, p);

    let mut list_plots = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            if let Some(loop_plot) = g.map.plot_xy(data.plot_x + i - 1, data.plot_y + j - 1) {
                if (i != 1 || j != 1) && !(g.plot_is_visible_enemy_unit(loop_plot, p) || g.plot_is_water(loop_plot) || g.plot_is_impassable(loop_plot)) {
                    list_plots.push(loop_plot);
                }
            }
        }
    }

    if !list_plots.is_empty() {
        for _ in 0..num_units {
            let i_plot = g.soren_rand_num(list_plots.len() as i32, "Partisan event placement") as usize;
            let (x, y) = g.map.xy(list_plots[i_plot]);
            // No conscript unit: C++ initUnit(NO_UNIT) would crash; skipped.
            if let Some(unit_type) = g.city_conscript_unit(capital) {
                init_unit(g, p, unit_type, x, y, UnitAIType::UNITAI_ATTACK);
            }
        }
    }
}

/// `canApplyPartisans2(argsList)` (always true: `max(1, n / 2) > 0`).
fn can_apply_partisans2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let plot = g.map.plot_xy(data.plot_x, data.plot_y);
    max(1, py_div(get_num_partisan_units(g, plot, data.player), 2)) > 0
}

/// `applyPartisans2(argsList)`: half as many conscript units (at least one) in the capital.
fn apply_partisans2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let capital = g.player_get_capital_city(p);
    let plot = g.map.plot_xy(data.plot_x, data.plot_y);

    let Some(capital) = capital else { return };

    let num_units = max(1, py_div(get_num_partisan_units(g, plot, p), 2));
    for _ in 0..num_units {
        let (x, y) = city_xy(g, Some(capital));
        // No conscript unit: C++ initUnit(NO_UNIT) would crash; skipped.
        if let Some(unit_type) = g.city_conscript_unit(capital) {
            init_unit(g, p, unit_type, x, y, UnitAIType::UNITAI_ATTACK);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// GREED
// ---------------------------------------------------------------------------------------------

/// AI: C++ `CvPlayerAI::AI_trueCombatValue` (FfH): base strength plus damage-type strength, bonus
/// affinities and the weapon promotion the player's resources give.
fn ai_true_combat_value(g: &Game, p: PlayerId, unit: UnitType) -> i32 {
    let d = g.d;
    let info = &d.units[unit];

    let mut combat = if info.domain_type == Some(DomainType::DOMAIN_AIR) { info.air_combat } else { info.combat };

    for damage_type in d.damages.keys() {
        combat += info.damage_type_combat[damage_type];
    }

    for bonus in d.bonuses.keys() {
        combat += info.bonus_affinity[bonus] * g.player_get_num_available_bonuses(p, bonus);
    }

    if info.weapon_tier > 0 {
        // (C++ indexes with the WEAPON_REQ_BONUS_TIER* defines unchecked; they name bonuses.)
        let available = |bonus_define: i32| opt_from_i32::<BonusType>(bonus_define).map_or(0, |b| g.player_get_num_available_bonuses(p, b));

        let mut combat_bonus = 0;
        if let Some(promotion) = opt_from_i32::<PromotionType>(d.defines.weapon_promotion_tier1) {
            if available(d.defines.weapon_req_bonus_tier1) > 0 {
                combat_bonus = d.promotions[promotion].extra_combat_str;
            }
        }
        if let Some(promotion) = opt_from_i32::<PromotionType>(d.defines.weapon_promotion_tier2) {
            if info.weapon_tier > 1 && available(d.defines.weapon_req_bonus_tier2) > 0 {
                combat_bonus = d.promotions[promotion].extra_combat_str;
            }
        }
        if let Some(promotion) = opt_from_i32::<PromotionType>(d.defines.weapon_promotion_tier3) {
            if info.weapon_tier > 2 && available(d.defines.weapon_req_bonus_tier3) > 0 {
                combat_bonus = d.promotions[promotion].extra_combat_str;
            }
        }
        combat += combat_bonus;
    }

    combat
}

/// AI: C++ `CvPlayerAI::AI_combatValue` (FfH).
fn ai_combat_value(g: &Game, p: PlayerId, unit: UnitType) -> i32 {
    let d = g.d;
    let info = &d.units[unit];
    let mut value = 100 * ai_true_combat_value(g, p, unit);
    value *= (((info.first_strikes * 2) + info.chance_first_strikes) * (d.defines.combat_damage / 5)) + 100;
    value /= 100;
    value /= g.best_land_unit_combat();
    value
}

/// AI: C++ `CvPlayerAI::AI_unitValue(eUnit, UNITAI_ATTACK, pArea)` (CvPlayerAI.cpp:8774-9701),
/// the only case `getGreedUnit` asks for (C++ integer arithmetic). `AI_isDoStrategy(
/// AI_STRATEGY_FASTMOVERS)` is AI strategy state with no rules-side equivalent (always false for
/// humans); taken as off.
fn ai_unit_value_attack(g: &Game, p: PlayerId, unit: UnitType, area: Option<AreaId>) -> i32 {
    let d = g.d;
    let info = &d.units[unit];
    let unit_ai = UnitAIType::UNITAI_ATTACK;

    // AI_unitAIDomainType(UNITAI_ATTACK) is DOMAIN_LAND.
    if info.domain_type != Some(DomainType::DOMAIN_LAND) {
        return 0;
    }

    if info.not_unit_ai_type[unit_ai] {
        return 0;
    }

    let mut valid = info.unit_ai_type[unit_ai];
    if !valid && info.combat > 0 && !info.only_defensive {
        valid = true;
    }
    if !valid {
        return 0;
    }

    // FfH: Modified by Kael 05/07/2008
    let combat_value = ai_combat_value(g, p, unit) * 3;

    let mut value = 1;
    value += info.ai_weight;

    // AI: AI_isDoStrategy(AI_STRATEGY_FASTMOVERS) ? 2 : 1
    let fast_mover_multiplier = 1;

    value += combat_value;
    value += (combat_value * info.moves * fast_mover_multiplier) / 2;
    value += (combat_value * info.withdrawal_probability) / 100;
    if info.combat_limit < 100 {
        value -= (combat_value * (125 - info.combat_limit)) / 100;
    }

    if combat_value > 0 {
        if let Some(area) = area {
            let area_ai = g.map.areas[area].area_ai_type[g.players[p].team];
            if area_ai == Some(AreaAIType::AREAAI_ASSAULT) || area_ai == Some(AreaAIType::AREAAI_ASSAULT_MASSING) {
                for promotion in d.promotions.keys() {
                    if info.free_promotions[promotion] && d.promotions[promotion].amphib {
                        value *= 133;
                        value /= 100;
                        break;
                    }
                }
            }
        }
    }

    max(0, value)
}

/// `getGreedUnit(player, plot)`: the land unit the player can train that needs the plot's bonus
/// (as its AND bonus or one of its OR bonuses) with the best `AI_unitValue(.., UNITAI_ATTACK,
/// plot.area())`; `None` is -1. For a plot without a (visible) bonus the Python compares with
/// -1, which matches units without an AND bonus and the empty OR-bonus slots.
fn get_greed_unit(g: &Game, p: PlayerId, plot: Option<PlotIdx>) -> Option<UnitType> {
    let d = g.d;
    let team = g.players[p].team;
    // NULL plot: getBonusType() is -1 and area() is None.
    let bonus = plot.and_then(|pl| g.plot_get_bonus_type(pl, Some(team)));
    let area = plot.and_then(|pl| g.map.plots[pl].area);
    let num_or_bonuses = max(d.defines.num_unit_prereq_or_bonuses, 0) as usize;

    let mut best_value = 0;
    let mut best_unit = None;
    for unit_class in d.unit_classes.keys() {
        let Some(unit) = civ_unit(g, p, Some(unit_class)) else { continue };
        if g.player_can_train(p, unit, false, false, false) && d.units[unit].domain_type == Some(DomainType::DOMAIN_LAND) {
            let info = &d.units[unit];
            let mut value = 0;
            if info.prereq_and_bonus == bonus {
                value = ai_unit_value_attack(g, p, unit, area);
            } else {
                // (the data keeps the listed OR bonuses; C++ pads the slots with -1)
                for j in 0..num_or_bonuses {
                    if info.prereq_or_bonuses.get(j).copied() == bonus {
                        value = ai_unit_value_attack(g, p, unit, area);
                        break;
                    }
                }
            }
            if value > best_value {
                best_value = value;
                best_unit = Some(unit);
            }
        }
    }

    best_unit
}

/// `canTriggerGreed(argsList)`: the other player has a revealed land plot with a strategic bonus
/// (iron, horse or copper) we lack and we could use. (No other player: the Python raises.)
fn can_trigger_greed(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let Some(other_player) = data.other_player else {
        return false;
    };
    let team = g.players[p].team;

    if !g.team_can_change_war_peace(team, g.players[other_player].team, false) {
        return false;
    }

    // (FfH removed BONUS_OIL from the list.)
    let mut list_bonuses = Vec::new();
    for name in ["BONUS_IRON", "BONUS_HORSE", "BONUS_COPPER"] {
        let bonus = g.d.ty::<BonusType>(name);
        if 0 == num_available_bonuses(g, p, bonus) {
            list_bonuses.push(bonus);
        }
    }

    let mut found = false;
    let mut list_plots = Vec::new();
    for &bonus in &list_bonuses {
        for plot in g.map.plot_ids() {
            let pl = &g.map.plots[plot];
            if pl.owner == Some(other_player) && g.plot_get_bonus_type(plot, Some(team)) == bonus && pl.revealed[team] && !g.plot_is_water(plot) {
                list_plots.push(plot);
                found = true;
            }
        }
        if found {
            break;
        }
    }

    if !found {
        return false;
    }

    let plot = list_plots[g.soren_rand_num(list_plots.len() as i32, "Greed event plot selection") as usize];

    if get_greed_unit(g, p, Some(plot)).is_none() {
        return false;
    }

    let (x, y) = g.map.xy(plot);
    data.plot_x = x;
    data.plot_y = y;

    true
}

/// `expireGreed1(argsList)`: the quest fails after `getGrowthPercent()` turns or if the plot
/// changes hands to a third party (never once we own it).
fn expire_greed1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let p = data.player;
    let plot = g.map.plot_xy(data.plot_x, data.plot_y);
    // NULL plot: getOwner() is -1.
    let owner = plot.and_then(|pl| g.map.plots[pl].owner);

    if owner == Some(p) || owner.is_none() {
        return false;
    }

    if g.game_turn() >= data.turn + g.d.game_speeds[g.game_speed_type()].growth_percent {
        return true;
    }

    if owner != data.other_player {
        return true;
    }

    false
}

/// `canTriggerGreedDone(argsList)`: we took the plot; carries over the plot and other player.
fn can_trigger_greed_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    // (No first event: `kOrigTriggeredData.iPlotX` raises in the Python.)
    let Some(orig) = orig_triggered_data(g, p, data.trigger) else {
        return false;
    };
    let plot = g.map.plot_xy(orig.plot_x, orig.plot_y);

    if plot.and_then(|pl| g.map.plots[pl].owner) != Some(orig.player) {
        return false;
    }

    if get_greed_unit(g, p, plot).is_none() {
        return false;
    }

    data.plot_x = orig.plot_x;
    data.plot_y = orig.plot_y;
    data.other_player = orig.other_player;

    true
}

/// `applyGreedDone1(argsList)`: free units that use the plot's bonus.
fn apply_greed_done1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let plot = g.map.plot_xy(data.plot_x, data.plot_y);

    let unit_type = get_greed_unit(g, p, plot);
    let num_units = py_div(default_players(g), 2) + 1;

    if let Some(unit_type) = unit_type {
        let (x, y) = plot_coords(g, plot);
        for _ in 0..num_units {
            init_unit(g, p, unit_type, x, y, UnitAIType::UNITAI_ATTACK);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// WAR CHARIOTS / ELITE SWORDSMEN / WARSHIPS / GUNS NOT BUTTER / NOBLE KNIGHTS / OVERWHELM
// ---------------------------------------------------------------------------------------------

/// `canTriggerWarChariots(argsList)`: remembers the state religion.
fn can_trigger_war_chariots(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    data.religion = g.player_get_state_religion(data.player);
    true
}

/// `canTriggerWarChariotsDone(argsList)`: enough chariots; carries over the religion.
fn can_trigger_war_chariots_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let num_units = default_players(g) + 1;
    let unit_class_type = g.d.ty::<UnitClassType>("UNITCLASS_CHARIOT");
    if unit_class_count(g, p, unit_class_type) < num_units {
        return false;
    }

    // (No first event: `kOrigTriggeredData.eReligion` raises in the Python.)
    let Some(orig) = orig_triggered_data(g, p, data.trigger) else {
        return false;
    };
    data.religion = orig.religion;

    true
}

/// `canTriggerEliteSwordsDone(argsList)`: enough swordsmen (not an FFH2 unit class).
fn can_trigger_elite_swords_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let num_units = default_players(g) + 1;
    let unit_class_type = g.d.ty::<UnitClassType>("UNITCLASS_SWORDSMAN");
    if unit_class_count(g, data.player, unit_class_type) < num_units {
        return false;
    }
    true
}

/// `canApplyEliteSwordsDone2(argsList)`. CIVIC_HEREDITARY_RULE is not in FFH2 (`isCivic(-1)`).
fn can_apply_elite_swords_done2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let civic = g.d.ty::<CivicType>("CIVIC_HEREDITARY_RULE");
    if !is_civic(g, data.player, civic) {
        return false;
    }
    true
}

/// `canTriggerWarships(argsList)`: 55%+ water.
fn can_trigger_warships(g: &mut Game, _data: &mut EventTriggeredData) -> bool {
    water_reaches_percent(g, 55)
}

/// `canTriggerWarshipsDone(argsList)`: enough triremes.
fn can_trigger_warships_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let num_units = default_players(g);
    let unit_class_type = g.d.ty::<UnitClassType>("UNITCLASS_TRIREME");

    if unit_class_count(g, data.player, unit_class_type) < num_units {
        return false;
    }
    true
}

/// `canApplyWarshipsDone2(argsList)`: we have the Great Lighthouse.
fn can_apply_warships_done2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    // (A missing building: `gc.getBuildingInfo(-1)` is None and the Python raises.)
    let Some(building) = g.d.ty::<BuildingType>("BUILDING_GREAT_LIGHTHOUSE") else {
        return false;
    };
    if building_class_count(g, data.player, g.d.buildings[building].building_class_type) == 0 {
        return false;
    }
    true
}

/// `canTriggerGunsButterDone(argsList)`: enough musketmen (not an FFH2 unit class).
fn can_trigger_guns_butter_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let num_units = default_players(g) + 1;
    let unit_class_type = g.d.ty::<UnitClassType>("UNITCLASS_MUSKETMAN");

    if unit_class_count(g, data.player, unit_class_type) < num_units {
        return false;
    }
    true
}

/// `canApplyGunsButterDone2(argsList)`. CIVIC_VASSALAGE is not in FFH2 (`isCivic(-1)`).
fn can_apply_guns_butter_done2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let civic = g.d.ty::<CivicType>("CIVIC_VASSALAGE");
    if !is_civic(g, data.player, civic) {
        return false;
    }
    true
}

/// `canApplyGunsButterDone3(argsList)`: we have the Taj Mahal. Not an FFH2 building:
/// `gc.getBuildingInfo(-1)` is None and the Python raises (false).
fn can_apply_guns_butter_done3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let Some(building) = g.d.ty::<BuildingType>("BUILDING_TAJ_MAHAL") else {
        return false;
    };
    if building_class_count(g, data.player, g.d.buildings[building].building_class_type) == 0 {
        return false;
    }
    true
}

/// `canTriggerNobleKnights(argsList)`: remembers the state religion.
fn can_trigger_noble_knights(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    data.religion = g.player_get_state_religion(data.player);
    true
}

/// `canTriggerNobleKnightsDone(argsList)`: enough knights; carries over the religion and moves
/// to the Oracle's city (not an FFH2 building, so the trigger keeps its own city).
fn can_trigger_noble_knights_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let num_units = default_players(g) + 1;
    let unit_class_type = g.d.ty::<UnitClassType>("UNITCLASS_KNIGHT");

    if unit_class_count(g, p, unit_class_type) < num_units {
        return false;
    }

    // (No first event: `kOrigTriggeredData.eReligion` raises in the Python.)
    let Some(orig) = orig_triggered_data(g, p, data.trigger) else {
        return false;
    };
    data.religion = orig.religion;

    let building = g.d.ty::<BuildingType>("BUILDING_ORACLE");

    for &c in &g.players[p].cities {
        if has_building(g, Some(c), building) {
            data.plot_x = g.cities[c].x;
            data.plot_y = g.cities[c].y;
            data.city = Some(c);
            break;
        }
    }

    true
}

/// `canApplyNobleKnightsDone2(argsList)`. CIVIC_ORGANIZED_RELIGION is not in FFH2
/// (`isCivic(-1)`).
fn can_apply_noble_knights_done2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let civic = g.d.ty::<CivicType>("CIVIC_ORGANIZED_RELIGION");
    if !is_civic(g, data.player, civic) {
        return false;
    }
    true
}

/// `canTriggerOverwhelm(argsList)`: 55%+ water.
fn can_trigger_overwhelm(g: &mut Game, _data: &mut EventTriggeredData) -> bool {
    water_reaches_percent(g, 55)
}

/// `canTriggerOverwhelmDone(argsList)`: a modern fleet (none of these classes is in FFH2).
fn can_trigger_overwhelm_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;

    let destroyer = g.d.ty::<UnitClassType>("UNITCLASS_DESTROYER");
    let num_destroyers = 4;
    if unit_class_count(g, p, destroyer) < num_destroyers {
        return false;
    }

    let battleship = g.d.ty::<UnitClassType>("UNITCLASS_BATTLESHIP");
    let num_battleships = 2;
    if unit_class_count(g, p, battleship) < num_battleships {
        return false;
    }

    let carrier = g.d.ty::<UnitClassType>("UNITCLASS_CARRIER");
    let num_carriers = 3;
    if unit_class_count(g, p, carrier) < num_carriers {
        return false;
    }

    let fighter = g.d.ty::<SpecialUnitType>("SPECIALUNIT_FIGHTER");
    let num_fighters = 9;
    let mut num_player_fighters = 0;
    for &u in &g.players[p].units {
        if g.unit_get_special_unit_type(u) == fighter {
            num_player_fighters += 1;
        }
    }

    if num_player_fighters < num_fighters {
        return false;
    }

    true
}

/// `canApplyOverwhelmDone3(argsList)`: the team built the Manhattan Project (not an FFH2
/// project: its count is taken as 0).
fn can_apply_overwhelm_done3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let project = g.d.ty::<ProjectType>("PROJECT_MANHATTAN_PROJECT");
    let team = g.players[data.player].team;
    if project.map_or(0, |pr| g.teams[team].project_count[pr]) == 0 {
        return false;
    }
    true
}

/// `applyOverwhelmDone3(argsList)`: `changeNoNukesCount(1)`.
fn apply_overwhelm_done3(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    g.no_nukes_count += 1;
}

// ---------------------------------------------------------------------------------------------
// CORPORATE EXPANSION / HOSTILE TAKEOVER
// ---------------------------------------------------------------------------------------------

/// The first building that founds `corporation` (`for iBuilding ...: if getFoundsCorporation()
/// == eCorporation: ... break`).
fn corporation_founding_building(g: &Game, corporation: CorporationType) -> Option<BuildingType> {
    g.d.buildings.keys().find(|&b| g.d.buildings[b].founds_corporation == Some(corporation))
}

/// `canTriggerCorporateExpansion(argsList)`: the corporation has headquarters; remembers it,
/// the corporation's current number of cities and its founding building.
fn can_trigger_corporate_expansion(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    // (getHeadquarters(NO_CORPORATION) would index C++ arrays at -1: no headquarters.)
    let Some(corporation) = data.corporation else {
        return false;
    };
    let Some(city) = g.get_headquarters(corporation) else {
        return false;
    };

    // Hack to remember the number of cities you already have with the Corporation: the count
    // is stored in the other-player-city id field.
    let levels = g.count_corporation_levels(corporation);
    data.other_player_city = Some(CityId(levels as u32));
    data.city = Some(city);
    data.plot_x = g.cities[city].x;
    data.plot_y = g.cities[city].y;

    match corporation_founding_building(g, corporation) {
        Some(b) => data.building = Some(b),
        None => return false,
    }

    true
}

/// `expireCorporateExpansion1(argsList)`: the headquarters city is no longer ours.
fn expire_corporate_expansion1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    player_city(g, data.player, data.city).is_none()
}

/// `canTriggerCorporateExpansionDone(argsList)`: the corporation spread to enough new cities;
/// carries over corporation, building, city and plot.
fn can_trigger_corporate_expansion_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    // (No first event: `kOrigTriggeredData.iOtherPlayerCityId` raises in the Python.)
    let Some(orig) = orig_triggered_data(g, data.player, data.trigger) else {
        return false;
    };

    // the count stored by canTriggerCorporateExpansion
    let start_levels = orig.other_player_city.map_or(-1, |c| c.0 as i32);
    let num_cities_required = default_players(g) + 1 + start_levels;

    let levels = orig.corporation.map_or(0, |c| g.count_corporation_levels(c));
    if num_cities_required > levels {
        return false;
    }

    data.corporation = orig.corporation;
    data.building = orig.building;
    data.city = orig.city;
    data.plot_x = orig.plot_x;
    data.plot_y = orig.plot_y;

    true
}

/// `setBuildingCommerceChange(gc.getBuildingInfo(eBuilding).getBuildingClassType(), eCommerce,
/// change)` on our city of the triggered data (`applyCorporateExpansionDone1`,
/// `applyHostileTakeoverDone1`).
fn headquarters_commerce_change(g: &mut Game, data: &EventTriggeredData, commerce: CommerceType, change: i32) {
    if let Some(c) = player_city(g, data.player, data.city) {
        // (NO_BUILDING: `gc.getBuildingInfo(-1)` is None and the Python raises.)
        let Some(building) = data.building else { return };
        if let Some(building_class) = g.d.buildings[building].building_class_type {
            g.city_set_building_commerce_change(c, building_class, commerce, change);
        }
    }
}

/// `applyCorporateExpansionDone1(argsList)`: +10 gold from the headquarters building.
fn apply_corporate_expansion_done1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    headquarters_commerce_change(g, data, CommerceType::COMMERCE_GOLD, 10);
}

/// `getHostileTakeoverListResources(corporation, player)`: the corporation's prerequisite bonuses
/// that none of the player's plots has (as the player's team sees them).
fn get_hostile_takeover_list_resources(g: &Game, corporation: CorporationType, p: PlayerId) -> Vec<BonusType> {
    let team = g.players[p].team;
    let mut list_have = Vec::new();
    for plot in g.map.plot_ids() {
        if g.map.plots[plot].owner == Some(p) {
            if let Some(bonus) = g.plot_get_bonus_type(plot, Some(team)) {
                if !list_have.contains(&bonus) {
                    list_have.push(bonus);
                }
            }
        }
    }

    // (the data keeps the listed bonuses; C++ pads its NUM_CORPORATION_PREREQ_BONUSES slots with
    // -1, which the Python skips)
    let mut list_need = Vec::new();
    for &bonus in &g.d.corporations[corporation].prereq_bonus {
        if !list_have.contains(&bonus) {
            list_need.push(bonus);
        }
    }
    list_need
}

/// `canTriggerHostileTakeover(argsList)`: not for a human in One City Challenge; the corporation
/// has headquarters and we lack some of its resources. Remembers city, plot and building.
fn can_trigger_hostile_takeover(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;

    if one_city_challenge_human(g, p) {
        return false;
    }

    // (getHeadquarters(NO_CORPORATION) would index C++ arrays at -1: no headquarters.)
    let Some(corporation) = data.corporation else {
        return false;
    };
    let Some(city) = g.get_headquarters(corporation) else {
        return false;
    };

    data.city = Some(city);
    data.plot_x = g.cities[city].x;
    data.plot_y = g.cities[city].y;

    match corporation_founding_building(g, corporation) {
        Some(b) => data.building = Some(b),
        None => return false,
    }

    let list_resources = get_hostile_takeover_list_resources(g, corporation, p);
    if list_resources.is_empty() {
        return false;
    }

    true
}

/// `expireHostileTakeover1(argsList)`: the headquarters city is no longer ours.
fn expire_hostile_takeover1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    player_city(g, data.player, data.city).is_none()
}

/// `canTriggerHostileTakeoverDone(argsList)`: we now have all the corporation's resources;
/// carries over corporation, building, city and plot.
fn can_trigger_hostile_takeover_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    // (No first event, or NO_CORPORATION (`gc.getCorporationInfo(-1)` is None): the Python raises.)
    let Some(orig) = orig_triggered_data(g, p, data.trigger) else {
        return false;
    };
    let Some(corporation) = orig.corporation else {
        return false;
    };

    let list_resources = get_hostile_takeover_list_resources(g, corporation, p);

    if !list_resources.is_empty() {
        return false;
    }

    data.corporation = orig.corporation;
    data.building = orig.building;
    data.city = orig.city;
    data.plot_x = orig.plot_x;
    data.plot_y = orig.plot_y;

    true
}

/// `applyHostileTakeoverDone1(argsList)`: +20 gold from the headquarters building.
fn apply_hostile_takeover_done1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    headquarters_commerce_change(g, data, CommerceType::COMMERCE_GOLD, 20);
}

// ---------------------------------------------------------------------------------------------
// GREAT BEAST / COMET FRAGMENT / IMMIGRANTS / CONTROVERSIAL PHILOSOPHER
// ---------------------------------------------------------------------------------------------

/// `doGreatBeast3(argsList)`: 40 turns of extra happiness in our cities with the religion.
fn do_great_beast3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    for c in g.players[p].cities.clone() {
        if city_has_religion(g, Some(c), data.religion) {
            g.city_change_happiness_timer(c, 40);
        }
    }
}

/// `canDoCometFragment(argsList)` (a trigger can-do: its only argument is the triggered data).
fn can_do_comet_fragment(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if g.players[data.player].space_production_modifier > 10 {
        return false;
    }
    true
}

/// `canTriggerImmigrantCity(argsList)`: a happy, healthy city with 55+ culture. The Python has no
/// final `return true` (it returns None, which the DLL cannot read as a number); taken as false.
fn can_trigger_immigrant_city(g: &Game, _trigger: EventTriggerType, p: PlayerId, c: CityId) -> bool {
    let Some(city) = player_city(g, p, Some(c)) else {
        return false;
    };

    if (g.city_happy_level(city) - g.city_unhappy_level(city, 0) < 1) || (g.city_good_health(city) - g.city_bad_health(city, true, 0) < 1) {
        return false;
    }

    if g.city_commerce_rate_times100(city, CommerceType::COMMERCE_CULTURE) < 5500 {
        return false;
    }

    false
}

/// `canTriggerControversialPhilosopherCity(argsList)`: the capital, with 35+ research.
fn can_trigger_controversial_philosopher_city(g: &Game, _trigger: EventTriggerType, p: PlayerId, c: CityId) -> bool {
    let Some(city) = player_city(g, p, Some(c)) else {
        return false;
    };
    if !g.city_is_capital(city) {
        return false;
    }
    if g.city_commerce_rate_times100(city, CommerceType::COMMERCE_RESEARCH) < 3500 {
        return false;
    }

    true
}

// ---------------------------------------------------------------------------------------------
// SPY DISCOVERED / NUCLEAR PROTEST
// ---------------------------------------------------------------------------------------------

/// `canDoSpyDiscovered3(argsList)`: 4+ cities and a capital.
fn can_do_spy_discovered3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let p = data.player;

    if g.players[p].num_cities() < 4 {
        return false;
    }

    if g.player_get_capital_city(p).is_none() {
        return false;
    }

    true
}

/// `doSpyDiscovered3(argsList)`: a tank per 4 cities in the capital (UNITCLASS_TANK is not in
/// FFH2, so none).
fn do_spy_discovered3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;

    // (No capital: `getCapitalCity().plot()` is None and the Python raises.)
    let Some(capital) = g.player_get_capital_city(p) else { return };
    let plot = g.city_plot(capital);
    let num_units = py_div(g.players[p].num_cities(), 4);
    let unit_type = civ_unit(g, p, g.d.ty::<UnitClassType>("UNITCLASS_TANK"));

    if let Some(unit_type) = unit_type {
        let (x, y) = g.map.xy(plot);
        for _ in 0..num_units {
            init_unit(g, p, unit_type, x, y, UnitAIType::UNITAI_ATTACK);
        }
    }
}

/// `canTriggerNuclearProtest(argsList)`: 10+ nukes (not FFH2 unit classes).
fn can_trigger_nuclear_protest(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    let icbm_class = g.d.ty::<UnitClassType>("UNITCLASS_ICBM");
    let tac_nuke_class = g.d.ty::<UnitClassType>("UNITCLASS_TACTICAL_NUKE");
    if unit_class_count(g, p, icbm_class) + unit_class_count(g, p, tac_nuke_class) < 10 {
        return false;
    }

    true
}

/// `doNuclearProtest1(argsList)`: our nukes are disbanded.
fn do_nuclear_protest1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let icbm_class = g.d.ty::<UnitClassType>("UNITCLASS_ICBM");
    let tac_nuke_class = g.d.ty::<UnitClassType>("UNITCLASS_TACTICAL_NUKE");

    // (firstUnit/nextUnit skip the units killed meanwhile, e.g. cargo of a killed transport)
    for u in g.players[p].units.clone() {
        if !g.units.contains(u) {
            continue;
        }
        let unit_class = Some(g.unit_get_unit_class_type(u));
        if unit_class == icbm_class || unit_class == tac_nuke_class {
            g.unit_kill(u, false, None);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// BTS civilization-specific events (none of these civilizations exists in FFH2)
// ---------------------------------------------------------------------------------------------

/// `canTriggerPreachingResearcherCity(argsList)`: the city has a university (not an FFH2
/// building).
fn can_trigger_preaching_researcher_city(g: &Game, _trigger: EventTriggerType, p: PlayerId, c: CityId) -> bool {
    let city = player_city(g, p, Some(c));
    if has_building(g, city, g.d.ty::<BuildingType>("BUILDING_UNIVERSITY")) {
        return true;
    }
    false
}

/// `canTriggerToxcatl(argsList)` (Aztec event).
fn can_trigger_toxcatl(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    is_civilization(g, data.player, "CIVILIZATION_AZTEC")
}

/// `canTriggerDissidentPriest(argsList)` (Egyptian event).
fn can_trigger_dissident_priest(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    is_civilization(g, data.player, "CIVILIZATION_EGYPT")
}

/// `canTriggerDissidentPriestCity(argsList)`: not the government center, 30+ culture, and no
/// state religion. (NULL city: `isGovernmentCenter()` is false and the culture rate -1.)
fn can_trigger_dissident_priest_city(g: &Game, _trigger: EventTriggerType, p: PlayerId, c: CityId) -> bool {
    let Some(city) = player_city(g, p, Some(c)) else {
        return false;
    };

    if g.city_is_government_center(city) {
        return false;
    }
    if g.city_commerce_rate_times100(city, CommerceType::COMMERCE_CULTURE) < 3000 {
        return false;
    }
    if g.player_get_state_religion(p).is_some() {
        return false;
    }

    true
}

/// `canTriggerRogueStation(argsList)` (Russian event).
fn can_trigger_rogue_station(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    is_civilization(g, data.player, "CIVILIZATION_RUSSIA")
}

/// `canTriggerAntiMonarchists(argsList)` (French event).
fn can_trigger_anti_monarchists(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    is_civilization(g, data.player, "CIVILIZATION_FRANCE")
}

/// `canTriggerImpeachment(argsList)` (American event).
fn can_trigger_impeachment(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    is_civilization(g, data.player, "CIVILIZATION_AMERICA")
}

/// `canTriggerImpeachmentCity(argsList)`: the capital.
fn can_trigger_impeachment_city(g: &Game, _trigger: EventTriggerType, p: PlayerId, c: CityId) -> bool {
    let city = player_city(g, p, Some(c));
    // NULL city: isCapital() is false.
    if city.map_or(false, |city| g.city_is_capital(city)) {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------------------------
// XML callback names with no Python definition
// ---------------------------------------------------------------------------------------------

/// `applyHeresy2` (EVENT_HERESY_2) and `applyTowerShield` (EVENT_TOWER_SHIELD_1): the XML names
/// these event callbacks, but CvRandomEventInterface.py defines neither. The DLL's
/// `callFunction` then fails (Python AttributeError) and `CvPlayer::applyEvent` ignores the
/// result, so the callback does nothing.
fn missing_python_callback(_g: &mut Game, _event: EventType, _data: &EventTriggeredData) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// BTS type names used above that FFH2 0.41 does not define (`findInfoTypeNum` /
    /// `getInfoTypeForString` give -1 for them, `d.ty` gives `None`).
    const NOT_IN_FFH2: &[&str] = &[
        "BONUS_MOVIES", "BONUS_SILVER", "BONUS_URANIUM", "BUILDINGCLASS_CASTLE", "BUILDINGCLASS_COLOSSEUM",
        "BUILDING_GREAT_WALL", "BUILDING_ORACLE", "BUILDING_STATUE_OF_ZEUS", "BUILDING_TAJ_MAHAL", "BUILDING_UNIVERSITY",
        "CIVIC_EMANCIPATION", "CIVIC_ENVIRONMENTALISM", "CIVIC_HEREDITARY_RULE", "CIVIC_ORGANIZED_RELIGION",
        "CIVIC_VASSALAGE", "CIVILIZATION_AMERICA", "CIVILIZATION_AZTEC", "CIVILIZATION_EGYPT", "CIVILIZATION_FRANCE",
        "CIVILIZATION_RUSSIA", "ERA_INDUSTRIAL", "PROJECT_MANHATTAN_PROJECT", "RELIGION_CHRISTIANITY", "RELIGION_ISLAM",
        "RELIGION_JUDAISM", "SPECIALUNIT_FIGHTER", "TECH_METAL_CASTING", "TECH_MONOTHEISM", "TECH_POLYTHEISM",
        "UNITCLASS_BATTLESHIP", "UNITCLASS_CARRIER", "UNITCLASS_DESTROYER", "UNITCLASS_ICBM", "UNITCLASS_MUSKETMAN",
        "UNITCLASS_SPEARMAN", "UNITCLASS_SWORDSMAN", "UNITCLASS_TACTICAL_NUKE", "UNITCLASS_TANK", "UNIT_SPEARMAN",
    ];

    /// Registry class of a type name used above, by its prefix.
    fn class_of(name: &str) -> &'static str {
        let prefixes: [(&str, &'static str); 20] = [
            ("BONUS_", BonusType::CLASS),
            ("BUILDINGCLASS_", BuildingClassType::CLASS),
            ("BUILDING_", BuildingType::CLASS),
            ("CIVICOPTION_", CivicOptionType::CLASS),
            ("CIVIC_", CivicType::CLASS),
            ("CIVILIZATION_", CivilizationType::CLASS),
            ("ERA_", EraType::CLASS),
            ("FEATURE_", FeatureType::CLASS),
            ("IMPROVEMENT_", ImprovementType::CLASS),
            ("PROJECT_", ProjectType::CLASS),
            ("PROMOTION_", PromotionType::CLASS),
            ("RELIGION_", ReligionType::CLASS),
            ("ROUTE_", RouteType::CLASS),
            ("SPECIALIST_", SpecialistType::CLASS),
            ("SPECIALUNIT_", SpecialUnitType::CLASS),
            ("TECH_", TechType::CLASS),
            ("TERRAIN_", TerrainType::CLASS),
            ("UNITCLASS_", UnitClassType::CLASS),
            ("UNIT_", UnitType::CLASS),
            ("WORLDSIZE_", WorldSizeType::CLASS),
        ];
        prefixes.iter().find(|(prefix, _)| name.starts_with(prefix)).map(|&(_, class)| class).unwrap_or_else(|| panic!("add the prefix of {name} to class_of"))
    }

    /// The `"TYPE_NAME"` string literals of the code above the tests.
    fn type_name_literals() -> Vec<&'static str> {
        let src = include_str!("random_events_b.rs");
        let code = &src[..src.find(concat!("#[cfg", "(test)]")).unwrap()];
        let mut names = Vec::new();
        for line in code.lines() {
            for (i, piece) in line.split('"').enumerate() {
                let type_name = i % 2 == 1
                    && piece.contains('_')
                    && !piece.starts_with('_')
                    && !piece.ends_with('_')
                    && piece.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
                if type_name && !names.contains(&piece) {
                    names.push(piece);
                }
            }
        }
        names
    }

    /// Every type name looked up above is an FFH2 type of the expected class, or one of the BTS
    /// names known to be missing (which the lookups then answer with `None`, as the Python gets
    /// -1). A typo would otherwise silently become `None`.
    #[test]
    fn type_names_resolve() {
        let d = ffh_data::global();
        let names = type_name_literals();
        for &name in &names {
            let class = class_of(name);
            if NOT_IN_FFH2.contains(&name) {
                assert!(d.reg.entry(name).is_none(), "{name} exists in FFH2: drop it from NOT_IN_FFH2");
            } else {
                assert_eq!(d.reg.entry(name).map(|e| e.class), Some(class), "{name}");
            }
        }
        for name in NOT_IN_FFH2 {
            assert!(names.contains(name), "{name} is not used");
        }
        assert!(names.len() > 80, "only {} names found", names.len());
    }

    #[test]
    fn every_function_is_registered() {
        let trigger_can_do = [
            "canTriggerDustbowlCont", "canTriggerChampion", "canTriggerElectricCompany", "canTriggerGoldRush",
            "canTriggerInfluenza", "canTriggerSoloFlight", "canTriggerAntelope", "canTriggerWhaleOfAThing",
            "canTriggerHiyoSilver", "canTriggerWiningMonks", "canTriggerIndependentFilms", "canTriggerAncientOlympics",
            "canTriggerModernOlympics", "canTriggerInterstate", "canTriggerHeroicGesture", "canTriggerGreatMediator",
            "canTriggerImpactCrater", "canTriggerTheHuns", "canTriggerTheVandals", "canTriggerTheGoths",
            "canTriggerThePhilistines", "canTriggerTheVedicAryans", "canTriggerSecurityTax", "canTriggerLiteracy",
            "canTriggerTea", "canTriggerHorseWhispering", "canTriggerHorseWhisperingDone", "canTriggerHarbormaster",
            "canTriggerHarbormasterDone", "canTriggerClassicLiterature", "canTriggerClassicLiteratureDone",
            "canTriggerMasterBlacksmith", "canTriggerMasterBlacksmithDone", "canTriggerBestDefense",
            "canTriggerBestDefenseDone", "canTriggerSportsLeague", "canTriggerSportsLeagueDone", "canTriggerCrusade",
            "canTriggerCrusadeDone", "canTriggerEsteemedPlaywright", "canTriggerHighWarlord",
            "canTriggerExperiencedCaptain", "canTriggerGreed", "canTriggerGreedDone", "canTriggerWarChariots",
            "canTriggerWarChariotsDone", "canTriggerEliteSwordsDone", "canTriggerWarships", "canTriggerWarshipsDone",
            "canTriggerGunsButterDone", "canTriggerNobleKnights", "canTriggerNobleKnightsDone", "canTriggerOverwhelm",
            "canTriggerOverwhelmDone", "canTriggerCorporateExpansion", "canTriggerCorporateExpansionDone",
            "canTriggerHostileTakeover", "canTriggerHostileTakeoverDone", "canDoCometFragment",
            "canTriggerNuclearProtest", "canTriggerToxcatl", "canTriggerDissidentPriest", "canTriggerRogueStation",
            "canTriggerAntiMonarchists", "canTriggerImpeachment",
        ];
        for name in trigger_can_do {
            assert!(find_trigger_can_do(name).is_some(), "{name}");
        }
        let trigger_city = [
            "canTriggerImmigrantCity", "canTriggerControversialPhilosopherCity", "canTriggerPreachingResearcherCity",
            "canTriggerDissidentPriestCity", "canTriggerImpeachmentCity",
        ];
        for name in trigger_city {
            assert!(find_trigger_city(name).is_some(), "{name}");
        }
        assert!(find_trigger_unit("canTriggerChampionUnit").is_some());
        let event_can_do = [
            "canApplySaltpeter", "canApplyEarthDay2", "canApplyFreedomConcert2", "canApplyClassicLiteratureDone2",
            "canApplyClassicLiteratureDone3", "canApplyMasterBlacksmithDone1", "canApplyMasterBlacksmithDone3",
            "canApplyBestDefenseDone3", "canApplySportsLeagueDone3", "canApplyCrusadeDone1", "canApplyCrusadeDone2",
            "canApplyCrusadeDone3", "canApplyPartisans1", "canApplyPartisans2", "canApplyEliteSwordsDone2",
            "canApplyWarshipsDone2", "canApplyGunsButterDone2", "canApplyGunsButterDone3", "canApplyNobleKnightsDone2",
            "canApplyOverwhelmDone3", "canDoSpyDiscovered3",
        ];
        for name in event_can_do {
            assert!(find_event_can_do(name).is_some(), "{name}");
        }
        let event_callback = [
            "applySaltpeter", "applyGreatDepression", "applyChampion", "applyInfluenza2", "applySoloFlight",
            "doAntelope2", "doWiningMonks2", "doIndependentFilms", "doAncientOlympics2", "applyModernOlympics",
            "applyInterstate", "applyEarthDay2", "applyFreedomConcert2", "doHeroicGesture2", "doGreatMediator2",
            "doAncientTexts2", "doImpactCrater2", "applyTheHuns1", "applyTheVandals1", "applyTheGoths1",
            "applyThePhilistines1", "applyTheVedicAryans1", "applyHorseWhisperingDone1", "applyClassicLiteratureDone2",
            "applyClassicLiteratureDone3", "applyMasterBlacksmithDone1", "applyBestDefenseDone2", "applyCrusadeDone1",
            "applyCrusadeDone2", "applyCrusadeDone3", "applySecretKnowledge2", "applyPartisans1", "applyPartisans2",
            "applyGreedDone1", "applyOverwhelmDone3", "applyCorporateExpansionDone1", "applyHostileTakeoverDone1",
            "doGreatBeast3", "doSpyDiscovered3", "doNuclearProtest1",
        ];
        for name in event_callback {
            assert!(find_event_callback(name).is_some(), "{name}");
        }
        let event_expire = [
            "expireMasterBlacksmith1", "expireCrusade1", "expireGreed1", "expireCorporateExpansion1",
            "expireHostileTakeover1",
        ];
        for name in event_expire {
            assert!(find_event_expire(name).is_some(), "{name}");
        }
        // Every callback of Python lines 3304-6623 (all functions there except the 56 `getHelp*`
        // texts, the 4 module helpers and the 2 popup button callbacks).
        let total = trigger_can_do.len() + trigger_city.len() + 1 + event_can_do.len() + event_callback.len() + event_expire.len();
        assert_eq!(total, 137);
        // Callbacks never cross slots (a trigger can-do name is not an event callback, ...).
        assert!(find_event_callback("canTriggerDustbowlCont").is_none());
        assert!(find_trigger_can_do("applyChampion").is_none());
        assert!(find_trigger_callback("doGreatBeast3").is_none());
    }

    /// Python sorts `(distance, object)` tuples; ties keep list order here.
    #[test]
    fn distance_sort_is_stable() {
        let mut list = vec![(3, 'a'), (1, 'b'), (3, 'c'), (1, 'd'), (0, 'e')];
        sort_by_distance(&mut list);
        assert_eq!(list, vec![(0, 'e'), (1, 'b'), (1, 'd'), (3, 'a'), (3, 'c')]);
    }
}
