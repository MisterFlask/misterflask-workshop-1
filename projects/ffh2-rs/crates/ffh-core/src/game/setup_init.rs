//! CvGame: new game creation. `init`, `reset`, `setInitialItems`, `regenerateMap`,
//! `initDiplomacy`, `initFreeState`, `initFreeUnits`, `assignStartingPlots`, start
//! normalization (`normalizeStartingPlots` and its `normalize*` steps, `getTeamClosenessScore`),
//! `initScoreCalculation`, `setFinalInitialized`, `initEvents`, `addPlayer`,
//! `addPlayerAdvanced`, `pythonIsBonusIgnoreLatitudes`, plus the exe-side new-game sequence
//! (`game_init`).

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::game::Game;
use crate::ids::*;
use crate::slab::Slab;
use crate::structs::*;
use ffh_data::*;

impl Game {
    /// The new-game sequence the Civ4 exe drives through the DLL, reconstructed from
    /// `CvGame::regenerateMap` (map generation -> `setInitialItems` -> `initScoreCalculation`
    /// -> `setFinalInitialized(true)` -> `gameStart`): `CvMap::init`, `CvGame::init`,
    /// `CvTeam::init`/`CvPlayer::init` for every slot, `initDiplomacy`, map generation and
    /// game elements, starting items, score calculation, events.
    /// ORDER NOTE: the exe's placement of `initEvents` (soren rand) is not visible in the DLL.
    pub fn game_init(&mut self) {
        // CvMap::init(NULL): grid size from the world size / map script, plots, areas.
        self.map_init(None);
        // CvGame::init(eHandicap)
        let handicap = self.handicap;
        self.init(handicap);
        // CvTeam::init(eTeam) for every team slot, then CvPlayer::init(ePlayer) for every
        // player slot.
        for t in self.team_ids() {
            self.team_init(t);
        }
        self.game_init_players();
        self.init_diplomacy();
        // CvMapGenerator::generateRandomMap() + addGameElements()
        self.mapgen_generate_random_map();
        self.mapgen_add_game_elements();
        self.set_initial_items();
        self.init_score_calculation();
        self.set_final_initialized(true);
        self.init_events();
        // CvEventReporter::getInstance().gameStart() (FfH: Kael 10/14/2009). PENDING: ffh/events.rs
        // has no `ev_game_start` yet. FFH2's onGameStart is NOT UI-only: GAMEOPTION_THAW temp
        // terrain (soren rand per land plot), a barbarian UNIT_ARCHER_SCORPION_CLAN on every
        // IMPROVEMENT_GOBLIN_FORT (unless NO_BARBARIANS), TROPHY_FEAT_INTRODUCTION.
    }

    /// `game_init` step: C++ `CvPlayer::init(ePlayer)` for every player slot (player port:
    /// replace with a loop over `player_ids()` calling it).
    fn game_init_players(&mut self) {
        todo!("CvPlayer::init for all slots (call the player port from game_init)")
    }

    /// C++ `CvGame::init(eHandicap)`: `reset`, RNG seeds (`mapRandSeed % 73637381`,
    /// `syncRandSeed % 52319761`), FfH RANDOM_CIVILIZATION leader picks (soren rand),
    /// GAMEOPTION_LOCK_MODS password (7 soren draws), start turn from the game speed turn
    /// increments and era start percent, estimate end turn / max turns, `START_YEAR`,
    /// always-valid special units/buildings, `AI_init`, `doUpdateCacheOnTurn`.
    pub fn init(&mut self, handicap: HandicapType) {
        todo!("CvGame::init")
    }

    /// C++ `CvGame::reset(eHandicap)` (with `uninit`): every serialized CvGame member back to its
    /// initial value, containers emptied, RNGs reset to seed 0. `AI_reset` only clears a pad.
    pub fn reset(&mut self, handicap: HandicapType) {
        let d = self.d;
        let num_players = self.num_players();
        let num_teams = self.num_teams();

        self.elapsed_game_turns = 0;
        self.start_turn = 0;
        self.start_year = 0;
        self.estimate_end_turn = 0;
        self.turn_slice = 0;
        self.cutoff_slice = 0;
        self.num_game_turn_active = 0;
        self.num_cities = 0;
        self.total_population = 0;
        self.trade_routes = 0;
        self.free_trade_count = 0;
        self.no_nukes_count = 0;
        self.nukes_exploded = 0;
        self.max_population = 0;
        self.max_land = 0;
        self.max_tech = 0;
        self.max_wonders = 0;
        self.init_population = 0;
        self.init_land = 0;
        self.init_tech = 0;
        self.init_wonders = 0;
        self.ai_auto_play = 0;

        self.score_dirty = false;
        self.circumnavigated = false;
        self.final_initialized = false;
        self.nukes_valid = false;

        self.handicap = handicap;
        self.best_land_unit = None;
        self.winner = None;
        self.victory = None;
        self.game_state = GameStateType::GAMESTATE_ON;

        self.script_data = String::new();

        // FfH: Added by Kael 08/07/2007
        self.crime = 10;
        self.cut_losers_counter = 100;
        self.flexible_difficulty_counter = 0;
        self.global_counter = 0;
        self.high_to_low_counter = 0;
        self.increasing_difficulty_counter = 0;
        self.max_global_counter = 0;
        self.global_counter_limit = d.defines.global_counter_limit_default;
        self.scenario_counter = 0;
        // FfH: End Add

        self.rank_player = vec![PlayerId(0); num_players];
        self.player_rank = IdxVec::from_elem(0, num_players);
        self.player_score = IdxVec::from_elem(0, num_players);
        self.rank_team = vec![TeamId(0); num_teams];
        self.team_rank = IdxVec::from_elem(0, num_teams);
        self.team_score = IdxVec::from_elem(0, num_teams);

        // FfH: Added by Kael 11/14/2007
        self.event_triggered = IdxVec::from_elem(false, d.event_triggers.len());
        self.gambling_ring = IdxVec::from_elem(false, d.vote_sources.len());
        self.no_bonus = IdxVec::from_elem(false, d.bonuses.len());
        self.no_outside_tech_trades = IdxVec::from_elem(false, d.vote_sources.len());
        self.slave_trade = IdxVec::from_elem(false, d.vote_sources.len());
        self.smuggling_ring = IdxVec::from_elem(false, d.vote_sources.len());
        // FfH: End Add

        self.unit_created_count = IdxVec::from_elem(0, d.units.len());
        self.unit_class_created_count = IdxVec::from_elem(0, d.unit_classes.len());
        self.building_class_created_count = IdxVec::from_elem(0, d.building_classes.len());
        self.project_created_count = IdxVec::from_elem(0, d.projects.len());
        self.force_civic_count = IdxVec::from_elem(0, d.civics.len());
        self.vote_outcome = IdxVec::from_elem(PlayerVote::NO_PLAYER_VOTE, d.votes.len());
        self.diplo_vote = IdxVec::from_elem(0, d.vote_sources.len());
        self.special_unit_valid = IdxVec::from_elem(false, d.special_units.len());
        self.special_building_valid = IdxVec::from_elem(false, d.special_buildings.len());
        self.religion_game_turn_founded = IdxVec::from_elem(-1, d.religions.len());
        self.holy_city = IdxVec::from_elem(None, d.religions.len());
        self.religion_slot_taken = IdxVec::from_elem(false, d.religions.len());
        self.corporation_game_turn_founded = IdxVec::from_elem(-1, d.corporations.len());
        self.headquarters = IdxVec::from_elem(None, d.corporations.len());
        self.shrine_building = vec![None; d.buildings.len()];
        self.shrine_religion = vec![None; d.buildings.len()];
        self.secretary_general_timer = IdxVec::from_elem(0, d.vote_sources.len());
        self.vote_timer = IdxVec::from_elem(0, d.vote_sources.len());

        // uninit(): name lists, containers, extra yields/costs, vote source religions, triggers
        self.destroyed_cities.clear();
        self.great_people_born.clear();
        self.deals = Slab::new();
        self.vote_selections.clear();
        self.votes_triggered.clear();
        self.next_vote_selection_id = 0;
        self.next_vote_triggered_id = 0;
        self.plot_extra_yields.clear();
        self.plot_extra_costs.clear();
        self.vote_source_religions = IdxVec::from_elem(None, d.vote_sources.len());
        self.inactive_triggers.clear();

        self.map_rand.reset(0);
        self.soren_rand.reset(0);

        self.shrine_building_count = 0;
        self.num_culture_victory_cities = 0;
        self.culture_victory_culture_level = None;

        // AI_reset(): CvGameAI::m_iPad = 0 (nothing to port).
    }

    /// C++ `CvGame::setInitialItems`: `initFreeState`, `assignStartingPlots`,
    /// `normalizeStartingPlots`, FfH GAMEOPTION_BARBARIAN_WORLD barbarian cities (one per alive
    /// non-barbarian player), `initFreeUnits`. (`AI_updateFoundValues` per player is AI.)
    pub fn set_initial_items(&mut self) {
        todo!("CvGame::setInitialItems")
    }

    /// C++ `CvGame::regenerateMap` (pre-game "regenerate map": kill units/cities/deals, clear
    /// starting plots and reveals, erase plots, regenerate, `setInitialItems`, ...).
    pub fn regenerate_map(&mut self) {
        todo!("CvGame::regenerateMap")
    }

    /// C++ `CvGame::initDiplomacy`: teams meet themselves; barbarians/minors declare war on all
    /// civ teams (FfH: except barbarian allies); GAMEOPTION_ADVANCED_START forced peace deals.
    pub fn init_diplomacy(&mut self) {
        todo!("CvGame::initDiplomacy")
    }

    /// C++ `CvGame::initFreeState`: handicap/era/civilization free techs per team (FfH: era
    /// techs only with a research cost != -1), map reveal techs, `CvPlayer::initFreeState`.
    pub fn init_free_state(&mut self) {
        todo!("CvGame::initFreeState")
    }

    /// C++ `CvGame::initFreeUnits`: `CvPlayer::initFreeUnits` for alive players without
    /// units and cities.
    pub fn init_free_units(&mut self) {
        todo!("CvGame::initFreeUnits")
    }

    /// C++ `CvGame::assignStartingPlots`: preset starting plots, map script override, then team
    /// game / multiplayer / single player orders (`StartingLocationPercent` human slot), and
    /// the final re-placement pass in assignment order.
    pub fn assign_starting_plots(&mut self) {
        todo!("CvGame::assignStartingPlots")
    }

    /// C++ `CvGame::normalizeStartingPlotLocations`: swaps start locations to minimize the team
    /// closeness score (path distances, 5x plot distance between areas).
    pub fn normalize_starting_plot_locations(&mut self) {
        todo!("CvGame::normalizeStartingPlotLocations")
    }

    /// C++ `CvGame::normalizeAddRiver`.
    pub fn normalize_add_river(&mut self) {
        todo!("CvGame::normalizeAddRiver")
    }

    /// C++ `CvGame::normalizeRemovePeaks` (peaks within 3 of a start become hills).
    pub fn normalize_remove_peaks(&mut self) {
        todo!("CvGame::normalizeRemovePeaks")
    }

    /// C++ `CvGame::normalizeAddLakes`.
    pub fn normalize_add_lakes(&mut self) {
        todo!("CvGame::normalizeAddLakes")
    }

    /// C++ `CvGame::normalizeFindLakePlot(ePlayer)`.
    pub fn normalize_find_lake_plot(&self, player: PlayerId) -> Option<PlotIdx> {
        todo!("CvGame::normalizeFindLakePlot")
    }

    /// C++ `CvGame::normalizeRemoveBadFeatures`.
    pub fn normalize_remove_bad_features(&mut self) {
        todo!("CvGame::normalizeRemoveBadFeatures")
    }

    /// C++ `CvGame::normalizeRemoveBadTerrain` (FfH: only to terrains with `bNormalize`).
    pub fn normalize_remove_bad_terrain(&mut self) {
        todo!("CvGame::normalizeRemoveBadTerrain")
    }

    /// C++ `CvGame::normalizeAddFoodBonuses`.
    pub fn normalize_add_food_bonuses(&mut self) {
        todo!("CvGame::normalizeAddFoodBonuses")
    }

    /// C++ `CvGame::normalizeAddGoodTerrain`.
    pub fn normalize_add_good_terrain(&mut self) {
        todo!("CvGame::normalizeAddGoodTerrain")
    }

    /// C++ `CvGame::normalizeAddExtras` (uses `AI_foundValue`, map-rand shuffles and soren rand).
    pub fn normalize_add_extras(&mut self) {
        todo!("CvGame::normalizeAddExtras")
    }

    /// C++ `CvGame::normalizeStartingPlots`: each step runs unless the map script overrides it.
    /// (Worldbuilder map scripts are not supported, so the C++ `getWBMapScript()` checks are
    /// always false.)
    pub fn normalize_starting_plots(&mut self) {
        let ms = self.map_script.clone();
        if !ms.normalize_starting_plot_locations(self) {
            self.normalize_starting_plot_locations();
        }
        if !ms.normalize_add_river(self) {
            self.normalize_add_river();
        }
        if !ms.normalize_remove_peaks(self) {
            self.normalize_remove_peaks();
        }
        if !ms.normalize_add_lakes(self) {
            self.normalize_add_lakes();
        }
        if !ms.normalize_remove_bad_features(self) {
            self.normalize_remove_bad_features();
        }
        if !ms.normalize_remove_bad_terrain(self) {
            self.normalize_remove_bad_terrain();
        }
        if !ms.normalize_add_food_bonuses(self) {
            self.normalize_add_food_bonuses();
        }
        if !ms.normalize_add_good_terrain(self) {
            self.normalize_add_good_terrain();
        }
        if !ms.normalize_add_extras(self) {
            self.normalize_add_extras();
        }
    }

    /// C++ `CvGame::getTeamClosenessScore(aaiDistances, aiStartingLocs)`: sum over alive teams
    /// of the average distance between teammates' (hypothetical) starts. `distances[i][j]`
    /// for `j < i` (civ players), player i starting at `starting_locs[i]`.
    pub fn team_closeness_score(&self, distances: &[Vec<i32>], starting_locs: &[usize]) -> i32 {
        todo!("CvGame::getTeamClosenessScore")
    }

    /// C++ `CvGame::initScoreCalculation`: max population/land/tech/wonder scores and the
    /// initial ones (FfH: initial land from 21 plots per starting settler; no initial tech).
    pub fn init_score_calculation(&mut self) {
        todo!("CvGame::initScoreCalculation")
    }

    /// C++ `CvGame::setFinalInitialized(bNewValue)`: on true updates plot groups and irrigation
    /// (and the AI area strategies).
    pub fn set_final_initialized(&mut self, new_value: bool) {
        todo!("CvGame::setFinalInitialized")
    }

    /// C++ `CvGame::initEvents`: one soren draw per trigger against `iPercentGamesActive`
    /// (all inactive with GAMEOPTION_NO_EVENTS).
    pub fn init_events(&mut self) {
        todo!("CvGame::initEvents")
    }

    /// C++ `CvGame::isEventActive(eTrigger)`: not disabled for this game by `initEvents`.
    pub fn is_event_active(&self, trigger: EventTriggerType) -> bool {
        !self.inactive_triggers.contains(&trigger)
    }

    /// C++ `CvGame::addPlayer(eNewPlayer, eLeader, eCiv)` (Python: add a civ mid-game).
    pub fn add_player(&mut self, new_player: PlayerId, leader: LeaderHeadType, civ: CivilizationType) {
        todo!("CvGame::addPlayer")
    }

    /// C++ `CvGame::addPlayerAdvanced(eNewPlayer, iNewTeam, eLeader, eCiv)` (FfH: also picks a
    /// fresh team, sets war with barbarians/minors and credits the team's techs).
    pub fn add_player_advanced(&mut self, new_player: PlayerId, new_team: Option<TeamId>, leader: LeaderHeadType, civ: CivilizationType) {
        todo!("CvGame::addPlayerAdvanced")
    }

    /// C++ `CvGame::pythonIsBonusIgnoreLatitudes` (map script `isBonusIgnoreLatitude`).
    pub fn python_is_bonus_ignore_latitudes(&self) -> bool {
        let ms = self.map_script.clone();
        ms.is_bonus_ignore_latitude(self).unwrap_or(false)
    }
}
