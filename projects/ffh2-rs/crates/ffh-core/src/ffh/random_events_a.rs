//! Port of CvRandomEventInterface.py, part a: Python lines 1-3303, every trigger/event callback
//! from `canTriggerAeronsChosen` through `applyVolcano1` (part b, `random_events_b.rs`,
//! continues with `canTriggerDustbowlCont`). Each Python function is a private Rust `fn` (its
//! doc comment names the Python function) registered by its Python name in the `find_*` table
//! of the XML slot that names it (see dispatch.rs). The callbacks of this range that FFH2's XML
//! does not reference (the BtS Blessed Sea, Holy Mountain, Spicy, Bard Tale and Tsunami
//! callbacks, `applyWBSplinteredCourtDefeatedThessa3`) are registered in the slot their
//! argument list fits.
//!
//! Not ported: the `help*` / `getHelp*` functions (event help text: UI) and
//! `weddingFeud3Callback` (the popup handler of `doWeddingFeud3`'s human-only branch).
//!
//! Porting notes:
//! - Argument lists follow the DLL. A trigger's `PythonCallback(kTriggeredData, ePlayer)` runs for
//!   every player a global/team trigger fires for (`CvPlayer::setTriggerFired`), with
//!   `kTriggeredData.ePlayer` the player it fired for first: each port reads the one its Python
//!   reads (`p` for `argsList[1]`, `data.player` for `kTriggeredData.ePlayer`).
//! - `pPlayer.getUnit(iUnitId)` / `getCity(iCityId)` with the trigger data's ids: `None` is the
//!   NULL CyUnit/CyCity (the object is gone since the trigger fired), whose setters do nothing
//!   and whose getters answer -1/false. RNG draws the Python makes regardless are kept. Where the
//!   Python would carry the NULL object's (-1, -1) into `initUnit`, `findClearPlot` or the lair
//!   code (a crash or an exception in the original), the port returns at that point; `setXY` to
//!   a NULL capital's (-1, -1) is ported as written.
//! - `CyMap().plot(x, y)` off the map is the NULL CyPlot (`None`): `getNumUnits()`, `getOwner()`,
//!   `getFeatureType()` and `getImprovementType()` answer -1 and its setters do nothing. The BtS
//!   functions' `plot == None` tests are never true (CyMap.plot always returns an object).
//! - `PyPlayer(p).getUnitList()` is a snapshot of the player's units that are not dead
//!   (PyHelpers); a unit deleted while such a loop runs (cargo of a killed transport) is skipped
//!   after any RNG draw the Python makes before touching it (Python would use a dangling
//!   CyUnit). `getCityList()` and `firstCity .. nextCity` are the player's city list.
//! - UI-only Python is omitted: `CyInterface().addMessage`, `CyEngine` effects, sounds,
//!   `cf.addPopup` / `cf.addPlayerPopup`, `CyCamera`, `setUnitArtStyleType`, dirty bits.
//! - `CyGame().getWBMapScript()` is always false (worldbuilder scenarios are out of scope). The
//!   `applyWB*` / `canDoWB*` callbacks belong to scenario events (weight 0, fired only by
//!   ScenarioFunctions.py); they are ported as written, with the scenarios' fixed player and team
//!   slot numbers.
//! - `CvUtil.findInfoTypeNum` returns -1 for unknown names (its `pyAssert` never fires): the BtS
//!   functions' `BONUS_SPICES`, `SPECIALBUILDING_CATHEDRAL` and `SPECIALBUILDING_MONASTERY` are
//!   not FFH2 types and are `None` here, compared as Python compares -1.
//! - Two Python bugs end functions early and are kept: `doOrderVsVeilTemple1` reads an undefined
//!   `player` (NameError before its city loop) and `applyWBLordOfTheBalorsTemptStatiusYes` reads
//!   `pPlayer` before assigning it (UnboundLocalError on its first statement: it does nothing).
//! - `AI_getAttitude` (CvPlayerAI) is `Game::player_ai_get_attitude`: see `ai_get_attitude` below.

use crate::ffh::dispatch::*;
use crate::ffh::game_utils::py_div;
use crate::game::Game;
use crate::ids::*;
use crate::structs::EventTriggeredData;
use crate::utils::{is_limited_wonder_class, is_world_unit_class};
use ffh_data::*;

/// `DirectionTypes.DIRECTION_SOUTH` (facing of units created by `initUnit`).
const SOUTH: Option<DirectionType> = Some(DirectionType::DIRECTION_SOUTH);

// ---------------------------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------------------------

/// Trigger PythonCanDo (`canTriggerX(argsList=[triggerData])`).
pub fn find_trigger_can_do(name: &str) -> Option<TriggerCanDoFn> {
    lookup(TRIGGER_CAN_DO, name)
}

/// Trigger PythonCanDoCity (`canTriggerXCity(eTrigger, ePlayer, iCity)`).
pub fn find_trigger_city(name: &str) -> Option<TriggerCityFn> {
    lookup(TRIGGER_CITY, name)
}

/// Trigger PythonCanDoUnit (`canTriggerXUnit(eTrigger, ePlayer, iUnit)`).
pub fn find_trigger_unit(name: &str) -> Option<TriggerUnitFn> {
    lookup(TRIGGER_UNIT, name)
}

/// Trigger PythonCallback (`doX(triggerData, ePlayer)`).
pub fn find_trigger_callback(name: &str) -> Option<TriggerCallbackFn> {
    lookup(TRIGGER_CALLBACK, name)
}

/// Event PythonCanDo (`canApplyX(eEvent, triggerData)`).
pub fn find_event_can_do(name: &str) -> Option<EventCanDoFn> {
    lookup(EVENT_CAN_DO, name)
}

/// Event PythonCallback (`applyX(eEvent, triggerData)`).
pub fn find_event_callback(name: &str) -> Option<EventCallbackFn> {
    lookup(EVENT_CALLBACK, name)
}

/// Event PythonExpireCheck (`expireX(eEvent, triggerData)`).
pub fn find_event_expire(name: &str) -> Option<EventExpireFn> {
    lookup(EVENT_EXPIRE, name)
}

fn lookup<F: Copy>(table: &[(&str, F)], name: &str) -> Option<F> {
    table.iter().find(|(n, _)| *n == name).map(|&(_, f)| f)
}

const TRIGGER_CAN_DO: &[(&str, TriggerCanDoFn)] = &[
    ("canTriggerAeronsChosen", can_trigger_aerons_chosen),
    ("canTriggerPlotEmpty", can_trigger_plot_empty),
    ("canTriggerParith", can_trigger_parith),
    ("canTriggerPenguins", can_trigger_penguins),
    ("canTriggerPickAlignment", can_trigger_pick_alignment),
    ("canTriggerPronCapria", can_trigger_pron_capria),
    ("canTriggerPronEthne", can_trigger_pron_ethne),
    ("canTriggerPronArendel", can_trigger_pron_arendel),
    ("canTriggerPronThessa", can_trigger_pron_thessa),
    ("canTriggerPronHannah", can_trigger_pron_hannah),
    ("canTriggerPronRhoanna", can_trigger_pron_rhoanna),
    ("canTriggerPronValledia", can_trigger_pron_valledia),
    ("canTriggerPronMahala", can_trigger_pron_mahala),
    ("canTriggerPronKeelyn", can_trigger_pron_keelyn),
    ("canTriggerPronSheelba", can_trigger_pron_sheelba),
    ("canTriggerPronFaeryl", can_trigger_pron_faeryl),
    ("canTriggerPronAlexis", can_trigger_pron_alexis),
    ("canTriggerUniqueFeatureAifonIsle", can_trigger_unique_feature_aifon_isle),
    ("canTriggerUniqueFeatureBradelinesWell", can_trigger_unique_feature_bradelines_well),
    ("canTriggerUniqueFeatureBrokenSepulcher", can_trigger_unique_feature_broken_sepulcher),
    ("canTriggerUniqueFeatureGuardian", can_trigger_unique_feature_guardian),
    ("canTriggerUniqueFeatureLetumFrigus", can_trigger_unique_feature_letum_frigus),
    ("canTriggerUniqueFeatureLetumFrigusIllians", can_trigger_unique_feature_letum_frigus_illians),
    ("canTriggerUniqueFeaturePyreOfTheSeraphic", can_trigger_unique_feature_pyre_of_the_seraphic),
    ("canTriggerSwitchCivs", can_trigger_switch_civs),
    ("canTriggerBlessedSea", can_trigger_blessed_sea),
    ("canTriggerBlessedSea2", can_trigger_blessed_sea2),
    ("canTriggerHolyMountain", can_trigger_holy_mountain),
    ("canTriggerHolyMountainDone", can_trigger_holy_mountain_done),
    ("canTriggerHolyMountainRevealed", can_trigger_holy_mountain_revealed),
    ("canTriggerMarathon", can_trigger_marathon),
    ("canTriggerSpicy", can_trigger_spicy),
    ("canTriggerBabyBoom", can_trigger_baby_boom),
    ("canTriggerBrothersInNeed", can_trigger_brothers_in_need),
];

const TRIGGER_CITY: &[(&str, TriggerCityFn)] = &[
    ("canTriggerCityFeud", can_trigger_city_feud),
    ("canTriggerCitySplit", can_trigger_city_split),
    ("canTriggerGuildOfTheNineMerc5", can_trigger_guild_of_the_nine_merc5),
    ("canTriggerLunaticCity", can_trigger_lunatic_city),
    ("canTriggerMerchantKeep", can_trigger_merchant_keep),
    ("canTriggerSageKeep", can_trigger_sage_keep),
    ("canTriggerSmugglers", can_trigger_smugglers),
    ("canTriggerTraitor", can_trigger_traitor),
    ("canTriggerHurricaneCity", can_trigger_hurricane_city),
    ("canTriggerCycloneCity", can_trigger_cyclone_city),
    ("canTriggerTsunamiCity", can_trigger_tsunami_city),
    ("canTriggerMonsoonCity", can_trigger_monsoon_city),
];

const TRIGGER_UNIT: &[(&str, TriggerUnitFn)] = &[
    ("canTriggerAmuriteTrialUnit", can_trigger_amurite_trial_unit),
    ("canTriggerFoodSicknessUnit", can_trigger_food_sickness_unit),
    ("canTriggerMutateUnit", can_trigger_mutate_unit),
    ("canTriggerSlaveRevoltUnit", can_trigger_slave_revolt_unit),
    ("canTriggerWarGamesUnit", can_trigger_war_games_unit),
];

const TRIGGER_CALLBACK: &[(&str, TriggerCallbackFn)] = &[
    ("doArmageddonApocalypse", do_armageddon_apocalypse),
    ("doArmageddonArs", do_armageddon_ars),
    ("doArmageddonBlight", do_armageddon_blight),
    ("doArmageddonBuboes", do_armageddon_buboes),
    ("doArmageddonHellfire", do_armageddon_hellfire),
    ("doArmageddonPestilence", do_armageddon_pestilence),
    ("doArmageddonStephanos", do_armageddon_stephanos),
    ("doArmageddonWrath", do_armageddon_wrath),
    ("doArmageddonYersinia", do_armageddon_yersinia),
    ("doAzer", do_azer),
    ("doCityFeudArson", do_city_feud_arson),
    ("doFlareEntropyNode", do_flare_entropy_node),
    ("doFlareFireNode", do_flare_fire_node),
    ("doFlareLifeNode", do_flare_life_node),
    ("doFlareNatureNode", do_flare_nature_node),
    ("doFlareWaterNode", do_flare_water_node),
    ("doFoodSickness", do_food_sickness),
    ("doFrostling", do_frostling),
    ("doGodslayer", do_godslayer),
    ("doGreatBeastGurid", do_great_beast_gurid),
    ("doGreatBeastLeviathan", do_great_beast_leviathan),
    ("doGreatBeastMargalard", do_great_beast_margalard),
    ("doMistforms", do_mistforms),
    ("doMushrooms", do_mushrooms),
    ("doPenguins", do_penguins),
    ("doSailorsDirge", do_sailors_dirge),
    ("doSailorsDirgeDefeated", do_sailors_dirge_defeated),
    ("doSignAeron", do_sign_aeron),
    ("doSignBhall", do_sign_bhall),
    ("doSignCamulos", do_sign_camulos),
    ("doSignDagda", do_sign_dagda),
    ("doSignEsus", do_sign_esus),
    ("doSignLugus", do_sign_lugus),
    ("doSignMulcarn", do_sign_mulcarn),
    ("doSignSirona", do_sign_sirona),
    ("doSignSucellus", do_sign_sucellus),
    ("doSignTali", do_sign_tali),
    ("doSlaveEscape", do_slave_escape),
    ("doSlaveRevolt", do_slave_revolt),
    ("doVolcanoCreation", do_volcano_creation),
];

const EVENT_CAN_DO: &[(&str, EventCanDoFn)] = &[
    ("canApplyDissent4", can_apply_dissent4),
    ("canApplyNoOrder", can_apply_no_order),
    ("canApplyOrderVsVeil4", can_apply_order_vs_veil4),
    ("canApplyTraitAggressive", can_apply_trait_aggressive),
    ("canApplyTraitArcane", can_apply_trait_arcane),
    ("canApplyTraitCharismatic", can_apply_trait_charismatic),
    ("canApplyTraitCreative", can_apply_trait_creative),
    ("canApplyTraitExpansive", can_apply_trait_expansive),
    ("canApplyTraitFinancial", can_apply_trait_financial),
    ("canApplyTraitIndustrious", can_apply_trait_industrious),
    ("canApplyTraitOrganized", can_apply_trait_organized),
    ("canApplyTraitPhilosophical", can_apply_trait_philosophical),
    ("canApplyTraitRaiders", can_apply_trait_raiders),
    ("canApplyTraitSpiritual", can_apply_trait_spiritual),
    ("canDoWBTheBlackTowerPickCivBannor", can_do_wb_the_black_tower_pick_civ_bannor),
    ("canDoWBTheBlackTowerPickCivLjosalfar", can_do_wb_the_black_tower_pick_civ_ljosalfar),
    ("canDoWBTheBlackTowerPickCivLuchuirp", can_do_wb_the_black_tower_pick_civ_luchuirp),
    ("canDoWBTheBlackTowerPickCivSvartalfar", can_do_wb_the_black_tower_pick_civ_svartalfar),
    ("canApplyBlessedSea2", can_apply_blessed_sea2),
    ("canDoWeddingFeud3", can_do_wedding_feud3),
    ("canApplyBardTale3", can_apply_bard_tale3),
    ("canApplyLooters3", can_apply_looters3),
    ("canDoBrothersInNeed1", can_do_brothers_in_need1),
    ("canApplyHurricane1", can_apply_hurricane1),
    ("canApplyHurricane2", can_apply_hurricane2),
    ("canApplyTsunami1", can_apply_tsunami1),
    ("canApplyTsunami2", can_apply_tsunami2),
    ("canApplyVolcano1", can_apply_volcano1),
];

const EVENT_CALLBACK: &[(&str, EventCallbackFn)] = &[
    ("applyAmuriteTrial1", apply_amurite_trial1),
    ("doBanditNietz3", do_bandit_nietz3),
    ("doCalabimSanctuary1", do_calabim_sanctuary1),
    ("doCityFeudStart1", do_city_feud_start1),
    ("doCityFeudStart3", do_city_feud_start3),
    ("doCitySplit1", do_city_split1),
    ("doSoverignCity1", do_soverign_city1),
    ("doDissent1", do_dissent1),
    ("doDissent2", do_dissent2),
    ("applyExploreLairDepths1", apply_explore_lair_depths1),
    ("applyExploreLairDwarfVsLizardmen1", apply_explore_lair_dwarf_vs_lizardmen1),
    ("applyExploreLairDwarfVsLizardmen2", apply_explore_lair_dwarf_vs_lizardmen2),
    ("applyExploreLairPortal1", apply_explore_lair_portal1),
    ("doGovernorAssassination", do_governor_assassination),
    ("doGuildOfTheNineMerc41", do_guild_of_the_nine_merc41),
    ("doGuildOfTheNineMerc51", do_guild_of_the_nine_merc51),
    ("doGuildOfTheNineMerc61", do_guild_of_the_nine_merc61),
    ("doGuildOfTheNineMerc71", do_guild_of_the_nine_merc71),
    ("doGuildOfTheNineMerc81", do_guild_of_the_nine_merc81),
    ("applyHyboremsWhisper1", apply_hyborems_whisper1),
    ("applyHyboremsWhisper2", apply_hyborems_whisper2),
    ("applyHyboremsWhisper3", apply_hyborems_whisper3),
    ("applyIronOrb3", apply_iron_orb3),
    ("doJudgementRight", do_judgement_right),
    ("doJudgementWrong", do_judgement_wrong),
    ("doLetumFrigus3", do_letum_frigus3),
    ("doMachineParts1", do_machine_parts1),
    ("doMachineParts2", do_machine_parts2),
    ("applyMalakimPilgrimage1", apply_malakim_pilgrimage1),
    ("doMarketTheft2", do_market_theft2),
    ("doOrderVsVeil1", do_order_vs_veil1),
    ("doOrderVsVeil2", do_order_vs_veil2),
    ("doOrderVsVeil3", do_order_vs_veil3),
    ("doOrderVsVeilTemple1", do_order_vs_veil_temple1),
    ("doOrderVsVeilTemple2", do_order_vs_veil_temple2),
    ("doOrderVsVeilTemple3", do_order_vs_veil_temple3),
    ("applyParithYes", apply_parith_yes),
    ("doPickAlignment1", do_pick_alignment1),
    ("doPickAlignment2", do_pick_alignment2),
    ("doPickAlignment3", do_pick_alignment3),
    ("doPigGiant3", do_pig_giant3),
    ("applyPronCapria", apply_pron_capria),
    ("applyPronEthne", apply_pron_ethne),
    ("applyPronArendel", apply_pron_arendel),
    ("applyPronThessa", apply_pron_thessa),
    ("applyPronHannah", apply_pron_hannah),
    ("applyPronRhoanna", apply_pron_rhoanna),
    ("applyPronValledia", apply_pron_valledia),
    ("applyPronMahala", apply_pron_mahala),
    ("applyPronKeelyn", apply_pron_keelyn),
    ("applyPronSheelba", apply_pron_sheelba),
    ("applyPronFaeryl", apply_pron_faeryl),
    ("applyPronAlexis", apply_pron_alexis),
    ("applyShrineCamulos2", apply_shrine_camulos2),
    ("doSpiderMine3", do_spider_mine3),
    ("applyTreasure1", apply_treasure1),
    ("doSwitchCivs2", do_switch_civs2),
    ("doVeilVsOrderTemple1", do_veil_vs_order_temple1),
    ("doVeilVsOrderTemple2", do_veil_vs_order_temple2),
    ("doVeilVsOrderTemple3", do_veil_vs_order_temple3),
    ("doTraitAggressive", do_trait_aggressive),
    ("doTraitArcane", do_trait_arcane),
    ("doTraitCharismatic", do_trait_charismatic),
    ("doTraitCreative", do_trait_creative),
    ("doTraitExpansive", do_trait_expansive),
    ("doTraitFinancial", do_trait_financial),
    ("doTraitIndustrious", do_trait_industrious),
    ("doTraitInsane", do_trait_insane),
    ("doTraitOrganized", do_trait_organized),
    ("doTraitPhilosophical", do_trait_philosophical),
    ("doTraitRaiders", do_trait_raiders),
    ("doTraitSpiritual", do_trait_spiritual),
    ("applyWBFallOfCuantineRosier1", apply_wb_fall_of_cuantine_rosier1),
    ("applyWBFallOfCuantineRosier2", apply_wb_fall_of_cuantine_rosier2),
    ("applyWBFallOfCuantineFleeCalabim", apply_wb_fall_of_cuantine_flee_calabim),
    ("applyWBFallOfCuantineFleeMalakim", apply_wb_fall_of_cuantine_flee_malakim),
    ("applyWBGiftOfKylorinMeshabberRight", apply_wb_gift_of_kylorin_meshabber_right),
    ("applyWBGiftOfKylorinMeshabberWrong", apply_wb_gift_of_kylorin_meshabber_wrong),
    ("applyWBGiftOfKylorinSecretDoorYes", apply_wb_gift_of_kylorin_secret_door_yes),
    ("applyWBLordOfTheBalorsTemptJudeccaYes", apply_wb_lord_of_the_balors_tempt_judecca_yes),
    ("applyWBLordOfTheBalorsTemptSallosYes", apply_wb_lord_of_the_balors_tempt_sallos_yes),
    ("applyWBLordOfTheBalorsTemptOuzzaYes", apply_wb_lord_of_the_balors_tempt_ouzza_yes),
    ("applyWBLordOfTheBalorsTemptMeresinYes", apply_wb_lord_of_the_balors_tempt_meresin_yes),
    ("applyWBLordOfTheBalorsTemptStatiusYes", apply_wb_lord_of_the_balors_tempt_statius_yes),
    ("applyWBLordOfTheBalorsTemptLetheYes", apply_wb_lord_of_the_balors_tempt_lethe_yes),
    ("applyWBSplinteredCourtDefeatedAmelanchier3", apply_wb_splintered_court_defeated_amelanchier3),
    ("applyWBSplinteredCourtDefeatedThessa3", apply_wb_splintered_court_defeated_thessa3),
    ("applyWBSplinteredCourtDefeatedRivanna3", apply_wb_splintered_court_defeated_rivanna3),
    ("applyWBSplinteredCourtDefeatedVolanna3", apply_wb_splintered_court_defeated_volanna3),
    ("applyWBSplinteredCourtParithYes", apply_wb_splintered_court_parith_yes),
    ("applyWBTheBlackTowerPickCivBannor", apply_wb_the_black_tower_pick_civ_bannor),
    ("applyWBTheBlackTowerPickCivHippus", apply_wb_the_black_tower_pick_civ_hippus),
    ("applyWBTheBlackTowerPickCivLanun", apply_wb_the_black_tower_pick_civ_lanun),
    ("applyWBTheBlackTowerPickCivLjosalfar", apply_wb_the_black_tower_pick_civ_ljosalfar),
    ("applyWBTheBlackTowerPickCivLuchuirp", apply_wb_the_black_tower_pick_civ_luchuirp),
    ("applyWBTheBlackTowerPickCivSvartalfar", apply_wb_the_black_tower_pick_civ_svartalfar),
    ("applyWBTheMomusBeerisOfferYes", apply_wb_the_momus_beeris_offer_yes),
    ("applyWBTheRadiantGuardChooseSidesBasium", apply_wb_the_radiant_guard_choose_sides_basium),
    ("applyWBTheRadiantGuardChooseSidesHyborem", apply_wb_the_radiant_guard_choose_sides_hyborem),
    ("doWerewolf1", do_werewolf1),
    ("doWerewolf3", do_werewolf3),
    ("applyBlessedSea2", apply_blessed_sea2),
    ("doHolyMountainRevealed", do_holy_mountain_revealed),
    ("doWeddingFeud2", do_wedding_feud2),
    ("doWeddingFeud3", do_wedding_feud3),
    ("doSpicy2", do_spicy2),
    ("applyBardTale3", apply_bard_tale3),
    ("applyLooters3", apply_looters3),
    ("applyHurricane1", apply_hurricane1),
    ("applyTsunami1", apply_tsunami1),
    ("applyTsunami2", apply_tsunami2),
    ("applyVolcano1", apply_volcano1),
];

const EVENT_EXPIRE: &[(&str, EventExpireFn)] = &[("expireHolyMountain1", expire_holy_mountain1)];

// ---------------------------------------------------------------------------------------------
// Helpers (Python idioms, Cy* wrapper behaviour)
// ---------------------------------------------------------------------------------------------

/// `gc.getPlayer(p).getUnit(iID)`: the unit if `p` owns it (C++ unit ids are per player);
/// `None` is the NULL `CyUnit`.
fn player_get_unit(g: &Game, p: PlayerId, id: Option<UnitId>) -> Option<UnitId> {
    let id = id?;
    match g.units.get(id) {
        Some(unit) if unit.owner == p => Some(id),
        _ => None,
    }
}

/// `gc.getPlayer(p).getCity(iID)`: the city if `p` owns it; `None` is the NULL `CyCity`.
fn player_get_city(g: &Game, p: PlayerId, id: Option<CityId>) -> Option<CityId> {
    let id = id?;
    match g.cities.get(id) {
        Some(city) if city.owner == p => Some(id),
        _ => None,
    }
}

/// `gc.getPlayer(kTriggeredData.ePlayer).getUnit(kTriggeredData.iUnitId)`.
fn data_unit(g: &Game, data: &EventTriggeredData) -> Option<UnitId> {
    player_get_unit(g, data.player, data.unit)
}

/// `gc.getPlayer(kTriggeredData.ePlayer).getCity(kTriggeredData.iCityId)`.
fn data_city(g: &Game, data: &EventTriggeredData) -> Option<CityId> {
    player_get_city(g, data.player, data.city)
}

/// `gc.getMap().plot(kTriggeredData.iPlotX, kTriggeredData.iPlotY)`; `None` is the NULL plot.
fn data_plot(g: &Game, data: &EventTriggeredData) -> Option<PlotIdx> {
    g.map.plot_xy(data.plot_x, data.plot_y)
}

/// `(pCity.getX(), pCity.getY())`.
fn city_xy(g: &Game, c: CityId) -> (i32, i32) {
    (g.cities[c].x, g.cities[c].y)
}

/// `pPlayer.initUnit(eUnit, iX, iY, UnitAITypes.NO_UNITAI, DirectionTypes.DIRECTION_SOUTH)`.
fn init_unit(g: &mut Game, p: PlayerId, unit: UnitType, x: i32, y: i32) -> UnitId {
    g.player_init_unit(p, unit, x, y, None, SOUTH)
}

/// `PyPlayer(p).getUnitList()` (PyHelpers): snapshot of the player's units, skipping dead ones.
fn py_unit_list(g: &Game, p: PlayerId) -> Vec<UnitId> {
    g.players[p].units.iter().copied().filter(|&u| !g.unit_is_dead(u)).collect()
}

/// `PyPlayer(p).getCityList()` / `pPlayer.firstCity(false) .. nextCity`: the player's cities.
fn city_list(g: &Game, p: PlayerId) -> Vec<CityId> {
    g.players[p].cities.clone()
}

/// `pPlot.area().getNumTiles()`: the NULL area (a plot without an area id) answers -1 (`CyArea`).
fn plot_area_num_tiles(g: &Game, plot: PlotIdx) -> i32 {
    match g.map.plots[plot].area {
        Some(area) => g.map.areas[area].num_tiles,
        None => -1,
    }
}

/// `for iX in range(x - r, x + r + 1): for iY in range(y - r, y + r + 1): pPlot =
/// CyMap().plot(iX, iY)`: the square's plots, x outer and y inner; `None` is the NULL plot.
/// (Python ints do not overflow: coordinates outside the C `int` range, which only occur around
/// `INVALID_PLOT_COORD`, are off the map.)
fn square(g: &Game, x: i32, y: i32, r: i32) -> Vec<Option<PlotIdx>> {
    let (x, y, r) = (i64::from(x), i64::from(y), i64::from(r));
    let mut v = Vec::new();
    for xx in x - r..x + r + 1 {
        for yy in y - r..y + r + 1 {
            let plot = match (i32::try_from(xx), i32::try_from(yy)) {
                (Ok(xx), Ok(yy)) => g.map.plot_xy(xx, yy),
                _ => None,
            };
            v.push(plot);
        }
    }
    v
}

/// The plots of the 3x3 square around the trigger plot (the center excluded) that have an
/// improvement: `for iDX in range(-1, 2): for iDY in range(-1, 2): loopPlot =
/// plotXY(kTriggeredData.iPlotX, kTriggeredData.iPlotY, iDX, iDY) ...` (Volcano).
fn volcano_plots(g: &Game, data: &EventTriggeredData) -> Vec<PlotIdx> {
    let mut plots = Vec::new();
    for dx in -1..2 {
        for dy in -1..2 {
            if let Some(plot) = g.plot_xy(data.plot_x, data.plot_y, dx, dy) {
                if (dx != 0 || dy != 0) && g.map.plots[plot].improvement_type.is_some() {
                    plots.push(plot);
                }
            }
        }
    }
    plots
}

/// `isLimitedWonderClass(gc.getBuildingInfo(iBuilding).getBuildingClassType())`.
fn is_limited_wonder(d: &GameData, building: BuildingType) -> bool {
    d.buildings[building].building_class_type.map_or(false, |bc| is_limited_wonder_class(d, bc))
}

/// `pPlayer.AI_getAttitude(ePlayer)`: `p`'s attitude towards `other` (CyPlayer forwards to
/// `CvPlayerAI::AI_getAttitude(ePlayer, bForced = true)`, the player-level attitude:
/// `Game::player_ai_get_attitude`, the forced cases of `AI_getAttitudeVal` and then the
/// controller).
fn ai_get_attitude(g: &Game, p: PlayerId, other: PlayerId) -> AttitudeType {
    g.player_ai_get_attitude(p, other, true)
}

/// `iUnit = gc.getInfoTypeForString(..); if CyGame().getUnitCreatedCount(iUnit) == 0:
/// cf.addUnit(iUnit)` (Armageddon horsemen, great beasts).
fn add_unit_once(g: &mut Game, unit: UnitType) {
    if g.unit_created_count[unit] == 0 {
        g.cf_add_unit(unit);
    }
}

/// `bPlayer.initUnit(eUnit, pPlot.getX(), pPlot.getY(), ..)` for the barbarians on the trigger
/// plot, if it is a real plot (`if pPlot.isNone() == False`).
fn barbarian_unit_at_data_plot(g: &mut Game, data: &EventTriggeredData, unit: UnitType) {
    if let Some(plot) = data_plot(g, data) {
        let barbarian = g.barbarian_player();
        let (x, y) = g.map.xy(plot);
        init_unit(g, barbarian, unit, x, y);
    }
}

/// `pUnit.setXY(pCity.getX(), pCity.getY(), false, true, true)`: the event unit goes to the
/// capital of the last player slot of `civ` (`cf.getCivilization`), if there is one.
fn move_to_civ_capital(g: &mut Game, data: &EventTriggeredData, civ: CivilizationType) {
    let unit = data_unit(g, data);
    if let Some(p2) = g.cf_get_civilization(civ) {
        // NULL capital: getX()/getY() are -1 (setXY then moves the unit to CvMap::plot(-1, -1),
        // off the map unless it wraps).
        let (x, y) = g.player_get_capital_city(p2).map_or((-1, -1), |c| city_xy(g, c));
        // NULL unit: setXY is a no-op.
        if let Some(u) = unit {
            g.unit_set_xy(u, x, y, false);
        }
    }
}

/// `if pCity.isHolyCityByType(eReligion) == False:` (the NULL city is no holy city).
fn not_holy(g: &Game, city: Option<CityId>, religion: ReligionType) -> bool {
    !city.map_or(false, |c| g.city_is_holy_city(c, religion))
}

/// `(loopCity, iter) = pPlayer.firstCity(false); while(loopCity): if
/// loopCity.isHasReligion(happy): loopCity.changeHappinessTimer(5); if
/// loopCity.isHasReligion(angry): loopCity.changeHurryAngerTimer(5)` (Order vs Veil events).
fn religion_timers(g: &mut Game, p: PlayerId, happy: ReligionType, angry: ReligionType) {
    for c in city_list(g, p) {
        if g.cities[c].has_religion[happy] {
            g.city_change_happiness_timer(c, 5);
        }
        if g.cities[c].has_religion[angry] {
            g.city_change_hurry_anger_timer(c, 5);
        }
    }
}

/// `pCity.changeOccupationTimer(n)` on the event city (no-op on the NULL city).
fn occupation(g: &mut Game, city: Option<CityId>, change: i32) {
    if let Some(c) = city {
        g.city_change_occupation_timer(c, change);
    }
}

/// `pCity.setHasReligion(eReligion, False, False, False)` on the event city.
fn remove_religion(g: &mut Game, city: Option<CityId>, religion: ReligionType) {
    if let Some(c) = city {
        g.city_set_has_religion(c, religion, false, false, false);
    }
}

/// `pCity.setNumRealBuilding(eBuilding, 0)` on the event city.
fn remove_building(g: &mut Game, city: Option<CityId>, building: BuildingType) {
    if let Some(c) = city {
        g.city_set_num_real_building(c, building, 0);
    }
}

/// `pCity.changePopulation(-1)` on the event city.
fn lose_population(g: &mut Game, city: Option<CityId>) {
    if let Some(c) = city {
        g.city_change_population(c, -1);
    }
}

/// `gc.getLeaderHeadInfo(pPlayer.getLeaderType()).getPermanentTrait()`.
fn permanent_trait(g: &Game, p: PlayerId) -> Option<TraitType> {
    g.player_get_leader_type(p).and_then(|leader| g.d.leader_heads[leader].permanent_trait)
}

/// `canApplyTraitX`: the leader's permanent trait is not `trait_type`.
fn can_apply_trait(g: &Game, data: &EventTriggeredData, trait_type: TraitType) -> bool {
    permanent_trait(g, data.player) != Some(trait_type)
}

/// `doTraitX`: drop every selectable trait but the leader's permanent one, then gain `trait_type`.
fn do_trait(g: &mut Game, data: &EventTriggeredData, trait_type: TraitType) {
    let d = g.d;
    let p = data.player;
    let permanent = permanent_trait(g, p);
    for t in d.traits.keys() {
        if g.players[p].traits[t] && d.traits[t].selectable && permanent != Some(t) {
            g.player_set_has_trait(p, t, false);
        }
    }
    g.player_set_has_trait(p, trait_type, true);
}

/// `applyPronX`: the leader's player (`cf.getLeader`, any slot) likes the event player more.
fn apply_pron(g: &mut Game, data: &EventTriggeredData, leader: LeaderHeadType) {
    let p = data.player;
    if let Some(p2) = g.cf_get_leader(leader) {
        g.players[p2].ai_attitude_extra[p] += 4;
        // UI: cf.addPlayerPopup(TXT_KEY_EVENT_PRON_*_POPUP, iPlayer).
    }
}

/// `canTriggerPronX`: the event player has met the leader's team, and the leader is Friendly
/// towards the event player.
fn can_trigger_pron(g: &Game, data: &EventTriggeredData, leader: LeaderHeadType) -> bool {
    let p = data.player;
    let team = g.players[p].team;
    if let Some(p2) = g.cf_get_leader(leader) {
        let team2 = g.players[p2].team;
        if g.teams[team].has_met[team2] && ai_get_attitude(g, p2, p) == AttitudeType::ATTITUDE_FRIENDLY {
            return true;
        }
    }
    false
}

/// `iCount += CyMap().getArea(i).getNumImprovements(iImp)` over `range(CyMap().getNumAreas())`
/// (`canTriggerUniqueFeatureX`): the number of these improvements on the map, summed over the
/// areas. (C++ reads area slots `0..getNumAreas()`; the Rust map has no slots, so every area is
/// counted: the same areas unless an area was deleted since the last recalculation.)
fn num_improvements(g: &Game, improvement: ImprovementType) -> i32 {
    g.map.areas.iter().map(|(_, area)| area.num_improvements[improvement]).sum()
}

/// `canTriggerUniqueFeatureX` without an extra condition: a human player, and the improvement
/// exists somewhere.
fn can_trigger_unique_feature(g: &Game, data: &EventTriggeredData, improvement: ImprovementType) -> bool {
    if !g.players[data.player].human {
        return false;
    }
    num_improvements(g, improvement) != 0
}

/// The shared tail of `applyWBSplinteredCourtDefeatedX3`: with the last living players of `civ`,
/// Ljosalfar and Svartalfar on teams, `civ`'s team makes peace with `peace_civ`'s team and
/// declares a limited war on `war_civ`'s team (`peace_civ`/`war_civ` are Ljosalfar and
/// Svartalfar in some order).
fn splintered_court_sides(g: &mut Game, civ: CivilizationType, peace_civ: CivilizationType, war_civ: CivilizationType) {
    let d = g.d;
    let ljosalfar = d.t::<CivilizationType>("CIVILIZATION_LJOSALFAR");
    let svartalfar = d.t::<CivilizationType>("CIVILIZATION_SVARTALFAR");
    let (mut civ_team, mut ljosalfar_team, mut svartalfar_team) = (None, None, None);
    for p in g.player_ids() {
        if g.players[p].alive {
            let loop_civ = g.player_get_civilization_type(p);
            let team = g.players[p].team;
            if loop_civ == Some(civ) {
                civ_team = Some(team);
            }
            if loop_civ == Some(ljosalfar) {
                ljosalfar_team = Some(team);
            }
            if loop_civ == Some(svartalfar) {
                svartalfar_team = Some(team);
            }
        }
    }
    if let (Some(team), Some(lj), Some(sv)) = (civ_team, ljosalfar_team, svartalfar_team) {
        let team_of = |c: CivilizationType| if c == ljosalfar { lj } else { sv };
        let (peace_team, war_team) = (team_of(peace_civ), team_of(war_civ));
        if g.teams[team].at_war[peace_team] {
            g.team_make_peace(team, peace_team, true);
        }
        if !g.teams[team].at_war[war_team] {
            g.team_declare_war(team, war_team, false, Some(WarPlanType::WARPLAN_LIMITED));
        }
    }
}

/// `applyWBTheBlackTowerPickCivX`: the city on the trigger plot becomes a city of `civ`
/// (`pPlot.getPlotCity().setCivilizationType(..)`; nothing without a city).
fn black_tower_pick_civ(g: &mut Game, data: &EventTriggeredData, civ: CivilizationType) {
    if let Some(plot) = data_plot(g, data) {
        if let Some(city) = g.map.plots[plot].plot_city {
            g.cities[city].civ = Some(civ);
        }
    }
}

/// `eTeam.setPermanentWarPeace(t, False); eTeam.makePeace(t); eTeam.setPermanentWarPeace(t, True)`
/// (Lord of the Balors temptations).
fn forced_peace(g: &mut Game, team: TeamId, t: TeamId) {
    g.teams[team].permanent_war_peace[t] = false;
    g.team_make_peace(team, t, true);
    g.teams[team].permanent_war_peace[t] = true;
}

/// The building the Blessed Sea events hand out: the first building of special type
/// `SPECIALBUILDING_TEMPLE` and the trigger's religion (`None` for NO_RELIGION or no such
/// building).
fn blessed_sea_temple(g: &Game, data: &EventTriggeredData) -> Option<BuildingType> {
    let d = g.d;
    let religion = data.religion?;
    let temple = d.ty::<SpecialBuildingType>("SPECIALBUILDING_TEMPLE");
    d.buildings
        .keys()
        .find(|&b| d.buildings[b].special_building_type == temple && d.buildings[b].religion_type == Some(religion))
}

/// The Blessed Sea landmass count: `for i in range(map.getIndexAfterLastArea()): area =
/// map.getArea(i); if not area.isNone() and not area.isWater() and
/// area.getCitiesPerPlayer(ePlayer) > 0: iOurLandmasses += 1`.
fn our_landmasses(g: &Game, p: PlayerId) -> i32 {
    g.map.areas.iter().filter(|(_, area)| !area.water && area.cities_per_player[p] > 0).count() as i32
}

/// `gc.getWorldInfo(gc.getMap().getWorldSize()).getDefaultPlayers()`.
fn default_players(g: &Game) -> i32 {
    g.d.worlds[g.setup.world_size].default_players
}

/// `player.getEventOccured(gc.getEventTriggerInfo(kTriggeredData.eTrigger).getPrereqEvent(0))`
/// (Holy Mountain): the plot and religion of that event's trigger data, `None` if it has not
/// occurred (or the trigger lists no prerequisite event).
fn prereq_event_occured(g: &Game, data: &EventTriggeredData) -> Option<(i32, i32, Option<ReligionType>)> {
    let event = *g.d.event_triggers[data.trigger].prereq_events.first()?;
    let orig = g.player_get_event_occured(data.player, event)?;
    Some((orig.plot_x, orig.plot_y, orig.religion))
}

/// `player.getGold() - 10 * player.getNumCities() < 0` is false (Wedding Feud 3, Bard Tale 3).
fn can_pay_ten_per_city(g: &Game, p: PlayerId) -> bool {
    g.players[p].gold - 10 * g.players[p].num_cities() >= 0
}

/// The buildings a disaster can hit in `city`: real ones with a positive cost that are not
/// limited wonders, and with a cost of at most 100 if `cheap` (Looters, Hurricane, Tsunami).
fn destructible_buildings(g: &Game, city: Option<CityId>, cheap: bool) -> Vec<BuildingType> {
    // NULL city: getNumRealBuilding() is -1.
    let Some(city) = city else { return Vec::new() };
    let d = g.d;
    d.buildings
        .keys()
        .filter(|&b| {
            let cost = d.buildings[b].production_cost;
            g.cities[city].num_real_building[b] > 0 && (!cheap || cost <= 100) && cost > 0 && !is_limited_wonder(d, b)
        })
        .collect()
}

/// `iTerrain = pPlot.getTerrainType(); if iTerrain == from: pPlot.setTempTerrainType(to,
/// CyGame().getSorenRandNum(10, "Bob") + 10) ...` (Sign of Bhall / Mulcarn), for each
/// `(from, to)` pair in order.
fn temp_terrain_shift(g: &mut Game, plot: PlotIdx, shifts: &[(TerrainType, TerrainType)]) {
    let terrain = g.map.plots[plot].terrain_type;
    for &(from, to) in shifts {
        if terrain == Some(from) {
            let timer = g.soren_rand_num(10, "Bob") + 10;
            g.plot_set_temp_terrain_type(plot, to, timer);
        }
    }
}

/// Sign of Bhall / Mulcarn: each plot of the event player without feature or improvement on land
/// has a 10% chance (`label`) to shift its terrain temporarily.
fn sign_terrain(g: &mut Game, data: &EventTriggeredData, label: &str, shifts: &[(TerrainType, TerrainType)]) {
    let p = data.player;
    for plot in g.map.plot_ids() {
        if g.map.plots[plot].owner == Some(p)
            && g.map.plots[plot].feature_type.is_none()
            && g.map.plots[plot].improvement_type.is_none()
            && !g.plot_is_water(plot)
            && g.soren_rand_num(100, label) < 10
        {
            temp_terrain_shift(g, plot, shifts);
        }
    }
}

/// Sign of Camulos / Dagda: the event player and every living civ player of another team change
/// their attitude towards each other by `change`.
fn sign_attitude(g: &mut Game, data: &EventTriggeredData, change: i32) {
    let p = data.player;
    let team = g.players[p].team;
    for lp in g.civ_player_ids() {
        if g.players[lp].alive && g.players[lp].team != team {
            g.players[lp].ai_attitude_extra[p] += change;
            g.players[p].ai_attitude_extra[lp] += change;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// FFH2 events (Python lines 22-2503)
// ---------------------------------------------------------------------------------------------

/// `canTriggerAeronsChosen(argsList)`: the event unit has reached level 5.
fn can_trigger_aerons_chosen(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    // NULL unit: getLevel() is -1.
    data_unit(g, data).map_or(-1, |u| g.units[u].level) >= 5
}

/// `canTriggerAmuriteTrialUnit(argsList)`.
fn can_trigger_amurite_trial_unit(g: &Game, _trigger: EventTriggerType, _p: PlayerId, u: UnitId) -> bool {
    !g.unit_is_hidden_nationality(u)
}

/// `applyAmuriteTrial1(argsList)`: the event unit goes to the Amurite capital.
fn apply_amurite_trial1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_AMURITES");
    move_to_civ_capital(g, data, civ);
}

/// `doArmageddonApocalypse(argsList)`: unless the player is Fallow, every city loses half its
/// population (keeping at least 1); each unit has APOCALYPSE_KILL_CHANCE% to die if alive.
fn do_armageddon_apocalypse(g: &mut Game, _data: &EventTriggeredData, p: PlayerId) {
    let d = g.d;
    let percent = d.defines.apocalypse_kill_chance;
    if !g.players[p].traits[d.t::<TraitType>("TRAIT_FALLOW")] {
        for city in city_list(g, p) {
            let mut pop = py_div(g.cities[city].population, 2);
            if pop == 0 {
                pop = 1;
            }
            g.city_set_population(city, pop);
        }
    }
    for u in py_unit_list(g, p) {
        if g.soren_rand_num(100, "Apocalypse") <= percent && g.units.contains(u) && g.unit_is_alive(u) {
            g.unit_kill(u, false, None);
        }
    }
    if g.players[p].human {
        let t = "TROPHY_FEAT_APOCALYPSE";
        if !g.is_has_trophy(t) {
            g.change_trophy_value(t, 1);
        }
    }
}

/// `doArmageddonArs(argsList)`.
fn do_armageddon_ars(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    let unit = g.d.t::<UnitType>("UNIT_ARS");
    add_unit_once(g, unit);
}

/// `doArmageddonBlight(argsList)`: non-Infernal cities get unhealthiness; every living unit
/// takes 25% death damage.
fn do_armageddon_blight(g: &mut Game, _data: &EventTriggeredData, p: PlayerId) {
    let d = g.d;
    if g.player_get_civilization_type(p) != Some(d.t::<CivilizationType>("CIVILIZATION_INFERNAL")) {
        for city in city_list(g, p) {
            // The roll is drawn and then overwritten (`i = 10`).
            g.soren_rand_num(15, "Blight");
            let mut i = 10;
            i += g.cities[city].population;
            i -= g.city_total_good_building_health(city);
            if i > 0 {
                g.cities[city].espionage_health_counter += i;
            }
        }
    }
    let death = d.t::<DamageType>("DAMAGE_DEATH");
    for u in py_unit_list(g, p) {
        if g.units.contains(u) && g.unit_is_alive(u) {
            g.unit_do_damage(u, 25, 100, None, Some(death), false);
        }
    }
}

/// `doArmageddonBuboes(argsList)`.
fn do_armageddon_buboes(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    let unit = g.d.t::<UnitType>("UNIT_BUBOES");
    add_unit_once(g, unit);
}

/// `doArmageddonHellfire(argsList)`: once (for player 0), each empty unimproved-or-impermanent
/// flat land plot without bonus or city has HELLFIRE_CHANCE/10000 to become Hellfire with a
/// demon Champion of the Infernals (the last living Infernal player, else the barbarians).
fn do_armageddon_hellfire(g: &mut Game, _data: &EventTriggeredData, p: PlayerId) {
    if p != PlayerId(0) {
        return;
    }
    let d = g.d;
    let champion = d.t::<UnitType>("UNIT_CHAMPION");
    let demon = d.t::<PromotionType>("PROMOTION_DEMON");
    let hellfire = d.t::<ImprovementType>("IMPROVEMENT_HELLFIRE");
    let hellfire_chance = d.defines.hellfire_chance;
    let infernal = d.t::<CivilizationType>("CIVILIZATION_INFERNAL");
    let mut player = g.barbarian_player();
    for p2 in g.player_ids() {
        if g.players[p2].alive && g.player_get_civilization_type(p2) == Some(infernal) {
            player = p2;
        }
    }
    for plot in g.map.plot_ids() {
        if !g.plot_is_water(plot)
            && g.map.plots[plot].units.is_empty()
            && g.map.plots[plot].plot_city.is_none()
            && g.plot_is_flatlands(plot)
            && g.map.plots[plot].bonus_type.is_none()
            && g.soren_rand_num(10000, "Hellfire") <= hellfire_chance
        {
            let mut valid = true;
            if let Some(improvement) = g.map.plots[plot].improvement_type {
                if d.improvements[improvement].permanent {
                    valid = false;
                }
            }
            if valid {
                g.plot_set_improvement_type(plot, Some(hellfire));
                let (x, y) = g.map.xy(plot);
                let unit = init_unit(g, player, champion, x, y);
                g.unit_set_has_promotion(unit, demon, true);
            }
        }
    }
}

/// `doArmageddonPestilence(argsList)`: like Blight for `kTriggeredData.ePlayer`, with a random
/// 0..8 plus a quarter of the population as unhealthiness (possibly negative).
fn do_armageddon_pestilence(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let d = g.d;
    let p = data.player;
    if g.player_get_civilization_type(p) != Some(d.t::<CivilizationType>("CIVILIZATION_INFERNAL")) {
        for city in city_list(g, p) {
            let mut i = g.soren_rand_num(9, "Pestilence");
            i += py_div(g.cities[city].population, 4);
            i -= g.city_total_good_building_health(city);
            g.cities[city].espionage_health_counter += i;
        }
    }
    let death = d.t::<DamageType>("DAMAGE_DEATH");
    for u in py_unit_list(g, p) {
        if g.units.contains(u) && g.unit_is_alive(u) {
            g.unit_do_damage(u, 25, 100, None, Some(death), false);
        }
    }
}

/// `doArmageddonStephanos(argsList)`.
fn do_armageddon_stephanos(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    let unit = g.d.t::<UnitType>("UNIT_STEPHANOS");
    add_unit_once(g, unit);
}

/// `doArmageddonWrath(argsList)`: Wrath appears; each living land unit of the player that is
/// not a world unit becomes Enraged with WRATH_CONVERT_CHANCE%.
fn do_armageddon_wrath(g: &mut Game, _data: &EventTriggeredData, p: PlayerId) {
    let d = g.d;
    let enraged = d.t::<PromotionType>("PROMOTION_ENRAGED");
    let wrath = d.t::<UnitType>("UNIT_WRATH");
    let convert_chance = d.defines.wrath_convert_chance;
    add_unit_once(g, wrath);
    for u in py_unit_list(g, p) {
        if g.units.contains(u)
            && g.unit_get_domain_type(u) == DomainType::DOMAIN_LAND
            && g.unit_is_alive(u)
            && g.soren_rand_num(100, "Wrath") < convert_chance
            && !is_world_unit_class(d, g.unit_get_unit_class_type(u))
        {
            g.unit_set_has_promotion(u, enraged, true);
        }
    }
}

/// `doArmageddonYersinia(argsList)`.
fn do_armageddon_yersinia(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    let unit = g.d.t::<UnitType>("UNIT_YERSINIA");
    add_unit_once(g, unit);
}

/// `doAzer(argsList)`.
fn do_azer(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let azer = g.d.t::<UnitType>("UNIT_AZER");
    barbarian_unit_at_data_plot(g, data, azer);
}

/// `doBanditNietz3(argsList)`: Nietz the Bandit Lord, a heroic horseman, joins in the city.
fn do_bandit_nietz3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    // NULL city: initUnit at (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let (x, y) = city_xy(g, city);
    let unit = init_unit(g, data.player, d.t::<UnitType>("UNIT_HORSEMAN"), x, y);
    g.unit_set_name(unit, "Nietz the Bandit Lord".to_string());
    g.unit_set_has_promotion(unit, d.t::<PromotionType>("PROMOTION_HERO"), true);
    g.unit_set_has_promotion(unit, d.t::<PromotionType>("PROMOTION_MOBILITY1"), true);
}

/// `doCalabimSanctuary1(argsList)`: the Calabim like the player less.
fn do_calabim_sanctuary1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_CALABIM");
    if let Some(p2) = g.cf_get_civilization(civ) {
        g.players[p2].ai_attitude_extra[data.player] -= 4;
    }
}

/// `canTriggerCityFeud(argsList)`.
fn can_trigger_city_feud(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    !g.city_is_capital(c)
}

/// `doCityFeudArson(argsList)`: fire in the capital.
fn do_city_feud_arson(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    // NULL capital: doCityFire finds no building, so it draws nothing (only its message).
    if let Some(capital) = g.player_get_capital_city(data.player) {
        g.cf_do_city_fire(capital);
    }
}

/// `doCityFeudStart1(argsList)`.
fn do_city_feud_start1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    if let Some(capital) = g.player_get_capital_city(data.player) {
        g.city_change_happiness_timer(capital, 5);
    }
}

/// `doCityFeudStart3(argsList)`.
fn do_city_feud_start3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    if let Some(capital) = g.player_get_capital_city(data.player) {
        g.city_change_occupation_timer(capital, 5);
    }
}

/// `canTriggerCitySplit(argsList)`: not the capital, a free player slot, not barbarian-trait
/// and no Koun in the game.
fn can_trigger_city_split(g: &Game, _trigger: EventTriggerType, p: PlayerId, c: CityId) -> bool {
    let d = g.d;
    if g.city_is_capital(c) {
        return false;
    }
    if g.cf_get_open_player().is_none() {
        return false;
    }
    // `CyGame().getWBMapScript()` is always false.
    if g.players[p].traits[d.t::<TraitType>("TRAIT_BARBARIAN")] {
        return false;
    }
    let koun = d.t::<LeaderHeadType>("LEADER_KOUN");
    if g.player_ids().any(|lp| g.player_get_leader_type(lp) == Some(koun)) {
        return false;
    }
    true
}

/// `doCitySplit1(argsList)`: the city secedes as a new Koun empire of the same civilization.
fn do_city_split1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    // NULL city: formEmpire would search around the NULL plot's (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let Some(civ) = g.player_get_civilization_type(p) else { return };
    let koun = g.d.t::<LeaderHeadType>("LEADER_KOUN");
    let alignment = g.players[p].alignment;
    g.cf_form_empire(civ, koun, None, city, alignment, None);
}

/// `doSoverignCity1(argsList)`: the city becomes a Koun empire on the player's team, with its techs.
fn do_soverign_city1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    // NULL city: formEmpire would search around the NULL plot's (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let Some(civ) = g.player_get_civilization_type(p) else { return };
    let koun = g.d.t::<LeaderHeadType>("LEADER_KOUN");
    let team = g.players[p].team;
    let alignment = g.players[p].alignment;
    g.cf_form_empire(civ, koun, Some(team), city, alignment, Some(p));
}

/// `doDissent1(argsList)`.
fn do_dissent1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let city = data_city(g, data);
    if g.soren_rand_num(100, "Dissent 1") < 50 {
        occupation(g, city, 2);
    }
}

/// `doDissent2(argsList)`.
fn do_dissent2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let city = data_city(g, data);
    if g.soren_rand_num(100, "Dissent 2") < 50 {
        occupation(g, city, 4);
    } else if let Some(c) = city {
        g.city_change_happiness_timer(c, 5);
    }
}

/// `canApplyDissent4(argsList)`: the player runs Social Order.
fn can_apply_dissent4(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let d = g.d;
    let cultural_values = d.t::<CivicOptionType>("CIVICOPTION_CULTURAL_VALUES");
    g.players[data.player].civics[cultural_values] == Some(d.t::<CivicType>("CIVIC_SOCIAL_ORDER"))
}

/// `applyExploreLairDepths1(argsList)`: a big bad or big good lair result, 50/50.
fn apply_explore_lair_depths1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let unit = data_unit(g, data);
    let rnd = g.soren_rand_num(100, "Explore Lair");
    // NULL unit: the lair code raises on its NULL plot / gc.getPlayer(-1).
    let Some(u) = unit else { return };
    if rnd < 50 {
        g.cf_explore_lair_big_bad(u);
    }
    if rnd >= 50 {
        g.cf_explore_lair_big_good(u);
    }
}

/// The shared body of `applyExploreLairDwarfVsLizardmen1/2`: the barbarians' Lizardmen (poisoned
/// blades with Hunting) and Dwarven Axemen (bronze weapons with Bronze Working) appear next to the
/// event unit; with `player_dwarves` the player gets 2 Axemen on the unit's plot and the
/// barbarians 3 Lizardmen on a clear adjacent plot, otherwise the player gets 2 Lizardmen and the
/// barbarians 3 Axemen.
fn explore_lair_dwarf_vs_lizardmen(g: &mut Game, data: &EventTriggeredData, player_dwarves: bool) {
    let d = g.d;
    let p = data.player;
    let barbarian = g.barbarian_player();
    let bronze = g.player_is_has_tech(barbarian, d.t::<TechType>("TECH_BRONZE_WORKING"));
    let poison = g.player_is_has_tech(barbarian, d.t::<TechType>("TECH_HUNTING"));
    // NULL unit: plot() is the NULL plot and findClearPlot would search around (-1, -1).
    let Some(u) = data_unit(g, data) else { return };
    let plot = g.unit_plot(u);
    let Some(new_plot) = g.cf_find_clear_plot(None, plot) else { return };
    let lizardman = d.t::<UnitType>("UNIT_LIZARDMAN");
    let axeman = d.t::<UnitType>("UNIT_AXEMAN");
    let poisoned_blade = d.t::<PromotionType>("PROMOTION_POISONED_BLADE");
    let dwarf = d.t::<PromotionType>("PROMOTION_DWARF");
    let bronze_weapons = d.t::<PromotionType>("PROMOTION_BRONZE_WEAPONS");
    let (x, y) = g.map.xy(plot);
    let (nx, ny) = g.map.xy(new_plot);
    if player_dwarves {
        let lizardmen = [
            init_unit(g, barbarian, lizardman, nx, ny),
            init_unit(g, barbarian, lizardman, nx, ny),
            init_unit(g, barbarian, lizardman, nx, ny),
        ];
        if poison {
            for unit in lizardmen {
                g.unit_set_has_promotion(unit, poisoned_blade, true);
            }
        }
        let axeman4 = init_unit(g, p, axeman, x, y);
        g.unit_set_has_promotion(axeman4, dwarf, true);
        let axeman5 = init_unit(g, p, axeman, x, y);
        g.unit_set_has_promotion(axeman5, dwarf, true);
        if bronze {
            g.unit_set_has_promotion(axeman4, bronze_weapons, true);
            g.unit_set_has_promotion(axeman5, bronze_weapons, true);
        }
    } else {
        let lizardmen = [init_unit(g, p, lizardman, x, y), init_unit(g, p, lizardman, x, y)];
        if poison {
            for unit in lizardmen {
                g.unit_set_has_promotion(unit, poisoned_blade, true);
            }
        }
        let mut axemen = Vec::new();
        for _ in 0..3 {
            let unit = init_unit(g, barbarian, axeman, nx, ny);
            g.unit_set_has_promotion(unit, dwarf, true);
            axemen.push(unit);
        }
        if bronze {
            for unit in axemen {
                g.unit_set_has_promotion(unit, bronze_weapons, true);
            }
        }
    }
}

/// `applyExploreLairDwarfVsLizardmen1(argsList)`: the player gets the dwarves.
fn apply_explore_lair_dwarf_vs_lizardmen1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    explore_lair_dwarf_vs_lizardmen(g, data, true);
}

/// `applyExploreLairDwarfVsLizardmen2(argsList)`: the player gets the lizardmen.
fn apply_explore_lair_dwarf_vs_lizardmen2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    explore_lair_dwarf_vs_lizardmen(g, data, false);
}

/// `applyExploreLairPortal1(argsList)`: the event unit is teleported to a random empty land plot
/// (unowned plots preferred).
fn apply_explore_lair_portal1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let unit = data_unit(g, data);
    let mut best_value = 0;
    let mut best_plot = None;
    for plot in g.map.plot_ids() {
        if !g.plot_is_water(plot) && !g.plot_is_peak(plot) && g.map.plots[plot].units.is_empty() {
            let mut value = g.soren_rand_num(1000, "Portal");
            if !g.plot_is_owned(plot) {
                value += 1000;
            }
            if value > best_value {
                best_value = value;
                best_plot = Some(plot);
            }
        }
    }
    if let Some(plot) = best_plot {
        // NULL unit: setXY is a no-op.
        if let Some(u) = unit {
            let (x, y) = g.map.xy(plot);
            g.unit_set_xy(u, x, y, false);
        }
    }
}

/// `doFlareEntropyNode(argsList)`: +100 plot counter around the node, +2 Armageddon counter.
fn do_flare_entropy_node(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    // UI: EFFECT_SPELL1, AS3D_SPELL_DEFILE.
    for plot in square(g, data.plot_x, data.plot_y, 1).into_iter().flatten() {
        g.plot_change_plot_counter(plot, 100);
    }
    g.change_global_counter(2);
}

/// `doFlareFireNode(argsList)`: forests and jungles around the node catch fire.
fn do_flare_fire_node(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let d = g.d;
    // UI: EFFECT_ARTILLERY_SHELL_EXPLODE, AS3D_UN_GRENADE_EXPLODE, TXT_KEY_MESSAGE_FLAMES.
    let flames = d.t::<FeatureType>("FEATURE_FLAMES");
    let forest = d.t::<FeatureType>("FEATURE_FOREST");
    let jungle = d.t::<FeatureType>("FEATURE_JUNGLE");
    for plot in square(g, data.plot_x, data.plot_y, 1).into_iter().flatten() {
        let feature = g.map.plots[plot].feature_type;
        if feature == Some(forest) || feature == Some(jungle) {
            g.plot_set_feature_type(plot, Some(flames));
        }
    }
}

/// `doFlareLifeNode(argsList)`: -100 plot counter in a 5x5 square, -2 Armageddon counter.
fn do_flare_life_node(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    // UI: EFFECT_SPELL1, AS3D_SPELL_SANCTIFY.
    for plot in square(g, data.plot_x, data.plot_y, 2).into_iter().flatten() {
        g.plot_change_plot_counter(plot, -100);
    }
    g.change_global_counter(-2);
}

/// `doFlareNatureNode(argsList)`: new forest on bare land around the node.
fn do_flare_nature_node(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    // UI: EFFECT_BLOOM, AS3D_SPELL_BLOOM.
    let forest_new = g.d.t::<FeatureType>("FEATURE_FOREST_NEW");
    for plot in square(g, data.plot_x, data.plot_y, 1).into_iter().flatten() {
        if g.map.plots[plot].improvement_type.is_none()
            && g.map.plots[plot].feature_type.is_none()
            && !g.plot_is_water(plot)
            && !g.plot_is_peak(plot)
        {
            g.plot_set_feature_type(plot, Some(forest_new));
        }
    }
}

/// `doFlareWaterNode(argsList)`: desert turns to plains, flames and smoke go out around the node.
fn do_flare_water_node(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let d = g.d;
    // UI: EFFECT_SPRING, AS3D_SPELL_SPRING.
    let flames = d.t::<FeatureType>("FEATURE_FLAMES");
    let desert = d.t::<TerrainType>("TERRAIN_DESERT");
    let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
    let plains = d.t::<TerrainType>("TERRAIN_PLAINS");
    for plot in square(g, data.plot_x, data.plot_y, 1).into_iter().flatten() {
        if g.map.plots[plot].terrain_type == Some(desert) {
            g.plot_set_terrain_type(plot, plains, true);
        }
        if g.map.plots[plot].feature_type == Some(flames) {
            g.plot_set_feature_type(plot, None);
        }
        if g.map.plots[plot].improvement_type == Some(smoke) {
            g.plot_set_improvement_type(plot, None);
        }
    }
}

/// `canTriggerPlotEmpty(argsList)`.
fn can_trigger_plot_empty(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    match data_plot(g, data) {
        None => false,
        Some(plot) => g.map.plots[plot].units.is_empty(),
    }
}

/// `canTriggerFoodSicknessUnit(argsList)`.
fn can_trigger_food_sickness_unit(g: &Game, _trigger: EventTriggerType, _p: PlayerId, u: UnitId) -> bool {
    g.unit_is_alive(u)
}

/// `doFoodSickness(argsList)`: +20 damage (at most 99) and 2 turns immobile.
fn do_food_sickness(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    // NULL unit: setDamage and changeImmobileTimer are no-ops.
    let Some(u) = data_unit(g, data) else { return };
    let mut damage = g.units[u].damage + 20;
    if damage > 99 {
        damage = 99;
    }
    g.unit_set_damage(u, damage, None);
    g.unit_change_immobile_timer(u, 2);
}

/// `doFrostling(argsList)`.
fn do_frostling(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let frostling = g.d.t::<UnitType>("UNIT_FROSTLING");
    barbarian_unit_at_data_plot(g, data, frostling);
}

/// `doGodslayer(argsList)`.
fn do_godslayer(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let godslayer = g.d.t::<UnitType>("EQUIPMENT_GODSLAYER");
    g.cf_place_treasure(data.player, godslayer);
}

/// `doGovernorAssassination(argsList)`: the people approve (3 turns happy) if the chosen
/// punishment matches the government (God King: any; Aristocracy: 1; City States or Republic:
/// 3; Theocracy: 4), else 3 turns of hurry anger; nothing under Despotism.
fn do_governor_assassination(g: &mut Game, event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    let mut is_match = false;
    let civic = g.players[data.player].civics[d.t::<CivicOptionType>("CIVICOPTION_GOVERNMENT")];
    let is_civic = |name: &str| civic == Some(d.t::<CivicType>(name));
    if !is_civic("CIVIC_DESPOTISM") {
        if is_civic("CIVIC_GOD_KING") {
            is_match = true;
        }
        if is_civic("CIVIC_ARISTOCRACY") && event == d.t::<EventType>("EVENT_GOVERNOR_ASSASSINATION_1") {
            is_match = true;
        }
        if (is_civic("CIVIC_CITY_STATES") || is_civic("CIVIC_REPUBLIC"))
            && event == d.t::<EventType>("EVENT_GOVERNOR_ASSASSINATION_3")
        {
            is_match = true;
        }
        if is_civic("CIVIC_THEOCRACY") && event == d.t::<EventType>("EVENT_GOVERNOR_ASSASSINATION_4") {
            is_match = true;
        }
        if let Some(c) = city {
            if is_match {
                g.city_change_happiness_timer(c, 3);
            } else {
                g.city_change_hurry_anger_timer(c, 3);
            }
        }
    }
}

/// `doGuildOfTheNineMerc41(argsList)`: elven mercenary, longbowman and ranger.
fn do_guild_of_the_nine_merc41(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let p = data.player;
    // NULL city: initUnit at (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let (x, y) = city_xy(g, city);
    let elf = d.t::<PromotionType>("PROMOTION_ELF");
    let unit = init_unit(g, p, d.t::<UnitType>("UNIT_MERCENARY"), x, y);
    g.unit_set_has_promotion(unit, elf, true);
    g.unit_set_has_promotion(unit, d.t::<PromotionType>("PROMOTION_WOODSMAN1"), true);
    let unit2 = init_unit(g, p, d.t::<UnitType>("UNIT_LONGBOWMAN"), x, y);
    g.unit_set_has_promotion(unit2, elf, true);
    g.unit_set_has_promotion(unit2, d.t::<PromotionType>("PROMOTION_DEXTEROUS"), true);
    let unit3 = init_unit(g, p, d.t::<UnitType>("UNIT_RANGER"), x, y);
    g.unit_set_has_promotion(unit3, elf, true);
    g.unit_set_has_promotion(unit3, d.t::<PromotionType>("PROMOTION_SINISTER"), true);
}

/// `canTriggerGuildOfTheNineMerc5(argsList)`.
fn can_trigger_guild_of_the_nine_merc5(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    g.city_is_coastal(c, 10)
}

/// `doGuildOfTheNineMerc51(argsList)`: amphibious mercenary, boarding party, hidden privateer.
fn do_guild_of_the_nine_merc51(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let p = data.player;
    // NULL city: initUnit at (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let (x, y) = city_xy(g, city);
    let unit = init_unit(g, p, d.t::<UnitType>("UNIT_MERCENARY"), x, y);
    g.unit_set_has_promotion(unit, d.t::<PromotionType>("PROMOTION_AMPHIBIOUS"), true);
    init_unit(g, p, d.t::<UnitType>("UNIT_BOARDING_PARTY"), x, y);
    let unit3 = init_unit(g, p, d.t::<UnitType>("UNIT_PRIVATEER"), x, y);
    g.unit_set_has_promotion(unit3, d.t::<PromotionType>("PROMOTION_HIDDEN_NATIONALITY"), true);
}

/// `doGuildOfTheNineMerc61(argsList)`: mutated mercenary, taskmaster, hunter.
fn do_guild_of_the_nine_merc61(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let p = data.player;
    // NULL city: initUnit at (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let (x, y) = city_xy(g, city);
    let unit = init_unit(g, p, d.t::<UnitType>("UNIT_MERCENARY"), x, y);
    g.unit_set_has_promotion(unit, d.t::<PromotionType>("PROMOTION_MUTATED"), true);
    init_unit(g, p, d.t::<UnitType>("UNIT_TASKMASTER"), x, y);
    init_unit(g, p, d.t::<UnitType>("UNIT_HUNTER"), x, y);
    // Graphics: newUnit3.setUnitArtStyleType(UNIT_ARTSTYLE_BALSERAPHS).
}

/// `doGuildOfTheNineMerc71(argsList)`: dwarven mercenary, champion and cannon.
fn do_guild_of_the_nine_merc71(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let p = data.player;
    // NULL city: initUnit at (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let (x, y) = city_xy(g, city);
    let dwarf = d.t::<PromotionType>("PROMOTION_DWARF");
    let unit = init_unit(g, p, d.t::<UnitType>("UNIT_MERCENARY"), x, y);
    g.unit_set_has_promotion(unit, d.t::<PromotionType>("PROMOTION_DEFENSIVE"), true);
    g.unit_set_has_promotion(unit, dwarf, true);
    let unit2 = init_unit(g, p, d.t::<UnitType>("UNIT_CHAMPION"), x, y);
    g.unit_set_has_promotion(unit2, dwarf, true);
    init_unit(g, p, d.t::<UnitType>("UNIT_DWARVEN_CANNON"), x, y);
}

/// `doGuildOfTheNineMerc81(argsList)`: orcish mercenary, ogre, lizardman.
fn do_guild_of_the_nine_merc81(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let p = data.player;
    // NULL city: initUnit at (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let (x, y) = city_xy(g, city);
    let unit = init_unit(g, p, d.t::<UnitType>("UNIT_MERCENARY"), x, y);
    g.unit_set_has_promotion(unit, d.t::<PromotionType>("PROMOTION_ORC"), true);
    init_unit(g, p, d.t::<UnitType>("UNIT_OGRE"), x, y);
    init_unit(g, p, d.t::<UnitType>("UNIT_LIZARDMAN"), x, y);
}

/// `doGreatBeastGurid(argsList)`.
fn do_great_beast_gurid(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    let unit = g.d.t::<UnitType>("UNIT_GURID");
    add_unit_once(g, unit);
}

/// `doGreatBeastLeviathan(argsList)`: the Leviathan appears on an empty water plot (random
/// 0..500 + 10 per tile of the plot's area).
fn do_great_beast_leviathan(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    let unit = g.d.t::<UnitType>("UNIT_LEVIATHAN");
    if g.unit_created_count[unit] == 0 {
        let mut best_plot = None;
        let mut best_value = -1;
        for plot in g.map.plot_ids() {
            let mut value = -1;
            if g.plot_is_water(plot) && g.map.plots[plot].units.is_empty() {
                value = g.soren_rand_num(500, "Leviathan");
                value += plot_area_num_tiles(g, plot) * 10;
            }
            if value > best_value {
                best_value = value;
                best_plot = Some(plot);
            }
        }
        if best_value != -1 {
            if let Some(plot) = best_plot {
                let barbarian = g.barbarian_player();
                let (x, y) = g.map.xy(plot);
                init_unit(g, barbarian, unit, x, y);
            }
        }
    }
}

/// `doGreatBeastMargalard(argsList)`.
fn do_great_beast_margalard(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    let unit = g.d.t::<UnitType>("UNIT_MARGALARD");
    add_unit_once(g, unit);
}

/// `applyHyboremsWhisper1/2/3`: the player takes the `num`-th best Ashen Veil city.
fn hyborems_whisper(g: &mut Game, data: &EventTriggeredData, num: i32) {
    // No such city: getAshenVeilCity returns -1 and acquireCity(-1, ..) raises.
    if let Some(city) = g.cf_get_ashen_veil_city(num) {
        g.player_acquire_city(data.player, city, false, false, true);
    }
}

/// `applyHyboremsWhisper1(argsList)`.
fn apply_hyborems_whisper1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    hyborems_whisper(g, data, 1);
}

/// `applyHyboremsWhisper2(argsList)`.
fn apply_hyborems_whisper2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    hyborems_whisper(g, data, 2);
}

/// `applyHyboremsWhisper3(argsList)`.
fn apply_hyborems_whisper3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    hyborems_whisper(g, data, 3);
}

/// `applyIronOrb3(argsList)`: one free tech (`chooseTech(1, text, True)`; text and bFront are UI).
/// `CvPlayer::chooseTech` only queues the tech-chooser popup, and `CvPlayer::addPopup` deletes
/// the popups of non-human players: an AI player gets nothing (unlike the DLL's own free techs,
/// which call `AI_chooseFreeTech` for AI players).
fn apply_iron_orb3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    if g.players[data.player].human {
        g.player_choose_tech(data.player, 1);
    }
}

/// `doJudgementRight(argsList)`.
fn do_judgement_right(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    if let Some(c) = data_city(g, data) {
        g.city_change_happiness_timer(c, 10);
    }
}

/// `doJudgementWrong(argsList)`.
fn do_judgement_wrong(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    if let Some(c) = data_city(g, data) {
        g.cities[c].crime += 3;
    }
}

/// `doLetumFrigus3(argsList)`.
fn do_letum_frigus3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let aggressive = g.d.t::<TraitType>("TRAIT_AGGRESSIVE");
    g.player_set_has_trait(data.player, aggressive, true);
}

/// `canTriggerLunaticCity(argsList)`: the city has the state religion's temple.
fn can_trigger_lunatic_city(g: &Game, _trigger: EventTriggerType, p: PlayerId, c: CityId) -> bool {
    const TEMPLES: [(&str, &str); 6] = [
        ("RELIGION_THE_ORDER", "BUILDING_TEMPLE_OF_THE_ORDER"),
        ("RELIGION_FELLOWSHIP_OF_LEAVES", "BUILDING_TEMPLE_OF_LEAVES"),
        ("RELIGION_THE_ASHEN_VEIL", "BUILDING_TEMPLE_OF_THE_VEIL"),
        ("RELIGION_OCTOPUS_OVERLORDS", "BUILDING_TEMPLE_OF_THE_OVERLORDS"),
        ("RELIGION_RUNES_OF_KILMORPH", "BUILDING_TEMPLE_OF_KILMORPH"),
        ("RELIGION_THE_EMPYREAN", "BUILDING_TEMPLE_OF_THE_EMPYREAN"),
    ];
    let d = g.d;
    let religion = g.player_get_state_religion(p);
    let mut temple = None;
    for (r, b) in TEMPLES {
        if religion == Some(d.t::<ReligionType>(r)) {
            temple = Some(d.t::<BuildingType>(b));
        }
    }
    let Some(temple) = temple else { return false };
    g.cities[c].num_real_building[temple] != 0
}

/// `doMachineParts1(argsList)`: a weak Clockwork Golem.
fn do_machine_parts1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let weak = g.d.t::<PromotionType>("PROMOTION_WEAK");
    clockwork_golem(g, data, weak);
}

/// `doMachineParts2(argsList)`: a strong Clockwork Golem.
fn do_machine_parts2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let strong = g.d.t::<PromotionType>("PROMOTION_STRONG");
    clockwork_golem(g, data, strong);
}

/// `doMachineParts1/2`: a Clockwork Golem with `promotion` in the event city.
fn clockwork_golem(g: &mut Game, data: &EventTriggeredData, promotion: PromotionType) {
    // NULL city: initUnit at (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let (x, y) = city_xy(g, city);
    let golem = g.d.t::<UnitType>("UNIT_CLOCKWORK_GOLEM");
    let unit = init_unit(g, data.player, golem, x, y);
    g.unit_set_has_promotion(unit, promotion, true);
}

/// `applyMalakimPilgrimage1(argsList)`: the event unit goes to the Malakim capital.
fn apply_malakim_pilgrimage1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_MALAKIM");
    move_to_civ_capital(g, data, civ);
}

/// `doMarketTheft2(argsList)`: city crime changes by -10..10.
fn do_market_theft2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let city = data_city(g, data);
    let rnd = g.soren_rand_num(21, "Market Theft 2") - 10;
    if let Some(c) = city {
        g.cities[c].crime += rnd;
    }
}

/// `canTriggerMerchantKeep(argsList)`.
fn can_trigger_merchant_keep(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    g.cities[c].specialist_count[g.d.t::<SpecialistType>("SPECIALIST_GREAT_MERCHANT")] != 0
}

/// `doMistforms(argsList)`: three barbarian Mistforms on the trigger plot.
fn do_mistforms(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    // NULL plot: getX()/getY() are -1 and initUnit would place the units off the map.
    let Some(plot) = data_plot(g, data) else { return };
    let barbarian = g.barbarian_player();
    let mistform = g.d.t::<UnitType>("UNIT_MISTFORM");
    let (x, y) = g.map.xy(plot);
    for _ in 0..3 {
        init_unit(g, barbarian, mistform, x, y);
    }
}

/// `doMushrooms(argsList)`.
fn do_mushrooms(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    if let Some(plot) = data_plot(g, data) {
        let mushrooms = g.d.t::<ImprovementType>("IMPROVEMENT_MUSHROOMS");
        g.plot_set_improvement_type(plot, Some(mushrooms));
    }
}

/// `canTriggerMutateUnit(argsList)`.
fn can_trigger_mutate_unit(g: &Game, _trigger: EventTriggerType, _p: PlayerId, u: UnitId) -> bool {
    !g.units[u].has_promotion[g.d.t::<PromotionType>("PROMOTION_MUTATED")]
}

/// `canApplyNoOrder(argsList)`.
fn can_apply_no_order(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    g.player_get_state_religion(data.player) != Some(g.d.t::<ReligionType>("RELIGION_THE_ORDER"))
}

/// `doOrderVsVeil1(argsList)`: side with the Order.
fn do_order_vs_veil1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    occupation(g, city, 1);
    let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
    let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
    if not_holy(g, city, veil) && g.soren_rand_num(100, "Order vs Veil 1") < 50 {
        remove_religion(g, city, veil);
    }
    religion_timers(g, data.player, order, veil);
}

/// `doOrderVsVeil2(argsList)`: side with the Veil.
fn do_order_vs_veil2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    occupation(g, city, 1);
    let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
    let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
    if not_holy(g, city, order) && g.soren_rand_num(100, "Order vs Veil 2") < 50 {
        remove_religion(g, city, order);
    }
    religion_timers(g, data.player, veil, order);
}

/// `doOrderVsVeil3(argsList)`: let them fight.
fn do_order_vs_veil3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    occupation(g, city, 3);
    let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
    let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
    if not_holy(g, city, veil) && g.soren_rand_num(100, "Order vs Veil 3") < 25 {
        remove_religion(g, city, veil);
    }
    if not_holy(g, city, order) && g.soren_rand_num(100, "Order vs Veil 3") < 25 {
        remove_religion(g, city, order);
    }
    if g.soren_rand_num(100, "Order vs Veil 3") < 50 {
        lose_population(g, city);
    }
}

/// `canApplyOrderVsVeil4(argsList)`: the city has a dungeon.
fn can_apply_order_vs_veil4(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let dungeon = g.d.t::<BuildingType>("BUILDING_DUNGEON");
    // NULL city: getNumRealBuilding() is -1, not 0.
    data_city(g, data).map_or(true, |c| g.cities[c].num_real_building[dungeon] != 0)
}

/// `doOrderVsVeilTemple1(argsList)`: side with the Order; the Veil temple is torn down.
fn do_order_vs_veil_temple1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    occupation(g, city, 1);
    let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
    remove_building(g, city, d.t::<BuildingType>("BUILDING_TEMPLE_OF_THE_VEIL"));
    if not_holy(g, city, veil) && g.soren_rand_num(100, "Order vs Veil Temple 1") < 50 {
        remove_religion(g, city, veil);
    }
    // Python bug: the city loop that follows (Order cities happy, Veil cities angry) reads
    // `player`, which this function never defines: NameError ends the function here.
}

/// `doOrderVsVeilTemple2(argsList)`.
fn do_order_vs_veil_temple2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    occupation(g, city, 1);
    let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
    if g.soren_rand_num(100, "Order vs Veil Temple 2") < 50 {
        remove_building(g, city, d.t::<BuildingType>("BUILDING_TEMPLE_OF_THE_VEIL"));
    }
    if not_holy(g, city, veil) && g.soren_rand_num(100, "Order vs Veil Temple 2") < 50 {
        remove_religion(g, city, veil);
    }
    if g.soren_rand_num(100, "Order vs Veil Temple 2") < 50 {
        lose_population(g, city);
    }
}

/// `doOrderVsVeilTemple3(argsList)`.
fn do_order_vs_veil_temple3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    occupation(g, city, 3);
    let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
    let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
    religion_timers(g, data.player, veil, order);
}

/// `canTriggerParith(argsList)`.
fn can_trigger_parith(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if !g.players[data.player].human {
        return false;
    }
    if g.trophy_value("TROPHY_WB_SPLINTERED_COURT_PARITH") != 1 {
        return false;
    }
    if g.is_option(GameOptionType::GAMEOPTION_WB_THE_SPLINTERED_COURT) {
        return false;
    }
    true
}

/// `applyParithYes(argsList)`: remember the event unit's type.
fn apply_parith_yes(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    // NULL unit: getUnitType() is -1 (trophy values are clamped to 0).
    let value = data_unit(g, data).map_or(-1, |u| g.units[u].unit_type.idx() as i32);
    g.set_trophy_value("TROPHY_WB_SPLINTERED_COURT_PARITH", value);
}

/// `canTriggerPenguins(argsList)`.
fn can_trigger_penguins(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    // NULL plot: isAdjacentToWater() is false.
    let Some(plot) = data_plot(g, data) else { return false };
    if !g.plot_is_adjacent_to_water(plot) {
        return false;
    }
    if g.plot_is_peak(plot) {
        return false;
    }
    true
}

/// `doPenguins(argsList)`.
fn do_penguins(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    if let Some(plot) = data_plot(g, data) {
        let penguins = g.d.t::<ImprovementType>("IMPROVEMENT_PENGUINS");
        g.plot_set_improvement_type(plot, Some(penguins));
    }
}

/// `canTriggerPickAlignment(argsList)`: `CyGame().getWBMapScript()` is always false.
fn can_trigger_pick_alignment(_g: &mut Game, _data: &mut EventTriggeredData) -> bool {
    true
}

/// `doPickAlignment1(argsList)`.
fn do_pick_alignment1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    g.players[data.player].alignment = Some(g.d.t::<AlignmentType>("ALIGNMENT_GOOD"));
}

/// `doPickAlignment2(argsList)`.
fn do_pick_alignment2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    g.players[data.player].alignment = Some(g.d.t::<AlignmentType>("ALIGNMENT_NEUTRAL"));
}

/// `doPickAlignment3(argsList)`.
fn do_pick_alignment3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    g.players[data.player].alignment = Some(g.d.t::<AlignmentType>("ALIGNMENT_EVIL"));
}

/// `doPigGiant3(argsList)`: a barbarian Hill Giant commando next to the city.
fn do_pig_giant3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    // NULL city: findClearPlot would search around the NULL plot's (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let city_plot = g.city_plot(city);
    if let Some(plot) = g.cf_find_clear_plot(None, city_plot) {
        let barbarian = g.barbarian_player();
        let (x, y) = g.map.xy(plot);
        let giant = d.t::<UnitType>("UNIT_HILL_GIANT");
        let unit = g.player_init_unit(barbarian, giant, x, y, Some(UnitAIType::UNITAI_ATTACK_CITY_LEMMING), SOUTH);
        g.unit_set_has_promotion(unit, d.t::<PromotionType>("PROMOTION_COMMANDO"), true);
    }
}

/// `applyPronCapria(argsList)`.
fn apply_pron_capria(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_CAPRIA");
    apply_pron(g, data, leader);
}

/// `canTriggerPronCapria(argsList)`.
fn can_trigger_pron_capria(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_CAPRIA"))
}

/// `applyPronEthne(argsList)`.
fn apply_pron_ethne(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_ETHNE");
    apply_pron(g, data, leader);
}

/// `canTriggerPronEthne(argsList)`.
fn can_trigger_pron_ethne(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_ETHNE"))
}

/// `applyPronArendel(argsList)`.
fn apply_pron_arendel(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_ARENDEL");
    apply_pron(g, data, leader);
}

/// `canTriggerPronArendel(argsList)`.
fn can_trigger_pron_arendel(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_ARENDEL"))
}

/// `applyPronThessa(argsList)`.
fn apply_pron_thessa(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_THESSA");
    apply_pron(g, data, leader);
}

/// `canTriggerPronThessa(argsList)`.
fn can_trigger_pron_thessa(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_THESSA"))
}

/// `applyPronHannah(argsList)`.
fn apply_pron_hannah(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_HANNAH");
    apply_pron(g, data, leader);
}

/// `canTriggerPronHannah(argsList)`.
fn can_trigger_pron_hannah(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_HANNAH"))
}

/// `applyPronRhoanna(argsList)`.
fn apply_pron_rhoanna(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_RHOANNA");
    apply_pron(g, data, leader);
}

/// `canTriggerPronRhoanna(argsList)`.
fn can_trigger_pron_rhoanna(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_RHOANNA"))
}

/// `applyPronValledia(argsList)`.
fn apply_pron_valledia(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_VALLEDIA");
    apply_pron(g, data, leader);
}

/// `canTriggerPronValledia(argsList)`.
fn can_trigger_pron_valledia(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_VALLEDIA"))
}

/// `applyPronMahala(argsList)`.
fn apply_pron_mahala(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_MAHALA");
    apply_pron(g, data, leader);
}

/// `canTriggerPronMahala(argsList)`.
fn can_trigger_pron_mahala(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_MAHALA"))
}

/// `applyPronKeelyn(argsList)`.
fn apply_pron_keelyn(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_KEELYN");
    apply_pron(g, data, leader);
}

/// `canTriggerPronKeelyn(argsList)`.
fn can_trigger_pron_keelyn(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_KEELYN"))
}

/// `applyPronSheelba(argsList)`.
fn apply_pron_sheelba(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_SHEELBA");
    apply_pron(g, data, leader);
}

/// `canTriggerPronSheelba(argsList)`.
fn can_trigger_pron_sheelba(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_SHEELBA"))
}

/// `applyPronFaeryl(argsList)`.
fn apply_pron_faeryl(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_FAERYL");
    apply_pron(g, data, leader);
}

/// `canTriggerPronFaeryl(argsList)`.
fn can_trigger_pron_faeryl(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_FAERYL"))
}

/// `applyPronAlexis(argsList)`.
fn apply_pron_alexis(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let leader = g.d.t::<LeaderHeadType>("LEADER_ALEXIS");
    apply_pron(g, data, leader);
}

/// `canTriggerPronAlexis(argsList)`.
fn can_trigger_pron_alexis(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_pron(g, data, g.d.t::<LeaderHeadType>("LEADER_ALEXIS"))
}

/// `canTriggerUniqueFeatureAifonIsle(argsList)`.
fn can_trigger_unique_feature_aifon_isle(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_unique_feature(g, data, g.d.t::<ImprovementType>("IMPROVEMENT_AIFON_ISLE"))
}

/// `canTriggerUniqueFeatureBradelinesWell(argsList)`.
fn can_trigger_unique_feature_bradelines_well(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_unique_feature(g, data, g.d.t::<ImprovementType>("IMPROVEMENT_BRADELINES_WELL"))
}

/// `canTriggerUniqueFeatureBrokenSepulcher(argsList)`.
fn can_trigger_unique_feature_broken_sepulcher(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_unique_feature(g, data, g.d.t::<ImprovementType>("IMPROVEMENT_BROKEN_SEPULCHER"))
}

/// `canTriggerUniqueFeatureGuardian(argsList)`.
fn can_trigger_unique_feature_guardian(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_unique_feature(g, data, g.d.t::<ImprovementType>("IMPROVEMENT_GUARDIAN"))
}

/// `canTriggerUniqueFeatureLetumFrigus(argsList)`: not for the Illians.
fn can_trigger_unique_feature_letum_frigus(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let d = g.d;
    let p = data.player;
    if !g.players[p].human {
        return false;
    }
    if g.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_ILLIANS")) {
        return false;
    }
    num_improvements(g, d.t::<ImprovementType>("IMPROVEMENT_LETUM_FRIGUS")) != 0
}

/// `canTriggerUniqueFeatureLetumFrigusIllians(argsList)`.
fn can_trigger_unique_feature_letum_frigus_illians(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    can_trigger_unique_feature(g, data, g.d.t::<ImprovementType>("IMPROVEMENT_LETUM_FRIGUS"))
}

/// `canTriggerUniqueFeaturePyreOfTheSeraphic(argsList)`: not for evil players.
fn can_trigger_unique_feature_pyre_of_the_seraphic(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let d = g.d;
    let p = data.player;
    if !g.players[p].human {
        return false;
    }
    if g.players[p].alignment == Some(d.t::<AlignmentType>("ALIGNMENT_EVIL")) {
        return false;
    }
    num_improvements(g, d.t::<ImprovementType>("IMPROVEMENT_PYRE_OF_THE_SERAPHIC")) != 0
}

/// `canTriggerSageKeep(argsList)`.
fn can_trigger_sage_keep(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    g.cities[c].specialist_count[g.d.t::<SpecialistType>("SPECIALIST_GREAT_SCIENTIST")] != 0
}

/// `doSailorsDirge(argsList)`: the Sailor's Dirge and three skeletons appear on an empty water
/// plot (random 0..500 + 10 per tile of its area, halved in owned waters).
fn do_sailors_dirge(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    let d = g.d;
    let unit = d.t::<UnitType>("UNIT_SAILORS_DIRGE");
    if g.unit_created_count[unit] == 0 {
        let mut best_plot = None;
        let mut best_value = -1;
        for plot in g.map.plot_ids() {
            if g.plot_is_water(plot) && g.map.plots[plot].units.is_empty() {
                let mut value = g.soren_rand_num(500, "Sailors Dirge");
                value += plot_area_num_tiles(g, plot) * 10;
                if g.plot_is_owned(plot) {
                    value = py_div(value, 2);
                }
                if value > best_value {
                    best_value = value;
                    best_plot = Some(plot);
                }
            }
        }
        if best_value != -1 {
            if let Some(plot) = best_plot {
                let barbarian = g.barbarian_player();
                let (x, y) = g.map.xy(plot);
                let dirge = init_unit(g, barbarian, unit, x, y);
                let skeleton = d.t::<UnitType>("UNIT_SKELETON");
                let (ux, uy) = (g.units[dirge].x, g.units[dirge].y);
                for _ in 0..3 {
                    init_unit(g, barbarian, skeleton, ux, uy);
                }
            }
        }
    }
}

/// `doSailorsDirgeDefeated(argsList)`.
fn do_sailors_dirge_defeated(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let treasure = g.d.t::<UnitType>("EQUIPMENT_TREASURE");
    g.cf_place_treasure(data.player, treasure);
}

/// `applyShrineCamulos2(argsList)`: 10% that a barbarian Pit Beast appears and attacks the city.
fn apply_shrine_camulos2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let city = data_city(g, data);
    if g.soren_rand_num(100, "Shrine Camulos") < 10 {
        // NULL city: findClearPlot would search around the NULL plot's (-1, -1).
        let Some(city) = city else { return };
        let city_plot = g.city_plot(city);
        if let Some(plot) = g.cf_find_clear_plot(None, city_plot) {
            let barbarian = g.barbarian_player();
            let (x, y) = g.map.xy(plot);
            let pit_beast = g.d.t::<UnitType>("UNIT_PIT_BEAST");
            let unit = init_unit(g, barbarian, pit_beast, x, y);
            g.unit_attack(unit, city_plot);
        }
    }
}

/// `doSignAeron(argsList)`.
fn do_sign_aeron(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    g.change_global_counter(3);
}

/// `doSignBhall(argsList)`: the player's land temporarily warms up one step.
fn do_sign_bhall(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let d = g.d;
    let desert = d.t::<TerrainType>("TERRAIN_DESERT");
    let grass = d.t::<TerrainType>("TERRAIN_GRASS");
    let plains = d.t::<TerrainType>("TERRAIN_PLAINS");
    let snow = d.t::<TerrainType>("TERRAIN_SNOW");
    let tundra = d.t::<TerrainType>("TERRAIN_TUNDRA");
    sign_terrain(g, data, "SignBhall", &[(snow, tundra), (tundra, grass), (grass, plains), (plains, desert)]);
}

/// `doSignCamulos(argsList)`.
fn do_sign_camulos(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    sign_attitude(g, data, -1);
}

/// `doSignDagda(argsList)`.
fn do_sign_dagda(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    sign_attitude(g, data, 1);
}

/// `doSignEsus(argsList)`: `CyGame().changeCrime(5)`.
fn do_sign_esus(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    g.crime += 5;
}

/// `doSignLugus(argsList)`: `CyGame().changeCrime(-5)`.
fn do_sign_lugus(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    g.crime -= 5;
}

/// `doSignMulcarn(argsList)`: the player's land temporarily cools down one step.
fn do_sign_mulcarn(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let d = g.d;
    let desert = d.t::<TerrainType>("TERRAIN_DESERT");
    let grass = d.t::<TerrainType>("TERRAIN_GRASS");
    let plains = d.t::<TerrainType>("TERRAIN_PLAINS");
    let snow = d.t::<TerrainType>("TERRAIN_SNOW");
    let tundra = d.t::<TerrainType>("TERRAIN_TUNDRA");
    sign_terrain(g, data, "SignMulcarn", &[(tundra, snow), (grass, tundra), (plains, tundra), (desert, plains)]);
}

/// `doSignSirona(argsList)`.
fn do_sign_sirona(g: &mut Game, _data: &EventTriggeredData, _p: PlayerId) {
    g.change_global_counter(-3);
}

/// `doSignSucellus(argsList)`: the player's units are cured of disease and heal half their damage.
fn do_sign_sucellus(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let diseased = g.d.t::<PromotionType>("PROMOTION_DISEASED");
    for u in py_unit_list(g, data.player) {
        if !g.units.contains(u) {
            continue;
        }
        if g.units[u].has_promotion[diseased] {
            g.unit_set_has_promotion(u, diseased, false);
        }
        let damage = g.units[u].damage;
        if damage > 0 {
            g.unit_set_damage(u, py_div(damage, 2), None);
        }
    }
}

/// `doSignTali(argsList)`: flames and smoke on the player's land go out.
fn do_sign_tali(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    let d = g.d;
    let p = data.player;
    let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
    let flames = d.t::<FeatureType>("FEATURE_FLAMES");
    for plot in g.map.plot_ids() {
        if g.map.plots[plot].owner == Some(p) {
            // UI: EFFECT_SPRING, AS3D_SPELL_SPRING.
            if g.map.plots[plot].feature_type == Some(flames) {
                g.plot_set_feature_type(plot, None);
            }
            if g.map.plots[plot].improvement_type == Some(smoke) {
                g.plot_set_improvement_type(plot, None);
            }
        }
    }
}

/// `canTriggerSmugglers(argsList)`.
fn can_trigger_smugglers(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    if g.cities[c].num_real_building[g.d.t::<BuildingType>("BUILDING_SMUGGLERS_PORT")] > 0 {
        return false;
    }
    if !g.city_is_coastal(c, 10) {
        return false;
    }
    true
}

/// `doSpiderMine3(argsList)`: a hidden barbarian Giant Spider on the empty trigger plot.
fn do_spider_mine3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    // NULL plot: getNumUnits() is -1.
    let Some(plot) = data_plot(g, data) else { return };
    if g.map.plots[plot].units.is_empty() {
        let barbarian = g.barbarian_player();
        let (x, y) = g.map.xy(plot);
        let unit = init_unit(g, barbarian, d.t::<UnitType>("UNIT_GIANT_SPIDER"), x, y);
        g.unit_set_has_promotion(unit, d.t::<PromotionType>("PROMOTION_HIDDEN_NATIONALITY"), true);
    }
}

/// `applyTreasure1(argsList)`.
fn apply_treasure1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let treasure = g.d.t::<UnitType>("EQUIPMENT_TREASURE");
    g.cf_place_treasure(data.player, treasure);
}

/// `canTriggerSwitchCivs(argsList)`: a human leading the score, from turn 20, offered a
/// non-vassal civilization.
fn can_trigger_switch_civs(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    if !g.players[p].human {
        return false;
    }
    if g.rank_player[0] != p {
        return false;
    }
    if g.game_turn() < 20 {
        return false;
    }
    // NO_PLAYER: gc.getPlayer(-1) is None and `otherPlayer.getTeam()` raises.
    let Some(other) = data.other_player else { return false };
    if g.team_is_a_vassal(g.players[other].team) {
        return false;
    }
    // `CyGame().getWBMapScript()` is always false.
    true
}

/// `doSwitchCivs2(argsList)`: the player takes over the other civilization.
fn do_switch_civs2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    // NO_PLAYER: reassignPlayerAdvanced's bounds check makes the call a no-op.
    if let Some(new) = data.other_player {
        crate::player::turn::reassign_player_advanced(g, data.player, new, -1);
    }
}

/// `canTriggerTraitor(argsList)`: the city is not unhappy.
fn can_trigger_traitor(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    g.city_happy_level(c) - g.city_unhappy_level(c, 0) >= 0
}

/// `doVeilVsOrderTemple1(argsList)`: side with the Veil; the Order temple is torn down.
fn do_veil_vs_order_temple1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    occupation(g, city, 1);
    let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
    let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
    remove_building(g, city, d.t::<BuildingType>("BUILDING_TEMPLE_OF_THE_ORDER"));
    if not_holy(g, city, order) && g.soren_rand_num(100, "Veil vs Order Temple 1") < 50 {
        remove_religion(g, city, order);
    }
    religion_timers(g, data.player, veil, order);
}

/// `doVeilVsOrderTemple2(argsList)`.
fn do_veil_vs_order_temple2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    occupation(g, city, 1);
    let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
    if g.soren_rand_num(100, "Veil Vs Order Temple 2") < 50 {
        remove_building(g, city, d.t::<BuildingType>("BUILDING_TEMPLE_OF_THE_ORDER"));
    }
    if not_holy(g, city, order) && g.soren_rand_num(100, "Veil Vs Order Temple 2") < 50 {
        remove_religion(g, city, order);
    }
    if g.soren_rand_num(100, "Veil Vs Order Temple 2") < 50 {
        lose_population(g, city);
    }
}

/// `doVeilVsOrderTemple3(argsList)`.
fn do_veil_vs_order_temple3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    occupation(g, city, 3);
    let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
    let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
    religion_timers(g, data.player, order, veil);
}

/// `doSlaveEscape(argsList)`.
fn do_slave_escape(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    // NULL unit: kill() is a no-op.
    if let Some(u) = data_unit(g, data) {
        g.unit_kill(u, false, None);
    }
}

/// `canTriggerSlaveRevoltUnit(argsList)`: the slave is alone on its plot.
fn can_trigger_slave_revolt_unit(g: &Game, _trigger: EventTriggerType, _p: PlayerId, u: UnitId) -> bool {
    g.map.plots[g.unit_plot(u)].units.len() == 1
}

/// `doSlaveRevolt(argsList)`: the slave becomes a barbarian Warrior of its race.
fn do_slave_revolt(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    // NULL unit: plot() is None and `plot.getX()` raises after the (no-op) kill.
    let Some(u) = data_unit(g, data) else { return };
    let race = g.units[u].race;
    let plot = g.unit_plot(u);
    g.unit_kill(u, false, None);
    let barbarian = g.barbarian_player();
    let (x, y) = g.map.xy(plot);
    let warrior = g.d.t::<UnitType>("UNIT_WARRIOR");
    let unit = g.player_init_unit(barbarian, warrior, x, y, Some(UnitAIType::UNITAI_ATTACK_CITY_LEMMING), SOUTH);
    if let Some(race) = race {
        g.unit_set_has_promotion(unit, race, true);
    }
}

/// `canApplyTraitAggressive(argsList)`.
fn can_apply_trait_aggressive(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_AGGRESSIVE"))
}

/// `doTraitAggressive(argsList)`.
fn do_trait_aggressive(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_AGGRESSIVE");
    do_trait(g, data, t);
}

/// `canApplyTraitArcane(argsList)`.
fn can_apply_trait_arcane(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_ARCANE"))
}

/// `doTraitArcane(argsList)`.
fn do_trait_arcane(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_ARCANE");
    do_trait(g, data, t);
}

/// `canApplyTraitCharismatic(argsList)`.
fn can_apply_trait_charismatic(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_CHARISMATIC"))
}

/// `doTraitCharismatic(argsList)`.
fn do_trait_charismatic(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_CHARISMATIC");
    do_trait(g, data, t);
}

/// `canApplyTraitCreative(argsList)`.
fn can_apply_trait_creative(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_CREATIVE"))
}

/// `doTraitCreative(argsList)`.
fn do_trait_creative(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_CREATIVE");
    do_trait(g, data, t);
}

/// `canApplyTraitExpansive(argsList)`.
fn can_apply_trait_expansive(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_EXPANSIVE"))
}

/// `doTraitExpansive(argsList)`.
fn do_trait_expansive(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_EXPANSIVE");
    do_trait(g, data, t);
}

/// `canApplyTraitFinancial(argsList)`.
fn can_apply_trait_financial(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_FINANCIAL"))
}

/// `doTraitFinancial(argsList)`.
fn do_trait_financial(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_FINANCIAL");
    do_trait(g, data, t);
}

/// `canApplyTraitIndustrious(argsList)`.
fn can_apply_trait_industrious(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_INDUSTRIOUS"))
}

/// `doTraitIndustrious(argsList)`.
fn do_trait_industrious(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_INDUSTRIOUS");
    do_trait(g, data, t);
}

/// `doTraitInsane(argsList)`: every trait but Insane is lost, then three different random
/// traits of the eleven selectable ones are gained.
fn do_trait_insane(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    const TRAITS: [&str; 11] = [
        "TRAIT_AGGRESSIVE",
        "TRAIT_ARCANE",
        "TRAIT_CHARISMATIC",
        "TRAIT_CREATIVE",
        "TRAIT_EXPANSIVE",
        "TRAIT_FINANCIAL",
        "TRAIT_INDUSTRIOUS",
        "TRAIT_ORGANIZED",
        "TRAIT_PHILOSOPHICAL",
        "TRAIT_RAIDERS",
        "TRAIT_SPIRITUAL",
    ];
    let d = g.d;
    let p = data.player;
    let insane = d.t::<TraitType>("TRAIT_INSANE");
    for t in d.traits.keys() {
        if g.players[p].traits[t] && t != insane {
            g.player_set_has_trait(p, t, false);
        }
    }
    let n = TRAITS.len() as i32;
    let rnd1 = g.soren_rand_num(n, "Insane");
    let mut rnd2 = g.soren_rand_num(n, "Insane");
    while rnd2 == rnd1 {
        rnd2 = g.soren_rand_num(n, "Insane");
    }
    let mut rnd3 = g.soren_rand_num(n, "Insane");
    while rnd3 == rnd1 || rnd3 == rnd2 {
        rnd3 = g.soren_rand_num(n, "Insane");
    }
    for rnd in [rnd1, rnd2, rnd3] {
        g.player_set_has_trait(p, d.t::<TraitType>(TRAITS[rnd as usize]), true);
    }
}

/// `canApplyTraitOrganized(argsList)`.
fn can_apply_trait_organized(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_ORGANIZED"))
}

/// `doTraitOrganized(argsList)`.
fn do_trait_organized(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_ORGANIZED");
    do_trait(g, data, t);
}

/// `canApplyTraitPhilosophical(argsList)`.
fn can_apply_trait_philosophical(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_PHILOSOPHICAL"))
}

/// `doTraitPhilosophical(argsList)`.
fn do_trait_philosophical(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_PHILOSOPHICAL");
    do_trait(g, data, t);
}

/// `canApplyTraitRaiders(argsList)`.
fn can_apply_trait_raiders(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_RAIDERS"))
}

/// `doTraitRaiders(argsList)`.
fn do_trait_raiders(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_RAIDERS");
    do_trait(g, data, t);
}

/// `canApplyTraitSpiritual(argsList)`.
fn can_apply_trait_spiritual(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_apply_trait(g, data, g.d.t::<TraitType>("TRAIT_SPIRITUAL"))
}

/// `doTraitSpiritual(argsList)`.
fn do_trait_spiritual(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let t = g.d.t::<TraitType>("TRAIT_SPIRITUAL");
    do_trait(g, data, t);
}

/// `doVolcanoCreation(argsList)`: the trigger plot becomes flat land with a volcano.
fn do_volcano_creation(g: &mut Game, data: &EventTriggeredData, _p: PlayerId) {
    // NULL plot: both setters are no-ops.
    let Some(plot) = data_plot(g, data) else { return };
    g.plot_set_plot_type(plot, PlotType::PLOT_LAND, true);
    let volcano = g.d.t::<FeatureType>("FEATURE_VOLCANO");
    g.plot_set_feature_type(plot, Some(volcano));
    // UI: EFFECT_ARTILLERY_SHELL_EXPLODE, AS3D_UN_GRENADE_EXPLODE.
}

/// `canTriggerWarGamesUnit(argsList)`.
fn can_trigger_war_games_unit(g: &Game, _trigger: EventTriggerType, _p: PlayerId, u: UnitId) -> bool {
    if !g.unit_is_alive(u) {
        return false;
    }
    if g.unit_is_only_defensive(u) {
        return false;
    }
    true
}

/// `applyWBFallOfCuantineRosier1(argsList)`.
fn apply_wb_fall_of_cuantine_rosier1(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    g.set_trophy_value("TROPHY_WB_FALL_OF_CUANTINE_ROSIER_ALLY", 0);
}

/// `applyWBFallOfCuantineRosier2(argsList)`.
fn apply_wb_fall_of_cuantine_rosier2(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    g.set_trophy_value("TROPHY_WB_FALL_OF_CUANTINE_ROSIER_ALLY", 1);
}

/// `applyWBFallOfCuantineFleeCalabim(argsList)`.
fn apply_wb_fall_of_cuantine_flee_calabim(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_CALABIM");
    g.set_trophy_value("TROPHY_WB_CIV_DECIUS", civ.idx() as i32);
}

/// `applyWBFallOfCuantineFleeMalakim(argsList)`.
fn apply_wb_fall_of_cuantine_flee_malakim(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_MALAKIM");
    g.set_trophy_value("TROPHY_WB_CIV_DECIUS", civ.idx() as i32);
}

/// `applyWBGiftOfKylorinMeshabberRight(argsList)`: the door plot goes quiet and Meshabber
/// (the first unit on plot (20, 16)) dies.
fn apply_wb_gift_of_kylorin_meshabber_right(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    if let Some(plot) = g.map.plot_xy(19, 16) {
        g.map.plots[plot].python_active = false;
    }
    // NULL plot: getUnit(0) is None and `.kill` raises (only the UI popup follows).
    let Some(plot) = g.map.plot_xy(20, 16) else { return };
    // No unit there: the NULL CyUnit's kill is a no-op.
    if let Some(&u) = g.map.plots[plot].units.first() {
        g.unit_kill(u, true, Some(PlayerId(0)));
    }
    // UI: cf.addPopup(TXT_KEY_WB_GIFT_OF_KYLORIN_MESHABBER_RIGHT).
}

/// `applyWBGiftOfKylorinMeshabberWrong(argsList)`: Meshabber is released and attacks the door plot.
fn apply_wb_gift_of_kylorin_meshabber_wrong(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    let plot1 = g.map.plot_xy(19, 16);
    if let Some(plot) = plot1 {
        g.map.plots[plot].python_active = false;
    }
    // NULL plot: getUnit(0) is None and `.setHasPromotion` raises.
    let Some(plot2) = g.map.plot_xy(20, 16) else { return };
    // No unit there: the NULL CyUnit's calls are no-ops.
    let Some(&u) = g.map.plots[plot2].units.first() else { return };
    g.unit_set_has_promotion(u, g.d.t::<PromotionType>("PROMOTION_HELD"), false);
    // attack(None) would dereference a NULL plot in C++.
    if let Some(plot) = plot1 {
        g.unit_attack(u, plot);
    }
    // UI: cf.addPopup(TXT_KEY_WB_GIFT_OF_KYLORIN_MESHABBER_WRONG).
}

/// `applyWBGiftOfKylorinSecretDoorYes(argsList)`: the secret door opens.
fn apply_wb_gift_of_kylorin_secret_door_yes(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    if let Some(plot) = g.map.plot_xy(23, 6) {
        g.map.plots[plot].python_active = false;
    }
    if let Some(plot) = g.map.plot_xy(23, 5) {
        g.plot_set_feature_type(plot, None);
    }
}

/// `applyWBLordOfTheBalorsTemptJudeccaYes(argsList)`: peace with Judecca (team 6), total war on
/// the other player.
fn apply_wb_lord_of_the_balors_tempt_judecca_yes(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    // NO_PLAYER: gc.getPlayer(-1) is None and `otherPlayer.getTeam()` raises.
    let Some(other) = data.other_player else { return };
    let enemy_team = g.players[other].team;
    let team = g.players[data.player].team;
    let judecca = TeamId(6);
    g.teams[team].permanent_war_peace[enemy_team] = false;
    g.teams[team].permanent_war_peace[judecca] = false;
    g.team_make_peace(team, judecca, true);
    g.team_declare_war(team, enemy_team, true, Some(WarPlanType::WARPLAN_TOTAL));
    g.teams[team].permanent_war_peace[enemy_team] = true;
    g.teams[team].permanent_war_peace[judecca] = true;
}

/// `applyWBLordOfTheBalorsTemptSallosYes(argsList)`: peace with Sallos (team 7).
fn apply_wb_lord_of_the_balors_tempt_sallos_yes(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let team = g.players[data.player].team;
    forced_peace(g, team, TeamId(7));
}

/// `applyWBLordOfTheBalorsTemptOuzzaYes(argsList)`: peace with Ouzza (team 8); every city
/// above size 1 loses a citizen.
fn apply_wb_lord_of_the_balors_tempt_ouzza_yes(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let team = g.players[p].team;
    forced_peace(g, team, TeamId(8));
    for city in city_list(g, p) {
        if g.cities[city].population > 1 {
            g.city_change_population(city, -1);
        }
    }
}

/// `applyWBLordOfTheBalorsTemptMeresinYes(argsList)`: peace with Meresin (team 9).
fn apply_wb_lord_of_the_balors_tempt_meresin_yes(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let team = g.players[data.player].team;
    forced_peace(g, team, TeamId(9));
}

/// `applyWBLordOfTheBalorsTemptStatiusYes(argsList)`: meant to make peace with Statius (team 10)
/// and give him the event city.
fn apply_wb_lord_of_the_balors_tempt_statius_yes(_g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    // Python bug: the first statement, `pCity = pPlayer.getCity(..)`, reads `pPlayer` before the
    // function assigns it (UnboundLocalError), so the callback does nothing.
}

/// `applyWBLordOfTheBalorsTemptLetheYes(argsList)`: peace with Lethe (team 11); the event unit dies.
fn apply_wb_lord_of_the_balors_tempt_lethe_yes(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let unit = data_unit(g, data);
    let team = g.players[data.player].team;
    forced_peace(g, team, TeamId(11));
    // NULL unit: kill() is a no-op. (A unit the peace bumped off the map and killed is gone.)
    if let Some(u) = unit {
        if g.units.contains(u) {
            g.unit_kill(u, true, Some(PlayerId(0)));
        }
    }
}

/// `applyWBSplinteredCourtDefeatedAmelanchier3(argsList)`.
fn apply_wb_splintered_court_defeated_amelanchier3(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    let d = g.d;
    let doviello = d.t::<CivilizationType>("CIVILIZATION_DOVIELLO");
    g.set_trophy_value("TROPHY_WB_CIV_AMELANCHIER", doviello.idx() as i32);
    let ljosalfar = d.t::<CivilizationType>("CIVILIZATION_LJOSALFAR");
    let svartalfar = d.t::<CivilizationType>("CIVILIZATION_SVARTALFAR");
    splintered_court_sides(g, doviello, svartalfar, ljosalfar);
}

/// `applyWBSplinteredCourtDefeatedThessa3(argsList)`.
fn apply_wb_splintered_court_defeated_thessa3(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    let d = g.d;
    let calabim = d.t::<CivilizationType>("CIVILIZATION_CALABIM");
    g.set_trophy_value("TROPHY_WB_CIV_THESSA", calabim.idx() as i32);
    let ljosalfar = d.t::<CivilizationType>("CIVILIZATION_LJOSALFAR");
    let svartalfar = d.t::<CivilizationType>("CIVILIZATION_SVARTALFAR");
    splintered_court_sides(g, calabim, svartalfar, ljosalfar);
}

/// `applyWBSplinteredCourtDefeatedRivanna3(argsList)`.
fn apply_wb_splintered_court_defeated_rivanna3(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    let d = g.d;
    let calabim = d.t::<CivilizationType>("CIVILIZATION_CALABIM");
    g.set_trophy_value("TROPHY_WB_CIV_RIVANNA", calabim.idx() as i32);
    let ljosalfar = d.t::<CivilizationType>("CIVILIZATION_LJOSALFAR");
    let svartalfar = d.t::<CivilizationType>("CIVILIZATION_SVARTALFAR");
    splintered_court_sides(g, calabim, ljosalfar, svartalfar);
}

/// `applyWBSplinteredCourtDefeatedVolanna3(argsList)`.
fn apply_wb_splintered_court_defeated_volanna3(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    let d = g.d;
    let doviello = d.t::<CivilizationType>("CIVILIZATION_DOVIELLO");
    g.set_trophy_value("TROPHY_WB_CIV_VOLANNA", doviello.idx() as i32);
    let ljosalfar = d.t::<CivilizationType>("CIVILIZATION_LJOSALFAR");
    let svartalfar = d.t::<CivilizationType>("CIVILIZATION_SVARTALFAR");
    splintered_court_sides(g, doviello, ljosalfar, svartalfar);
}

/// `applyWBSplinteredCourtParithYes(argsList)`.
fn apply_wb_splintered_court_parith_yes(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    g.set_trophy_value("TROPHY_WB_SPLINTERED_COURT_PARITH", 1);
}

/// `canDoWBTheBlackTowerPickCivBannor(argsList)`.
fn can_do_wb_the_black_tower_pick_civ_bannor(g: &Game, _event: EventType, _data: &EventTriggeredData) -> bool {
    g.is_has_trophy("TROPHY_WB_THE_RADIANT_GUARD_CAPRIA_ALLY")
}

/// `applyWBTheBlackTowerPickCivBannor(argsList)`.
fn apply_wb_the_black_tower_pick_civ_bannor(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_BANNOR");
    black_tower_pick_civ(g, data, civ);
}

/// `applyWBTheBlackTowerPickCivHippus(argsList)`.
fn apply_wb_the_black_tower_pick_civ_hippus(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_HIPPUS");
    black_tower_pick_civ(g, data, civ);
}

/// `applyWBTheBlackTowerPickCivLanun(argsList)`.
fn apply_wb_the_black_tower_pick_civ_lanun(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_LANUN");
    black_tower_pick_civ(g, data, civ);
}

/// `canDoWBTheBlackTowerPickCivLjosalfar(argsList)`.
fn can_do_wb_the_black_tower_pick_civ_ljosalfar(g: &Game, _event: EventType, _data: &EventTriggeredData) -> bool {
    g.is_has_trophy("TROPHY_WB_THE_SPLINTERED_COURT_LJOSALFAR")
}

/// `applyWBTheBlackTowerPickCivLjosalfar(argsList)`.
fn apply_wb_the_black_tower_pick_civ_ljosalfar(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_LJOSALFAR");
    black_tower_pick_civ(g, data, civ);
}

/// `canDoWBTheBlackTowerPickCivLuchuirp(argsList)`.
fn can_do_wb_the_black_tower_pick_civ_luchuirp(g: &Game, _event: EventType, _data: &EventTriggeredData) -> bool {
    g.is_has_trophy("TROPHY_WB_THE_MOMUS_BEERI_ALLY")
}

/// `applyWBTheBlackTowerPickCivLuchuirp(argsList)`.
fn apply_wb_the_black_tower_pick_civ_luchuirp(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_LUCHUIRP");
    black_tower_pick_civ(g, data, civ);
}

/// `canDoWBTheBlackTowerPickCivSvartalfar(argsList)`.
fn can_do_wb_the_black_tower_pick_civ_svartalfar(g: &Game, _event: EventType, _data: &EventTriggeredData) -> bool {
    g.is_has_trophy("TROPHY_WB_THE_SPLINTERED_COURT_SVARTALFAR")
}

/// `applyWBTheBlackTowerPickCivSvartalfar(argsList)`.
fn apply_wb_the_black_tower_pick_civ_svartalfar(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let civ = g.d.t::<CivilizationType>("CIVILIZATION_SVARTALFAR");
    black_tower_pick_civ(g, data, civ);
}

/// `applyWBTheMomusBeerisOfferYes(argsList)`: Falamar (team 0) and the Beeri (team 7) make
/// peace and both declare total war on team 1.
fn apply_wb_the_momus_beeris_offer_yes(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    g.change_trophy_value("TROPHY_WB_THE_MOMUS_BEERI_ALLY", 1);
    let (falamar, team1, beeri) = (TeamId(0), TeamId(1), TeamId(7));
    g.teams[falamar].permanent_war_peace[team1] = false;
    g.teams[falamar].permanent_war_peace[beeri] = false;
    g.team_declare_war(falamar, team1, true, Some(WarPlanType::WARPLAN_TOTAL));
    g.team_declare_war(beeri, team1, true, Some(WarPlanType::WARPLAN_TOTAL));
    g.team_make_peace(falamar, beeri, true);
    g.teams[falamar].permanent_war_peace[team1] = true;
    g.teams[falamar].permanent_war_peace[beeri] = true;
}

/// `applyWBTheRadiantGuardChooseSidesBasium(argsList)`.
fn apply_wb_the_radiant_guard_choose_sides_basium(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    g.set_trophy_value("TROPHY_WB_THE_RADIANT_GUARD_HYBOREM_ALLY", 0);
    g.set_trophy_value("TROPHY_WB_THE_RADIANT_GUARD_BASIUM_ALLY", 1);
}

/// `applyWBTheRadiantGuardChooseSidesHyborem(argsList)`: player 0's Empyrean units defect to
/// Basium (player 1, at his capital); Falamar (team 0) goes to war with Basium (team 1) and makes
/// peace with team 2.
fn apply_wb_the_radiant_guard_choose_sides_hyborem(g: &mut Game, _event: EventType, _data: &EventTriggeredData) {
    let d = g.d;
    g.set_trophy_value("TROPHY_WB_THE_RADIANT_GUARD_HYBOREM_ALLY", 1);
    g.set_trophy_value("TROPHY_WB_THE_RADIANT_GUARD_BASIUM_ALLY", 0);
    let basium = PlayerId(1);
    let capital = g.player_get_capital_city(basium);
    let empyrean = d.t::<ReligionType>("RELIGION_THE_EMPYREAN");
    for u in py_unit_list(g, PlayerId(0)) {
        if !g.units.contains(u) {
            continue;
        }
        let unit_type = g.units[u].unit_type;
        if d.units[unit_type].religion_type == Some(empyrean) {
            // NULL capital: initUnit at (-1, -1).
            let Some(city) = capital else { return };
            let (x, y) = city_xy(g, city);
            let unit = g.player_init_unit(basium, unit_type, x, y, Some(UnitAIType::UNITAI_ATTACK), SOUTH);
            g.unit_convert(unit, u);
        }
    }
    let (falamar, team1, team2) = (TeamId(0), TeamId(1), TeamId(2));
    g.teams[falamar].permanent_war_peace[team1] = false;
    g.teams[falamar].permanent_war_peace[team2] = false;
    g.team_declare_war(falamar, team1, true, Some(WarPlanType::WARPLAN_TOTAL));
    g.team_make_peace(falamar, team2, true);
    g.teams[falamar].permanent_war_peace[team1] = true;
    g.teams[falamar].permanent_war_peace[team2] = true;
}

/// `doWerewolf1(argsList)`: a barbarian Werewolf is released next to the city.
fn do_werewolf1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    // NULL city: findClearPlot would search around the NULL plot's (-1, -1).
    let Some(city) = data_city(g, data) else { return };
    let city_plot = g.city_plot(city);
    if let Some(plot) = g.cf_find_clear_plot(None, city_plot) {
        let barbarian = g.barbarian_player();
        let (x, y) = g.map.xy(plot);
        let werewolf = g.d.t::<UnitType>("UNIT_WEREWOLF");
        g.player_init_unit(barbarian, werewolf, x, y, Some(UnitAIType::UNITAI_ATTACK_CITY_LEMMING), SOUTH);
    }
}

/// `doWerewolf3(argsList)`: only a message (TXT_KEY_MESSAGE_WEREWOLF_KILLED, UI).
fn do_werewolf3(_g: &mut Game, _event: EventType, _data: &EventTriggeredData) {}

// ---------------------------------------------------------------------------------------------
// BtS events (Python lines 2504-3303)
// ---------------------------------------------------------------------------------------------

/// `canTriggerBlessedSea(argsList)`: a map of many landmasses, the player on few of them, with a
/// galley, caravel or galleon.
fn can_trigger_blessed_sea(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let d = g.d;
    let p = data.player;
    if g.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && g.players[p].human {
        return false;
    }
    let map_min_landmass = 2 * default_players(g);
    let our_max_landmass = py_div(default_players(g), 2);
    if g.map.get_num_land_areas() < map_min_landmass {
        return false;
    }
    if our_landmasses(g, p) > our_max_landmass {
        return false;
    }
    let count = |class: &str| g.players[p].unit_class_count[d.t::<UnitClassType>(class)];
    if count("UNITCLASS_GALLEY") == 0 && count("UNITCLASS_CARAVEL") == 0 && count("UNITCLASS_GALLEON") == 0 {
        return false;
    }
    true
}

/// `canTriggerBlessedSea2(argsList)`: the player has settled enough landmasses.
fn can_trigger_blessed_sea2(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let our_min_landmass = py_div(3 * default_players(g), 2);
    our_landmasses(g, data.player) >= our_min_landmass
}

/// `applyBlessedSea2(argsList)`: every city of size 5+ that could build the religion's temple
/// gets it.
fn apply_blessed_sea2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let Some(building) = blessed_sea_temple(g, data) else { return };
    for city in city_list(g, data.player) {
        if g.cities[city].population >= 5 && g.city_can_construct(city, building, false, false, true) {
            g.city_set_num_real_building(city, building, 1);
        }
    }
}

/// `canApplyBlessedSea2(argsList)`.
fn can_apply_blessed_sea2(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    let Some(building) = blessed_sea_temple(g, data) else { return false };
    g.players[data.player]
        .cities
        .iter()
        .any(|&city| g.cities[city].population >= 5 && g.city_can_construct(city, building, false, false, true))
}

/// `canTriggerHolyMountain(argsList)`: the mountain is unowned.
fn can_trigger_holy_mountain(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    if g.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && g.players[data.player].human {
        return false;
    }
    // NULL plot: getOwner() is -1.
    data_plot(g, data).and_then(|plot| g.map.plots[plot].owner).is_none()
}

/// `expireHolyMountain1(argsList)`: someone else took the mountain.
fn expire_holy_mountain1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    // NULL plot: getOwner() is -1 (`plot == None` is never true).
    let owner = data_plot(g, data).and_then(|plot| g.map.plots[plot].owner);
    owner != Some(data.player) && owner.is_some()
}

/// `canTriggerHolyMountainDone(argsList)`: the player owns the mountain of the earlier event.
fn can_trigger_holy_mountain_done(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let Some((x, y, _)) = prereq_event_occured(g, data) else { return false };
    // NULL plot: getOwner() is -1.
    g.map.plot_xy(x, y).and_then(|plot| g.map.plots[plot].owner) == Some(data.player)
}

/// `canTriggerHolyMountainRevealed(argsList)`: enough religious buildings (cathedrals count 4,
/// temples and monasteries 1) of the earlier event's religion reveal the mountain, which becomes
/// this trigger's plot.
fn can_trigger_holy_mountain_revealed(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let d = g.d;
    let p = data.player;
    let Some((x, y, religion)) = prereq_event_occured(g, data) else { return false };
    // Not FFH2 types: `findInfoTypeNum` gives -1, which matches buildings without a special type.
    let cathedral = d.ty::<SpecialBuildingType>("SPECIALBUILDING_CATHEDRAL");
    let temple = d.ty::<SpecialBuildingType>("SPECIALBUILDING_TEMPLE");
    let monastery = d.ty::<SpecialBuildingType>("SPECIALBUILDING_MONASTERY");
    let mut num_points = 0;
    for b in d.buildings.keys() {
        let info = &d.buildings[b];
        if info.religion_type == religion {
            if info.special_building_type == cathedral {
                num_points += 4 * g.player_count_num_buildings(p, b);
            } else if info.special_building_type == temple || info.special_building_type == monastery {
                num_points += g.player_count_num_buildings(p, b);
            }
        }
    }
    if num_points < 2 * default_players(g) {
        return false;
    }
    // NULL plot: setRevealed is a no-op (`plot == None` is never true).
    if let Some(plot) = g.map.plot_xy(x, y) {
        let team = g.players[p].team;
        g.plot_set_revealed(plot, team, true, true, None, true);
    }
    // `player.getEventTriggered(kTriggeredData.iId)`: the trigger data being set up.
    data.plot_x = x;
    data.plot_y = y;
    true
}

/// `doHolyMountainRevealed(argsList)`: only centers the camera for the active player (UI).
fn do_holy_mountain_revealed(_g: &mut Game, _event: EventType, _data: &EventTriggeredData) {}

/// `canTriggerMarathon(argsList)`: in the first turn of a war, a unit of the enemy stands on the
/// player's land.
fn can_trigger_marathon(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let p = data.player;
    // NO_PLAYER: gc.getPlayer(-1) is None and `otherPlayer.getTeam()` raises.
    let Some(other) = data.other_player else { return false };
    let team = g.players[p].team;
    let other_team = g.players[other].team;
    if g.teams[team].ai_at_war_counter[other_team] == 1 {
        for &u in &g.players[other].units {
            // NULL plot (a unit off the map): `plot.isNone()`.
            if let Some(plot) = g.unit_plot_opt(u) {
                if g.map.plots[plot].owner == Some(p) {
                    return true;
                }
            }
        }
    }
    false
}

/// `doWeddingFeud2(argsList)`: 30 turns of happiness in the cities of the trigger's religion.
fn do_wedding_feud2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    for city in city_list(g, data.player) {
        if data.religion.map_or(false, |r| g.cities[city].has_religion[r]) {
            g.city_change_happiness_timer(city, 30);
        }
    }
}

/// `doWeddingFeud3(argsList)`: players of the same state religion grow fonder of each other, and
/// the other player declares a limited war if it can. For a human other player the Python asks
/// with a Yes/No popup (`weddingFeud3Callback` declares the same war on Yes); the port takes the
/// AI branch for every player.
fn do_wedding_feud3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let state_religion = g.player_get_state_religion(p);
    for lp in g.civ_player_ids() {
        if g.players[lp].alive && g.player_get_state_religion(lp) == state_religion {
            g.players[lp].ai_attitude_extra[p] += 1;
            g.players[p].ai_attitude_extra[lp] += 1;
        }
    }
    // NO_PLAYER: gc.getPlayer(-1) is None and `destPlayer.getTeam()` raises.
    let Some(dest) = data.other_player else { return };
    let dest_team = g.players[dest].team;
    let team = g.players[p].team;
    if g.team_can_declare_war(dest_team, team) {
        g.team_declare_war(dest_team, team, false, Some(WarPlanType::WARPLAN_LIMITED));
    }
}

/// `canDoWeddingFeud3(argsList)`.
fn can_do_wedding_feud3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_pay_ten_per_city(g, data.player)
}

/// `canTriggerSpicy(argsList)`: at most 4 happiness resources and no spices yet, and the plot
/// can have spices. (`BONUS_SPICES` is not an FFH2 bonus: -1, which no bonus index equals and
/// every plot "can have".)
fn can_trigger_spicy(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let d = g.d;
    let p = data.player;
    let spice = d.ty::<BonusType>("BONUS_SPICES");
    let mut happy_bonuses = 0;
    for b in d.bonuses.keys() {
        if g.player_get_num_available_bonuses(p, b) > 0 {
            if d.bonuses[b].happiness > 0 {
                happy_bonuses += 1;
                if happy_bonuses > 4 {
                    return false;
                }
            }
            if Some(b) == spice {
                return false;
            }
        }
    }
    // NULL plot: canHaveBonus() is false.
    let Some(plot) = data_plot(g, data) else { return false };
    g.plot_can_have_bonus(plot, spice, false)
}

/// `doSpicy2(argsList)`: a plantation on the trigger plot.
fn do_spicy2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    if let Some(plot) = data_plot(g, data) {
        let plantation = g.d.ty::<ImprovementType>("IMPROVEMENT_PLANTATION");
        g.plot_set_improvement_type(plot, plantation);
    }
}

/// `canTriggerBabyBoom(argsList)`: at peace with everyone, and a peace began last turn.
fn can_trigger_baby_boom(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    let team = g.players[data.player].team;
    if g.team_get_at_war_count(team, true) > 0 {
        return false;
    }
    g.civ_team_ids().any(|t| t != team && g.teams[team].ai_at_peace_counter[t] == 1)
}

/// `applyBardTale3(argsList)`: 10 gold per city.
fn apply_bard_tale3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let p = data.player;
    let cost = 10 * g.players[p].num_cities();
    g.players[p].gold -= cost;
}

/// `canApplyBardTale3(argsList)`.
fn can_apply_bard_tale3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    can_pay_ten_per_city(g, data.player)
}

/// `canApplyLooters3(argsList)`: the other player's city has a cheap building to loot.
fn can_apply_looters3(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    // NO_PLAYER: gc.getPlayer(-1) is None and `otherPlayer.getCity` raises.
    let Some(other) = data.other_player else { return false };
    let city = player_get_city(g, other, data.other_player_city);
    !destructible_buildings(g, city, true).is_empty()
}

/// `applyLooters3(argsList)`: one or two cheap buildings of the other player's city are destroyed.
fn apply_looters3(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    // NO_PLAYER: gc.getPlayer(-1) is None and `otherPlayer.getCity` raises.
    let Some(other) = data.other_player else { return };
    let city = player_get_city(g, other, data.other_player_city);
    let num_buildings = g.soren_rand_num(2, "Looters event number of buildings destroyed");
    let mut list = destructible_buildings(g, city, true);
    for _ in 0..num_buildings + 1 {
        if !list.is_empty() {
            let i = g.soren_rand_num(list.len() as i32, "Looters event building destroyed") as usize;
            remove_building(g, city, list[i]);
            list.remove(i);
        }
    }
}

/// `canTriggerBrothersInNeed(argsList)`.
fn can_trigger_brothers_in_need(g: &mut Game, data: &mut EventTriggeredData) -> bool {
    brothers_in_need(g, data)
}

/// The body of `canTriggerBrothersInNeed` (also `canDoBrothersInNeed1`): the player can trade
/// with the other player, has a spare strategic resource the other lacks, and a third team is
/// at war with the other player but not with the player.
fn brothers_in_need(g: &Game, data: &EventTriggeredData) -> bool {
    const RESOURCES: [&str; 4] = ["BONUS_COPPER", "BONUS_IRON", "BONUS_HORSE", "BONUS_IVORY"];
    let d = g.d;
    let p = data.player;
    // NO_PLAYER: CvPlayer::canTradeNetworkWith(NO_PLAYER) reads GET_PLAYER(-1); the trigger
    // always picks the other player.
    let Some(other) = data.other_player else { return false };
    if !g.player_can_trade_network_with(p, other) {
        return false;
    }
    let found = RESOURCES.iter().any(|&name| {
        let resource = d.t::<BonusType>(name);
        g.player_get_num_tradeable_bonuses(p, resource) > 1 && g.player_get_num_available_bonuses(other, resource) <= 0
    });
    if !found {
        return false;
    }
    let team = g.players[p].team;
    let other_team = g.players[other].team;
    g.civ_team_ids().any(|t| {
        t != team
            && t != other_team
            && g.team_is_alive(t)
            && g.teams[t].at_war[other_team]
            && !g.teams[t].at_war[team]
    })
}

/// `canDoBrothersInNeed1(argsList)`: `canTriggerBrothersInNeed((kTriggeredData, ))`.
fn can_do_brothers_in_need1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    brothers_in_need(g, data)
}

/// `canTriggerHurricaneCity(argsList)`: a coastal city of size 2+ in the northern hemisphere.
fn can_trigger_hurricane_city(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    // `city.isNone()`: the DLL passes one of the player's cities.
    if !g.city_is_coastal(c, g.d.defines.min_water_size_for_ocean) {
        return false;
    }
    if g.plot_get_latitude(g.city_plot(c)) <= 0 {
        return false;
    }
    if g.cities[c].population < 2 {
        return false;
    }
    true
}

/// `canApplyHurricane1(argsList)`: the city has a building to lose.
fn can_apply_hurricane1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    !destructible_buildings(g, data_city(g, data), false).is_empty()
}

/// `canApplyHurricane2(argsList)`: `not canApplyHurricane1(argsList)`.
fn can_apply_hurricane2(g: &Game, event: EventType, data: &EventTriggeredData) -> bool {
    !can_apply_hurricane1(g, event, data)
}

/// `applyHurricane1(argsList)`: one cheap and one expensive building are destroyed.
fn apply_hurricane1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let city = data_city(g, data);
    let all = destructible_buildings(g, city, false);
    let (cheap, expensive): (Vec<BuildingType>, Vec<BuildingType>) =
        all.into_iter().partition(|&b| d.buildings[b].production_cost <= 100);
    if !cheap.is_empty() {
        let i = g.soren_rand_num(cheap.len() as i32, "Hurricane event cheap building destroyed") as usize;
        remove_building(g, city, cheap[i]);
    }
    if !expensive.is_empty() {
        let i = g.soren_rand_num(expensive.len() as i32, "Hurricane event expensive building destroyed") as usize;
        remove_building(g, city, expensive[i]);
    }
}

/// `canTriggerCycloneCity(argsList)`: a coastal city in the southern hemisphere.
fn can_trigger_cyclone_city(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    // `city.isNone()`: the DLL passes one of the player's cities.
    if !g.city_is_coastal(c, g.d.defines.min_water_size_for_ocean) {
        return false;
    }
    if g.plot_get_latitude(g.city_plot(c)) >= 0 {
        return false;
    }
    true
}

/// `canTriggerTsunamiCity(argsList)`: a coastal city.
fn can_trigger_tsunami_city(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    // `city.isNone()`: the DLL passes one of the player's cities.
    g.city_is_coastal(c, g.d.defines.min_water_size_for_ocean)
}

/// `canApplyTsunami1(argsList)`: a city below size 6.
fn can_apply_tsunami1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    // NULL city: getPopulation() is -1.
    data_city(g, data).map_or(-1, |c| g.cities[c].population) < 6
}

/// `canApplyTsunami2(argsList)`: `not canApplyTsunami1(argsList)`.
fn can_apply_tsunami2(g: &Game, event: EventType, data: &EventTriggeredData) -> bool {
    !can_apply_tsunami1(g, event, data)
}

/// `applyTsunami1(argsList)`: the city is destroyed.
fn apply_tsunami1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    // CyCity::kill() is `kill(true)`; a no-op on the NULL city.
    if let Some(c) = data_city(g, data) {
        g.city_kill(c, true);
    }
}

/// `applyTsunami2(argsList)`: up to five buildings are destroyed.
fn apply_tsunami2(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let city = data_city(g, data);
    let mut list = destructible_buildings(g, city, false);
    for _ in 0..5 {
        if !list.is_empty() {
            let i = g.soren_rand_num(list.len() as i32, "Tsunami event building destroyed") as usize;
            remove_building(g, city, list[i]);
            list.remove(i);
        }
    }
}

/// `canTriggerMonsoonCity(argsList)`: an inland city with jungle within 3 plots.
fn can_trigger_monsoon_city(g: &Game, _trigger: EventTriggerType, _p: PlayerId, c: CityId) -> bool {
    // `city.isNone()`: the DLL passes one of the player's cities.
    if g.city_is_coastal(c, g.d.defines.min_water_size_for_ocean) {
        return false;
    }
    let jungle = g.d.t::<FeatureType>("FEATURE_JUNGLE");
    let (x, y) = city_xy(g, c);
    for dx in -3..4 {
        for dy in -3..4 {
            if let Some(plot) = g.plot_xy(x, y, dx, dy) {
                if g.map.plots[plot].feature_type == Some(jungle) {
                    return true;
                }
            }
        }
    }
    false
}

/// `canApplyVolcano1(argsList)`: an improvement next to the volcano.
fn can_apply_volcano1(g: &Game, _event: EventType, data: &EventTriggeredData) -> bool {
    !volcano_plots(g, data).is_empty()
}

/// `applyVolcano1(argsList)`: two or three improvements next to the volcano are destroyed
/// (cottages and their upgrades leave city ruins).
fn apply_volcano1(g: &mut Game, _event: EventType, data: &EventTriggeredData) {
    let d = g.d;
    let mut plots = volcano_plots(g, data);
    let ruins_list = [
        d.ty::<ImprovementType>("IMPROVEMENT_COTTAGE"),
        d.ty::<ImprovementType>("IMPROVEMENT_HAMLET"),
        d.ty::<ImprovementType>("IMPROVEMENT_VILLAGE"),
        d.ty::<ImprovementType>("IMPROVEMENT_TOWN"),
    ];
    let ruins = d.ty::<ImprovementType>("IMPROVEMENT_CITY_RUINS");
    for i in 0..3 {
        if !plots.is_empty() {
            let k = g.soren_rand_num(plots.len() as i32, "Volcano event improvement destroyed") as usize;
            let plot = plots[k];
            let improvement = g.map.plots[plot].improvement_type;
            if ruins_list.contains(&improvement) {
                g.plot_set_improvement_type(plot, ruins);
            } else {
                g.plot_set_improvement_type(plot, None);
            }
            plots.remove(k);
            if i == 1 && g.soren_rand_num(100, "Volcano event num improvements destroyed") < 50 {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffh::pycall::PyCall;
    use crate::player::Player;
    use crate::setup::GameSetup;
    use std::collections::BTreeSet;

    /// The slot each registered name is in.
    fn slot_of(name: &str) -> Vec<&'static str> {
        let mut slots = Vec::new();
        if find_trigger_can_do(name).is_some() {
            slots.push("EventTriggerInfo.PythonCanDo");
        }
        if find_trigger_city(name).is_some() {
            slots.push("EventTriggerInfo.PythonCanDoCity");
        }
        if find_trigger_unit(name).is_some() {
            slots.push("EventTriggerInfo.PythonCanDoUnit");
        }
        if find_trigger_callback(name).is_some() {
            slots.push("EventTriggerInfo.PythonCallback");
        }
        if find_event_can_do(name).is_some() {
            slots.push("EventInfo.PythonCanDo");
        }
        if find_event_callback(name).is_some() {
            slots.push("EventInfo.PythonCallback");
        }
        if find_event_expire(name).is_some() {
            slots.push("EventInfo.PythonExpireCheck");
        }
        slots
    }

    fn all_names() -> Vec<&'static str> {
        let mut names = Vec::new();
        names.extend(TRIGGER_CAN_DO.iter().map(|&(n, _)| n));
        names.extend(TRIGGER_CITY.iter().map(|&(n, _)| n));
        names.extend(TRIGGER_UNIT.iter().map(|&(n, _)| n));
        names.extend(TRIGGER_CALLBACK.iter().map(|&(n, _)| n));
        names.extend(EVENT_CAN_DO.iter().map(|&(n, _)| n));
        names.extend(EVENT_CALLBACK.iter().map(|&(n, _)| n));
        names.extend(EVENT_EXPIRE.iter().map(|&(n, _)| n));
        names
    }

    /// 232 callbacks (every function of Python lines 1-3303 but the 28 help-text functions and
    /// `weddingFeud3Callback`), each registered once.
    #[test]
    fn every_callback_is_registered_once() {
        let names = all_names();
        assert_eq!(names.len(), 232);
        let unique: BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len(), "a name is registered twice");
        for name in names {
            assert_eq!(slot_of(name).len(), 1, "{name} is in more than one slot");
        }
        assert!(find_trigger_can_do("canTriggerDustbowlCont").is_none(), "part b");
    }

    /// Every callback string in the event XML that names a function of this part is registered in
    /// the slot that names it; 213 distinct names are referenced.
    #[test]
    fn xml_callbacks_resolve_in_their_slot() {
        let d = ffh_data::global();
        let mut referenced = BTreeSet::new();
        let mut check = |slot: &'static str, s: &str| {
            let Some(call) = PyCall::parse(s) else { return };
            let slots = slot_of(&call.name);
            if !slots.is_empty() {
                assert_eq!(slots, vec![slot], "{} is registered in the wrong slot", call.name);
                referenced.insert(call.name.clone());
            }
        };
        for t in d.event_triggers.keys() {
            let info = &d.event_triggers[t];
            check("EventTriggerInfo.PythonCanDo", &info.python_can_do);
            check("EventTriggerInfo.PythonCanDoCity", &info.python_can_do_city);
            check("EventTriggerInfo.PythonCanDoUnit", &info.python_can_do_unit);
            check("EventTriggerInfo.PythonCallback", &info.python_callback);
        }
        for e in d.events.keys() {
            let info = &d.events[e];
            check("EventInfo.PythonCanDo", &info.python_can_do);
            check("EventInfo.PythonCallback", &info.python_callback);
            check("EventInfo.PythonExpireCheck", &info.python_expire_check);
        }
        assert_eq!(referenced.len(), 213);
    }

    /// Every hard-coded type name in this file (`d.t` with a string literal) exists in the data:
    /// `t` panics on unknown names, and most of this code only runs when an event fires.
    #[test]
    fn hard_coded_type_names_exist() {
        let d = ffh_data::global();
        let src = include_str!("random_events_a.rs");
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
                "AlignmentType" => d.ty::<AlignmentType>(name).is_some(),
                "BonusType" => d.ty::<BonusType>(name).is_some(),
                "BuildingType" => d.ty::<BuildingType>(name).is_some(),
                "CivicOptionType" => d.ty::<CivicOptionType>(name).is_some(),
                "CivicType" => d.ty::<CivicType>(name).is_some(),
                "CivilizationType" => d.ty::<CivilizationType>(name).is_some(),
                "DamageType" => d.ty::<DamageType>(name).is_some(),
                "EventType" => d.ty::<EventType>(name).is_some(),
                "FeatureType" => d.ty::<FeatureType>(name).is_some(),
                "ImprovementType" => d.ty::<ImprovementType>(name).is_some(),
                "LeaderHeadType" => d.ty::<LeaderHeadType>(name).is_some(),
                "PromotionType" => d.ty::<PromotionType>(name).is_some(),
                "ReligionType" => d.ty::<ReligionType>(name).is_some(),
                "SpecialistType" => d.ty::<SpecialistType>(name).is_some(),
                "TechType" => d.ty::<TechType>(name).is_some(),
                "TerrainType" => d.ty::<TerrainType>(name).is_some(),
                "TraitType" => d.ty::<TraitType>(name).is_some(),
                "UnitClassType" => d.ty::<UnitClassType>(name).is_some(),
                "UnitType" => d.ty::<UnitType>(name).is_some(),
                other => panic!("type {other} not handled by this test ({name})"),
            };
            assert!(found, "unknown {ty} {name}");
            checked += 1;
        }
        assert!(checked > 150, "only {checked} names found");
        // Names passed through tables or closures.
        let (temples, traits) = (
            [
                ("RELIGION_THE_ORDER", "BUILDING_TEMPLE_OF_THE_ORDER"),
                ("RELIGION_FELLOWSHIP_OF_LEAVES", "BUILDING_TEMPLE_OF_LEAVES"),
                ("RELIGION_THE_ASHEN_VEIL", "BUILDING_TEMPLE_OF_THE_VEIL"),
                ("RELIGION_OCTOPUS_OVERLORDS", "BUILDING_TEMPLE_OF_THE_OVERLORDS"),
                ("RELIGION_RUNES_OF_KILMORPH", "BUILDING_TEMPLE_OF_KILMORPH"),
                ("RELIGION_THE_EMPYREAN", "BUILDING_TEMPLE_OF_THE_EMPYREAN"),
            ],
            ["TRAIT_AGGRESSIVE", "TRAIT_ORGANIZED", "TRAIT_PHILOSOPHICAL", "TRAIT_RAIDERS", "TRAIT_SPIRITUAL"],
        );
        for (r, b) in temples {
            assert!(d.ty::<ReligionType>(r).is_some() && d.ty::<BuildingType>(b).is_some(), "{r} {b}");
        }
        for t in traits {
            assert!(d.ty::<TraitType>(t).is_some(), "{t}");
        }
        for c in ["CIVIC_DESPOTISM", "CIVIC_GOD_KING", "CIVIC_ARISTOCRACY", "CIVIC_CITY_STATES", "CIVIC_REPUBLIC", "CIVIC_THEOCRACY"] {
            assert!(d.ty::<CivicType>(c).is_some(), "{c}");
        }
        for c in ["UNITCLASS_GALLEY", "UNITCLASS_CARAVEL", "UNITCLASS_GALLEON"] {
            assert!(d.ty::<UnitClassType>(c).is_some(), "{c}");
        }
        for b in ["BONUS_COPPER", "BONUS_IRON", "BONUS_HORSE", "BONUS_IVORY"] {
            assert!(d.ty::<BonusType>(b).is_some(), "{b}");
        }
        // `findInfoTypeNum` names: present in FFH2 ...
        for i in ["IMPROVEMENT_COTTAGE", "IMPROVEMENT_HAMLET", "IMPROVEMENT_VILLAGE", "IMPROVEMENT_TOWN", "IMPROVEMENT_CITY_RUINS", "IMPROVEMENT_PLANTATION"] {
            assert!(d.ty::<ImprovementType>(i).is_some(), "{i}");
        }
        assert!(d.ty::<SpecialBuildingType>("SPECIALBUILDING_TEMPLE").is_some());
        // ... and the BtS names 0.41 does not define (-1 in Python).
        assert!(d.ty::<BonusType>("BONUS_SPICES").is_none());
        assert!(d.ty::<SpecialBuildingType>("SPECIALBUILDING_CATHEDRAL").is_none());
        assert!(d.ty::<SpecialBuildingType>("SPECIALBUILDING_MONASTERY").is_none());
    }

    /// A game whose first slots hold freshly reset players (C++ `reset`), alive on their own
    /// teams, with the given civilizations/leaders.
    fn game(civs: &[(&str, &str)]) -> Game {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, civs.len(), "WORLDSIZE_DUEL", 7);
        for (i, (civ, leader)) in civs.iter().enumerate() {
            setup.players[i].civ = Some(d.t::<CivilizationType>(civ));
            setup.players[i].leader = Some(d.t::<LeaderHeadType>(leader));
        }
        let mut g = Game::new(d, setup);
        let (n_players, n_teams) = (g.num_players(), g.num_teams());
        for i in 0..civs.len() {
            let p = PlayerId(i as u8);
            g.players[p] = Player::new(d, p, &g.setup.players[i], n_players, n_teams);
            g.players[p].alive = true;
            g.players[p].ever_alive = true;
        }
        g
    }

    /// A small map (all ocean plots, x wraps) on `g`.
    fn with_map(g: &mut Game, w: i32, h: i32) {
        let init = crate::map::MapInitData { grid_w: w, grid_h: h, wrap_x: true, ..Default::default() };
        g.map = crate::map::Map::new(g.d, &init, g.num_players(), g.num_teams());
    }

    fn data(p: PlayerId) -> EventTriggeredData {
        EventTriggeredData {
            id: TriggeredId(1),
            trigger: EventTriggerType(0),
            turn: 0,
            player: p,
            city: None,
            plot_x: INVALID_PLOT_COORD,
            plot_y: INVALID_PLOT_COORD,
            unit: None,
            other_player: None,
            other_player_city: None,
            religion: None,
            corporation: None,
            building: None,
            text: String::new(),
            global_text: String::new(),
        }
    }

    const INVALID_PLOT_COORD: i32 = crate::defs::INVALID_PLOT_COORD;

    #[test]
    fn signs_of_esus_and_lugus_change_game_crime() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let before = g.crime;
        let p = PlayerId(0);
        do_sign_esus(&mut g, &data(p), p);
        assert_eq!(g.crime, before + 5);
        do_sign_lugus(&mut g, &data(p), p);
        do_sign_lugus(&mut g, &data(p), p);
        assert_eq!(g.crime, before - 5);
    }

    #[test]
    fn pick_alignment_sets_the_alignment() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let event = EventType(0);
        let ev = data(PlayerId(0));
        do_pick_alignment3(&mut g, event, &ev);
        assert_eq!(g.players[PlayerId(0)].alignment, d.ty::<AlignmentType>("ALIGNMENT_EVIL"));
        do_pick_alignment1(&mut g, event, &ev);
        assert_eq!(g.players[PlayerId(0)].alignment, d.ty::<AlignmentType>("ALIGNMENT_GOOD"));
        do_pick_alignment2(&mut g, event, &ev);
        assert_eq!(g.players[PlayerId(0)].alignment, d.ty::<AlignmentType>("ALIGNMENT_NEUTRAL"));
    }

    #[test]
    fn attitude_events_change_attitude_extra() {
        let mut g = game(&[
            ("CIVILIZATION_BANNOR", "LEADER_CAPRIA"),
            ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL"),
            ("CIVILIZATION_CALABIM", "LEADER_ALEXIS"),
        ]);
        let (capria, arendel, alexis) = (PlayerId(0), PlayerId(1), PlayerId(2));
        let event = EventType(0);
        // Capria likes the event player (Arendel) more; nobody else changes.
        apply_pron_capria(&mut g, event, &data(arendel));
        assert_eq!(g.players[capria].ai_attitude_extra[arendel], 4);
        assert_eq!(g.players[arendel].ai_attitude_extra[capria], 0);
        // No such leader in the game: nothing happens.
        apply_pron_hannah(&mut g, event, &data(arendel));
        // The Calabim (Alexis' civilization) dislike the player.
        do_calabim_sanctuary1(&mut g, event, &data(arendel));
        assert_eq!(g.players[alexis].ai_attitude_extra[arendel], -4);
        // Sign of Camulos: the player and every living civ player of another team, both ways.
        g.players[alexis].alive = false;
        do_sign_camulos(&mut g, &data(capria), capria);
        assert_eq!(g.players[capria].ai_attitude_extra[arendel], 3);
        assert_eq!(g.players[arendel].ai_attitude_extra[capria], -1);
        assert_eq!(g.players[alexis].ai_attitude_extra[capria], 0, "dead players are skipped");
        assert_eq!(g.players[capria].ai_attitude_extra[capria], 0, "not towards itself");
        do_sign_dagda(&mut g, &data(capria), capria);
        assert_eq!(g.players[arendel].ai_attitude_extra[capria], 0);
    }

    /// `pPlayer2.AI_getAttitude(iPlayer)` is the player-level attitude: a leader whose team is an
    /// uncapitulated vassal of the event player's team is Friendly towards him, whatever other
    /// vassals the master has (the team-level average would count them in).
    #[test]
    fn pron_reads_the_player_attitude() {
        let mut g = game(&[
            ("CIVILIZATION_BANNOR", "LEADER_CAPRIA"),
            ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL"),
            ("CIVILIZATION_CALABIM", "LEADER_ALEXIS"),
        ]);
        let (capria, arendel, alexis) = (PlayerId(0), PlayerId(1), PlayerId(2));
        let (tc, ta, tx) = (g.players[capria].team, g.players[arendel].team, g.players[alexis].team);
        g.teams[tc].vassal[ta] = true;
        g.teams[tx].vassal[ta] = true;
        g.teams[ta].has_met[tc] = true;
        assert_eq!(ai_get_attitude(&g, capria, arendel), AttitudeType::ATTITUDE_FRIENDLY);
        assert_eq!(g.team_ai_get_attitude(tc, ta, true), AttitudeType::ATTITUDE_PLEASED);
        assert!(can_trigger_pron_capria(&mut g, &mut data(arendel)));
    }

    /// Counts the free-tech choices the rules ask a controller for.
    struct CountFreeTechs(std::sync::atomic::AtomicUsize);

    impl crate::controller::Controller for CountFreeTechs {
        fn choose_free_tech(&self, _g: &Game, _p: PlayerId) -> Option<TechType> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            None
        }
    }

    /// `chooseTech` only opens a popup, which `CvPlayer::addPopup` deletes for AI players.
    #[test]
    fn iron_orb_tech_is_a_human_popup() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let p = PlayerId(0);
        let counter = std::sync::Arc::new(CountFreeTechs(AtomicUsize::new(0)));
        g.controllers[p] = counter.clone();
        apply_iron_orb3(&mut g, EventType(0), &data(p));
        assert_eq!(counter.0.load(Ordering::Relaxed), 0, "AI: no tech choice");
        g.players[p].human = true;
        apply_iron_orb3(&mut g, EventType(0), &data(p));
        assert_eq!(counter.0.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn forced_attitudes() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_CAPRIA"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")]);
        let (a, b) = (PlayerId(0), PlayerId(1));
        let barbarian = g.barbarian_player();
        // Game::new holds a placeholder there; player_init puts it on the barbarian team.
        g.players[barbarian].team = g.barbarian_team();
        assert_ne!(g.players[a].team, g.players[b].team);
        assert_eq!(ai_get_attitude(&g, a, barbarian), AttitudeType::ATTITUDE_FURIOUS);
        assert_eq!(ai_get_attitude(&g, barbarian, a), AttitudeType::ATTITUDE_FURIOUS);
        g.players[b].team = g.players[a].team;
        assert_eq!(ai_get_attitude(&g, a, b), AttitudeType::ATTITUDE_FRIENDLY);
    }

    #[test]
    fn trait_events_respect_the_permanent_trait() {
        let g = game(&[("CIVILIZATION_BANNOR", "LEADER_CAPRIA")]);
        let d = g.d;
        let leader = d.t::<LeaderHeadType>("LEADER_CAPRIA");
        let ev = data(PlayerId(0));
        let event = EventType(0);
        let permanent = d.leader_heads[leader].permanent_trait;
        for (name, f) in [
            ("TRAIT_AGGRESSIVE", can_apply_trait_aggressive as EventCanDoFn),
            ("TRAIT_ARCANE", can_apply_trait_arcane),
            ("TRAIT_CHARISMATIC", can_apply_trait_charismatic),
            ("TRAIT_CREATIVE", can_apply_trait_creative),
            ("TRAIT_EXPANSIVE", can_apply_trait_expansive),
            ("TRAIT_FINANCIAL", can_apply_trait_financial),
            ("TRAIT_INDUSTRIOUS", can_apply_trait_industrious),
            ("TRAIT_ORGANIZED", can_apply_trait_organized),
            ("TRAIT_PHILOSOPHICAL", can_apply_trait_philosophical),
            ("TRAIT_RAIDERS", can_apply_trait_raiders),
            ("TRAIT_SPIRITUAL", can_apply_trait_spiritual),
        ] {
            assert_eq!(f(&g, event, &ev), permanent != d.ty::<TraitType>(name), "{name}");
        }
    }

    #[test]
    fn gold_events() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let p = PlayerId(0);
        let event = EventType(0);
        let ev = data(p);
        // No cities: the cost is 0.
        g.players[p].gold = 0;
        assert!(can_do_wedding_feud3(&g, event, &ev) && can_apply_bard_tale3(&g, event, &ev));
        apply_bard_tale3(&mut g, event, &ev);
        assert_eq!(g.players[p].gold, 0);
    }

    #[test]
    fn plot_triggers_read_the_trigger_plot() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")]);
        with_map(&mut g, 5, 4);
        let (a, b) = (PlayerId(0), PlayerId(1));
        let event = EventType(0);
        let mut ev = data(a);
        // No plot: the NULL plot.
        assert!(!can_trigger_plot_empty(&mut g, &mut ev));
        assert!(!can_trigger_penguins(&mut g, &mut ev));
        assert!(can_trigger_holy_mountain(&mut g, &mut ev));
        assert!(!expire_holy_mountain1(&g, event, &ev));
        ev.plot_x = 2;
        ev.plot_y = 1;
        let plot = g.map.plot_xy(2, 1).unwrap();
        assert!(can_trigger_plot_empty(&mut g, &mut ev));
        // Ocean all around: adjacent to water and not a peak.
        assert!(can_trigger_penguins(&mut g, &mut ev));
        g.map.plots[plot].plot_type = PlotType::PLOT_PEAK;
        assert!(!can_trigger_penguins(&mut g, &mut ev));
        // Holy Mountain: free while unowned, expires once another player owns it.
        g.map.plots[plot].owner = Some(a);
        assert!(!can_trigger_holy_mountain(&mut g, &mut ev));
        assert!(!expire_holy_mountain1(&g, event, &ev));
        g.map.plots[plot].owner = Some(b);
        assert!(expire_holy_mountain1(&g, event, &ev));
        // Volcano: an improvement on a neighbour (not the volcano plot itself) counts.
        let mushrooms = g.d.t::<ImprovementType>("IMPROVEMENT_MUSHROOMS");
        g.map.plots[plot].improvement_type = Some(mushrooms);
        assert!(!can_apply_volcano1(&g, event, &ev));
        let neighbour = g.map.plot_xy(3, 2).unwrap();
        g.map.plots[neighbour].improvement_type = Some(mushrooms);
        assert!(can_apply_volcano1(&g, event, &ev));
        assert_eq!(volcano_plots(&g, &ev), vec![neighbour]);
    }

    #[test]
    fn flares_change_plot_and_global_counters() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        with_map(&mut g, 6, 6);
        g.global_counter_limit = 100;
        let p = PlayerId(0);
        let mut ev = data(p);
        ev.plot_x = 0;
        ev.plot_y = 0;
        do_flare_entropy_node(&mut g, &ev, p);
        assert_eq!(g.global_counter, 2);
        // 3x3 around (0, 0): x wraps to 5, y = -1 is off the map.
        for (x, y) in [(5, 0), (5, 1), (0, 0), (0, 1), (1, 0), (1, 1)] {
            assert_eq!(g.map.plots[g.map.plot_xy(x, y).unwrap()].plot_counter, 100, "({x}, {y})");
        }
        assert_eq!(g.map.plots[g.map.plot_xy(2, 0).unwrap()].plot_counter, 0);
        assert_eq!(g.map.plots[g.map.plot_xy(0, 2).unwrap()].plot_counter, 0);
        // Life node: 5x5 back down to 0 (clamped), -2 on the Armageddon counter.
        do_flare_life_node(&mut g, &ev, p);
        assert_eq!(g.global_counter, 0);
        assert!(g.map.plots.iter().all(|plot| plot.plot_counter == 0));
    }

    #[test]
    fn square_handles_invalid_coordinates() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        with_map(&mut g, 4, 3);
        let sq = square(&g, INVALID_PLOT_COORD, INVALID_PLOT_COORD, 2);
        assert_eq!(sq.len(), 25);
        assert!(sq.iter().all(|plot| plot.is_none()));
    }
}
