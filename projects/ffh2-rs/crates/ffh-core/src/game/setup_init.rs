//! CvGame: new game creation. `init`, `reset`, `setInitialItems`, `regenerateMap`,
//! `initDiplomacy`, `initFreeState`, `initFreeUnits`, `assignStartingPlots`, start
//! normalization (`normalizeStartingPlots` and its `normalize*` steps, `getTeamClosenessScore`),
//! `initScoreCalculation`, `setFinalInitialized`, `initEvents`, `addPlayer`,
//! `addPlayerAdvanced`, `pythonIsBonusIgnoreLatitudes`, plus the exe-side new-game sequence
//! (`game_init`).
//!
//! Start normalization (`normalizeAddExtras`) needs `CvPlayerAI::AI_foundValue(x, y, -1, true)`
//! (`Game::player_ai_found_value`, found_value.rs).

use std::cmp::{max, min};

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::setup::SlotStatus;
use crate::slab::Slab;
use crate::structs::*;
use crate::utils::*;
use ffh_data::*;


/// C++ `MAX_PLAYERS` of FFH2 0.41 (`MAX_CIV_PLAYERS + 1`): `assignStartingPlots` runs
/// `2 * MAX_PLAYERS` team passes whatever the number of players in the game.
const CPP_MAX_PLAYERS: i32 = MAX_CIV_PLAYERS as i32 + 1;

const FOOD: usize = 0;
const PRODUCTION: usize = 1;

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
        // CvEventReporter::getInstance().gameStart() (FfH: Kael 10/14/2009, also at the end of
        // `regenerateMap`): GAMEOPTION_THAW temporary terrain, goblin-fort archers, trophies.
        self.ev_game_start();
    }

    /// `game_init` step: C++ `CvPlayer::init(ePlayer)` for every player slot.
    fn game_init_players(&mut self) {
        for p in self.player_ids() {
            self.player_init(p);
        }
    }

    /// C++ `CvGame::init(eHandicap)`: `reset`, RNG seeds (`mapRandSeed % 73637381`,
    /// `syncRandSeed % 52319761`), FfH RANDOM_CIVILIZATION leader picks (soren rand),
    /// GAMEOPTION_LOCK_MODS password (7 soren draws), start turn from the game speed turn
    /// increments and era start percent, estimate end turn / max turns, `START_YEAR`,
    /// always-valid special units/buildings, `AI_init`, `doUpdateCacheOnTurn`.
    pub fn init(&mut self, handicap: HandicapType) {
        let d = self.d;

        //--------------------------------
        // Init saved data
        self.reset(handicap);

        //--------------------------------
        // Init containers: m_deals / m_voteSelections / m_votesTriggered are empty after reset.

        self.map_rand.reset(self.setup.map_rand_seed % 73637381);
        self.soren_rand.reset(self.setup.sync_rand_seed % 52319761);

        // Multiplayer options are not ported (single process): C++ turns them all off for
        // single player games, so MPOPTION_SHUFFLE_TEAMS (soren-rand team shuffle) never runs.

        // FfH: Added by Kael 05/28/2008
        self.init_random_civilizations();
        // FfH: End Add

        if self.is_option(GameOptionType::GAMEOPTION_LOCK_MODS) {
            if self.is_game_multi_player() {
                self.setup.options[GameOptionType::GAMEOPTION_LOCK_MODS] = false;
            } else {
                // Random admin password (`setAdminPassword`; not stored): 7 soren draws.
                const PASSWORD_SIZE: i32 = 8;
                for _ in 0..PASSWORD_SIZE - 1 {
                    self.soren_rand_num(128, "");
                }
            }
        }

        if self.game_turn() == 0 {
            let speed = &d.game_speeds[self.game_speed_type()];
            let mut start_turn = 0;
            for i in 0..speed.num_turn_increments() {
                start_turn += speed.game_turn_info[i as usize].num_game_turns_per_increment;
            }

            start_turn *= d.eras[self.start_era()].start_percent;
            start_turn /= 100;

            self.set_game_turn(start_turn);
        }

        self.start_turn = self.game_turn();

        if self.max_turns() == 0 {
            let speed = &d.game_speeds[self.game_speed_type()];
            let mut estimate_end_turn = 0;
            for i in 0..speed.num_turn_increments() {
                estimate_end_turn += speed.game_turn_info[i as usize].num_game_turns_per_increment;
            }

            self.estimate_end_turn = estimate_end_turn;

            if self.estimate_end_turn > self.game_turn() {
                let mut valid = false;
                for v in d.victories.keys() {
                    if self.is_victory_valid(v) && d.victories[v].end_score {
                        valid = true;
                        break;
                    }
                }

                if valid {
                    self.setup.max_turns = self.estimate_end_turn - self.game_turn();
                }
            }
        } else {
            self.estimate_end_turn = self.game_turn() + self.max_turns();
        }

        self.start_year = d.defines.start_year;

        for su in d.special_units.keys() {
            if d.special_units[su].valid {
                // makeSpecialUnitValid
                self.special_unit_valid[su] = true;
            }
        }

        for sb in d.special_buildings.keys() {
            if d.special_buildings[sb].valid {
                self.make_special_building_valid(sb, false);
            }
        }

        // AI_init(): CvGameAI::AI_reset only clears a pad (nothing to port).

        self.do_update_cache_on_turn();
    }

    /// FfH block of `CvGame::init` (Kael 05/28/2008): every computer/taken civ slot without a
    /// civilization (or with `RANDOM_CIVILIZATION`) gets the best-scoring AI-playable
    /// civilization/leader pair (40000 + soren rand 1000, -2000 per slot already led by that
    /// leader, -1000 per slot of that civilization). A slot that chose CIVILIZATION_RANDOM with
    /// a leader is restricted to leaders of that leader's alignment.
    fn init_random_civilizations(&mut self) {
        let d = self.d;
        let rnd_civ_i = d.defines.random_civilization;
        if rnd_civ_i == -1 {
            return;
        }
        let rnd_civ = CivilizationType::from_usize(rnd_civ_i as usize);
        let num_civ_players = self.num_civ_players();

        let mut best_leader: Option<LeaderHeadType> = None;
        let mut best_civ: Option<CivilizationType> = None;
        let mut best_value = -1;

        for i in 0..num_civ_players {
            let status = self.setup.players[i].slot_status;
            if status == SlotStatus::Computer || status == SlotStatus::Taken {
                let mut alignment = 0;
                if self.setup.players[i].civ.is_none() {
                    self.setup.players[i].civ = Some(rnd_civ);
                    alignment = -1;
                }
                if self.setup.players[i].civ == Some(rnd_civ) {
                    if alignment != -1 {
                        // C++ reads the slot leader's alignment (undefined for NO_LEADER; taken
                        // as -1 = any alignment here).
                        alignment = self.setup.players[i].leader.map_or(-1, |l| opt_to_i32(d.leader_heads[l].alignment));
                    }
                    for civ in d.civilizations.keys() {
                        if self.civ_is_ai_playable(civ) {
                            for leader in d.leader_heads.keys() {
                                if d.civilizations[civ].leaders[leader] {
                                    if alignment == -1 || opt_to_i32(d.leader_heads[leader].alignment) == alignment {
                                        let mut value = 40000 + self.soren_rand_num(1000, "Random Leader");
                                        for ii in 0..num_civ_players {
                                            if self.setup.players[ii].leader == Some(leader) {
                                                value -= 2000;
                                            }
                                            if self.setup.players[ii].civ == Some(civ) {
                                                value -= 1000;
                                            }
                                        }

                                        if value > best_value {
                                            best_civ = Some(civ);
                                            best_leader = Some(leader);
                                            best_value = value;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    self.setup.players[i].civ = best_civ;
                    self.setup.players[i].leader = best_leader;
                    best_civ = None;
                    best_leader = None;
                    best_value = -1;
                }
            }
        }
    }

    /// C++ `CvCivilizationInfo::isAIPlayable()` (FfH: false for a graphical-only civilization
    /// once the game is final initialized).
    fn civ_is_ai_playable(&self, civ: CivilizationType) -> bool {
        let info = &self.d.civilizations[civ];
        if self.final_initialized && info.base.graphical_only {
            return false;
        }
        info.ai_playable
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
        self.init_free_state();
        self.assign_starting_plots();
        self.normalize_starting_plots();

        // FfH: Added by Kael 09/16/2008
        if self.is_option(GameOptionType::GAMEOPTION_BARBARIAN_WORLD) {
            let barbarian = self.barbarian_player();
            for p in self.player_ids() {
                if self.players[p].alive && p != barbarian {
                    self.found_barbarian_city();
                }
            }
        }
        // FfH: End Add

        self.init_free_units();

        // AI: CvPlayerAI::AI_updateFoundValues() for every alive player (refreshes the AI
        // found-value caches of plots and areas; no rules effect).
    }

    /// C++ `CvGame::regenerateMap` (pre-game "regenerate map": kill units/cities/deals, clear
    /// starting plots and reveals, erase plots, regenerate, `setInitialItems`, ...).
    /// (The C++ early return for world builder map scripts never applies: not supported.)
    pub fn regenerate_map(&mut self) {
        self.set_final_initialized(false);

        for p in self.player_ids() {
            self.player_kill_units(p);
        }

        // FfH: Added by Kael 10/14/2009
        for uc in self.d.unit_classes.keys() {
            self.unit_class_created_count[uc] = 0;
        }
        // FfH: End Add

        for p in self.player_ids() {
            self.player_kill_cities(p);
        }

        for p in self.player_ids() {
            self.player_kill_all_deals(p);
        }

        for p in self.player_ids() {
            // setFoundedFirstCity(false): only UI side effects besides the member.
            self.players[p].founded_first_city = false;
            self.player_set_starting_plot(p, None, false);
        }

        for t in self.team_ids() {
            self.map_set_revealed_plots(t, false, false);
        }

        // gDLL->getEngineIFace()->clearSigns() (graphics)

        self.map_erase_plots();

        self.mapgen_generate_random_map();
        self.mapgen_add_game_elements();

        // RebuildAllPlots (graphics); CvEventReporter::resetStatistics() (CvStatistics, not ported)

        self.set_initial_items();

        self.init_score_calculation();
        self.set_final_initialized(true);

        // setupGraphical, dirty bits, selection cycling, autosave, camera (UI)

        // FfH: Added by Kael 10/14/2009
        self.ev_game_start();
        // FfH: End Add
    }

    /// C++ `CvGame::initDiplomacy`: teams meet themselves; barbarians/minors declare war on all
    /// civ teams (FfH: except barbarian allies); GAMEOPTION_ADVANCED_START forced peace deals.
    pub fn init_diplomacy(&mut self) {
        for t in self.team_ids() {
            self.team_meet(t, t, false);

            if self.team_is_barbarian(t) || self.team_is_minor_civ(t) {
                for j in self.civ_team_ids() {
                    if t != j {
                        // FfH: Modified by Kael 08/02/2007
                        if !self.teams[j].barbarian_ally {
                            self.team_declare_war(t, j, false, None);
                        }
                        // FfH: End Add
                    }
                }
            }
        }

        // Forced peace at the beginning of Advanced starts
        if self.is_option(GameOptionType::GAMEOPTION_ADVANCED_START) {
            let trade = set_trade_item(TradeableItem::TRADE_PEACE_TREATY, 0);
            let player1_list = vec![trade.clone()];
            let player2_list = vec![trade];

            let num_civ_players = self.num_civ_players();
            for i1 in 0..num_civ_players {
                let p1 = PlayerId(i1 as u8);
                if self.players[p1].alive {
                    for i2 in (i1 + 1)..num_civ_players {
                        let p2 = PlayerId(i2 as u8);
                        if self.players[p2].alive {
                            let (t1, t2) = (self.players[p1].team, self.players[p2].team);
                            if self.team_can_change_war_peace(t1, t2, false) {
                                self.implement_deal(p1, p2, &player1_list, &player2_list, false);
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::initFreeState`: handicap/era/civilization free techs per team (FfH: era
    /// techs only with a research cost != -1), map reveal techs, `CvPlayer::initFreeState`.
    pub fn init_free_state(&mut self) {
        let d = self.d;
        let handicap = self.handicap;
        let start_era = self.start_era();

        for tech in d.techs.keys() {
            for t in self.team_ids() {
                if self.team_is_alive(t) {
                    let mut valid = d.handicaps[handicap].free_techs[tech]
                        || (!self.team_is_human(t) && d.handicaps[handicap].ai_free_techs[tech])
                        || (d.techs[tech].era < Some(start_era)
                            // FfH: Added by Kael 09/13/2007
                            && d.techs[tech].research_cost != -1);

                    if !valid {
                        for p in self.player_ids() {
                            if self.players[p].alive && self.players[p].team == t {
                                if let Some(civ) = self.player_get_civilization_type(p) {
                                    if d.civilizations[civ].civilization_free_techs[tech] {
                                        valid = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    self.team_set_has_tech(t, tech, valid, None, false, false);
                    if valid && d.techs[tech].map_visible {
                        self.map_set_revealed_plots(t, true, true);
                    }
                }
            }
        }

        for p in self.player_ids() {
            if self.players[p].alive {
                self.player_init_free_state(p);
            }
        }
    }

    /// C++ `CvGame::initFreeUnits`: `CvPlayer::initFreeUnits` for alive players without
    /// units and cities.
    pub fn init_free_units(&mut self) {
        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].num_units() == 0 && self.players[p].num_cities() == 0 {
                    self.player_init_free_units(p);
                }
            }
        }
    }

    /// C++ `CvGame::assignStartingPlots`: preset starting plots, map script override, then team
    /// game / multiplayer / single player orders (`StartingLocationPercent` human slot), and
    /// the final re-placement pass in assignment order.
    pub fn assign_starting_plots(&mut self) {
        let d = self.d;
        let mut player_order: Vec<PlayerId> = Vec::new();

        for p in self.civ_player_ids() {
            if self.players[p].alive && self.player_get_starting_plot(p).is_none() {
                let mut best_value = 0;
                let mut best_plot: Option<PlotIdx> = None;

                for plot in self.map.plot_ids() {
                    if self.map.plots[plot].starting_plot {
                        let mut valid = true;

                        for k in self.civ_player_ids() {
                            if self.players[k].alive && self.player_get_starting_plot(k) == Some(plot) {
                                valid = false;
                                break;
                            }
                        }

                        if valid {
                            let value = 1 + self.soren_rand_num(1000, "Starting Plot");

                            if value > best_value {
                                best_value = value;
                                best_plot = Some(plot);
                            }
                        }
                    }
                }

                if best_plot.is_some() {
                    self.player_set_starting_plot(p, best_plot, true);
                }
            }
        }

        let ms = self.map_script.clone();
        if ms.assign_starting_plots(self) {
            // Python override
            return;
        }

        if self.is_team_game() {
            // C++ runs 2 * MAX_PLAYERS passes (36 slots in FfH, independent of the players in
            // this game); the count only matters while some findStartingPlot keeps failing.
            for _pass in 0..2 * CPP_MAX_PLAYERS {
                let mut start_found = false;

                let num_alive_teams = self.count_civ_teams_alive();
                let rand_offset = self.soren_rand_num(num_alive_teams, "Team Starting Plot");

                // `% MAX_CIV_TEAMS` over the sized team list: alive teams are visited in the
                // same rotated order as with the C++ 35 slots.
                let num_civ_teams = self.num_civ_teams() as i32;
                for i in 0..num_civ_teams {
                    let loop_team = TeamId(((i + rand_offset) % num_civ_teams) as u8);

                    if self.team_is_alive(loop_team) {
                        for p in self.civ_player_ids() {
                            if self.players[p].alive
                                && self.players[p].team == loop_team
                                && self.player_get_starting_plot(p).is_none()
                            {
                                let starting_plot = self.player_find_starting_plot(p, false);

                                if starting_plot.is_some() {
                                    self.player_set_starting_plot(p, starting_plot, true);
                                    player_order.push(p);
                                }
                                start_found = true;
                                break;
                            }
                        }
                    }
                }

                if !start_found {
                    break;
                }
            }

            // check all players have starting plots
            for p in self.civ_player_ids() {
                debug_assert!(
                    !self.players[p].alive || self.player_get_starting_plot(p).is_some(),
                    "Player has no starting plot"
                );
            }
        } else if self.is_game_multi_player() {
            let num_alive = self.count_civ_players_alive();
            let rand_offset = self.soren_rand_num(num_alive, "Player Starting Plot");

            let num_civ_players = self.num_civ_players() as i32;
            for i in 0..num_civ_players {
                let loop_player = PlayerId(((i + rand_offset) % num_civ_players) as u8);

                if self.players[loop_player].alive
                    && self.players[loop_player].human
                    && self.player_get_starting_plot(loop_player).is_none()
                {
                    let starting_plot = self.player_find_starting_plot(loop_player, false);
                    self.player_set_starting_plot(loop_player, starting_plot, true);
                    player_order.push(loop_player);
                }
            }

            for p in self.civ_player_ids() {
                if self.players[p].alive && !self.players[p].human && self.player_get_starting_plot(p).is_none() {
                    let starting_plot = self.player_find_starting_plot(p, false);
                    self.player_set_starting_plot(p, starting_plot, true);
                    player_order.push(p);
                }
            }
        } else {
            let num_alive = self.count_civ_players_alive();
            let human_slot = range(
                ((num_alive - 1) * d.handicaps[self.handicap].starting_location_percent) / 100,
                0,
                num_alive - 1,
            );

            for i in 0..human_slot {
                let p = PlayerId(i as u8);
                if self.players[p].alive && !self.players[p].human && self.player_get_starting_plot(p).is_none() {
                    let starting_plot = self.player_find_starting_plot(p, false);
                    self.player_set_starting_plot(p, starting_plot, true);
                    player_order.push(p);
                }
            }

            for p in self.civ_player_ids() {
                if self.players[p].alive && self.players[p].human && self.player_get_starting_plot(p).is_none() {
                    let starting_plot = self.player_find_starting_plot(p, false);
                    self.player_set_starting_plot(p, starting_plot, true);
                    player_order.push(p);
                }
            }

            for p in self.civ_player_ids() {
                if self.players[p].alive && self.player_get_starting_plot(p).is_none() {
                    let starting_plot = self.player_find_starting_plot(p, false);
                    self.player_set_starting_plot(p, starting_plot, true);
                    player_order.push(p);
                }
            }
        }

        // Now iterate over the player starts in the original order and re-place them.
        for p in player_order {
            let starting_plot = self.player_find_starting_plot(p, false);
            self.player_set_starting_plot(p, starting_plot, true);
        }
    }

    /// C++ `CvGame::normalizeStartingPlotLocations`: swaps start locations to minimize the team
    /// closeness score (path distances, 5x plot distance between areas).
    pub fn normalize_starting_plot_locations(&mut self) {
        let num_civ_players = self.num_civ_players();

        // Precalculate distances between all starting positions (C++ rows of length iI; a NULL
        // row for dead players is an empty Vec here).
        let mut distances: Vec<Vec<i32>> = Vec::with_capacity(num_civ_players);
        for i in 0..num_civ_players {
            let pi = PlayerId(i as u8);
            if self.players[pi].alive {
                let mut row = vec![0; i];
                if let Some(plot_i) = self.player_get_starting_plot(pi) {
                    for (j, entry) in row.iter_mut().enumerate() {
                        let pj = PlayerId(j as u8);
                        if self.players[pj].alive {
                            if let Some(plot_j) = self.player_get_starting_plot(pj) {
                                let mut dist = self.map_calculate_path_distance(plot_i, plot_j);
                                if dist == -1 {
                                    // 5x penalty for not being on the same area, or having no passable route
                                    let (xi, yi) = self.map.xy(plot_i);
                                    let (xj, yj) = self.map.xy(plot_j);
                                    dist = 5 * self.plot_distance(xi, yi, xj, yj);
                                }
                                *entry = dist;
                            }
                        }
                    }
                }
                distances.push(row);
            } else {
                distances.push(Vec::new());
            }
        }

        // each player starting in own location
        let mut starting_locs: Vec<usize> = (0..num_civ_players).collect();

        let mut best_score = self.team_closeness_score(&distances, &starting_locs);
        let mut found_swap = true;
        while found_swap {
            found_swap = false;
            for i in 0..num_civ_players {
                if self.players[PlayerId(i as u8)].alive {
                    for j in 0..i {
                        if self.players[PlayerId(j as u8)].alive {
                            starting_locs.swap(i, j);
                            let score = self.team_closeness_score(&distances, &starting_locs);
                            if score < best_score {
                                best_score = score;
                                found_swap = true;
                            } else {
                                // Swap them back:
                                starting_locs.swap(i, j);
                            }
                        }
                    }
                }
            }
        }

        let mut new_start_plots: Vec<Option<PlotIdx>> = vec![None; num_civ_players];
        for (i, new_start) in new_start_plots.iter_mut().enumerate() {
            if self.players[PlayerId(i as u8)].alive && starting_locs[i] != i {
                *new_start = self.player_get_starting_plot(PlayerId(starting_locs[i] as u8));
            }
        }

        for (i, new_start) in new_start_plots.into_iter().enumerate() {
            let p = PlayerId(i as u8);
            if self.players[p].alive && new_start.is_some() {
                self.player_set_starting_plot(p, new_start, false);
            }
        }
    }

    /// C++ `CvGame::normalizeAddRiver`.
    pub fn normalize_add_river(&mut self) {
        let d = self.d;
        for p in self.civ_player_ids() {
            if !self.players[p].alive {
                continue;
            }
            let Some(starting_plot) = self.player_get_starting_plot(p) else {
                continue;
            };
            if self.plot_is_fresh_water(starting_plot) {
                continue;
            }

            // if we will be able to add a lake, then use old river code
            if self.normalize_find_lake_plot(p).is_some() {
                self.mapgen_do_river(starting_plot, None, None, -1);
            }
            // otherwise, use new river code which is much more likely to succeed
            else {
                self.mapgen_add_river(starting_plot);
            }

            // add floodplains to any desert tiles the new river passes through
            for plot in self.map.plot_ids() {
                for f in d.features.keys() {
                    if d.features[f].requires_river && self.plot_can_have_feature(plot, Some(f)) {
                        if d.features[f].appearance_probability == 10000 {
                            if self.map.plots[plot].bonus_type.is_some() {
                                self.plot_set_bonus_type(plot, None);
                            }
                            self.plot_set_feature_type(plot, Some(f));
                            break;
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::normalizeRemovePeaks` (peaks within 3 of a start become hills).
    pub fn normalize_remove_peaks(&mut self) {
        for p in self.civ_player_ids() {
            if !self.players[p].alive {
                continue;
            }
            if let Some(starting_plot) = self.player_get_starting_plot(p) {
                let (sx, sy) = self.map.xy(starting_plot);
                let range = 3;

                for dx in -range..=range {
                    for dy in -range..=range {
                        if let Some(loop_plot) = self.plot_xy(sx, sy, dx, dy) {
                            if self.plot_is_peak(loop_plot) {
                                self.plot_set_plot_type(loop_plot, PlotType::PLOT_HILLS, true);
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::normalizeAddLakes`.
    pub fn normalize_add_lakes(&mut self) {
        for p in self.civ_player_ids() {
            if self.players[p].alive {
                if let Some(lake_plot) = self.normalize_find_lake_plot(p) {
                    self.plot_set_plot_type(lake_plot, PlotType::PLOT_OCEAN, true);
                }
            }
        }
    }

    /// C++ `CvGame::normalizeFindLakePlot(ePlayer)`.
    pub fn normalize_find_lake_plot(&self, player: PlayerId) -> Option<PlotIdx> {
        if !self.players[player].alive {
            return None;
        }

        let starting_plot = self.player_get_starting_plot(player)?;
        if !self.plot_is_fresh_water(starting_plot) {
            let (sx, sy) = self.map.xy(starting_plot);
            for j in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(sx, sy, j) {
                    if !self.plot_is_water(loop_plot)
                        && !self.plot_is_coastal_land(loop_plot, -1)
                        && !self.plot_is_river(loop_plot)
                        && self.map.plots[loop_plot].bonus_type.is_none()
                    {
                        let mut starting = false;

                        for k in self.civ_player_ids() {
                            if self.players[k].alive && self.player_get_starting_plot(k) == Some(loop_plot) {
                                starting = true;
                                break;
                            }
                        }

                        if !starting {
                            return Some(loop_plot);
                        }
                    }
                }
            }
        }

        None
    }

    /// C++ `CvGame::normalizeRemoveBadFeatures`.
    pub fn normalize_remove_bad_features(&mut self) {
        let d = self.d;
        let bad_feature = |f: FeatureType| {
            d.features[f].yield_change[FOOD] <= 0 && d.features[f].yield_change[PRODUCTION] <= 0
        };

        for p in self.civ_player_ids() {
            if !self.players[p].alive {
                continue;
            }
            let Some(starting_plot) = self.player_get_starting_plot(p) else {
                continue;
            };
            let (sx, sy) = self.map.xy(starting_plot);

            for j in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(sx, sy, j) {
                    if let Some(f) = self.map.plots[loop_plot].feature_type {
                        if bad_feature(f) {
                            self.plot_set_feature_type(loop_plot, None);
                        }
                    }
                }
            }

            let city_range = CITY_PLOTS_RADIUS;
            let extra_range = 2;
            let max_range = city_range + extra_range;

            for ix in -max_range..=max_range {
                for iy in -max_range..=max_range {
                    let Some(loop_plot) = self.plot_xy(sx, sy, ix, iy) else {
                        continue;
                    };
                    let (lx, ly) = self.map.xy(loop_plot);
                    let distance = self.plot_distance(sx, sy, lx, ly);
                    if distance > max_range {
                        continue;
                    }
                    let Some(f) = self.map.plots[loop_plot].feature_type else {
                        continue;
                    };
                    if !bad_feature(f) {
                        continue;
                    }
                    if self.plot_is_water(loop_plot) {
                        if self.plot_is_adjacent_to_land(loop_plot)
                            || (distance != max_range && self.soren_rand_num(2, "Remove Bad Feature") == 0)
                        {
                            self.plot_set_feature_type(loop_plot, None);
                        }
                    } else {
                        // C++ quirk: `getSorenRandNum((2 + (getBonusType() == NO_BONUS) ? 0 : 2), ..)`
                        // parses as `(2 + bool) ? 0 : 2`, always 0: the draw still happens but
                        // always returns 0, so every bad land feature short of the outer ring goes.
                        let num = if 2 + (self.map.plots[loop_plot].bonus_type.is_none() as i32) != 0 { 0 } else { 2 };
                        if distance != max_range && self.soren_rand_num(num, "Remove Bad Feature") == 0 {
                            self.plot_set_feature_type(loop_plot, None);
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::normalizeRemoveBadTerrain` (FfH: only to terrains with `bNormalize`).
    pub fn normalize_remove_bad_terrain(&mut self) {
        let d = self.d;
        let city_range = CITY_PLOTS_RADIUS;
        let extra_range = 1;
        let max_range = city_range + extra_range;

        for p in self.civ_player_ids() {
            if !self.players[p].alive {
                continue;
            }
            let Some(starting_plot) = self.player_get_starting_plot(p) else {
                continue;
            };
            let (sx, sy) = self.map.xy(starting_plot);
            let team = self.players[p].team;

            for ix in -max_range..=max_range {
                for iy in -max_range..=max_range {
                    let Some(loop_plot) = self.plot_xy(sx, sy, ix, iy) else {
                        continue;
                    };
                    let (lx, ly) = self.map.xy(loop_plot);
                    let distance = self.plot_distance(sx, sy, lx, ly);
                    if distance > max_range {
                        continue;
                    }
                    if !(!self.plot_is_water(loop_plot)
                        && (distance <= city_range
                            || self.plot_is_coastal_land(loop_plot, -1)
                            || 0 == self.soren_rand_num(1 + distance - city_range, "Map Upgrade Terrain Food")))
                    {
                        continue;
                    }

                    let terrain = self.map.plots[loop_plot].terrain_type.expect("land plot without terrain");
                    let plot_food = d.terrains[terrain].yield_[FOOD];
                    let plot_production = d.terrains[terrain].yield_[PRODUCTION];
                    if plot_food + plot_production > 1 {
                        continue;
                    }

                    let target_food;
                    let target_total;
                    if self.plot_get_bonus_type(loop_plot, Some(team)).is_some() {
                        target_food = 1;
                        target_total = 2;
                    } else if plot_food == 1 || distance <= city_range {
                        target_food = 1 + self.soren_rand_num(2, "Map Upgrade Terrain Food");
                        target_total = 2;
                    } else {
                        target_food = if self.plot_is_coastal_land(loop_plot, -1) { 2 } else { 1 };
                        target_total = 2;
                    }

                    for k in d.terrains.keys() {
                        let kt = &d.terrains[k];
                        if !kt.water && kt.yield_[FOOD] >= target_food && kt.yield_[FOOD] + kt.yield_[PRODUCTION] == target_total {
                            let feature_ok = match self.map.plots[loop_plot].feature_type {
                                None => true,
                                Some(f) => d.features[f].terrain[k],
                            };
                            if feature_ok {
                                // FfH: Modified by Kael 08/02/2007
                                if kt.normalize {
                                    self.plot_set_terrain_type(loop_plot, k, true);
                                }
                                // FfH: End Modify
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::normalizeAddFoodBonuses`.
    pub fn normalize_add_food_bonuses(&mut self) {
        let d = self.d;
        let ignore_latitude = self.python_is_bonus_ignore_latitudes();
        let start_era = self.start_era();
        let city_trade_in_start_era = |b: BonusType| match d.bonuses[b].tech_city_trade {
            None => true,
            Some(t) => d.techs[t].era <= Some(start_era),
        };

        for p in self.civ_player_ids() {
            if !self.players[p].alive {
                continue;
            }
            let Some(starting_plot) = self.player_get_starting_plot(p) else {
                continue;
            };
            let (sx, sy) = self.map.xy(starting_plot);
            let team = self.players[p].team;

            let mut food_bonus = 0;
            let mut good_nature_tile_count = 0;

            for j in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(sx, sy, j) {
                    match self.plot_get_bonus_type(loop_plot, Some(team)) {
                        Some(bonus) => {
                            if d.bonuses[bonus].yield_change[FOOD] > 0 {
                                if city_trade_in_start_era(bonus) {
                                    if self.plot_is_water(loop_plot) {
                                        food_bonus += 2;
                                    } else {
                                        food_bonus += 3;
                                    }
                                }
                            } else if self.plot_calculate_best_nature_yield(loop_plot, YieldType::YIELD_FOOD, Some(team)) >= 2 {
                                good_nature_tile_count += 1;
                            }
                        }
                        None => {
                            if self.plot_calculate_best_nature_yield(loop_plot, YieldType::YIELD_FOOD, Some(team)) >= 3 {
                                good_nature_tile_count += 1;
                            }
                        }
                    }
                }
            }

            let mut target_food_bonus_count = 3;
            target_food_bonus_count += if good_nature_tile_count == 0 { 2 } else { 0 };

            for j in 0..NUM_CITY_PLOTS {
                if food_bonus >= target_food_bonus_count {
                    break;
                }

                let Some(loop_plot) = self.plot_city(sx, sy, j) else {
                    continue;
                };
                if loop_plot == starting_plot || self.map.plots[loop_plot].bonus_type.is_some() {
                    continue;
                }
                for k in d.bonuses.keys() {
                    let kb = &d.bonuses[k];
                    if kb.normalize
                        && kb.yield_change[FOOD] > 0
                        && city_trade_in_start_era(k)
                        && self.team_is_has_tech(team, kb.tech_reveal)
                        && self.plot_can_have_bonus(loop_plot, Some(k), ignore_latitude)
                    {
                        self.plot_set_bonus_type(loop_plot, Some(k));
                        if self.plot_is_water(loop_plot) {
                            food_bonus += 2;
                        } else {
                            food_bonus += 3;
                        }
                        break;
                    }
                }
            }
        }
    }

    /// C++ `CvGame::normalizeAddGoodTerrain`.
    pub fn normalize_add_good_terrain(&mut self) {
        let d = self.d;
        let food_per_pop = d.defines.food_consumption_per_population;

        for p in self.civ_player_ids() {
            if !self.players[p].alive {
                continue;
            }
            let Some(starting_plot) = self.player_get_starting_plot(p) else {
                continue;
            };
            let (sx, sy) = self.map.xy(starting_plot);
            let team = self.players[p].team;

            let mut good_plot = 0;

            for j in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(sx, sy, j) {
                    if loop_plot != starting_plot
                        && self.plot_calculate_nature_yield(loop_plot, YieldType::YIELD_FOOD, Some(team), false) >= food_per_pop
                        && self.plot_calculate_nature_yield(loop_plot, YieldType::YIELD_PRODUCTION, Some(team), false) > 0
                    {
                        good_plot += 1;
                    }
                }
            }

            for j in 0..NUM_CITY_PLOTS {
                if good_plot >= 4 {
                    break;
                }

                let Some(loop_plot) = self.plot_city(sx, sy, j) else {
                    continue;
                };
                if loop_plot == starting_plot
                    || self.plot_is_water(loop_plot)
                    || self.plot_is_hills(loop_plot)
                    || self.map.plots[loop_plot].bonus_type.is_some()
                {
                    continue;
                }

                let mut changed = false;

                if self.plot_calculate_nature_yield(loop_plot, YieldType::YIELD_FOOD, Some(team), false) < food_per_pop {
                    for k in d.terrains.keys() {
                        if !d.terrains[k].water && d.terrains[k].yield_[FOOD] >= food_per_pop {
                            self.plot_set_terrain_type(loop_plot, k, true);
                            changed = true;
                            break;
                        }
                    }
                }

                if self.plot_calculate_nature_yield(loop_plot, YieldType::YIELD_PRODUCTION, Some(team), false) == 0 {
                    for k in d.features.keys() {
                        let kf = &d.features[k];
                        if kf.yield_change[FOOD] >= 0 && kf.yield_change[PRODUCTION] > 0 {
                            // C++ indexes isTerrain() with the plot's terrain (always set here).
                            let on_terrain = self.map.plots[loop_plot].terrain_type.is_some_and(|t| kf.terrain[t]);
                            if on_terrain {
                                self.plot_set_feature_type(loop_plot, Some(k));
                                changed = true;
                                break;
                            }
                        }
                    }
                }

                if changed {
                    good_plot += 1;
                }
            }
        }
    }

    /// C++ `CvGame::normalizeAddExtras` (uses `AI_foundValue`, map-rand shuffles and soren rand).
    pub fn normalize_add_extras(&mut self) {
        let d = self.d;
        let ignore_latitude = self.python_is_bonus_ignore_latitudes();
        let start_era = self.start_era();
        let city_trade_in_start_era = |b: BonusType| match d.bonuses[b].tech_city_trade {
            None => true,
            Some(t) => d.techs[t].era <= Some(start_era),
        };
        // C++ `isNormalize()` with no negative food/production yield ("no bonuses with negative
        // yields?") and a city trade tech no later than the start era.
        let normalize_bonus = |b: BonusType| {
            let kb = &d.bonuses[b];
            kb.normalize && kb.yield_change[FOOD] >= 0 && kb.yield_change[PRODUCTION] >= 0 && city_trade_in_start_era(b)
        };

        // C++ also sums iTotalValue / iPlayerCount / iWorstValue, which are never used.
        let mut best_value = 0;

        for p in self.civ_player_ids() {
            if self.players[p].alive {
                if let Some(starting_plot) = self.player_get_starting_plot(p) {
                    let (sx, sy) = self.map.xy(starting_plot);
                    let value = self.player_ai_found_value(p, sx, sy, -1, true, &[]);
                    best_value = max(value, best_value);
                }
            }
        }

        //iTargetValue = (iTotalValue + iBestValue) / (iPlayerCount + 1);
        let target_value = (best_value * 4) / 5;

        for p in self.civ_player_ids() {
            if !self.players[p].alive {
                continue;
            }
            let Some(starting_plot) = self.player_get_starting_plot(p) else {
                continue;
            };
            let (sx, sy) = self.map.xy(starting_plot);
            let team = self.players[p].team;

            let mut count = 0;
            let mut feature_count = 0;
            let mut shuffle_order = [0i32; NUM_CITY_PLOTS];
            shuffle_array(&mut shuffle_order, &mut self.map_rand);

            for &index in shuffle_order.iter() {
                if self.player_ai_found_value(p, sx, sy, -1, true, &[]) >= target_value {
                    break;
                }
                if self.soren_rand_num(count + 2, "Setting Feature Type") <= 1 {
                    let Some(loop_plot) = self.plot_city(sx, sy, index as usize) else {
                        continue;
                    };
                    if loop_plot == starting_plot || self.map.plots[loop_plot].bonus_type.is_some() {
                        continue;
                    }
                    if self.map.plots[loop_plot].feature_type.is_none() {
                        for k in d.features.keys() {
                            if d.features[k].yield_change[FOOD] + d.features[k].yield_change[PRODUCTION] > 0
                                && self.plot_can_have_feature(loop_plot, Some(k))
                            {
                                self.plot_set_feature_type(loop_plot, Some(k));
                                count += 1;
                                break;
                            }
                        }
                    }

                    feature_count += self.map.plots[loop_plot].feature_type.is_some() as i32;
                }
            }

            let mut coast_food_count = 0;
            let mut ocean_food_count = 0;
            let mut other_count = 0;
            let mut water_count = 0;
            for j in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(sx, sy, j) {
                    if loop_plot != starting_plot {
                        if self.plot_is_water(loop_plot) {
                            water_count += 1;
                            if self.map.plots[loop_plot].bonus_type.is_some() {
                                if self.plot_is_adjacent_to_land(loop_plot) {
                                    coast_food_count += 1;
                                } else {
                                    ocean_food_count += 1;
                                }
                            }
                        } else if self.map.plots[loop_plot].bonus_type.is_some() {
                            other_count += 1;
                        }
                    }
                }
            }

            let land_bias = water_count > (NUM_CITY_PLOTS / 2) as i32;

            shuffle_array(&mut shuffle_order, &mut self.map_rand);

            for &index in shuffle_order.iter() {
                let Some(loop_plot) = self.plot_city(sx, sy, index as usize) else {
                    continue;
                };
                if loop_plot == starting_plot {
                    continue;
                }
                let num = if land_bias && self.plot_is_water(loop_plot) { 2 } else { 1 };
                if self.soren_rand_num(num, "Placing Bonuses") != 0 {
                    continue;
                }

                if other_count * 3 + ocean_food_count * 2 + coast_food_count * 2 >= 12 {
                    break;
                }

                if self.player_ai_found_value(p, sx, sy, -1, true, &[]) >= target_value {
                    break;
                }

                let coast = self.plot_is_water(loop_plot) && self.plot_is_adjacent_to_land(loop_plot);
                let ocean = self.plot_is_water(loop_plot) && !coast;
                if loop_plot == starting_plot || (coast && coast_food_count > 2) || (ocean && ocean_food_count > 2) {
                    continue;
                }
                let (lx, ly) = self.map.xy(loop_plot);

                for pass in 0..2 {
                    if self.map.plots[loop_plot].bonus_type.is_some() {
                        continue;
                    }
                    for k in d.bonuses.keys() {
                        if normalize_bonus(k) && self.team_is_has_tech(team, d.bonuses[k].tech_reveal) {
                            let can_place = if pass == 0 {
                                self.mapgen_can_place_bonus_at(k, lx, ly, ignore_latitude)
                            } else {
                                self.plot_can_have_bonus(loop_plot, Some(k), ignore_latitude)
                            };
                            if can_place {
                                self.plot_set_bonus_type(loop_plot, Some(k));
                                coast_food_count += coast as i32;
                                ocean_food_count += ocean as i32;
                                other_count += !(coast || ocean) as i32;
                                break;
                            }
                        }
                    }

                    if land_bias && !self.plot_is_water(loop_plot) && self.map.plots[loop_plot].bonus_type.is_none() {
                        if (feature_count > 4 && self.map.plots[loop_plot].feature_type.is_some())
                            && (coast_food_count + ocean_food_count > 2)
                        {
                            if self.soren_rand_num(2, "Clear feature to add bonus") == 0 {
                                self.plot_set_feature_type(loop_plot, None);

                                // (no TechReveal check in this C++ loop)
                                for k in d.bonuses.keys() {
                                    if normalize_bonus(k) {
                                        let can_place = if pass == 0 {
                                            self.mapgen_can_place_bonus_at(k, lx, ly, ignore_latitude)
                                        } else {
                                            self.plot_can_have_bonus(loop_plot, Some(k), ignore_latitude)
                                        };
                                        if can_place {
                                            self.plot_set_bonus_type(loop_plot, Some(k));
                                            other_count += 1;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            shuffle_array(&mut shuffle_order, &mut self.map_rand);

            for &index in shuffle_order.iter() {
                if self.player_ai_found_value(p, sx, sy, -1, true, &[]) >= target_value {
                    break;
                }

                let Some(loop_plot) = self.plot_city(sx, sy, index as usize) else {
                    continue;
                };
                if loop_plot != starting_plot
                    && self.map.plots[loop_plot].bonus_type.is_none()
                    && self.map.plots[loop_plot].feature_type.is_none()
                {
                    for k in d.features.keys() {
                        if d.features[k].yield_change[FOOD] + d.features[k].yield_change[PRODUCTION] > 0
                            && self.plot_can_have_feature(loop_plot, Some(k))
                        {
                            self.plot_set_feature_type(loop_plot, Some(k));
                            break;
                        }
                    }
                }
            }

            let mut hills_count = 0;

            for j in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(sx, sy, j) {
                    if self.plot_is_hills(loop_plot) {
                        hills_count += 1;
                    }
                }
            }
            shuffle_array(&mut shuffle_order, &mut self.map_rand);
            for &index in shuffle_order.iter() {
                if hills_count >= 3 {
                    break;
                }
                // (the starting plot itself is not excluded here)
                let Some(loop_plot) = self.plot_city(sx, sy, index as usize) else {
                    continue;
                };
                if !self.plot_is_water(loop_plot) && !self.plot_is_hills(loop_plot) {
                    let feature_ok = self.map.plots[loop_plot].feature_type.is_none_or(|f| !d.features[f].requires_flatlands);
                    let bonus_ok = self.map.plots[loop_plot].bonus_type.is_none_or(|b| d.bonuses[b].hills);
                    if feature_ok && bonus_ok {
                        self.plot_set_plot_type(loop_plot, PlotType::PLOT_HILLS, false);
                        hills_count += 1;
                    }
                }
            }
        }
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
    ///
    /// C++ quirk: the teammate loop does not check `isAlive`, so a dead slot on an alive team
    /// counts as an edge of distance 0 (its row is NULL in C++ when it has the larger index,
    /// which would crash; here an empty row also reads as 0).
    pub fn team_closeness_score(&self, distances: &[Vec<i32>], starting_locs: &[usize]) -> i32 {
        let mut score = 0;

        for t in self.civ_team_ids() {
            if !self.team_is_alive(t) {
                continue;
            }
            let mut team_total_dist = 0;
            let mut num_edges = 0;
            for p in self.civ_player_ids() {
                if self.players[p].alive && self.players[p].team == t {
                    for o in 0..p.idx() {
                        if self.players[PlayerId(o as u8)].team == t {
                            // Add the edge between these two players that are on the same team
                            num_edges += 1;
                            let mut player_start = starting_locs[p.idx()];
                            let mut other_player_start = starting_locs[o];

                            // Make sure that iPlayerStart > iOtherPlayerStart
                            if player_start < other_player_start {
                                std::mem::swap(&mut player_start, &mut other_player_start);
                            } else {
                                debug_assert!(
                                    player_start != other_player_start,
                                    "Two players are (hypothetically) assigned to the same starting location!"
                                );
                            }
                            team_total_dist += distances[player_start].get(other_player_start).copied().unwrap_or(0);
                        }
                    }
                }
            }

            // the avg distance between team edges is the team score
            let team_score = if num_edges == 0 { 0 } else { team_total_dist / num_edges };

            score += team_score;
        }
        score
    }

    /// C++ `CvGame::initScoreCalculation`: max population/land/tech/wonder scores and the
    /// initial ones (FfH: initial land from 21 plots per starting settler; no initial tech).
    pub fn init_score_calculation(&mut self) {
        let d = self.d;

        // initialize score calculation
        let mut max_food = 0;
        for plot in self.map.plot_ids() {
            if !self.plot_is_water(plot) || self.plot_is_adjacent_to_land(plot) {
                max_food += self.plot_calculate_best_nature_yield(plot, YieldType::YIELD_FOOD, None);
            }
        }
        self.max_population = get_population_score(max_food / max(1, d.defines.food_consumption_per_population));
        self.max_land = get_land_plots_score(self.map.land_plots);
        self.max_tech = 0;
        for tech in d.techs.keys() {
            self.max_tech += get_tech_score(d, tech);
        }
        self.max_wonders = 0;
        for bc in d.building_classes.keys() {
            self.max_wonders += get_wonder_score(d, bc);
        }

        // C++ `if (NO_ERA != getStartEra())`: the start era is always set here.
        let era = &d.eras[self.start_era()];
        let num_settlers = era.starting_unit_multiplier;
        self.init_population = get_population_score(num_settlers * (era.free_population + 1));

        // FfH: Modified by Kael 11/18/2007 (21 = the radius-2 city plots, not NUM_CITY_PLOTS)
        self.init_land = get_land_plots_score(num_settlers * NUM_CITY_PLOTS_RADIUS_2);

        // FfH: Modified by Kael 01/31/2009 (the initial tech score is no longer counted)
        self.init_tech = 0;

        self.init_wonders = 0;
    }

    /// C++ `CvGame::setFinalInitialized(bNewValue)`: on true updates plot groups and irrigation
    /// (and the AI area strategies).
    pub fn set_final_initialized(&mut self, new_value: bool) {
        if self.final_initialized != new_value {
            self.final_initialized = new_value;

            if self.final_initialized {
                self.update_plot_groups();

                self.map_update_irrigated();

                // AI: CvTeamAI::AI_updateAreaStragies() for every alive team.
            }
        }
    }

    /// C++ `CvGame::initEvents`: one soren draw per trigger against `iPercentGamesActive`
    /// (all inactive with GAMEOPTION_NO_EVENTS).
    pub fn init_events(&mut self) {
        let d = self.d;
        for trigger in d.event_triggers.keys() {
            if self.is_option(GameOptionType::GAMEOPTION_NO_EVENTS)
                || self.soren_rand_num(100, "Event Active?") >= d.event_triggers[trigger].percent_games_active
            {
                self.inactive_triggers.push(trigger);
            }
        }
    }

    /// C++ `CvGame::isEventActive(eTrigger)`: not disabled for this game by `initEvents`.
    pub fn is_event_active(&self, trigger: EventTriggerType) -> bool {
        !self.inactive_triggers.contains(&trigger)
    }

    /// C++ `CvGame::addPlayer(eNewPlayer, eLeader, eCiv)` (Python: add a civ mid-game).
    ///
    /// The C++ first picks a player color not used by any civ slot (graphics only; `GameSetup`
    /// has no color slot, so that part is not ported).
    pub fn add_player(&mut self, new_player: PlayerId, leader: LeaderHeadType, civ: CivilizationType) {
        let team = self.players[new_player].team;
        self.setup.players[new_player.idx()].leader = Some(leader);
        self.setup.players[new_player.idx()].civ = Some(civ);
        self.setup_set_slot_status(new_player, SlotStatus::Computer);
        self.team_init(team);
        self.player_init(new_player);
    }

    /// C++ `CvInitCore::setTeam(eID, eTeam)`: on change, `CvPlayer::updateTeamType`.
    fn setup_set_team(&mut self, p: PlayerId, team: TeamId) {
        if self.setup.players[p.idx()].team != team {
            self.setup.players[p.idx()].team = team;
            self.player_update_team_type(p);
        }
    }

    /// C++ `CvInitCore::setSlotStatus(eID, eSlotStatus)`: on change, `CvPlayer::updateHuman`.
    fn setup_set_slot_status(&mut self, p: PlayerId, status: SlotStatus) {
        if self.setup.players[p.idx()].slot_status != status {
            self.setup.players[p.idx()].slot_status = status;
            self.player_update_human(p);
        }
    }

    /// C++ `CvGame::addPlayerAdvanced(eNewPlayer, iNewTeam, eLeader, eCiv)` (FfH: also picks a
    /// fresh team, sets war with barbarians/minors and credits the team's techs).
    ///
    /// The C++ player color pick is not ported (see [`Game::add_player`]).
    pub fn add_player_advanced(&mut self, new_player: PlayerId, new_team: Option<TeamId>, leader: LeaderHeadType, civ: CivilizationType) {
        let d = self.d;
        let mut new_team = new_team;
        if let Some(t) = new_team {
            self.player_set_team(new_player, t);
            // gDLL->getInterfaceIFace()->setDirty(Fog_DIRTY_BIT, true) (UI)
        } else {
            let mut valid = false;
            for i in self.civ_team_ids() {
                if !self.team_is_ever_alive(i) && !valid {
                    new_team = Some(i);
                    valid = true;
                }
            }
            // (bValid <=> a never-alive team slot was found)
            if let Some(nt) = new_team {
                self.team_init(nt);
                for j in self.team_ids() {
                    if (self.team_is_barbarian(j) || self.team_is_minor_civ(j)) && nt != j {
                        // C++ quirk: CvTeam::setAtWar is one-sided; the barbarian/minor team's
                        // own flag towards the new team stays false (cleared by `init`).
                        self.teams[nt].at_war[j] = true;
                    }
                }
            }
        }
        // C++ continues with NO_TEAM when every civ team slot was ever alive (undefined
        // behaviour in CvPlayer::init); keep the slot's current team instead.
        let new_team = new_team.unwrap_or(self.setup.players[new_player.idx()].team);

        self.setup_set_team(new_player, new_team);
        self.setup.players[new_player.idx()].leader = Some(leader);
        self.setup.players[new_player.idx()].civ = Some(civ);
        self.setup_set_slot_status(new_player, SlotStatus::Computer);
        self.player_init(new_player);

        for tech in d.techs.keys() {
            if self.team_is_has_tech(new_team, Some(tech)) {
                // changeAssets / changePower: plain member changes.
                self.players[new_player].assets += d.techs[tech].asset_value;
                self.players[new_player].power += d.techs[tech].power_value;
                self.player_change_tech_score(new_player, get_tech_score(d, tech));
            }
        }
    }

    /// C++ `CvGame::pythonIsBonusIgnoreLatitudes` (map script `isBonusIgnoreLatitude`).
    pub fn python_is_bonus_ignore_latitudes(&self) -> bool {
        let ms = self.map_script.clone();
        ms.is_bonus_ignore_latitude(self).unwrap_or(false)
    }

    // ---------------------------------------------------------------------------------------
    // AI_foundValue for starting locations (used by start normalization)
    // ---------------------------------------------------------------------------------------

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::GameSetup;

    fn game(n_civs: usize) -> Game {
        let d = ffh_data::global();
        Game::new(d, GameSetup::standard(d, n_civs, "WORLDSIZE_SMALL", 4242))
    }

    fn set_alive(g: &mut Game, p: u8, team: u8) {
        g.players[PlayerId(p)].alive = true;
        g.players[PlayerId(p)].team = TeamId(team);
        g.teams[TeamId(team)].alive_count += 1;
    }

    #[test]
    fn team_closeness_score_averages_team_edges() {
        let mut g = game(4);
        // Teams: {0, 2} and {1, 3}; every other slot has its own (dead) team.
        set_alive(&mut g, 0, 0);
        set_alive(&mut g, 1, 1);
        set_alive(&mut g, 2, 0);
        set_alive(&mut g, 3, 1);
        let n = g.num_civ_players();
        for i in 4..n {
            g.players[PlayerId(i as u8)].team = TeamId(i as u8);
        }
        // distances[i][j], j < i
        let mut distances: Vec<Vec<i32>> = (0..n).map(|i| vec![0; i]).collect();
        distances[1][0] = 10;
        distances[2][0] = 4;
        distances[2][1] = 6;
        distances[3][0] = 8;
        distances[3][1] = 12;
        distances[3][2] = 2;
        let mut locs: Vec<usize> = (0..n).collect();
        // team 0: edge (2, 0) = 4; team 1: edge (3, 1) = 12
        assert_eq!(g.team_closeness_score(&distances, &locs), 4 + 12);
        // player 1 starts at 2's location and vice versa: team 0 edge (1, 0) = 10,
        // team 1 edge (3, 2) = 2.
        locs.swap(1, 2);
        assert_eq!(g.team_closeness_score(&distances, &locs), 10 + 2);
    }

    #[test]
    fn team_closeness_score_counts_dead_teammates() {
        let mut g = game(4);
        // Player 3 is alive on team 1, whose own slot (player 1) is dead: C++ counts the edge
        // (3, 1) with the zero distance of a dead player's column.
        set_alive(&mut g, 0, 0);
        set_alive(&mut g, 3, 1);
        g.players[PlayerId(1)].team = TeamId(1);
        g.players[PlayerId(2)].team = TeamId(2);
        let n = g.num_civ_players();
        for i in 4..n {
            g.players[PlayerId(i as u8)].team = TeamId(i as u8);
        }
        let mut distances: Vec<Vec<i32>> = (0..n).map(|i| vec![7; i]).collect();
        distances[1] = Vec::new(); // dead row (C++ NULL)
        distances[2] = Vec::new();
        let locs: Vec<usize> = (0..n).collect();
        // team 0 has no edges (0); team 1: one edge (3 -> 1) with distance 7 / 1 edge.
        assert_eq!(g.team_closeness_score(&distances, &locs), 7);
        // Player 3 hypothetically at location 0: the edge reads row 1 (dead: empty) -> 0.
        let mut locs2 = locs.clone();
        locs2.swap(0, 3);
        assert_eq!(g.team_closeness_score(&distances, &locs2), 0);
    }

    #[test]
    fn init_events_draws_once_per_trigger() {
        let d = ffh_data::global();
        let mut g = game(2);
        g.soren_rand.reset(99);
        g.init_events();
        // Reference: same draws by hand.
        let mut r = crate::rng::Rng::new(99);
        let expect: Vec<EventTriggerType> = d
            .event_triggers
            .keys()
            .filter(|&t| r.get_i(100, "") >= d.event_triggers[t].percent_games_active)
            .collect();
        assert_eq!(g.inactive_triggers, expect);
        assert_eq!(g.soren_rand, r);
        assert!(d.event_triggers.keys().all(|t| g.is_event_active(t) != expect.contains(&t)));

        // GAMEOPTION_NO_EVENTS: everything inactive, no draws.
        let mut g = game(2);
        g.setup.options[GameOptionType::GAMEOPTION_NO_EVENTS] = true;
        g.soren_rand.reset(5);
        g.init_events();
        assert_eq!(g.inactive_triggers.len(), d.event_triggers.len());
        assert_eq!(g.soren_rand.seed(), 5);
    }

    #[test]
    fn random_civilizations_pick_playable_civ_and_leader() {
        let d = ffh_data::global();
        let mut g = game(6);
        g.soren_rand.reset(1234);
        g.init_random_civilizations();
        let n_civs = 6;
        let mut leaders = Vec::new();
        for i in 0..g.num_civ_players() {
            let s = &g.setup.players[i];
            if i < n_civs {
                let civ = s.civ.expect("civ picked");
                let leader = s.leader.expect("leader picked");
                assert!(d.civilizations[civ].ai_playable, "{}", d.type_name(civ));
                assert!(d.civilizations[civ].leaders[leader]);
                leaders.push(leader);
            } else {
                // open slots are left alone
                assert_eq!(s.civ, None);
                assert_eq!(s.leader, None);
            }
        }
        // -2000 per slot already led by a leader: no leader is picked twice.
        let mut sorted = leaders.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), leaders.len());
    }

    #[test]
    fn find_lake_plot_needs_alive_player() {
        let g = game(2);
        assert_eq!(g.normalize_find_lake_plot(PlayerId(0)), None);
    }
}
