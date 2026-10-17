//! Event hooks: port of the rules parts of CvEventManager.py, called where the DLL
//! calls `CvEventReporter::getInstance().<event>(..)`.
//!
//! Call sites in ported rules code must call these at exactly the point the C++
//! calls the reporter (often before the object is destroyed). Handlers whose Python
//! is UI/logging only are empty by design (each says so).
//!
//! Porting notes:
//! - Scenarios are out of scope: every `CyGame().getWBMapScript()` branch (the
//!   ScenarioFunctions `sf.*` calls) takes the "not a scenario" path.
//! - UI-only Python is omitted: popups (wonder/tech/religion movies, Dawn of Man, the civics
//!   screen, `cf.addPopup`, `sf.addPopupWB`), `CyEngine` landmarks and effects,
//!   `CvTopCivs.turnChecker`, the `CvAdvisorUtils` nags and city advice, and `CvUtil.pyPrint`
//!   debug output. `CyInterface().addMessage` texts go to the game log (`ev_msg`).
//! - Human-only popup choice without an AI branch: after the Mercurian Gate and the Infernal
//!   Pact a human founder is offered control of the new player (`reassignPlayer`); there is
//!   no controller hook for it, so it is not ported.
//! - AI policy (docs/PORTING.md, "Changes from FFH2"): `cf.warScript` (onBeginPlayerTurn)
//!   and the Sephi UNITAI choice for AI-built adepts, devouts and priests of leaves
//!   (onUnitBuilt) are left to the new AI. AI bookkeeping applied to every player (player
//!   conquest mode, the unit group flag set by `CyUnit.startConquestMode`, the vampires'
//!   UNITAI) is ported as field writes / `AI_setUnitAIType`.
//! - `CvAdvisorUtils` feats (`unitBuiltFeats`, `buildingBuiltFeats`, `endTurnFeats`) set
//!   player feat flags; they are ported (`ev_*_feats`), only their popups are UI.
//! - Helpers from other Python modules with game-state effects are ported here:
//!   `cf.showUniqueImprovements` (reveals plots; not in custom.rs) and the Somnium card
//!   game's `cs.doTurn()` (memory decay, `ev_somnium_do_turn`).
//! - `PyPlayer(p).getUnitList()` (BtS PyHelpers) skips dead units (`ev_unit_list`);
//!   `getCityList()` is the owner's city list in order. Both are snapshots: a unit destroyed
//!   while its list is walked (Python would hold a dangling CyUnit) is skipped.
//! - Python `lList[getSorenRandNum(len(lList), s) - 1]` reads the last element on a roll of
//!   0 (`py_list_index`). Python 2 `/` on ints floors (`py_div`).
//! - `CyMap().plot(x, y)` is `self.map.plot_xy(x, y)`; `None` is the Python NULL plot.
//! - `pPlot.getUnit(i)` loops re-read the plot's unit list each step; an index past its end
//!   is a NULL CyUnit whose calls do nothing.

#![allow(unused_variables)]

use crate::ffh::game_utils::py_div;
use crate::game::Game;
use crate::ids::*;
use crate::utils::{is_national_unit_class, is_national_wonder_class, is_world_unit_class};
use ffh_data::*;

/// C++ `GROUPFLAG_CONQUEST` (CvDefines.h, Better AI group flags): set by
/// `CyUnit::startConquestMode` (`AI_setGroupflag(GROUPFLAG_CONQUEST)`).
const GROUPFLAG_CONQUEST: i32 = 10;

/// onUnitCreated: mana bonus -> (tier 1, tier 2, tier 3) promotions for new adepts.
const ADEPT_MANA_PROMOTIONS: [(&str, &str, &str, &str); 18] = [
    ("BONUS_MANA_AIR", "PROMOTION_AIR1", "PROMOTION_AIR2", "PROMOTION_AIR3"),
    ("BONUS_MANA_BODY", "PROMOTION_BODY1", "PROMOTION_BODY2", "PROMOTION_BODY3"),
    ("BONUS_MANA_CHAOS", "PROMOTION_CHAOS1", "PROMOTION_CHAOS2", "PROMOTION_CHAOS3"),
    ("BONUS_MANA_DEATH", "PROMOTION_DEATH1", "PROMOTION_DEATH2", "PROMOTION_DEATH3"),
    ("BONUS_MANA_EARTH", "PROMOTION_EARTH1", "PROMOTION_EARTH2", "PROMOTION_EARTH3"),
    ("BONUS_MANA_ENCHANTMENT", "PROMOTION_ENCHANTMENT1", "PROMOTION_ENCHANTMENT2", "PROMOTION_ENCHANTMENT3"),
    ("BONUS_MANA_ENTROPY", "PROMOTION_ENTROPY1", "PROMOTION_ENTROPY2", "PROMOTION_ENTROPY3"),
    ("BONUS_MANA_FIRE", "PROMOTION_FIRE1", "PROMOTION_FIRE2", "PROMOTION_FIRE3"),
    ("BONUS_MANA_ICE", "PROMOTION_ICE1", "PROMOTION_ICE2", "PROMOTION_ICE3"),
    ("BONUS_MANA_LAW", "PROMOTION_LAW1", "PROMOTION_LAW2", "PROMOTION_LAW3"),
    ("BONUS_MANA_LIFE", "PROMOTION_LIFE1", "PROMOTION_LIFE2", "PROMOTION_LIFE3"),
    ("BONUS_MANA_METAMAGIC", "PROMOTION_METAMAGIC1", "PROMOTION_METAMAGIC2", "PROMOTION_METAMAGIC3"),
    ("BONUS_MANA_MIND", "PROMOTION_MIND1", "PROMOTION_MIND2", "PROMOTION_MIND3"),
    ("BONUS_MANA_NATURE", "PROMOTION_NATURE1", "PROMOTION_NATURE2", "PROMOTION_NATURE3"),
    ("BONUS_MANA_SHADOW", "PROMOTION_SHADOW1", "PROMOTION_SHADOW2", "PROMOTION_SHADOW3"),
    ("BONUS_MANA_SPIRIT", "PROMOTION_SPIRIT1", "PROMOTION_SPIRIT2", "PROMOTION_SPIRIT3"),
    ("BONUS_MANA_SUN", "PROMOTION_SUN1", "PROMOTION_SUN2", "PROMOTION_SUN3"),
    ("BONUS_MANA_WATER", "PROMOTION_WATER1", "PROMOTION_WATER2", "PROMOTION_WATER3"),
];

/// onCityBuilt / onCityAcquired: the buildings every Infernal city gets.
const INFERNAL_CITY_BUILDINGS: [&str; 6] = [
    "BUILDING_ELDER_COUNCIL",
    "BUILDING_TRAINING_YARD",
    "BUILDING_OBSIDIAN_GATE",
    "BUILDING_FORGE",
    "BUILDING_MAGE_GUILD",
    "BUILDING_DEMONIC_CITIZENS",
];

/// onVictory: winner civilization -> trophy.
const VICTORY_CIV_TROPHIES: [(&str, &str); 21] = [
    ("CIVILIZATION_AMURITES", "TROPHY_VICTORY_AMURITES"),
    ("CIVILIZATION_BALSERAPHS", "TROPHY_VICTORY_BALSERAPHS"),
    ("CIVILIZATION_BANNOR", "TROPHY_VICTORY_BANNOR"),
    ("CIVILIZATION_CALABIM", "TROPHY_VICTORY_CALABIM"),
    ("CIVILIZATION_CLAN_OF_EMBERS", "TROPHY_VICTORY_CLAN_OF_EMBERS"),
    ("CIVILIZATION_DOVIELLO", "TROPHY_VICTORY_DOVIELLO"),
    ("CIVILIZATION_ELOHIM", "TROPHY_VICTORY_ELOHIM"),
    ("CIVILIZATION_GRIGORI", "TROPHY_VICTORY_GRIGORI"),
    ("CIVILIZATION_HIPPUS", "TROPHY_VICTORY_HIPPUS"),
    ("CIVILIZATION_ILLIANS", "TROPHY_VICTORY_ILLIANS"),
    ("CIVILIZATION_INFERNAL", "TROPHY_VICTORY_INFERNAL"),
    ("CIVILIZATION_KHAZAD", "TROPHY_VICTORY_KHAZAD"),
    ("CIVILIZATION_KURIOTATES", "TROPHY_VICTORY_KURIOTATES"),
    ("CIVILIZATION_LANUN", "TROPHY_VICTORY_LANUN"),
    ("CIVILIZATION_LJOSALFAR", "TROPHY_VICTORY_LJOSALFAR"),
    ("CIVILIZATION_LUCHUIRP", "TROPHY_VICTORY_LUCHUIRP"),
    ("CIVILIZATION_MALAKIM", "TROPHY_VICTORY_MALAKIM"),
    ("CIVILIZATION_MERCURIANS", "TROPHY_VICTORY_MERCURIANS"),
    ("CIVILIZATION_SHEAIM", "TROPHY_VICTORY_SHEAIM"),
    ("CIVILIZATION_SIDAR", "TROPHY_VICTORY_SIDAR"),
    ("CIVILIZATION_SVARTALFAR", "TROPHY_VICTORY_SVARTALFAR"),
];

/// onVictory: victory type -> trophy.
const VICTORY_TYPE_TROPHIES: [(&str, &str); 8] = [
    ("VICTORY_ALTAR_OF_THE_LUONNOTAR", "TROPHY_VICTORY_ALTAR_OF_THE_LUONNOTAR"),
    ("VICTORY_CONQUEST", "TROPHY_VICTORY_CONQUEST"),
    ("VICTORY_CULTURAL", "TROPHY_VICTORY_CULTURAL"),
    ("VICTORY_DOMINATION", "TROPHY_VICTORY_DOMINATION"),
    ("VICTORY_RELIGIOUS", "TROPHY_VICTORY_RELIGIOUS"),
    ("VICTORY_SCORE", "TROPHY_VICTORY_SCORE"),
    ("VICTORY_TIME", "TROPHY_VICTORY_TIME"),
    ("VICTORY_TOWER_OF_MASTERY", "TROPHY_VICTORY_TOWER_OF_MASTERY"),
];

impl Game {
    /// `onGameStart` (`gameStart`; FfH also calls it at the end of `regenerateMap`):
    /// GAMEOPTION_THAW temporary terrain, unique improvements revealed to human Elohim, the
    /// introduction trophy and Scorpion Clan archers on goblin forts.
    pub fn ev_game_start(&mut self) {
        let d = self.d;
        // Scenario: `sf.gameStart()`; otherwise the intro movie (UI).
        if self.is_option(GameOptionType::GAMEOPTION_THAW) {
            let desert = d.t::<TerrainType>("TERRAIN_DESERT");
            let grass = d.t::<TerrainType>("TERRAIN_GRASS");
            let plains = d.t::<TerrainType>("TERRAIN_PLAINS");
            let snow = d.t::<TerrainType>("TERRAIN_SNOW");
            let tundra = d.t::<TerrainType>("TERRAIN_TUNDRA");
            for plot in self.map.plot_ids() {
                if self.map.plots[plot].feature_type.is_none()
                    && self.map.plots[plot].improvement_type.is_none()
                    && !self.plot_is_water(plot)
                {
                    let terrain = self.map.plots[plot].terrain_type;
                    if terrain == Some(tundra) {
                        let timer = self.soren_rand_num(90, "Bob") + 10;
                        self.plot_set_temp_terrain_type(plot, snow, timer);
                    }
                    if terrain == Some(grass) {
                        let timer = self.soren_rand_num(90, "Bob") + 10;
                        self.plot_set_temp_terrain_type(plot, tundra, timer);
                    }
                    if terrain == Some(plains) {
                        let timer = self.soren_rand_num(90, "Bob") + 10;
                        self.plot_set_temp_terrain_type(plot, tundra, timer);
                    }
                    if terrain == Some(desert) {
                        let timer = self.soren_rand_num(90, "Bob") + 10;
                        self.plot_set_temp_terrain_type(plot, plains, timer);
                    }
                }
            }
        }

        let elohim = d.t::<CivilizationType>("CIVILIZATION_ELOHIM");
        for p in self.player_ids() {
            if self.players[p].alive && self.players[p].human && self.player_get_civilization_type(p) == Some(elohim) {
                self.ev_show_unique_improvements(p);
            }
        }

        if !self.is_network_multi_player() {
            let trophy = "TROPHY_FEAT_INTRODUCTION";
            if !self.is_has_trophy(trophy) {
                self.change_trophy_value(trophy, 1);
                // UI: sf.addPopupWB(TXT_KEY_FFH_INTRO).
            }
        }

        if !self.is_option(GameOptionType::GAMEOPTION_NO_BARBARIANS) {
            let goblin_fort = d.t::<ImprovementType>("IMPROVEMENT_GOBLIN_FORT");
            let archer = d.t::<UnitType>("UNIT_ARCHER_SCORPION_CLAN");
            let barbarian = self.barbarian_player();
            for plot in self.map.plot_ids() {
                if self.map.plots[plot].improvement_type == Some(goblin_fort) {
                    let (x, y) = self.map.xy(plot);
                    self.player_init_unit(barbarian, archer, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
                }
            }
        }

        // UI: Dawn of Man popups for humans at START_YEAR (or `setSoundSelectionReady`), PBEM
        // details popups, CvAdvisorUtils.resetNoLiberateCities().
    }

    /// `CvEventReporter::gameEnd` -> `onGameEnd`: logging only.
    pub fn ev_game_end(&mut self) {}

    /// `beginGameTurn` -> `onBeginGameTurn`: Orthus appears (75 turns on normal speed, scaled),
    /// hell terrain spreads (`doHellTerrain`), Somnium memories decay (`cs.doTurn`).
    pub fn ev_begin_game_turn(&mut self, game_turn: i32) {
        let d = self.d;
        let orthus_turn = 75;
        if !self.is_unit_class_maxed_out(d.t::<UnitClassType>("UNITCLASS_ORTHUS"), 0) {
            if !self.is_option(GameOptionType::GAMEOPTION_NO_ORTHUS) {
                let mut orthus = false;
                let speed = self.game_speed_type();
                if speed == d.t::<GameSpeedType>("GAMESPEED_QUICK") && game_turn >= py_div(orthus_turn, 3) * 2 {
                    orthus = true;
                }
                if speed == d.t::<GameSpeedType>("GAMESPEED_NORMAL") && game_turn >= orthus_turn {
                    orthus = true;
                }
                if speed == d.t::<GameSpeedType>("GAMESPEED_EPIC") && game_turn >= py_div(orthus_turn * 3, 2) {
                    orthus = true;
                }
                if speed == d.t::<GameSpeedType>("GAMESPEED_MARATHON") && game_turn >= orthus_turn * 3 {
                    orthus = true;
                }
                if orthus {
                    self.cf_add_unit(d.t::<UnitType>("UNIT_ORTHUS"));
                    // UI: cf.addPopup(TXT_KEY_POPUP_ORTHUS_CREATION).
                }
            }
        }

        if !self.is_option(GameOptionType::GAMEOPTION_NO_PLOT_COUNTER) {
            self.cf_do_hell_terrain();
        }

        // Scenario: sf.doTurn().

        // FfH Card Game: cs.doTurn()
        self.ev_somnium_do_turn();

        // UI: CvTopCivs.turnChecker(iGameTurn) (top civilizations screen).
    }

    /// `endGameTurn` -> `onEndGameTurn`: does nothing.
    pub fn ev_end_game_turn(&mut self, game_turn: i32) {}

    /// `beginPlayerTurn` -> `onBeginPlayerTurn`: religion eras, Crusade, Khazad/Luchuirp turns,
    /// Insane and Adaptive trait events, barbarians declare war on leading human barbarian-trait
    /// players.
    pub fn ev_begin_player_turn(&mut self, game_turn: i32, p: PlayerId) {
        let d = self.d;
        // The state religion picks the era (FfH religion art eras); `setCurrentEra` has only
        // graphics side effects (docs/porting/player.md).
        const RELIGION_ERAS: [(&str, &str); 7] = [
            ("RELIGION_THE_ASHEN_VEIL", "ERA_VEIL"),
            ("RELIGION_THE_ORDER", "ERA_ORDE"),
            ("RELIGION_FELLOWSHIP_OF_LEAVES", "ERA_LEAF"),
            ("RELIGION_RUNES_OF_KILMORPH", "ERA_RUNE"),
            ("RELIGION_OCTOPUS_OVERLORDS", "ERA_OCTO"),
            ("RELIGION_THE_EMPYREAN", "ERA_EMPY"),
            ("RELIGION_COUNCIL_OF_ESUS", "ERA_COUN"),
        ];
        for (religion, era) in RELIGION_ERAS {
            if self.player_get_state_religion(p) == Some(d.t::<ReligionType>(religion)) {
                let era = d.t::<EraType>(era);
                if self.players[p].current_era != era {
                    self.players[p].current_era = era;
                }
            }
        }

        // AI: `if not pPlayer.isHuman(): cf.warScript(iPlayer)` (not in scenarios) -- AI war
        // policy, replaced by the new AI (docs/PORTING.md).

        let cultural_values = d.t::<CivicOptionType>("CIVICOPTION_CULTURAL_VALUES");
        if self.players[p].civics[cultural_values] == Some(d.t::<CivicType>("CIVIC_CRUSADE")) {
            self.cf_do_crusade(p);
        }

        if self.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_KHAZAD")) {
            self.cf_do_turn_khazad(p);
        }

        if self.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_LUCHUIRP")) {
            self.cf_do_turn_luchuirp(p);
        }

        // Better AI: Doviello `startConquestMode()` is commented out in the Python.

        if self.players[p].traits[d.t::<TraitType>("TRAIT_INSANE")] {
            if self.soren_rand_num(1000, "Insane") < 20 {
                let trigger = d.t::<EventTriggerType>("EVENTTRIGGER_TRAIT_INSANE");
                // Python passes -1 (not INVALID_PLOT_COORD) for the plot coordinates.
                self.player_init_triggered_data(p, trigger, true, None, -1, -1, Some(p), None, None, None, None, None);
            }
        }

        if self.players[p].traits[d.t::<TraitType>("TRAIT_ADAPTIVE")] {
            let mut cycle = 100;
            let speed = self.game_speed_type();
            if speed == d.t::<GameSpeedType>("GAMESPEED_QUICK") {
                cycle = 75;
            }
            if speed == d.t::<GameSpeedType>("GAMESPEED_EPIC") {
                cycle = 150;
            }
            if speed == d.t::<GameSpeedType>("GAMESPEED_MARATHON") {
                cycle = 300;
            }
            for i in 0..10 {
                if i * cycle - 5 == game_turn {
                    let trigger = d.t::<EventTriggerType>("EVENTTRIGGER_TRAIT_ADAPTIVE");
                    self.player_init_triggered_data(p, trigger, true, None, -1, -1, Some(p), None, None, None, None, None);
                }
            }
        }

        if self.players[p].human {
            if self.players[p].traits[d.t::<TraitType>("TRAIT_BARBARIAN")] {
                let barbarian_team = self.players[self.barbarian_player()].team;
                let team = self.players[p].team;
                if !self.teams[barbarian_team].at_war[team] {
                    let runner_up = self.rank_player[1];
                    if 2 * self.player_score[p] >= 3 * self.player_score[runner_up] {
                        if game_turn >= 20 {
                            self.team_declare_war(barbarian_team, team, false, Some(WarPlanType::WARPLAN_TOTAL));
                            // UI: popup TXT_KEY_POPUP_BARBARIAN_DECLARE_WAR for the active player.
                        }
                    }
                }
            }
        }
    }

    /// `endPlayerTurn` -> `onEndPlayerTurn`: advisor feats (`CvAdvisorUtils.endTurnFeats`).
    pub fn ev_end_player_turn(&mut self, game_turn: i32, p: PlayerId) {
        // UI: on the first elapsed turn, humans who can change civics get the civics screen
        // (BUTTONPOPUP_CHANGECIVIC); CvAdvisorUtils.resetAdvisorNags().
        self.ev_end_turn_feats(p);
    }

    /// `firstContact` -> `onFirstContact`: logging only.
    pub fn ev_first_contact(&mut self, team1: TeamId, team2: TeamId) {}

    /// `combatResult` -> `onCombatResult`: logging only.
    pub fn ev_combat_result(&mut self, winner: UnitId, loser: UnitId) {}

    /// `improvementBuilt` -> `onImprovementBuilt`: the Ring of Carcer sets a minimum level and
    /// holds Brigit.
    pub fn ev_improvement_built(&mut self, improvement: ImprovementType, x: i32, y: i32) {
        let d = self.d;
        if d.improvements[improvement].unique {
            // UI: CyEngine().addLandmark(pPlot, description).
            if improvement == d.t::<ImprovementType>("IMPROVEMENT_RING_OF_CARCER") {
                if let Some(plot) = self.map.plot_xy(x, y) {
                    // `setMinLevel`: field (docs/porting/map.md).
                    self.map.plots[plot].min_level = 15;
                    let barbarian = self.barbarian_player();
                    let (px, py) = self.map.xy(plot);
                    let brigit = d.t::<UnitType>("UNIT_BRIGIT_HELD");
                    self.player_init_unit(barbarian, brigit, px, py, None, Some(DirectionType::DIRECTION_SOUTH));
                }
            }
        }
    }

    /// `improvementDestroyed` -> `onImprovementDestroyed`: the Ring of Carcer's minimum level is
    /// lifted; a destroyed Necrototem lowers the Armageddon counter by 2.
    pub fn ev_improvement_destroyed(&mut self, improvement: ImprovementType, player: Option<PlayerId>, x: i32, y: i32) {
        let d = self.d;
        if d.improvements[improvement].unique {
            if let Some(plot) = self.map.plot_xy(x, y) {
                // UI: CyEngine().removeLandmark(pPlot).
                if improvement == d.t::<ImprovementType>("IMPROVEMENT_RING_OF_CARCER") {
                    self.map.plots[plot].min_level = -1;
                }
            }
        }

        if improvement == d.t::<ImprovementType>("IMPROVEMENT_NECROTOTEM") {
            self.change_global_counter(-2);
        }

        // Scenario: sf.onImprovementDestroyed(..).
    }

    /// `routeBuilt` -> `onRouteBuilt`: logging only.
    pub fn ev_route_built(&mut self, route: RouteType, x: i32, y: i32) {}

    /// `plotRevealed` -> `onPlotRevealed`: does nothing.
    pub fn ev_plot_revealed(&mut self, plot: PlotIdx, team: TeamId) {}

    /// `plotFeatureRemoved` -> `onPlotFeatureRemoved`: does nothing.
    pub fn ev_plot_feature_removed(&mut self, plot: PlotIdx, feature: FeatureType, city: Option<CityId>) {}

    /// `cityBuilt` -> `onCityBuilt`: Infernal cities start with the Ashen Veil, 3 population and
    /// their demonic buildings; barbarian cities get an orc garrison.
    pub fn ev_city_built(&mut self, c: CityId) {
        let d = self.d;
        let p = self.cities[c].owner;

        if self.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_INFERNAL")) {
            self.city_set_has_religion(c, d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL"), true, true, true);
            self.city_set_population(c, 3);
            for building in INFERNAL_CITY_BUILDINGS {
                self.city_set_num_real_building(c, d.t::<BuildingType>(building), 1);
            }
        }

        if self.player_get_civilization_type(p) == Some(d.t::<CivilizationType>("CIVILIZATION_BARBARIAN")) {
            let barbarian_team = self.players[self.barbarian_player()].team;
            let tech = |name: &str| Some(d.t::<TechType>(name));
            let start_era = self.start_era().i();
            let (x, y) = (self.cities[c].x, self.cities[c].y);

            let mut unit = d.t::<UnitType>("UNIT_WARRIOR");
            if self.team_is_has_tech(barbarian_team, tech("TECH_BRONZE_WORKING")) || start_era > d.t::<EraType>("ERA_ANCIENT").i() {
                unit = d.t::<UnitType>("UNIT_AXEMAN");
            }
            if self.team_is_has_tech(barbarian_team, tech("TECH_IRON_WORKING")) || start_era > d.t::<EraType>("ERA_CLASSICAL").i() {
                unit = d.t::<UnitType>("UNIT_OGRE");
            }
            let orc = d.t::<PromotionType>("PROMOTION_ORC");
            let new_unit = self.player_init_unit(p, unit, x, y, Some(UnitAIType::UNITAI_ATTACK), Some(DirectionType::DIRECTION_SOUTH));
            self.unit_set_has_promotion(new_unit, orc, true);

            let mut unit = d.t::<UnitType>("UNIT_ARCHER");
            if self.team_is_has_tech(barbarian_team, tech("TECH_BOWYERS")) {
                unit = d.t::<UnitType>("UNIT_LONGBOWMAN");
            }
            let defense = Some(UnitAIType::UNITAI_CITY_DEFENSE);
            let new_unit2 = self.player_init_unit(p, unit, x, y, defense, Some(DirectionType::DIRECTION_SOUTH));
            let new_unit3 = self.player_init_unit(p, unit, x, y, defense, Some(DirectionType::DIRECTION_SOUTH));
            self.unit_set_has_promotion(new_unit2, orc, true);
            self.unit_set_has_promotion(new_unit3, orc, true);
            if !self.team_is_has_tech(barbarian_team, tech("TECH_ARCHERY")) {
                let weak = d.t::<PromotionType>("PROMOTION_WEAK");
                self.unit_set_has_promotion(new_unit2, weak, true);
                self.unit_set_has_promotion(new_unit3, weak, true);
            }
        }

        // Scenario: sf.onCityBuilt(city). UI: city name popup for the active player.
    }

    /// `cityRazed` -> `onCityRazed` (city still exists when called): the dead rise according to
    /// the alignment of the city's founder (Manes for the Infernals, Angels for the Mercurians).
    pub fn ev_city_razed(&mut self, c: CityId, p: PlayerId) {
        let d = self.d;
        // `city.findHighestCulture()` is only used by the commented-out Partisans event.
        let angel = d.t::<UnitType>("UNIT_ANGEL");
        let infernal = d.t::<CivilizationType>("CIVILIZATION_INFERNAL");
        let manes = d.t::<UnitType>("UNIT_MANES");
        let mercurians = d.t::<CivilizationType>("CIVILIZATION_MERCURIANS");
        let original_owner = self.cities[c].original_owner;

        // Each `range(..)` is evaluated once, when its loop starts (a gifted Manes may grow a
        // city of the receiving player).
        if self.players[original_owner].alignment == Some(d.t::<AlignmentType>("ALIGNMENT_EVIL")) {
            if self.player_get_civilization_type(original_owner) != Some(infernal) {
                for _ in 0..self.cities[c].population {
                    let (plot, owner) = (self.city_plot(c), self.cities[c].owner);
                    self.cf_gift_unit(manes, infernal, 0, Some(plot), Some(owner));
                }
            }
        }

        if self.players[original_owner].alignment == Some(d.t::<AlignmentType>("ALIGNMENT_NEUTRAL")) {
            for _ in 0..py_div(self.cities[c].population, 4) + 1 {
                let (plot, owner) = (self.city_plot(c), self.cities[c].owner);
                self.cf_gift_unit(manes, infernal, 0, Some(plot), Some(owner));
                let (plot, owner) = (self.city_plot(c), self.cities[c].owner);
                self.cf_gift_unit(manes, infernal, 0, Some(plot), Some(owner));
                let (plot, owner) = (self.city_plot(c), self.cities[c].owner);
                self.cf_gift_unit(angel, mercurians, 0, Some(plot), Some(owner));
            }
        }

        if self.players[original_owner].alignment == Some(d.t::<AlignmentType>("ALIGNMENT_GOOD")) {
            for _ in 0..py_div(self.cities[c].population, 2) + 1 {
                let (plot, owner) = (self.city_plot(c), self.cities[c].owner);
                self.cf_gift_unit(angel, mercurians, 0, Some(plot), Some(owner));
            }
        }

        // Scenario: sf.onCityRazed(city, iPlayer).
    }

    /// `cityAcquired` -> `onCityAcquired` (`c` is the new city): Infernal cities lose the
    /// Obsidian Gate when taken; Infernal conquerors convert the city (Ashen Veil, buildings).
    pub fn ev_city_acquired(&mut self, old_owner: PlayerId, p: PlayerId, c: CityId, conquest: bool, trade: bool) {
        let d = self.d;
        let infernal = d.t::<CivilizationType>("CIVILIZATION_INFERNAL");

        if self.player_get_civilization_type(old_owner) == Some(infernal) {
            self.city_set_num_real_building(c, d.t::<BuildingType>("BUILDING_OBSIDIAN_GATE"), 0);
        }

        if self.player_get_civilization_type(p) == Some(infernal) {
            self.city_set_has_religion(c, d.t::<ReligionType>("RELIGION_THE_ORDER"), false, true, true);
            self.city_set_has_religion(c, d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL"), true, true, true);
            for building in INFERNAL_CITY_BUILDINGS {
                self.city_set_num_real_building(c, d.t::<BuildingType>(building), 1);
            }
        }

        // Scenario: sf.onCityAcquired(..).
    }

    /// `cityAcquiredAndKept` -> `onCityAcquiredAndKept`: logging only.
    pub fn ev_city_acquired_and_kept(&mut self, p: PlayerId, c: CityId) {}

    /// `cityLost` -> `onCityLost`: logging only.
    pub fn ev_city_lost(&mut self, c: CityId) {}

    /// `cultureExpansion` -> `onCultureExpansion`: logging only.
    pub fn ev_culture_expansion(&mut self, c: CityId, p: PlayerId) {}

    /// `cityGrowth` -> `onCityGrowth`: logging only.
    pub fn ev_city_growth(&mut self, c: CityId, p: PlayerId) {}

    /// `cityDoTurn` -> `onCityDoTurn`: Citadel of Light, Hall of Mirrors, Eyes and Ears Network,
    /// Planar Gate, Infernal/Mercurian religion purity, Shrine of Sirona, the Dragon's Horde.
    pub fn ev_city_do_turn(&mut self, c: CityId, p: PlayerId) {
        let d = self.d;
        let plot = self.city_plot(c);
        // Python re-reads the player from the city: `iPlayer = pCity.getOwner()`.
        let p = self.cities[c].owner;

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_CITADEL_OF_LIGHT")] > 0 {
            self.ev_citadel_of_light(c, p, plot);
        }

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_HALL_OF_MIRRORS")] > 0 {
            self.ev_hall_of_mirrors(c, p);
        }

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_EYES_AND_EARS_NETWORK")] > 0 {
            self.ev_eyes_and_ears_network(p);
        }

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_PLANAR_GATE")] > 0 {
            self.ev_planar_gate(c, p, plot);
        }

        let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
        let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
        if self.player_get_civilization_type(self.cities[c].owner) == Some(d.t::<CivilizationType>("CIVILIZATION_INFERNAL")) {
            if self.cities[c].has_religion[order] {
                self.city_set_has_religion(c, order, false, true, true);
            }
            if !self.cities[c].has_religion[veil] {
                self.city_set_has_religion(c, veil, true, true, true);
            }
        }

        if self.player_get_civilization_type(self.cities[c].owner) == Some(d.t::<CivilizationType>("CIVILIZATION_MERCURIANS")) {
            if self.cities[c].has_religion[veil] {
                self.city_set_has_religion(c, veil, false, true, true);
            }
        }

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_SHRINE_OF_SIRONA")] > 0 {
            self.players[p].feat_accomplished[FeatType::FEAT_HEAL_UNIT_PER_TURN] = true;
        }

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_THE_DRAGONS_HORDE")] > 0 {
            if self.player_is_barbarian(p) {
                if self.soren_rand_num(100, "Bob") <= d.handicaps[self.handicap].lair_spawn_rate {
                    let mut unit = d.t::<UnitType>("UNIT_DISCIPLE_OF_ACHERON");
                    let team = self.players[p].team;
                    if self.team_is_has_tech(team, Some(d.t::<TechType>("TECH_SORCERY"))) {
                        unit = d.t::<UnitType>("UNIT_SON_OF_THE_INFERNO");
                    }
                    let (x, y) = self.map.xy(plot);
                    self.player_init_unit(p, unit, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
                }
            }
        }

        // UI: CvAdvisorUtils.cityAdvise(pCity, iPlayer) (advisor popups for humans).
    }

    /// `cityBuildingUnit` -> `onCityBuildingUnit`: logging only.
    pub fn ev_city_building_unit(&mut self, c: CityId, unit: UnitType) {}

    /// `cityBuildingBuilding` -> `onCityBuildingBuilding`: logging only.
    pub fn ev_city_building_building(&mut self, c: CityId, building: BuildingType) {}

    /// `cityHurry` -> `onCityHurry`: does nothing.
    pub fn ev_city_hurry(&mut self, c: CityId, hurry: HurryType) {}

    /// `selectionGroupPushMission` -> `onSelectionGroupPushMission`: logging only.
    pub fn ev_selection_group_push_mission(&mut self, g: GroupId, mission: MissionType) {}

    /// `unitMove` -> `onUnitMove`: logging only.
    pub fn ev_unit_move(&mut self, plot: PlotIdx, u: UnitId, old_plot: Option<PlotIdx>) {}

    /// `unitSetXY` -> `onUnitSetXY`: logging only.
    pub fn ev_unit_set_xy(&mut self, plot: PlotIdx, u: UnitId) {}

    /// `unitCreated` -> `onUnitCreated` (USE_ON_UNIT_CREATED_CALLBACK is enabled in FFH2):
    /// conquest mode for heroes, vampires feast, adepts get mana promotions, elementals and
    /// undead are strong with their tower.
    pub fn ev_unit_created(&mut self, u: UnitId) {
        let d = self.d;
        let p = self.units[u].owner;
        let channeling2 = d.t::<PromotionType>("PROMOTION_CHANNELING2");
        let channeling3 = d.t::<PromotionType>("PROMOTION_CHANNELING3");

        // Conquest mode for heroes (`unit.startConquestMode()`).
        let unit_class = self.unit_get_unit_class_type(u);
        if unit_class == d.t::<UnitClassType>("UNITCLASS_DONAL") {
            self.units[u].ai_groupflag = GROUPFLAG_CONQUEST;
        }
        if unit_class == d.t::<UnitClassType>("UNITCLASS_DEMAGOG") {
            self.units[u].ai_groupflag = GROUPFLAG_CONQUEST;
        }
        if unit_class == d.t::<UnitClassType>("UNITCLASS_LOSHA") {
            self.units[u].ai_groupflag = GROUPFLAG_CONQUEST;
        }
        if self.units[u].unit_type == d.t::<UnitType>("UNIT_VAMPIRE") {
            self.unit_ai_set_unit_ai_type(u, d.t::<UnitAIType>("UNITAI_FEASTING"));
        }

        // Conquest mode for religion heroes.
        if self.players[p].conquest_mode {
            if let Some(favorite) = self.players[p].favorite_religion {
                let unit_class = Some(self.unit_get_unit_class_type(u));
                if unit_class == d.religions[favorite].religion_hero1 {
                    self.units[u].ai_groupflag = GROUPFLAG_CONQUEST;
                } else if unit_class == d.religions[favorite].religion_hero2 {
                    self.units[u].ai_groupflag = GROUPFLAG_CONQUEST;
                }
            }
        }

        if self.unit_get_unit_combat_type(u) == Some(d.t::<UnitCombatType>("UNITCOMBAT_ADEPT")) {
            for (bonus, tier1, tier2, tier3) in ADEPT_MANA_PROMOTIONS {
                let num = self.player_get_num_available_bonuses(p, d.t::<BonusType>(bonus));
                if num > 1 {
                    self.unit_set_has_promotion(u, d.t::<PromotionType>(tier1), true);
                    if num > 2 && self.units[u].has_promotion[channeling2] {
                        self.unit_set_has_promotion(u, d.t::<PromotionType>(tier2), true);
                        if num > 3 && self.units[u].has_promotion[channeling3] {
                            self.unit_set_has_promotion(u, d.t::<PromotionType>(tier3), true);
                        }
                    }
                }
            }
        }

        let strong = d.t::<PromotionType>("PROMOTION_STRONG");
        if self.units[u].has_promotion[d.t::<PromotionType>("PROMOTION_ELEMENTAL")] {
            if self.player_get_num_building(p, d.t::<BuildingType>("BUILDING_TOWER_OF_THE_ELEMENTS")) > 0 {
                self.unit_set_has_promotion(u, strong, true);
            }
        }

        if self.units[u].has_promotion[d.t::<PromotionType>("PROMOTION_UNDEAD")] {
            if self.player_get_num_building(p, d.t::<BuildingType>("BUILDING_TOWER_OF_NECROMANCY")) > 0 {
                self.unit_set_has_promotion(u, strong, true);
            }
        }

        // Scenario: sf.onUnitCreated(unit).
    }

    /// `unitBuilt` -> `onUnitBuilt`: building effects on new units (Beast of Agares, Chancel of
    /// Guardians, Cave of Ancestors, golem workshops, Asylum, Acheron, Brewery, Demon's Altar,
    /// Warrens) and the advisor feats.
    pub fn ev_unit_built(&mut self, c: CityId, u: UnitId) {
        let d = self.d;
        // `pPlayer` is the unit's owner.
        let p = self.units[u].owner;
        let free_promotion_pick = self.units[u].free_promotion_pick;

        // AI: (Sephi) for AI players (not barbarians) the Python picks the UNITAI of new adepts
        // (UNITAI_TERRAFORMER up to a quota from the city count and Sorcery, else UNITAI_MAGE
        // or UNITAI_WARWIZARD by `AI_neededPermDefenseReserve`), Devouts and Priests of Leaves
        // (UNITAI_TERRAFORMER quotas): AI unit roles, left to the new AI (docs/PORTING.md).

        let (x, y) = (self.cities[c].x, self.cities[c].y);

        if self.units[u].unit_type == d.t::<UnitType>("UNIT_BEAST_OF_AGARES") {
            if self.city_civilization_type(c) != d.t::<CivilizationType>("CIVILIZATION_INFERNAL") {
                let mut pop = self.cities[c].population - 4;
                if pop <= 1 {
                    pop = 1;
                }
                self.city_set_population(c, pop);
                self.city_set_occupation_timer(c, 4);
            }
        }

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_CHANCEL_OF_GUARDIANS")] > 0 {
            if self.soren_rand_num(100, "Chancel of Guardians") < 20 {
                self.unit_set_has_promotion(u, d.t::<PromotionType>("PROMOTION_DEFENSIVE"), true);
            }
        }

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_CAVE_OF_ANCESTORS")] > 0
            && self.unit_get_unit_combat_type(u) == Some(d.t::<UnitCombatType>("UNITCOMBAT_ADEPT"))
        {
            let mana = d.t::<BonusClassType>("BONUSCLASS_MANA");
            let mut i = 0;
            for bonus in d.bonuses.keys() {
                if d.bonuses[bonus].bonus_class_type == Some(mana) && self.city_has_bonus(c, bonus) {
                    i += 1;
                }
            }
            if i >= 1 {
                self.unit_change_experience(u, i, -1, false, false, false);
            }
        }

        if self.units[u].has_promotion[d.t::<PromotionType>("PROMOTION_GOLEM")] {
            if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_BLASTING_WORKSHOP")] > 0 {
                self.unit_set_has_promotion(u, d.t::<PromotionType>("PROMOTION_FIRE2"), true);
            }
            if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_PALLENS_ENGINE")] > 0 {
                self.unit_set_has_promotion(u, d.t::<PromotionType>("PROMOTION_PERFECT_SIGHT"), true);
            }
            if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_ADULARIA_CHAMBER")] > 0 {
                self.unit_set_has_promotion(u, d.t::<PromotionType>("PROMOTION_HIDDEN"), true);
            }
        }

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_ASYLUM")] > 0 {
            if self.unit_is_alive(u) {
                if !is_world_unit_class(d, self.unit_get_unit_class_type(u)) {
                    if self.soren_rand_num(100, "Bob") <= 10 {
                        self.unit_set_has_promotion(u, d.t::<PromotionType>("PROMOTION_CRAZED"), true);
                        self.unit_set_has_promotion(u, d.t::<PromotionType>("PROMOTION_ENRAGED"), true);
                    }
                }
            }
        }

        if self.units[u].unit_type == d.t::<UnitType>("UNIT_ACHERON") {
            self.unit_set_has_promotion(u, d.t::<PromotionType>("PROMOTION_HELD"), true);
            self.city_set_num_real_building(c, d.t::<BuildingType>("BUILDING_THE_DRAGONS_HORDE"), 1);
            let forest = d.t::<FeatureType>("FEATURE_FOREST");
            let jungle = d.t::<FeatureType>("FEATURE_JUNGLE");
            let flames = d.t::<FeatureType>("FEATURE_FLAMES");
            for xx in x - 1..x + 2 {
                for yy in y - 1..y + 2 {
                    // A NULL plot answers NO_FEATURE.
                    if let Some(plot) = self.map.plot_xy(xx, yy) {
                        let feature = self.map.plots[plot].feature_type;
                        if feature == Some(forest) || feature == Some(jungle) {
                            self.plot_set_feature_type(plot, Some(flames));
                        }
                    }
                }
            }
            // UI: cf.addPopup(TXT_KEY_POPUP_ACHERON_CREATION).
        }

        if self.units[u].has_promotion[d.t::<PromotionType>("PROMOTION_DWARF")] {
            if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_BREWERY")] > 0 {
                self.unit_change_experience(u, 2, -1, false, false, false);
            }
        }

        if self.units[u].has_promotion[d.t::<PromotionType>("PROMOTION_DEMON")] {
            if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_DEMONS_ALTAR")] > 0 {
                self.unit_change_experience(u, 2, -1, false, false, false);
            }
        }

        if self.units[u].free_promotion_pick < free_promotion_pick {
            let change = free_promotion_pick - self.units[u].free_promotion_pick;
            self.unit_change_free_promotion_pick(u, change);
        }

        if self.cities[c].num_real_building[d.t::<BuildingType>("BUILDING_WARRENS")] > 0 {
            let unit_class = self.unit_get_unit_class_type(u);
            if !is_world_unit_class(d, unit_class) && !is_national_unit_class(d, unit_class) {
                let combat = self.unit_get_unit_combat_type(u);
                if combat != Some(d.t::<UnitCombatType>("UNITCOMBAT_SIEGE"))
                    && combat != Some(d.t::<UnitCombatType>("UNITCOMBAT_NAVAL"))
                {
                    let unit_type = self.units[u].unit_type;
                    let new_unit = self.player_init_unit(p, unit_type, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
                    // `CyCity::applyBuildEffects` forwards to `CvCity::addProductionExperience`.
                    self.city_add_production_experience(c, new_unit, false);
                }
            }
        }

        self.ev_unit_built_feats(c, u);
    }

    /// `unitKilled` -> `onUnitKilled` (unit still exists when called): Soul Forge production,
    /// Mokka's Cauldron, Manes and Angels rising, the Spirit Guide's experience, Acheron.
    pub fn ev_unit_killed(&mut self, u: UnitId, attacker: Option<PlayerId>) {
        let d = self.d;
        let p = self.units[u].owner;

        if self.unit_is_alive(u) && !self.unit_is_immortal(u) {
            let (x, y) = (self.units[u].x, self.units[u].y);
            let soul_forge = d.t::<BuildingType>("BUILDING_SOUL_FORGE");
            for xx in x - 1..x + 2 {
                for yy in y - 1..y + 2 {
                    let Some(plot2) = self.map.plot_xy(xx, yy) else { continue };
                    if let Some(city) = self.map.plots[plot2].plot_city {
                        if self.cities[city].num_real_building[soul_forge] > 0 {
                            let production = self.units[u].experience + 10;
                            self.city_change_production(city, production);
                            let owner = self.cities[city].owner;
                            self.ev_msg(owner, "TXT_KEY_MESSAGE_SOUL_FORGE");
                        }
                    }
                }
            }

            if let Some(plot) = self.map.plot_xy(x, y) {
                if let Some(city) = self.map.plots[plot].plot_city {
                    if self.cities[city].num_real_building[d.t::<BuildingType>("BUILDING_MOKKAS_CAULDRON")] > 0 {
                        if self.cities[city].owner == self.units[u].owner {
                            if let Some(unholy) = self.cf_get_unholy_version(u) {
                                let (cx, cy) = (self.cities[city].x, self.cities[city].y);
                                let new_unit = self.player_init_unit(p, unholy, cx, cy, None, Some(DirectionType::DIRECTION_SOUTH));
                                self.unit_set_has_promotion(new_unit, d.t::<PromotionType>("PROMOTION_DEMON"), true);
                                self.unit_set_damage(new_unit, 50, None);
                                self.unit_finish_moves(new_unit);
                                self.ev_msg(p, "TXT_KEY_MESSAGE_MOKKAS_CAULDRON");
                            }
                        }
                    }
                }
            }

            let religion = self.units[u].religion;
            let is_religion = |name: &str| religion == Some(d.t::<ReligionType>(name));
            if is_religion("RELIGION_COUNCIL_OF_ESUS")
                || is_religion("RELIGION_THE_ASHEN_VEIL")
                || is_religion("RELIGION_OCTOPUS_OVERLORDS")
                || self.units[u].has_promotion[d.t::<PromotionType>("PROMOTION_DEATH1")]
                || self.units[u].has_promotion[d.t::<PromotionType>("PROMOTION_ENTROPY1")]
            {
                let manes = d.t::<UnitType>("UNIT_MANES");
                let infernal = d.t::<CivilizationType>("CIVILIZATION_INFERNAL");
                let plot = self.unit_plot(u);
                self.cf_gift_unit(manes, infernal, 0, Some(plot), Some(p));
                let plot = self.unit_plot(u);
                self.cf_gift_unit(manes, infernal, 0, Some(plot), Some(p));
            }

            // (The Python tests the unit combat before the civilization; both are pure.)
            let mercurians = d.t::<CivilizationType>("CIVILIZATION_MERCURIANS");
            if is_religion("RELIGION_THE_EMPYREAN")
                || is_religion("RELIGION_THE_ORDER")
                || is_religion("RELIGION_RUNES_OF_KILMORPH")
                || (self.player_get_civilization_type(p) == Some(mercurians)
                    && self.unit_get_unit_combat_type(u) != Some(d.t::<UnitCombatType>("UNITCOMBAT_ANIMAL")))
            {
                let angel = d.t::<UnitType>("UNIT_ANGEL");
                let (xp, plot) = (self.units[u].experience, self.unit_plot(u));
                self.cf_gift_unit(angel, mercurians, xp, Some(plot), Some(p));
            }

            if self.units[u].has_promotion[d.t::<PromotionType>("PROMOTION_SPIRIT_GUIDE")] {
                if self.units[u].experience > 0 {
                    let mut candidates = Vec::new();
                    for loop_unit in self.ev_unit_list(p) {
                        if self.unit_is_alive(loop_unit)
                            && !self.unit_is_only_defensive(loop_unit)
                            && !self.units[loop_unit].death_delay
                        {
                            candidates.push(loop_unit);
                        }
                    }
                    if !candidates.is_empty() {
                        let roll = self.soren_rand_num(candidates.len() as i32, "Spirit Guide");
                        let target = candidates[py_list_index(roll, candidates.len())];
                        let xp = py_div(self.units[u].experience, 2);
                        self.unit_change_experience(target, xp, -1, false, false, false);
                        self.unit_change_experience(u, xp * -1, -1, false, false, false);
                        self.ev_msg(p, "TXT_KEY_MESSAGE_SPIRIT_GUIDE");
                    }
                }
            }
        }

        if self.units[u].unit_type == d.t::<UnitType>("UNIT_ACHERON") {
            self.unit_set_has_promotion(u, d.t::<PromotionType>("PROMOTION_HELD"), false);
        }

        // Scenario: sf.onUnitKilled(unit, iAttacker). `CvUtil.pyPrint` of the kill (logging).
    }

    /// `unitLost` -> `onUnitLost`: logging only.
    pub fn ev_unit_lost(&mut self, u: UnitId) {}

    /// `unitPromoted` -> `onUnitPromoted`: logging only.
    pub fn ev_unit_promoted(&mut self, u: UnitId, promotion: PromotionType) {}

    /// `unitPillage` -> `onUnitPillage`: logging only.
    pub fn ev_unit_pillage(&mut self, u: UnitId, improvement: Option<ImprovementType>, route: Option<RouteType>, p: PlayerId) {}

    /// `unitSpreadReligionAttempt` -> `onUnitSpreadReligionAttempt`: does nothing.
    pub fn ev_unit_spread_religion_attempt(&mut self, u: UnitId, religion: ReligionType, success: bool) {}

    /// `unitGifted` -> `onUnitGifted`: does nothing.
    pub fn ev_unit_gifted(&mut self, u: UnitId, gifting_player: PlayerId, plot: PlotIdx) {}

    /// `unitBuildImprovement` -> `onUnitBuildImprovement`: does nothing.
    pub fn ev_unit_build_improvement(&mut self, u: UnitId, build: BuildType, finished: bool) {}

    /// `goodyReceived` -> `onGoodyReceived`: logging only.
    pub fn ev_goody_received(&mut self, p: PlayerId, plot: PlotIdx, u: Option<UnitId>, goody: GoodyType) {}

    /// `greatPersonBorn` -> `onGreatPersonBorn`: logging only.
    pub fn ev_great_person_born(&mut self, u: UnitId, p: PlayerId, c: Option<CityId>) {}

    /// `buildingBuilt` -> `onBuildingBuilt`: FfH wonder effects (Infernal Grimoire, the Altar of
    /// the Luonnotar stages, Mercurian Gate, Tower of the Elements, Tower of Necromancy, Temple
    /// of the Hand, Grand Menagerie trophy) and the advisor feats.
    pub fn ev_building_built(&mut self, c: CityId, building: BuildingType) {
        let d = self.d;
        let player = self.cities[c].owner;
        let plot = self.city_plot(c);
        let is = |name: &str| building == d.t::<BuildingType>(name);

        // UI: wonder movie popup for the active player.

        if is("BUILDING_INFERNAL_GRIMOIRE") {
            if self.soren_rand_num(100, "Bob") <= 20 {
                if let Some(plot2) = self.cf_find_clear_plot(None, plot) {
                    let barbarian = self.barbarian_player();
                    let (x2, y2) = self.map.xy(plot2);
                    let balor = d.t::<UnitType>("UNIT_BALOR");
                    self.player_init_unit(barbarian, balor, x2, y2, None, Some(DirectionType::DIRECTION_NORTH));
                    self.ev_msg(player, "TXT_KEY_MESSAGE_INFERNAL_GRIMOIRE_BALOR");
                    // UI: popup TXT_KEY_POPUP_INFERNAL_GRIMOIRE_BALOR for the active player.
                }
            }
        }

        // Each altar stage replaces the previous one.
        const ALTAR_STAGES: [(&str, &str); 6] = [
            ("BUILDING_ALTAR_OF_THE_LUONNOTAR_FINAL", "BUILDING_ALTAR_OF_THE_LUONNOTAR_EXALTED"),
            ("BUILDING_ALTAR_OF_THE_LUONNOTAR_EXALTED", "BUILDING_ALTAR_OF_THE_LUONNOTAR_DIVINE"),
            ("BUILDING_ALTAR_OF_THE_LUONNOTAR_DIVINE", "BUILDING_ALTAR_OF_THE_LUONNOTAR_CONSECRATED"),
            ("BUILDING_ALTAR_OF_THE_LUONNOTAR_CONSECRATED", "BUILDING_ALTAR_OF_THE_LUONNOTAR_BLESSED"),
            ("BUILDING_ALTAR_OF_THE_LUONNOTAR_BLESSED", "BUILDING_ALTAR_OF_THE_LUONNOTAR_ANOINTED"),
            ("BUILDING_ALTAR_OF_THE_LUONNOTAR_ANOINTED", "BUILDING_ALTAR_OF_THE_LUONNOTAR"),
        ];
        for (stage, previous) in ALTAR_STAGES {
            if is(stage) {
                self.city_set_num_real_building(c, d.t::<BuildingType>(previous), 0);
            }
        }

        if is("BUILDING_MERCURIAN_GATE") {
            self.ev_mercurian_gate(player, plot);
        }

        if is("BUILDING_TOWER_OF_THE_ELEMENTS") {
            const ELEMENTALS: [&str; 4] =
                ["UNIT_AIR_ELEMENTAL", "UNIT_EARTH_ELEMENTAL", "UNIT_FIRE_ELEMENTAL", "UNIT_WATER_ELEMENTAL"];
            let roll = self.soren_rand_num(ELEMENTALS.len() as i32, "Pick Elemental");
            let unit = d.t::<UnitType>(ELEMENTALS[py_list_index(roll, ELEMENTALS.len())]);
            let (x, y) = (self.cities[c].x, self.cities[c].y);
            let new_unit = self.player_init_unit(player, unit, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
            self.unit_set_has_promotion(new_unit, d.t::<PromotionType>("PROMOTION_HELD"), true);
            self.ev_msg(player, "TXT_KEY_MESSAGE_TOWER_OF_THE_ELEMENTS_SPAWN");
            let elemental = d.t::<PromotionType>("PROMOTION_ELEMENTAL");
            let strong = d.t::<PromotionType>("PROMOTION_STRONG");
            for unit in self.ev_unit_list(player) {
                if self.units[unit].has_promotion[elemental] {
                    self.unit_set_has_promotion(unit, strong, true);
                }
            }
        }

        if is("BUILDING_TOWER_OF_NECROMANCY") {
            let undead = d.t::<PromotionType>("PROMOTION_UNDEAD");
            let strong = d.t::<PromotionType>("PROMOTION_STRONG");
            for unit in self.ev_unit_list(player) {
                if self.units[unit].has_promotion[undead] {
                    self.unit_set_has_promotion(unit, strong, true);
                }
            }
        }

        if is("BUILDING_TEMPLE_OF_THE_HAND") {
            let snow = d.t::<TerrainType>("TERRAIN_SNOW");
            let flames = d.t::<FeatureType>("FEATURE_FLAMES");
            let flood_plains = d.t::<FeatureType>("FEATURE_FLOOD_PLAINS");
            let forest = d.t::<FeatureType>("FEATURE_FOREST");
            let jungle = d.t::<FeatureType>("FEATURE_JUNGLE");
            let scrub = d.t::<FeatureType>("FEATURE_SCRUB");
            let smoke = d.t::<ImprovementType>("IMPROVEMENT_SMOKE");
            let (x, y) = (self.cities[c].x, self.cities[c].y);
            for xx in x - 2..x + 3 {
                for yy in y - 2..y + 3 {
                    let Some(loop_plot) = self.map.plot_xy(xx, yy) else { continue };
                    if !self.plot_is_water(loop_plot) {
                        self.plot_set_terrain_type(loop_plot, snow, true);
                        if self.map.plots[loop_plot].improvement_type == Some(smoke) {
                            self.plot_set_improvement_type(loop_plot, None);
                        }
                        // Read once; every test below sees the feature from before the changes.
                        let feature = self.map.plots[loop_plot].feature_type;
                        if feature == Some(forest) {
                            self.plot_set_feature_type(loop_plot, Some(forest));
                        }
                        if feature == Some(jungle) {
                            self.plot_set_feature_type(loop_plot, Some(forest));
                        }
                        if feature == Some(flames) {
                            self.plot_set_feature_type(loop_plot, None);
                        }
                        if feature == Some(flood_plains) {
                            self.plot_set_feature_type(loop_plot, None);
                        }
                        if feature == Some(scrub) {
                            self.plot_set_feature_type(loop_plot, None);
                        }
                    }
                }
            }
            // UI: EFFECT_SNOWFALL.
        }

        if is("BUILDING_GRAND_MENAGERIE") {
            if self.players[player].human {
                // Not a scenario.
                let trophy = "TROPHY_FEAT_GRAND_MENAGERIE";
                if !self.is_has_trophy(trophy) {
                    self.change_trophy_value(trophy, 1);
                }
            }
        }

        self.ev_building_built_feats(c, building);
    }

    /// `projectBuilt` -> `onProjectBuilt`: the FfH rituals (Bane Divine, Genesis, Glory
    /// Everlasting, Rites of Oghma, Nature's Revolt, Blood of the Phoenix, Purge the Unfaithful,
    /// Birthright Regained, Samhain, The White Hand, The Deepening, Stir from Slumber, The Draw,
    /// Ascension, Pact of the Nilhorn).
    pub fn ev_project_built(&mut self, c: CityId, project: ProjectType) {
        let d = self.d;
        let p = self.cities[c].owner;
        let is = |name: &str| project == d.t::<ProjectType>(name);
        let (x, y) = (self.cities[c].x, self.cities[c].y);

        // UI: wonder movie popup for the active player.

        if is("PROJECT_BANE_DIVINE") {
            let disciple = d.t::<UnitCombatType>("UNITCOMBAT_DISCIPLE");
            for loop_player in self.player_ids() {
                if self.players[loop_player].alive {
                    for unit in self.ev_unit_list(loop_player) {
                        if self.units.contains(unit) && self.unit_get_unit_combat_type(unit) == Some(disciple) {
                            self.unit_kill(unit, false, Some(p));
                        }
                    }
                }
            }
        }

        if is("PROJECT_GENESIS") {
            self.cf_genesis(p);
        }

        if is("PROJECT_GLORY_EVERLASTING") {
            let demon = d.t::<PromotionType>("PROMOTION_DEMON");
            let undead = d.t::<PromotionType>("PROMOTION_UNDEAD");
            for loop_player in self.player_ids() {
                if self.players[loop_player].alive {
                    for unit in self.ev_unit_list(loop_player) {
                        if self.units.contains(unit)
                            && (self.units[unit].has_promotion[demon] || self.units[unit].has_promotion[undead])
                        {
                            self.unit_kill(unit, false, Some(p));
                        }
                    }
                }
            }
        }

        if is("PROJECT_RITES_OF_OGHMA") {
            let mut i = 7;
            let world_size = self.setup.world_size;
            if world_size == d.t::<WorldSizeType>("WORLDSIZE_DUEL") {
                i -= 3;
            }
            if world_size == d.t::<WorldSizeType>("WORLDSIZE_TINY") {
                i -= 2;
            }
            if world_size == d.t::<WorldSizeType>("WORLDSIZE_SMALL") {
                i -= 1;
            }
            if world_size == d.t::<WorldSizeType>("WORLDSIZE_LARGE") {
                i += 1;
            }
            if world_size == d.t::<WorldSizeType>("WORLDSIZE_HUGE") {
                i += 3;
            }
            self.cf_add_bonus(d.t::<BonusType>("BONUS_MANA"), i);
        }

        if is("PROJECT_NATURES_REVOLT") {
            self.ev_natures_revolt();
        }

        if is("PROJECT_BLOOD_OF_THE_PHOENIX") {
            let immortal = d.t::<PromotionType>("PROMOTION_IMMORTAL");
            for unit in self.ev_unit_list(p) {
                if self.unit_is_alive(unit) {
                    self.unit_set_has_promotion(unit, immortal, true);
                }
            }
        }

        if is("PROJECT_PURGE_THE_UNFAITHFUL") {
            self.ev_purge_the_unfaithful(p);
        }

        if is("PROJECT_BIRTHRIGHT_REGAINED") {
            self.players[p].feat_accomplished[FeatType::FEAT_GLOBAL_SPELL] = false;
        }

        if is("PROJECT_SAMHAIN") {
            for city in self.ev_city_list(p) {
                self.city_change_happiness_timer(city, 20);
            }
            let count = self.count_civ_players_alive() + self.handicap.i() - 5;
            for _ in 0..count {
                self.cf_add_unit(d.t::<UnitType>("UNIT_FROSTLING"));
                self.cf_add_unit(d.t::<UnitType>("UNIT_FROSTLING"));
                self.cf_add_unit(d.t::<UnitType>("UNIT_FROSTLING_ARCHER"));
                self.cf_add_unit(d.t::<UnitType>("UNIT_FROSTLING_WOLF_RIDER"));
            }
            self.cf_add_unit(d.t::<UnitType>("UNIT_MOKKA"));
        }

        if is("PROJECT_THE_WHITE_HAND") {
            let priest = d.t::<UnitType>("UNIT_PRIEST_OF_WINTER");
            for name in ["Dumannios", "Riuros", "Anagantios"] {
                let new_unit = self.player_init_unit(p, priest, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
                self.unit_set_name(new_unit, name.to_string());
            }
        }

        if is("PROJECT_THE_DEEPENING") {
            self.ev_the_deepening();
        }

        if is("PROJECT_STIR_FROM_SLUMBER") {
            let drifa = d.t::<UnitType>("UNIT_DRIFA");
            self.player_init_unit(p, drifa, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
        }

        if is("PROJECT_THE_DRAW") {
            self.ev_the_draw(p);
        }

        if is("PROJECT_ASCENSION") {
            self.ev_ascension(p, x, y);
        }

        if is("PROJECT_PACT_OF_THE_NILHORN") {
            let giant = d.t::<UnitType>("UNIT_HILL_GIANT");
            let hidden_nationality = d.t::<PromotionType>("PROMOTION_HIDDEN_NATIONALITY");
            for name in ["Larry", "Curly", "Moe"] {
                let new_unit = self.player_init_unit(p, giant, x, y, None, Some(DirectionType::DIRECTION_NORTH));
                self.unit_set_has_promotion(new_unit, hidden_nationality, true);
                self.unit_set_name(new_unit, name.to_string());
            }
        }
    }

    /// `techAcquired` -> `onTechAcquired` (`p` may be None): conquest mode (Better AI) for
    /// civilizations and leaders reaching their key tech, religion disciples for religion
    /// techs, and Hyborem's arrival with the first Infernal Pact.
    pub fn ev_tech_acquired(&mut self, tech: TechType, team: TeamId, p: Option<PlayerId>, announce: bool) {
        let d = self.d;
        // UI: tech splash screen for the active player.

        if let Some(player) = p.filter(|&player| player != self.barbarian_player()) {
            self.ev_tech_conquest_mode(player, tech);

            const RELIGION_TECHS: [(&str, &str, &str); 7] = [
                ("TECH_CORRUPTION_OF_SPIRIT", "UNIT_DISCIPLE_THE_ASHEN_VEIL", "RELIGION_THE_ASHEN_VEIL"),
                ("TECH_ORDERS_FROM_HEAVEN", "UNIT_DISCIPLE_THE_ORDER", "RELIGION_THE_ORDER"),
                ("TECH_WAY_OF_THE_FORESTS", "UNIT_DISCIPLE_FELLOWSHIP_OF_LEAVES", "RELIGION_FELLOWSHIP_OF_LEAVES"),
                ("TECH_WAY_OF_THE_EARTHMOTHER", "UNIT_DISCIPLE_RUNES_OF_KILMORPH", "RELIGION_RUNES_OF_KILMORPH"),
                ("TECH_MESSAGE_FROM_THE_DEEP", "UNIT_DISCIPLE_OCTOPUS_OVERLORDS", "RELIGION_OCTOPUS_OVERLORDS"),
                ("TECH_HONOR", "UNIT_DISCIPLE_EMPYREAN", "RELIGION_THE_EMPYREAN"),
                ("TECH_DECEPTION", "UNIT_NIGHTWATCH", "RELIGION_COUNCIL_OF_ESUS"),
            ];
            let mut religion_unit = None;
            for (religion_tech, unit, religion) in RELIGION_TECHS {
                if tech == d.t::<TechType>(religion_tech) {
                    religion_unit = Some((d.t::<ReligionType>(religion), d.t::<UnitType>(unit)));
                }
            }
            if let Some((religion, unit)) = religion_unit {
                if Some(religion) == self.players[player].favorite_religion {
                    // A NULL capital (CyCity) ignores `setHasReligion`.
                    if let Some(capital) = self.player_get_capital_city(player) {
                        self.city_set_has_religion(capital, religion, true, true, true);
                    }
                }
                if self.is_religion_founded(religion) {
                    if let Some(civ) = self.player_get_civilization_type(player) {
                        self.cf_gift_unit(unit, civ, 0, None, None);
                    }
                }
            }
        }

        if !self.is_option(GameOptionType::GAMEOPTION_NO_HYBOREM_OR_BASIUM) {
            let infernal_pact = d.t::<TechType>("TECH_INFERNAL_PACT");
            if tech == infernal_pact {
                if let Some(player) = p {
                    let mut count = 0;
                    for loop_team in self.team_ids() {
                        if self.team_is_has_tech(loop_team, Some(infernal_pact)) {
                            count += 1;
                        }
                    }
                    if count == 1 {
                        self.ev_infernal_pact(player);
                    }
                }
            }
        }

        // Scenario: sf.onTechAcquired(..).
    }

    /// `techSelected` -> `onTechSelected`: logging only.
    pub fn ev_tech_selected(&mut self, tech: TechType, p: PlayerId) {}

    /// `religionFounded` -> `onReligionFounded`: religion movie popup (UI) and scenario hook only.
    pub fn ev_religion_founded(&mut self, religion: ReligionType, p: PlayerId) {}

    /// `religionSpread` -> `onReligionSpread`: the Order spawns a Crusader (with Fanaticism) or a
    /// Disciple when it spreads to a city of an Order player.
    pub fn ev_religion_spread(&mut self, religion: ReligionType, p: PlayerId, c: CityId) {
        let d = self.d;
        let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
        if religion == order && self.game_turn() != self.start_turn {
            if self.player_get_state_religion(p) == Some(order) && self.cities[c].occupation_timer <= 0 {
                if self.soren_rand_num(100, "Order Spawn") < d.defines.order_spawn_chance {
                    let team = self.players[p].team;
                    let unit = if self.team_is_has_tech(team, Some(d.t::<TechType>("TECH_FANATICISM"))) {
                        self.ev_msg(p, "TXT_KEY_MESSAGE_ORDER_SPAWN_CRUSADER");
                        d.t::<UnitType>("UNIT_CRUSADER")
                    } else {
                        self.ev_msg(p, "TXT_KEY_MESSAGE_ORDER_SPAWN_ACOLYTE");
                        d.t::<UnitType>("UNIT_DISCIPLE_THE_ORDER")
                    };
                    let (x, y) = (self.cities[c].x, self.cities[c].y);
                    self.player_init_unit(p, unit, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
                }
            }
        }
    }

    /// `religionRemove` -> `onReligionRemove`: logging only.
    pub fn ev_religion_remove(&mut self, religion: ReligionType, p: PlayerId, c: CityId) {}

    /// `corporationFounded` -> `onCorporationFounded`: logging only.
    pub fn ev_corporation_founded(&mut self, corp: CorporationType, p: PlayerId) {}

    /// `corporationSpread` -> `onCorporationSpread`: logging only.
    pub fn ev_corporation_spread(&mut self, corp: CorporationType, p: PlayerId, c: CityId) {}

    /// `corporationRemove` -> `onCorporationRemove`: logging only.
    pub fn ev_corporation_remove(&mut self, corp: CorporationType, p: PlayerId, c: CityId) {}

    /// `goldenAge` -> `onGoldenAge`: logging only.
    pub fn ev_golden_age(&mut self, p: PlayerId) {}

    /// `endGoldenAge` -> `onEndGoldenAge`: logging only.
    pub fn ev_end_golden_age(&mut self, p: PlayerId) {}

    /// `changeWar` -> `onChangeWar`: logging only.
    pub fn ev_change_war(&mut self, war: bool, team: TeamId, other: TeamId) {}

    /// `setPlayerAlive` -> `onSetPlayerAlive`: a dying good player raises the Armageddon counter
    /// by 5, a dying evil player lowers it by 5 (from game year 5 on).
    pub fn ev_set_player_alive(&mut self, p: PlayerId, alive: bool) {
        let d = self.d;
        // `CvUtil.pyPrint` of the new alive status (logging).
        if !alive && self.game_turn_year() >= 5 {
            if self.players[p].alignment == Some(d.t::<AlignmentType>("ALIGNMENT_GOOD")) {
                self.change_global_counter(5);
            }
            if self.players[p].alignment == Some(d.t::<AlignmentType>("ALIGNMENT_EVIL")) {
                self.change_global_counter(-5);
            }
            // Scenario: sf.playerDefeated(pPlayer); otherwise the leader's defeat popup (UI).
        }
    }

    /// `playerChangeStateReligion` -> `onPlayerChangeStateReligion`: does nothing.
    pub fn ev_player_change_state_religion(&mut self, p: PlayerId, new: Option<ReligionType>, old: Option<ReligionType>) {}

    /// `playerGoldTrade` -> `onPlayerGoldTrade`: does nothing.
    pub fn ev_player_gold_trade(&mut self, from: PlayerId, to: PlayerId, amount: i32) {}

    /// `victory` -> `onVictory`: victory trophies for the winning team's human players.
    pub fn ev_victory(&mut self, winner: TeamId, victory: VictoryType) {
        let d = self.d;
        // Python: `if iVictory >= 0 and iVictory < gc.getNumVictoryInfos()` (always true here).
        for p in self.player_ids() {
            if self.players[p].alive && self.players[p].human && self.players[p].team == winner {
                // Not a scenario (sf.onVictory otherwise).
                let civ = self.player_get_civilization_type(p);
                for (civ_name, trophy) in VICTORY_CIV_TROPHIES {
                    if civ == Some(d.t::<CivilizationType>(civ_name)) {
                        self.change_trophy_value(trophy, 1);
                    }
                }
                for (victory_name, trophy) in VICTORY_TYPE_TROPHIES {
                    if victory == d.t::<VictoryType>(victory_name) {
                        self.change_trophy_value(trophy, 1);
                    }
                }
                if self.is_option(GameOptionType::GAMEOPTION_BARBARIAN_WORLD) {
                    self.change_trophy_value("TROPHY_VICTORY_BARBARIAN_WORLD", 1);
                }
                if self.is_option(GameOptionType::GAMEOPTION_CHALLENGE_CUT_LOSERS) {
                    self.change_trophy_value("TROPHY_VICTORY_FINAL_FIVE", 1);
                }
                if self.is_option(GameOptionType::GAMEOPTION_CHALLENGE_HIGH_TO_LOW) {
                    self.change_trophy_value("TROPHY_VICTORY_HIGH_TO_LOW", 1);
                }
                if self.is_option(GameOptionType::GAMEOPTION_CHALLENGE_INCREASING_DIFFICULTY) {
                    self.change_trophy_value("TROPHY_VICTORY_INCREASING_DIFFICULTY", 1);
                }
            }
        }
        // `CvUtil.pyPrint` of the victory (logging).
    }

    /// `vassalState` -> `onVassalState`: logging only.
    pub fn ev_vassal_state(&mut self, master: TeamId, vassal: TeamId, is_vassal: bool) {}

    // ---------------------------------------------------------------------------------------
    // Parts of the handlers above, split out for readability (same order of effects)
    // ---------------------------------------------------------------------------------------

    /// onTechAcquired (Sephi, Better AI): `pPlayer.startConquestMode()` when a civilization or
    /// leader learns its key tech.
    fn ev_tech_conquest_mode(&mut self, p: PlayerId, tech: TechType) {
        let d = self.d;
        let civ = self.player_get_civilization_type(p);
        let leader = self.player_get_leader_type(p);
        let is_civ = |name: &str| civ == Some(d.t::<CivilizationType>(name));
        let is_leader = |name: &str| leader == Some(d.t::<LeaderHeadType>(name));
        let is_tech = |name: &str| tech == d.t::<TechType>(name);
        let mut start = false;

        if is_civ("CIVILIZATION_AMURITES") && is_tech("TECH_BOWYERS") {
            start = true;
        }
        if is_leader("LEADER_PERPENTACH") && is_tech("TECH_CONSTRUCTION") {
            start = true;
        }
        if is_leader("LEADER_KEELYN") && is_tech("TECH_IRON_WORKING") {
            start = true;
        }
        if is_civ("CIVILIZATION_BALSERAPHS") && is_tech("TECH_IRON_WORKING") {
            start = true;
        }
        if is_civ("CIVILIZATION_BANNOR") && is_tech("TECH_FANATICISM") {
            start = true;
        }
        if is_leader("LEADER_ALEXIS") {
            if is_tech("TECH_BRONZE_WORKING") {
                start = true;
            }
        } else if is_civ("CIVILIZATION_CALABIM") {
            if is_tech("TECH_FEUDALISM") {
                start = true;
            }
        }
        if is_civ("CIVILIZATION_CLAN_OF_EMBERS") && is_tech("TECH_MASONRY") {
            start = true;
        }
        if is_civ("CIVILIZATION_DOVIELLO") && is_tech("TECH_FESTIVALS") {
            start = true;
        }
        if is_civ("CIVILIZATION_ELOHIM") && is_tech("TECH_PRIESTHOOD") {
            start = true;
        }
        if is_civ("CIVILIZATION_GRIGORI") && is_tech("TECH_MEDICINE") {
            start = true;
        }
        if is_leader("LEADER_TASUNKE") {
            if is_tech("TECH_HORSEBACK_RIDING") {
                start = true;
            }
        } else if is_leader("LEADER_RHOANNA") {
            if is_tech("TECH_STIRRUPS") {
                start = true;
            }
        }
        const CIV_TECHS: [(&str, &str); 13] = [
            ("CIVILIZATION_HIPPUS", "TECH_STIRRUPS"),
            ("CIVILIZATION_ILLIANS", "TECH_IRON_WORKING"),
            ("CIVILIZATION_INFERNAL", "TECH_IRON_WORKING"),
            ("CIVILIZATION_KHAZAD", "TECH_IRON_WORKING"),
            ("CIVILIZATION_KURIOTATES", "TECH_STIRRUPS"),
            ("CIVILIZATION_LANUN", "TECH_IRON_WORKING"),
            ("CIVILIZATION_LJOSALFAR", "TECH_SORCERY"),
            ("CIVILIZATION_LUCHUIRP", "TECH_SORCERY"),
            ("CIVILIZATION_MALAKIM", "TECH_PRIESTHOOD"),
            ("CIVILIZATION_MERCURIANS", "TECH_IRON_WORKING"),
            ("CIVILIZATION_SHEAIM", "TECH_BRONZE_WORKING"),
            ("CIVILIZATION_SIDAR", "TECH_POISONS"),
            ("CIVILIZATION_SVARTALFAR", "TECH_SORCERY"),
        ];
        for (civ_name, tech_name) in CIV_TECHS {
            if is_civ(civ_name) && is_tech(tech_name) {
                start = true;
            }
        }

        if start {
            // `CvPlayer::startConquestMode`: `m_bConquestMode = true`.
            self.players[p].conquest_mode = true;
        }
    }

    /// onTechAcquired, first Infernal Pact: Hyborem's Infernals appear on the best empty land
    /// plot (random 0..500, plus area size, unowned area tiles, unowned plot and the founder's
    /// territory), share the founder's techs and wars, make peace with the barbarians.
    fn ev_infernal_pact(&mut self, p: PlayerId) {
        let d = self.d;
        let infernal_player = self.cf_get_open_player();
        let mut best_plot = None;
        let mut best_value = -1;
        for plot in self.map.plot_ids() {
            let mut value = -1;
            if !self.plot_is_water(plot)
                && self.map.plots[plot].units.is_empty()
                && self.map.plots[plot].plot_city.is_none()
                && !self.plot_is_impassable(plot)
            {
                value = self.soren_rand_num(500, "Place Hyborem");
                value += self.ev_plot_area_num_tiles(plot) * 2;
                value += self.ev_plot_area_num_unowned_tiles(plot) * 10;
                if !self.plot_is_owned(plot) {
                    value += 500;
                }
                if self.map.plots[plot].owner == Some(p) {
                    value += 200;
                }
            }
            if value > best_value {
                best_value = value;
                best_plot = Some(plot);
            }
        }

        let (Some(infernal_player), Some(best_plot)) = (infernal_player, best_plot) else { return };
        let hyborem = d.t::<LeaderHeadType>("LEADER_HYBOREM");
        let infernal = d.t::<CivilizationType>("CIVILIZATION_INFERNAL");
        self.add_player_advanced(infernal_player, None, hyborem, infernal);
        let founder_team = self.players[p].team;
        let infernal_team = self.players[infernal_player].team;
        for tech in d.techs.keys() {
            if self.team_is_has_tech(founder_team, Some(tech)) {
                self.team_set_has_tech(infernal_team, tech, true, Some(infernal_player), true, false);
            }
        }
        // Python quirk: `eFounderTeam.signOpenBorders(iFounderTeam)` and
        // `eInfernalTeam.signOpenBorders(iInfernalTeam)` sign with their own team, which
        // `CvTeam::signOpenBorders` ignores (`getID() != eTeam` test), so nothing is signed.
        let barbarian_team = self.players[self.barbarian_player()].team;
        self.team_make_peace(infernal_team, barbarian_team, true);
        for loop_team in self.team_ids() {
            if loop_team != barbarian_team
                && self.team_is_alive(loop_team)
                && self.teams[founder_team].at_war[loop_team]
            {
                self.team_declare_war(infernal_team, loop_team, false, Some(WarPlanType::WARPLAN_LIMITED));
            }
        }
        self.players[infernal_player].ai_attitude_extra[p] += 4;

        let (x, y) = self.map.xy(best_plot);
        let north = Some(DirectionType::DIRECTION_NORTH);
        let promotion = |name: &str| d.t::<PromotionType>(name);
        let units: [(&str, &[&str]); 12] = [
            ("UNIT_HYBOREM", &["PROMOTION_IMMORTAL"]),
            ("UNIT_LONGBOWMAN", &["PROMOTION_MOBILITY1"]),
            ("UNIT_LONGBOWMAN", &["PROMOTION_MOBILITY1"]),
            ("UNIT_CHAMPION", &["PROMOTION_IRON_WEAPONS", "PROMOTION_MOBILITY1"]),
            ("UNIT_CHAMPION", &["PROMOTION_IRON_WEAPONS", "PROMOTION_MOBILITY1"]),
            ("UNIT_WORKER", &["PROMOTION_DEMON"]),
            ("UNIT_IMP", &["PROMOTION_MOBILITY1"]),
            ("UNIT_MANES", &[]),
            ("UNIT_MANES", &[]),
            ("UNIT_MANES", &[]),
            ("UNIT_SETTLER", &["PROMOTION_STARTING_SETTLER", "PROMOTION_DEMON"]),
            ("UNIT_SETTLER", &["PROMOTION_STARTING_SETTLER", "PROMOTION_DEMON"]),
        ];
        for (i, (unit, promotions)) in units.into_iter().enumerate() {
            let new_unit = self.player_init_unit(infernal_player, d.t::<UnitType>(unit), x, y, None, north);
            for &name in promotions {
                self.unit_set_has_promotion(new_unit, promotion(name), true);
            }
            if i == 0 {
                // Hyborem: `newUnit1.setHasCasted(true)` (field).
                self.units[new_unit].has_casted = true;
            }
        }
        // `if gc.getPlayer(iPlayer).isHuman()`: the TXT_KEY_POPUP_CONTROL_INFERNAL popup, whose
        // `reassignPlayer` callback moves the human to the Infernals on "yes" (button 0).
        self.ev_offer_control(p, infernal_player);
    }

    /// The human-only "take control" popup of the Infernal Pact and the Mercurian Gate
    /// (BUTTONPOPUP_PYTHON with `CvScreensInterface.reassignPlayer`): on "yes" (the controller's
    /// `take_control`), `reassignPlayerAdvanced(player, new, -1)`. C++ moves the human when the
    /// popup is answered; here at once, at the end of the event.
    fn ev_offer_control(&mut self, player: PlayerId, new: PlayerId) {
        if self.players[player].human && self.controller(player).take_control(self, player, new) {
            crate::player::turn::reassign_player_advanced(self, player, new, -1);
        }
    }

    /// onBuildingBuilt, Mercurian Gate: the city's units step aside and Basium's Mercurians join
    /// the builder's team in the city.
    fn ev_mercurian_gate(&mut self, player: PlayerId, plot: PlotIdx) {
        let d = self.d;
        let mercurian_player = self.cf_get_open_player();
        let team = self.players[player].team;
        let plot2 = self.cf_find_clear_plot(None, plot);
        let (Some(mercurian_player), Some(plot2)) = (mercurian_player, plot2) else { return };
        let (x2, y2) = self.map.xy(plot2);
        // `for i in range(pPlot.getNumUnits(), -1, -1): pPlot.getUnit(i).setXY(..)`.
        let num_units = self.map.plots[plot].units.len();
        for i in (0..=num_units).rev() {
            if let Some(&unit) = self.map.plots[plot].units.get(i) {
                self.unit_set_xy(unit, x2, y2, true);
            }
        }
        let basium = d.t::<LeaderHeadType>("LEADER_BASIUM");
        let mercurians = d.t::<CivilizationType>("CIVILIZATION_MERCURIANS");
        self.add_player_advanced(mercurian_player, Some(team), basium, mercurians);
        let (x, y) = self.map.xy(plot);
        const UNITS: [&str; 10] = [
            "UNIT_BASIUM",
            "UNIT_SETTLER",
            "UNIT_ANGEL",
            "UNIT_ANGEL",
            "UNIT_ANGEL",
            "UNIT_ANGEL",
            "UNIT_ANGEL",
            "UNIT_ANGEL",
            "UNIT_WORKER",
            "UNIT_WORKER",
        ];
        for unit in UNITS {
            self.player_init_unit(mercurian_player, d.t::<UnitType>(unit), x, y, None, Some(DirectionType::DIRECTION_NORTH));
        }
        // `if pPlayer.isHuman()`: the TXT_KEY_POPUP_CONTROL_MERCURIANS popup.
        self.ev_offer_control(player, mercurian_player);
    }

    /// onProjectBuilt, Nature's Revolt: barbarian workers, scouts, warriors, hunters and axemen
    /// each turn into three animals; every animal gets the heroic promotions.
    fn ev_natures_revolt(&mut self) {
        let d = self.d;
        let barbarian = self.barbarian_player();
        let axeman = d.t::<UnitClassType>("UNITCLASS_AXEMAN");
        let bear = d.t::<UnitType>("UNIT_BEAR");
        let heroic_defense = d.t::<PromotionType>("PROMOTION_HEROIC_DEFENSE");
        let heroic_defense2 = d.t::<PromotionType>("PROMOTION_HEROIC_DEFENSE2");
        let heroic_strength = d.t::<PromotionType>("PROMOTION_HEROIC_STRENGTH");
        let heroic_strength2 = d.t::<PromotionType>("PROMOTION_HEROIC_STRENGTH2");
        let hunter = d.t::<UnitClassType>("UNITCLASS_HUNTER");
        let lion = d.t::<UnitType>("UNIT_LION");
        let scout = d.t::<UnitClassType>("UNITCLASS_SCOUT");
        let tiger = d.t::<UnitType>("UNIT_TIGER");
        let warrior = d.t::<UnitClassType>("UNITCLASS_WARRIOR");
        let wolf = d.t::<UnitType>("UNIT_WOLF");
        let worker = d.t::<UnitClassType>("UNITCLASS_WORKER");
        for unit in self.ev_unit_list(barbarian) {
            if !self.units.contains(unit) {
                continue;
            }
            let unit_class = self.unit_get_unit_class_type(unit);
            let mut new_unit = None;
            if unit_class == worker {
                new_unit = Some(wolf);
            }
            if unit_class == scout {
                new_unit = Some(lion);
            }
            if unit_class == warrior {
                new_unit = Some(lion);
            }
            if unit_class == hunter {
                new_unit = Some(tiger);
            }
            if unit_class == axeman {
                new_unit = Some(bear);
            }
            if let Some(new_unit) = new_unit {
                let (x, y) = (self.units[unit].x, self.units[unit].y);
                for _ in 0..3 {
                    self.player_init_unit(barbarian, new_unit, x, y, None, Some(DirectionType::DIRECTION_NORTH));
                }
                self.unit_kill(unit, true, None);
            }
        }
        for loop_player in self.player_ids() {
            if self.players[loop_player].alive {
                for unit in self.ev_unit_list(loop_player) {
                    if self.unit_is_animal(unit) {
                        self.unit_set_has_promotion(unit, heroic_defense, true);
                        self.unit_set_has_promotion(unit, heroic_defense2, true);
                        self.unit_set_has_promotion(unit, heroic_strength, true);
                        self.unit_set_has_promotion(unit, heroic_strength2, true);
                    }
                }
            }
        }
    }

    /// onProjectBuilt, Purge the Unfaithful: every non-holy foreign religion (and its
    /// buildings) leaves the player's cities; the cities fall into disorder for a few turns.
    fn ev_purge_the_unfaithful(&mut self, p: PlayerId) {
        let d = self.d;
        let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
        for city in self.ev_city_list(p) {
            let mut rnd = self.soren_rand_num(2, "Bob");
            let state_belief = self.player_get_state_religion(p);
            if state_belief == Some(order) {
                rnd -= 1;
            }
            for target in d.religions.keys() {
                if state_belief != Some(target)
                    && self.cities[city].has_religion[target]
                    && !self.city_is_holy_city(city, target)
                {
                    self.city_set_has_religion(city, target, false, true, true);
                    rnd += 1;
                    for building in d.buildings.keys() {
                        if d.buildings[building].prereq_religion == Some(target) {
                            self.city_set_num_real_building(city, building, 0);
                        }
                    }
                }
            }
            if rnd > 0 {
                self.city_set_occupation_timer(city, rnd);
            }
        }
    }

    /// onProjectBuilt, The Deepening: a quarter of the land cools for a while (tundra -> snow,
    /// grass/marsh/plains -> tundra, desert -> plains); cold plots may get a blizzard.
    fn ev_the_deepening(&mut self) {
        let d = self.d;
        let desert = d.t::<TerrainType>("TERRAIN_DESERT");
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        let marsh = d.t::<TerrainType>("TERRAIN_MARSH");
        let plains = d.t::<TerrainType>("TERRAIN_PLAINS");
        let snow = d.t::<TerrainType>("TERRAIN_SNOW");
        let tundra = d.t::<TerrainType>("TERRAIN_TUNDRA");
        let blizzard = d.t::<FeatureType>("FEATURE_BLIZZARD");
        let timer = 40 + self.game_speed_type().i() * 20;
        for plot in self.map.plot_ids() {
            let mut valid = false;
            if !self.plot_is_water(plot) {
                if self.soren_rand_num(100, "The Deepening") < 25 {
                    let terrain = self.map.plots[plot].terrain_type;
                    if terrain == Some(snow) {
                        valid = true;
                    }
                    if terrain == Some(tundra) {
                        let turns = self.soren_rand_num(timer, "Bob") + 10;
                        self.plot_set_temp_terrain_type(plot, snow, turns);
                        valid = true;
                    }
                    if terrain == Some(grass) || terrain == Some(marsh) {
                        let turns = self.soren_rand_num(timer, "Bob") + 10;
                        self.plot_set_temp_terrain_type(plot, tundra, turns);
                        valid = true;
                    }
                    if terrain == Some(plains) {
                        let turns = self.soren_rand_num(timer, "Bob") + 10;
                        self.plot_set_temp_terrain_type(plot, tundra, turns);
                        valid = true;
                    }
                    if terrain == Some(desert) {
                        let turns = self.soren_rand_num(timer, "Bob") + 10;
                        self.plot_set_temp_terrain_type(plot, plains, turns);
                    }
                    if valid {
                        if self.soren_rand_num(750, "The Deepening") < 10 {
                            self.plot_set_feature_type(plot, Some(blizzard));
                        }
                    }
                }
            }
        }
    }

    /// onProjectBuilt, The Draw: no diplomacy with enemies, war on every other living
    /// non-vassal team, the player's units at least half hurt and its cities halved.
    fn ev_the_draw(&mut self, p: PlayerId) {
        self.players[p].no_diplomacy_with_enemies += 1;
        let team = self.players[p].team;
        let barbarian_team = self.players[self.barbarian_player()].team;
        for loop_team in self.team_ids() {
            if loop_team != team && loop_team != barbarian_team {
                if self.team_is_alive(loop_team) && !self.team_is_a_vassal(loop_team) {
                    self.team_declare_war(team, loop_team, false, Some(WarPlanType::WARPLAN_LIMITED));
                }
            }
        }
        for unit in self.ev_unit_list(p) {
            if !self.units.contains(unit) {
                continue;
            }
            let mut damage = self.units[unit].damage * 2;
            if damage > 99 {
                damage = 99;
            }
            if damage < 50 {
                damage = 50;
            }
            self.unit_set_damage(unit, damage, Some(p));
        }
        for city in self.ev_city_list(p) {
            let mut pop = py_div(self.cities[city].population, 2);
            if pop < 1 {
                pop = 1;
            }
            self.city_set_population(city, pop);
        }
    }

    /// onProjectBuilt, Ascension: Auric ascends; the strongest rival (random, humans first,
    /// better ranks first) receives the Godslayer (a human through the Godslayer event).
    fn ev_ascension(&mut self, p: PlayerId, x: i32, y: i32) {
        let d = self.d;
        let auric = d.t::<UnitType>("UNIT_AURIC_ASCENDED");
        self.player_init_unit(p, auric, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
        if self.players[p].human {
            let trophy = "TROPHY_FEAT_ASCENSION";
            if !self.is_has_trophy(trophy) {
                self.change_trophy_value(trophy, 1);
            }
        }

        // Not a scenario.
        let mut best_player = None;
        let mut best_value = 0;
        for loop_player in self.player_ids() {
            if self.players[loop_player].alive
                && !self.player_is_barbarian(loop_player)
                && self.players[loop_player].team != self.players[p].team
            {
                let mut value = self.soren_rand_num(500, "Ascension");
                if self.players[loop_player].human {
                    value += 2000;
                }
                value += (20 - self.player_rank[loop_player]) * 50;
                if value > best_value {
                    best_value = value;
                    best_player = Some(loop_player);
                }
            }
        }
        if let Some(best_player) = best_player {
            let best_city = self.player_get_capital_city(best_player);
            if self.players[best_player].human {
                // A NULL capital (CyCity) answers -1 for its coordinates.
                let (cx, cy) = best_city.map_or((-1, -1), |city| (self.cities[city].x, self.cities[city].y));
                let trigger = d.t::<EventTriggerType>("EVENTTRIGGER_GODSLAYER");
                self.player_init_triggered_data(best_player, trigger, true, None, cx, cy, Some(best_player), None, None, None, None, None);
            } else if let Some(city) = best_city {
                // (Without a capital the Python would place the unit at (-1, -1); no unit here.)
                let (cx, cy) = (self.cities[city].x, self.cities[city].y);
                let godslayer = d.t::<UnitType>("EQUIPMENT_GODSLAYER");
                self.player_init_unit(best_player, godslayer, cx, cy, None, Some(DirectionType::DIRECTION_SOUTH));
            }
        }
    }

    /// onCityDoTurn, Citadel of Light: burns the strongest plot of enemy units (at war, no
    /// neutral units) within 2 plots; burning forest or jungle may catch fire.
    fn ev_citadel_of_light(&mut self, c: CityId, p: PlayerId, city_plot: PlotIdx) {
        let d = self.d;
        let (x, y) = (self.cities[c].x, self.cities[c].y);
        let team = self.players[p].team;
        let mut best_value = 0;
        let mut best_plot = None;
        for xx in x - 2..x + 3 {
            for yy in y - 2..y + 3 {
                let mut neutral = false;
                let mut value = 0;
                let Some(loop_plot) = self.map.plot_xy(xx, yy) else { continue };
                if self.plot_is_visible_enemy_unit(loop_plot, p) {
                    for i in 0..self.map.plots[loop_plot].units.len() {
                        let unit = self.map.plots[loop_plot].units[i];
                        let unit_team = self.unit_get_team(unit);
                        if self.teams[team].at_war[unit_team] {
                            value += 5 * self.unit_base_combat_str(unit);
                        } else {
                            neutral = true;
                        }
                    }
                    if value > best_value && !neutral {
                        best_value = value;
                        best_plot = Some(loop_plot);
                    }
                }
            }
        }
        let Some(best_plot) = best_plot else { return };
        let fire = d.t::<DamageType>("DAMAGE_FIRE");
        let num_units = self.map.plots[best_plot].units.len();
        for i in 0..num_units {
            if let Some(&unit) = self.map.plots[best_plot].units.get(i) {
                // `pUnit.doDamageNoCaster(20, 40, DAMAGE_FIRE, False)`.
                self.unit_do_damage(unit, 20, 40, None, Some(fire), false);
            }
        }
        let feature = self.map.plots[best_plot].feature_type;
        if feature == Some(d.t::<FeatureType>("FEATURE_FOREST")) || feature == Some(d.t::<FeatureType>("FEATURE_JUNGLE")) {
            let mut valid = true;
            // Python quirk: tests the improvement on the city's plot (`pPlot`), not `pBestPlot`.
            if let Some(improvement) = self.map.plots[city_plot].improvement_type {
                if d.improvements[improvement].permanent {
                    valid = false;
                }
            }
            if valid {
                if self.soren_rand_num(100, "Flames Spread") <= d.defines.flames_spread_chance {
                    self.plot_set_improvement_type(best_plot, Some(d.t::<ImprovementType>("IMPROVEMENT_SMOKE")));
                }
            }
        }
        // UI: EFFECT_PILLAR_OF_FIRE.
    }

    /// onCityDoTurn, Hall of Mirrors: an illusion copy of the last enemy unit (at war) seen next
    /// to the city appears in the city for 3 turns (5 with the Summoner trait).
    fn ev_hall_of_mirrors(&mut self, c: CityId, p: PlayerId) {
        let d = self.d;
        if self.soren_rand_num(100, "Hall of Mirrors") <= 100 {
            let mut copied = None;
            let (x, y) = (self.cities[c].x, self.cities[c].y);
            let team = self.players[p].team;
            for xx in x - 1..x + 2 {
                for yy in y - 1..y + 2 {
                    let Some(loop_plot) = self.map.plot_xy(xx, yy) else { continue };
                    if self.plot_is_visible_enemy_unit(loop_plot, p) {
                        for i in 0..self.map.plots[loop_plot].units.len() {
                            let unit = self.map.plots[loop_plot].units[i];
                            if self.teams[team].at_war[self.unit_get_team(unit)] {
                                copied = Some(unit);
                            }
                        }
                    }
                }
            }
            if let Some(copied) = copied {
                let unit_type = self.units[copied].unit_type;
                let new_unit = self.player_init_unit(p, unit_type, x, y, None, Some(DirectionType::DIRECTION_NORTH));
                self.unit_set_has_promotion(new_unit, d.t::<PromotionType>("PROMOTION_ILLUSION"), true);
                // `setDuration`: field (docs/porting/unit.md).
                if self.players[p].traits[d.t::<TraitType>("TRAIT_SUMMONER")] {
                    self.units[new_unit].duration = 5;
                } else {
                    self.units[new_unit].duration = 3;
                }
            }
        }
    }

    /// onCityDoTurn, Eyes and Ears Network: with open borders to at least three players, the
    /// team learns every tech (but a few) known to at least three of their teams.
    fn ev_eyes_and_ears_network(&mut self, p: PlayerId) {
        let d = self.d;
        let arete = d.t::<TechType>("TECH_ARETE");
        let hidden_paths = d.t::<TechType>("TECH_HIDDEN_PATHS");
        let infernal_pact = d.t::<TechType>("TECH_INFERNAL_PACT");
        let mind_stapling = d.t::<TechType>("TECH_MIND_STAPLING");
        let seafaring = d.t::<TechType>("TECH_SEAFARING");
        let team = self.players[p].team;
        // One entry per player (teammates repeat their team).
        let mut list_teams = Vec::new();
        for p2 in self.player_ids() {
            if self.players[p2].alive && p2 != p {
                let team2 = self.players[p2].team;
                if self.team_is_open_borders(team, team2) {
                    list_teams.push(team2);
                }
            }
        }
        if list_teams.len() >= 3 {
            for tech in d.techs.keys() {
                if tech != arete && tech != mind_stapling && tech != hidden_paths && tech != infernal_pact && tech != seafaring {
                    if !self.team_is_has_tech(team, Some(tech)) {
                        let count = list_teams.iter().filter(|&&t| self.team_is_has_tech(t, Some(tech))).count();
                        if count >= 3 {
                            self.team_set_has_tech(team, tech, true, Some(p), false, true);
                            self.ev_msg(p, "TXT_KEY_MESSAGE_EYES_AND_EARS_NETWORK_FREE_TECH");
                        }
                    }
                }
            }
        }
    }

    /// onCityDoTurn, Planar Gate: a chance (rising with the Armageddon counter) to summon a
    /// creature matching one of the city's buildings, limited per Planar Gate.
    fn ev_planar_gate(&mut self, c: CityId, p: PlayerId, plot: PlotIdx) {
        let d = self.d;
        let mut max = 1;
        // Python `iMult` (1, 1.5, 2, 2.5) doubled, so the chance test stays exact in integers:
        // `rand <= PLANAR_GATE_CHANCE * iMult` (an int against a float) is
        // `2 * rand <= PLANAR_GATE_CHANCE * (2 * iMult)`.
        let mut mult2 = 2;
        if self.get_global_counter() >= 50 {
            max = 2;
            mult2 = 3;
        }
        if self.get_global_counter() >= 75 {
            max = 3;
            mult2 = 4;
        }
        if self.get_global_counter() == 100 {
            max = 4;
            mult2 = 5;
        }
        if 2 * self.soren_rand_num(10000, "Planar Gate") <= d.defines.planar_gate_chance * mult2 {
            const SUMMONS: [(&str, &str, &str); 7] = [
                ("BUILDING_GAMBLING_HOUSE", "UNITCLASS_REVELERS", "UNIT_REVELERS"),
                ("BUILDING_MAGE_GUILD", "UNITCLASS_MOBIUS_WITCH", "UNIT_MOBIUS_WITCH"),
                ("BUILDING_CARNIVAL", "UNITCLASS_CHAOS_MARAUDER", "UNIT_CHAOS_MARAUDER"),
                ("BUILDING_GROVE", "UNITCLASS_MANTICORE", "UNIT_MANTICORE"),
                ("BUILDING_PUBLIC_BATHS", "UNITCLASS_SUCCUBUS", "UNIT_SUCCUBUS"),
                ("BUILDING_OBSIDIAN_GATE", "UNITCLASS_MINOTAUR", "UNIT_MINOTAUR"),
                ("BUILDING_TEMPLE_OF_THE_VEIL", "UNITCLASS_TAR_DEMON", "UNIT_TAR_DEMON"),
            ];
            let mut list_units = Vec::new();
            let max = max * self.player_count_num_buildings(p, d.t::<BuildingType>("BUILDING_PLANAR_GATE"));
            for (building, unit_class, unit) in SUMMONS {
                if self.city_num_building(c, d.t::<BuildingType>(building)) > 0
                    && self.players[p].unit_class_count[d.t::<UnitClassType>(unit_class)] < max
                {
                    list_units.push(d.t::<UnitType>(unit));
                }
            }
            if !list_units.is_empty() {
                let roll = self.soren_rand_num(list_units.len() as i32, "Planar Gate");
                let unit = list_units[roll as usize];
                let (x, y) = self.map.xy(plot);
                let new_unit = self.player_init_unit(p, unit, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
                self.ev_msg(p, "TXT_KEY_MESSAGE_PLANAR_GATE");
                if unit == d.t::<UnitType>("UNIT_MOBIUS_WITCH") {
                    const PROMOTIONS: [&str; 16] = [
                        "PROMOTION_AIR1",
                        "PROMOTION_BODY1",
                        "PROMOTION_CHAOS1",
                        "PROMOTION_DEATH1",
                        "PROMOTION_EARTH1",
                        "PROMOTION_ENCHANTMENT1",
                        "PROMOTION_ENTROPY1",
                        "PROMOTION_FIRE1",
                        "PROMOTION_LAW1",
                        "PROMOTION_LIFE1",
                        "PROMOTION_MIND1",
                        "PROMOTION_NATURE1",
                        "PROMOTION_SHADOW1",
                        "PROMOTION_SPIRIT1",
                        "PROMOTION_SUN1",
                        "PROMOTION_WATER1",
                    ];
                    self.unit_set_level(new_unit, 4);
                    self.unit_set_experience(new_unit, 14, -1);
                    for promotion in PROMOTIONS {
                        if self.soren_rand_num(10, "Bob") == 1 {
                            self.unit_set_has_promotion(new_unit, d.t::<PromotionType>(promotion), true);
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Helpers (Python helper modules and Cy* wrapper behaviour not declared elsewhere)
    // ---------------------------------------------------------------------------------------

    /// `PyPlayer(iPlayer).getUnitList()` (BtS PyHelpers): the player's units in order,
    /// skipping dead ones (`isDead()`: damage at max hit points).
    fn ev_unit_list(&self, p: PlayerId) -> Vec<UnitId> {
        self.players[p].units.iter().copied().filter(|&u| !self.unit_is_dead(u)).collect()
    }

    /// `PyPlayer(iPlayer).getCityList()` (BtS PyHelpers): the player's cities in order.
    fn ev_city_list(&self, p: PlayerId) -> Vec<CityId> {
        self.players[p].cities.clone()
    }

    /// `pPlot.area().getNumTiles()`: the Python NULL area answers -1 (`CyArea`).
    fn ev_plot_area_num_tiles(&self, plot: PlotIdx) -> i32 {
        match self.map.plots[plot].area {
            Some(area) => self.map.areas[area].num_tiles,
            None => -1,
        }
    }

    /// `pPlot.area().getNumUnownedTiles()`: the Python NULL area answers -1 (`CyArea`).
    fn ev_plot_area_num_unowned_tiles(&self, plot: PlotIdx) -> i32 {
        match self.map.plots[plot].area {
            Some(area) => self.area_get_num_unowned_tiles(area),
            None => -1,
        }
    }

    /// `cf.showUniqueImprovements(iPlayer)` (CustomFunctions.py; not in custom.rs): reveals
    /// every plot with a unique improvement to the player's team
    /// (`pPlot.setRevealed(iTeam, True, False, NO_TEAM)`, which updates plot groups).
    fn ev_show_unique_improvements(&mut self, p: PlayerId) {
        let d = self.d;
        let team = self.players[p].team;
        for plot in self.map.plot_ids() {
            if let Some(improvement) = self.map.plots[plot].improvement_type {
                if d.improvements[improvement].unique {
                    self.plot_set_revealed(plot, team, true, false, None, true);
                }
            }
        }
    }

    /// `cs.doTurn()` (CvSomniumInterface.py, FfH card game): once an AI civ's Somnium delay
    /// toward a human civ has run out, its positive and negative Somnium memories of that
    /// human are cleared (`while count > 0: AI_changeMemoryCount(.., -1)`).
    fn ev_somnium_do_turn(&mut self) {
        let positive = MemoryType::MEMORY_SOMNIUM_POSITIVE;
        let negative = MemoryType::MEMORY_SOMNIUM_NEGATIVE;
        let delay = MemoryType::MEMORY_SOMNIUM_DELAY;
        for ai in self.civ_player_ids() {
            if self.players[ai].human {
                continue;
            }
            for human in self.civ_player_ids() {
                if !self.players[human].human {
                    continue;
                }
                let memory = &mut self.players[ai].ai_memory_count[human];
                if memory[delay] <= 0 {
                    if memory[positive] > 0 {
                        memory[positive] = 0;
                    }
                    if memory[negative] > 0 {
                        memory[negative] = 0;
                    }
                }
            }
        }
    }

    /// `CvAdvisorUtils.unitBuiltFeats(pCity, pUnit)`: the first archer, mounted, melee, siege,
    /// naval and adept unit feats (the adept feat uses FEAT_UNITCOMBAT_HELICOPTER). Popups: UI.
    fn ev_unit_built_feats(&mut self, c: CityId, u: UnitId) {
        let d = self.d;
        let owner = self.cities[c].owner;
        const FEATS: [(FeatType, &str); 6] = [
            (FeatType::FEAT_UNITCOMBAT_ARCHER, "UNITCOMBAT_ARCHER"),
            (FeatType::FEAT_UNITCOMBAT_MOUNTED, "UNITCOMBAT_MOUNTED"),
            (FeatType::FEAT_UNITCOMBAT_MELEE, "UNITCOMBAT_MELEE"),
            (FeatType::FEAT_UNITCOMBAT_SIEGE, "UNITCOMBAT_SIEGE"),
            (FeatType::FEAT_UNITCOMBAT_NAVAL, "UNITCOMBAT_NAVAL"),
            (FeatType::FEAT_UNITCOMBAT_HELICOPTER, "UNITCOMBAT_ADEPT"),
        ];
        for (feat, unit_combat) in FEATS {
            if !self.players[owner].feat_accomplished[feat]
                && self.unit_get_unit_combat_type(u) == Some(d.t::<UnitCombatType>(unit_combat))
            {
                self.players[owner].feat_accomplished[feat] = true;
            }
        }
    }

    /// `CvAdvisorUtils.buildingBuiltFeats(pCity, iBuildingType)`: the first national wonder
    /// feat. Popup: UI.
    fn ev_building_built_feats(&mut self, c: CityId, building: BuildingType) {
        let d = self.d;
        let owner = self.cities[c].owner;
        if !self.players[owner].feat_accomplished[FeatType::FEAT_NATIONAL_WONDER] {
            if let Some(building_class) = d.buildings[building].building_class_type {
                if is_national_wonder_class(d, building_class) {
                    self.players[owner].feat_accomplished[FeatType::FEAT_NATIONAL_WONDER] = true;
                }
            }
        }
    }

    /// `CvAdvisorUtils.endTurnFeats(iPlayer)`: trade route, copper, horse, luxury and food
    /// feats (FfH comments out the population feats). Popups: UI.
    fn ev_end_turn_feats(&mut self, p: PlayerId) {
        let d = self.d;
        if !self.players[p].feat_accomplished[FeatType::FEAT_TRADE_ROUTE] {
            for city in self.ev_city_list(p) {
                if !self.city_is_capital(city) && self.city_is_connected_to_capital(city, Some(p)) {
                    self.players[p].feat_accomplished[FeatType::FEAT_TRADE_ROUTE] = true;
                    break;
                }
            }
        }

        let Some(capital) = self.player_get_capital_city(p) else { return };

        for (feat, bonus) in [(FeatType::FEAT_COPPER_CONNECTED, "BONUS_COPPER"), (FeatType::FEAT_HORSE_CONNECTED, "BONUS_HORSE")] {
            if !self.players[p].feat_accomplished[feat] {
                if let Some(bonus) = d.ty::<BonusType>(bonus) {
                    if self.city_has_bonus(capital, bonus) {
                        self.players[p].feat_accomplished[feat] = true;
                    }
                }
            }
        }

        if !self.players[p].feat_accomplished[FeatType::FEAT_LUXURY_CONNECTED] {
            for bonus in d.bonuses.keys() {
                if d.bonuses[bonus].happiness > 0 && self.city_has_bonus(capital, bonus) {
                    self.players[p].feat_accomplished[FeatType::FEAT_LUXURY_CONNECTED] = true;
                    break;
                }
            }
        }

        if !self.players[p].feat_accomplished[FeatType::FEAT_FOOD_CONNECTED] {
            for bonus in d.bonuses.keys() {
                if d.bonuses[bonus].health > 0 && self.city_has_bonus(capital, bonus) {
                    self.players[p].feat_accomplished[FeatType::FEAT_FOOD_CONNECTED] = true;
                    break;
                }
            }
        }
    }

    /// Game-log stand-in for `CyInterface().addMessage(iPlayer, .., text(key), ..)` (UI).
    fn ev_msg(&mut self, p: PlayerId, key: &'static str) {
        let d = self.d;
        let turn = self.game_turn();
        self.log.msg(turn, Some(p), || d.text.get(key).to_string());
    }
}

/// Index read by Python `lList[roll - 1]` for a roll in `0..len`: a roll of 0 is index -1, the
/// last element.
fn py_list_index(roll: i32, len: usize) -> usize {
    (roll - 1).rem_euclid(len as i32) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use crate::setup::GameSetup;

    /// A game whose first `n` slots hold freshly reset players (not `player_init`ed), alive on
    /// their own teams; the map is empty.
    fn game(n: usize) -> Game {
        let d = ffh_data::global();
        let setup = GameSetup::standard(d, n, "WORLDSIZE_DUEL", 7);
        let mut g = Game::new(d, setup);
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        for i in 0..n {
            let p = PlayerId(i as u8);
            g.players[p].alive = true;
            g.players[p].ever_alive = true;
        }
        g
    }

    #[test]
    fn python_list_index_wraps_roll_zero_to_last() {
        assert_eq!(py_list_index(0, 4), 3);
        assert_eq!(py_list_index(1, 4), 0);
        assert_eq!(py_list_index(3, 4), 2);
        assert_eq!(py_list_index(0, 1), 0);
    }

    #[test]
    fn somnium_memories_clear_once_the_delay_runs_out() {
        let mut g = game(3);
        let (ai, human, other_ai) = (PlayerId(0), PlayerId(1), PlayerId(2));
        g.players[human].human = true;
        let positive = MemoryType::MEMORY_SOMNIUM_POSITIVE;
        let negative = MemoryType::MEMORY_SOMNIUM_NEGATIVE;
        let delay = MemoryType::MEMORY_SOMNIUM_DELAY;
        g.players[ai].ai_memory_count[human][positive] = 2;
        g.players[ai].ai_memory_count[human][negative] = 1;
        g.players[ai].ai_memory_count[human][delay] = 1;
        g.players[other_ai].ai_memory_count[human][positive] = 3;
        // Memories of other AIs and a human's own memories are never touched.
        g.players[ai].ai_memory_count[other_ai][positive] = 4;
        g.players[human].ai_memory_count[ai][positive] = 5;

        g.ev_somnium_do_turn();
        assert_eq!(g.players[ai].ai_memory_count[human][positive], 2, "delay still running");
        assert_eq!(g.players[ai].ai_memory_count[human][negative], 1);
        assert_eq!(g.players[other_ai].ai_memory_count[human][positive], 0);

        g.players[ai].ai_memory_count[human][delay] = 0;
        g.ev_somnium_do_turn();
        assert_eq!(g.players[ai].ai_memory_count[human][positive], 0);
        assert_eq!(g.players[ai].ai_memory_count[human][negative], 0);
        assert_eq!(g.players[ai].ai_memory_count[other_ai][positive], 4);
        assert_eq!(g.players[human].ai_memory_count[ai][positive], 5);
    }

    #[test]
    fn state_religion_sets_the_religion_era() {
        let mut g = game(2);
        let d = g.d;
        let p = PlayerId(0);
        let start_era = g.players[p].current_era;
        g.ev_begin_player_turn(1, p);
        assert_eq!(g.players[p].current_era, start_era, "no state religion, no era change");

        g.players[p].state_religion_count = 1;
        g.players[p].last_state_religion = Some(d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL"));
        let seed = g.soren_rand.seed();
        g.ev_begin_player_turn(1, p);
        assert_eq!(g.players[p].current_era, d.t::<EraType>("ERA_VEIL"));
        g.players[p].last_state_religion = Some(d.t::<ReligionType>("RELIGION_COUNCIL_OF_ESUS"));
        g.ev_begin_player_turn(1, p);
        assert_eq!(g.players[p].current_era, d.t::<EraType>("ERA_COUN"));
        // Without the Insane trait no random number is drawn.
        assert_eq!(g.soren_rand.seed(), seed);
    }

    #[test]
    fn coming_alive_leaves_the_armageddon_counter_alone() {
        let mut g = game(2);
        let d = g.d;
        let p = PlayerId(0);
        g.global_counter = 20;
        g.global_counter_limit = 100;
        g.players[p].alignment = Some(d.t::<AlignmentType>("ALIGNMENT_GOOD"));
        g.ev_set_player_alive(p, true);
        assert_eq!(g.global_counter, 20);
    }

    #[test]
    fn dying_players_move_the_armageddon_counter_by_alignment() {
        let mut g = game(2);
        let d = g.d;
        let p = PlayerId(0);
        g.global_counter = 20;
        g.global_counter_limit = 100;

        // Before game year 5 nothing happens.
        g.start_year = -100;
        assert!(g.game_turn_year() < 5);
        g.players[p].alignment = Some(d.t::<AlignmentType>("ALIGNMENT_GOOD"));
        g.ev_set_player_alive(p, false);
        assert_eq!(g.global_counter, 20);

        g.start_year = 100;
        assert!(g.game_turn_year() >= 5);
        g.ev_set_player_alive(p, false);
        assert_eq!(g.global_counter, 25);
        g.players[p].alignment = Some(d.t::<AlignmentType>("ALIGNMENT_EVIL"));
        g.ev_set_player_alive(p, false);
        assert_eq!(g.global_counter, 20);
        g.players[p].alignment = Some(d.t::<AlignmentType>("ALIGNMENT_NEUTRAL"));
        g.ev_set_player_alive(p, false);
        assert_eq!(g.global_counter, 20);
    }

    #[test]
    fn destroyed_necrototem_lowers_the_armageddon_counter() {
        let mut g = game(2);
        let d = g.d;
        g.global_counter = 10;
        g.global_counter_limit = 100;
        g.ev_improvement_destroyed(d.t::<ImprovementType>("IMPROVEMENT_NECROTOTEM"), None, 0, 0);
        assert_eq!(g.global_counter, 8);
        g.ev_improvement_destroyed(d.t::<ImprovementType>("IMPROVEMENT_FARM"), None, 0, 0);
        assert_eq!(g.global_counter, 8);
    }

    #[test]
    fn introduction_trophy_is_awarded_once() {
        let mut g = game(2);
        g.ev_game_start();
        assert_eq!(g.trophy_value("TROPHY_FEAT_INTRODUCTION"), 1);
        g.ev_game_start();
        assert_eq!(g.trophy_value("TROPHY_FEAT_INTRODUCTION"), 1);
    }

    /// Every type name hard-coded in this file (string literals with a type prefix) exists in
    /// the game data, in the class its prefix names (Python `getInfoTypeForString` never -1).
    #[test]
    fn hard_coded_type_names_resolve() {
        let d = ffh_data::global();
        let mut checked = 0;
        let mut missing = Vec::new();
        for line in include_str!("events.rs").lines() {
            let code = line.split("//").next().unwrap_or("");
            for (i, s) in code.split('"').enumerate() {
                let is_name = i % 2 == 1
                    && s.len() > 2
                    && s.contains('_')
                    && !s.ends_with('_')
                    && s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
                if !is_name {
                    continue;
                }
                let prefixes: [(&str, fn(&GameData, &str) -> bool); 29] = [
                    ("UNITCLASS_", |d, s| d.ty::<UnitClassType>(s).is_some()),
                    ("UNITCOMBAT_", |d, s| d.ty::<UnitCombatType>(s).is_some()),
                    ("UNITAI_", |d, s| d.ty::<UnitAIType>(s).is_some()),
                    ("UNIT_", |d, s| d.ty::<UnitType>(s).is_some()),
                    ("EQUIPMENT_", |d, s| d.ty::<UnitType>(s).is_some()),
                    ("PROMOTION_", |d, s| d.ty::<PromotionType>(s).is_some()),
                    ("BONUSCLASS_", |d, s| d.ty::<BonusClassType>(s).is_some()),
                    ("BONUS_", |d, s| d.ty::<BonusType>(s).is_some()),
                    ("TERRAIN_", |d, s| d.ty::<TerrainType>(s).is_some()),
                    ("FEATURE_", |d, s| d.ty::<FeatureType>(s).is_some()),
                    ("IMPROVEMENT_", |d, s| d.ty::<ImprovementType>(s).is_some()),
                    ("TECH_", |d, s| d.ty::<TechType>(s).is_some()),
                    ("CIVILIZATION_", |d, s| d.ty::<CivilizationType>(s).is_some()),
                    ("CIVICOPTION_", |d, s| d.ty::<CivicOptionType>(s).is_some()),
                    ("CIVIC_", |d, s| d.ty::<CivicType>(s).is_some()),
                    ("RELIGION_", |d, s| d.ty::<ReligionType>(s).is_some()),
                    ("ALIGNMENT_", |d, s| d.ty::<AlignmentType>(s).is_some()),
                    ("BUILDING_", |d, s| d.ty::<BuildingType>(s).is_some()),
                    ("DAMAGE_", |d, s| d.ty::<DamageType>(s).is_some()),
                    ("EVENTTRIGGER_", |d, s| d.ty::<EventTriggerType>(s).is_some()),
                    ("PROJECT_", |d, s| d.ty::<ProjectType>(s).is_some()),
                    ("GAMESPEED_", |d, s| d.ty::<GameSpeedType>(s).is_some()),
                    ("WORLDSIZE_", |d, s| d.ty::<WorldSizeType>(s).is_some()),
                    ("ERA_", |d, s| d.ty::<EraType>(s).is_some()),
                    ("LEADER_", |d, s| d.ty::<LeaderHeadType>(s).is_some()),
                    ("TRAIT_", |d, s| d.ty::<TraitType>(s).is_some()),
                    ("VICTORY_", |d, s| d.ty::<VictoryType>(s).is_some()),
                    ("GAMEOPTION_", |d, s| d.ty::<GameOptionType>(s).is_some()),
                    ("HANDICAP_", |d, s| d.ty::<HandicapType>(s).is_some()),
                ];
                // Trophy names and text keys are not types.
                let Some((_, found)) = prefixes.iter().find(|(prefix, _)| s.starts_with(prefix)) else { continue };
                checked += 1;
                if !found(d, s) {
                    missing.push(s.to_string());
                }
            }
        }
        assert!(missing.is_empty(), "unknown type names: {missing:?}");
        assert!(checked > 300, "only {checked} names checked");
    }
}
