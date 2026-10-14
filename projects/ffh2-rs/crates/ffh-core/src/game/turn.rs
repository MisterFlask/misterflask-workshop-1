//! CvGame: the update loop and turn processing (`update`, `doTurn`, turn bookkeeping,
//! `updateMoves`, `updateWar`, timers, `testAlive`), world updates (plot groups, building
//! commerce, city sight, trade routes, global warming), deals, global queries/counters,
//! score ranking (`updateScore`) and victory (`testVictory`, `setWinner`, `setGameState`).
//!
//! Single process, no networking: MP options (`MPOPTION_SIMULTANEOUS_TURNS`,
//! `MPOPTION_TURN_TIMER`), PBEM, hot seat and simultaneous team turns are never on, so turns
//! are sequential in player slot order. The exe's world builder mode, autosaves, interface
//! dirty bits and engine calls are not ported.

use std::collections::BTreeSet;

use crate::deal::Deal;
use crate::defs::{MAX_CIV_PLAYERS, MIN_INT, RANDPLOT_LAND, RANDPLOT_NOT_CITY};
use crate::game::Game;
use crate::ids::*;
use crate::player::set_turn_history;
use crate::player::turn::reassign_player_advanced;
use crate::structs::*;
use crate::utils::get_turn_year_for_game;
use ffh_data::*;

impl Game {
    // ---------------------------------------------------------------------------------------
    // Update loop and turn processing
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::update`: one turn slice. Starts the next game turn (`doTurn`) when no
    /// player's turn is active, then `updateScore`, `updateWar`, `updateMoves`, `updateTimers`,
    /// (MP turn timer), citizen assignment for human players (`AI_updateAssignWork` ->
    /// governor), `testAlive`, game over when no human is alive (unless AI autoplay, autorun or
    /// extended), `turn_slice += 1`.
    pub fn update(&mut self) {
        // C++: if (!gDLL->GetWorldBuilderMode() || isInAdvancedStart()) -- there is no world
        // builder here, so the slice always runs.

        // sendPlayerOptions(): network. FfH sends the "gameUpdate" generic Python event only in
        // network games (isNetworkMultiPlayer()). AutoSave(true) on turn slice 0: not ported.

        if self.num_game_turn_active == 0 {
            // C++: if (!isPbem() || !getPbemTurnSent()) -- no PBEM games.
            self.do_turn();
        }

        self.update_score(false);

        self.update_war();

        self.update_moves();

        self.update_timers();

        // updateTurnTimer(): MP turn timer (MPOPTION_TURN_TIMER), never set.

        self.ai_update_assign_work();

        self.test_alive();

        if self.ai_auto_play == 0 && !self.autorun && self.game_state != GameStateType::GAMESTATE_EXTENDED {
            if self.count_human_players_alive() == 0 {
                self.set_game_state(GameStateType::GAMESTATE_OVER);
            }
        }

        // changeTurnSlice(1)
        self.turn_slice += 1;

        // Advanced start interface / world builder for the active player: UI.
    }

    /// C++ `CvGame::doTurn`: end of game turn. Score, deals, `CvTeam::doTurn`, `CvMap::doTurn`,
    /// barbarian cities/units, global warming, holy cities, headquarters, diplo votes, AI
    /// autoplay countdown, `incrementGameTurn`/`incrementElapsedGameTurns`, turn activation
    /// (sequential: first alive player), FfH challenge options (cut losers, high to low,
    /// increasing/flexible difficulty, scaled by the speed's growth percent), `testVictory`.
    /// Python `beginGameTurn`/`endGameTurn` events -> crate::ffh (FfH's per-turn world effects
    /// such as hell terrain and Orthus live in `onBeginGameTurn`).
    pub fn do_turn(&mut self) {
        // END OF TURN
        let turn = self.game_turn();
        self.ev_begin_game_turn(turn);

        self.do_update_cache_on_turn();

        self.update_score(false);

        self.do_deals();

        for t in self.team_ids() {
            if self.team_is_alive(t) {
                self.team_do_turn(t);
            }
        }

        self.map_do_turn();

        self.create_barbarian_cities();

        self.create_barbarian_units();

        self.do_global_warming();

        self.do_holy_city();

        self.do_headquarters();

        self.do_diplo_vote();

        // setEndTurnMessage(false), setHasMovedUnit(false): UI.

        if self.ai_auto_play > 0 {
            self.change_ai_auto_play(-1);

            // C++: if (getAIAutoPlay() == 0) reviveActivePlayer(); -- revives the dead active
            // (human) player when autoplay ends. Not ported (docs/porting/game.md): it acts on
            // the UI's active player, which does not exist headless (C++ would index
            // GET_PLAYER(NO_PLAYER)).
        }

        let turn = self.game_turn();
        self.ev_end_game_turn(turn);

        self.increment_game_turn();
        // incrementElapsedGameTurns()
        self.elapsed_game_turns += 1;

        // MPOPTION_SIMULTANEOUS_TURNS (every alive player starts at once, in a soren-rand
        // shuffled order) is a network option: never set.
        if self.is_simultaneous_team_turns() {
            for t in self.team_ids() {
                if self.team_is_alive(t) {
                    self.team_set_turn_active(t, true, true);
                    debug_assert_eq!(self.num_game_turn_active, self.teams[t].alive_count);

                    // FfH: Modified by Kael 06/20/2010 prevents a WoC when the first player is
                    // killed (the break moved inside the alive check)
                    break;
                }
            }
        } else {
            for p in self.player_ids() {
                if self.players[p].alive {
                    // (C++ PBEM branch for human players -- hot seat for the active player,
                    // e-mailed turns for the others -- does not apply: no PBEM games.)
                    self.player_set_turn_active(p, true, true);
                    debug_assert_eq!(self.num_game_turn_active, 1);

                    break;
                }
            }
        }

        // FfH: Added by Kael 09/26/2008 (challenge options). Xienwolf Tweak 12/13/08 (merged by
        // Sephi): the escalation turn counts are scaled by the game speed's growth percent.
        let growth_percent = self.d.game_speeds[self.game_speed_type()].growth_percent;

        if self.is_option(GameOptionType::GAMEOPTION_CHALLENGE_CUT_LOSERS) {
            if self.count_civ_players_alive() > 5 {
                // changeCutLosersCounter(-1)
                self.cut_losers_counter -= 1;
                if self.cut_losers_counter == 0 {
                    // GET_PLAYER(getRankPlayer(countCivPlayersAlive() - 1)).setAlive(false): an
                    // empty C++ slot is not alive (see `cpp_rank_player`), so that is a no-op.
                    let rank = self.count_civ_players_alive() - 1;
                    if let Some(loser) = self.cpp_rank_player(rank) {
                        self.player_set_alive(loser, false);
                    }
                    // changeCutLosersCounter(50 * growth percent / 100)
                    self.cut_losers_counter += 50 * growth_percent / 100;
                }
            }
        }

        if self.is_option(GameOptionType::GAMEOPTION_CHALLENGE_HIGH_TO_LOW) {
            if !self.is_game_multi_player() {
                if self.game_turn() >= 50 * growth_percent / 100 {
                    if self.high_to_low_counter < 2 {
                        for p in self.player_ids() {
                            if self.players[p].alive {
                                if self.players[p].human {
                                    if self.player_rank[p] == 0 {
                                        // GC.getInitCore().reassignPlayerAdvanced(p,
                                        // getRankPlayer(countCivPlayersAlive() - 1), -1). Where
                                        // C++ names an empty slot (see `cpp_rank_player`) the
                                        // human would move to that slot; this game has no
                                        // such slot, so nothing is reassigned.
                                        let rank = self.count_civ_players_alive() - 1;
                                        if let Some(target) = self.cpp_rank_player(rank) {
                                            reassign_player_advanced(self, p, target, -1);
                                        }
                                        // changeHighToLowCounter(1)
                                        self.high_to_low_counter += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let num_handicaps = self.d.handicaps.len() as i32;

        if self.is_option(GameOptionType::GAMEOPTION_CHALLENGE_INCREASING_DIFFICULTY) {
            // changeIncreasingDifficultyCounter(1)
            self.increasing_difficulty_counter += 1;

            if self.increasing_difficulty_counter >= 50 * growth_percent / 100 {
                if self.handicap.i() < num_handicaps - 1 {
                    for p in self.player_ids() {
                        if self.players[p].alive {
                            if self.players[p].human {
                                // GC.getInitCore().setHandicap(p, getHandicapType() + 1)
                                self.setup.players[p.idx()].handicap = Some(HandicapType(self.handicap.0 + 1));
                            }
                        }
                    }
                    // setHandicapType(getHandicapType() + 1)
                    self.handicap = HandicapType(self.handicap.0 + 1);
                    // changeIncreasingDifficultyCounter(getIncreasingDifficultyCounter() * -1)
                    let counter = self.increasing_difficulty_counter;
                    self.increasing_difficulty_counter += counter * -1;
                }
            }
        }

        if self.is_option(GameOptionType::GAMEOPTION_FLEXIBLE_DIFFICULTY) {
            if !self.is_game_multi_player() {
                // changeFlexibleDifficultyCounter(1)
                self.flexible_difficulty_counter += 1;

                if self.flexible_difficulty_counter >= 20 * growth_percent / 100 {
                    for p in self.player_ids() {
                        if self.players[p].alive {
                            if self.players[p].human {
                                if self.player_rank[p] <= self.count_civ_players_alive() / 3 && self.handicap.i() < num_handicaps - 1 {
                                    // GC.getInitCore().setHandicap(p, getHandicapType() + 1)
                                    self.setup.players[p.idx()].handicap = Some(HandicapType(self.handicap.0 + 1));
                                    self.handicap = HandicapType(self.handicap.0 + 1);
                                    let counter = self.flexible_difficulty_counter;
                                    self.flexible_difficulty_counter += counter * -1;
                                }
                                if self.player_rank[p] > self.count_civ_players_alive() * 2 / 3 && self.handicap.0 > 0 {
                                    // GC.getInitCore().setHandicap(p, getHandicapType() - 1)
                                    self.setup.players[p.idx()].handicap = Some(HandicapType(self.handicap.0 - 1));
                                    self.handicap = HandicapType(self.handicap.0 - 1);
                                    let counter = self.flexible_difficulty_counter;
                                    self.flexible_difficulty_counter += counter * -1;
                                }
                            }
                        }
                    }
                }
            }
        }
        // FfH: End Add

        self.test_victory();

        // Globe texture dirty bit, engine DoTurn, DLL profiling and AutoSave: not ported.
    }

    /// C++ `CvGame::updateMoves`: for each alive player with an active turn, unit updates
    /// (`AI_unitUpdate` -> the player's controller), auto moves (`autoMission` of every group),
    /// and the end of the player's turn once no unit is busy (`setAutoMoves(false)`). FfH's
    /// end-of-turn AI spell casting (Sephi BETTER AI) is left to the controller.
    pub fn update_moves(&mut self) {
        // MPOPTION_SIMULTANEOUS_TURNS (players visited in a soren-rand shuffled order) is never
        // set: players are visited in slot order. A player whose turn ends here activates the
        // next alive player, who is then visited in the same pass, as in C++.
        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].turn_active {
                    if !self.players[p].auto_moves {
                        // player.AI_unitUpdate(): the controller plays all of the player's units.
                        let c = self.controller(p);
                        c.play_turn(self, p);

                        if !self.players[p].human {
                            if !self.player_has_busy_unit(p) && !self.player_has_ready_unit(p, true) {
                                self.player_set_auto_moves(p, true);
                            }
                        }
                    }

                    if self.players[p].auto_moves {
                        for g in self.players[p].groups.clone() {
                            if self.groups.contains(g) {
                                self.group_auto_mission(g);
                            }
                        }

                        if !self.player_has_busy_unit(p) {
                            // AI: BETTER AI (Spellcasting end of turn, Sephi) -- for non-human
                            // players: AI_setSummonSuicideMode(true); every unit casts the spell
                            // chosen by chooseSpell(), then AI_BuffCast() / AI_SummonCast();
                            // AI_setSummonSuicideMode(false). The controller casts its own
                            // end-of-turn spells inside play_turn.

                            self.player_set_auto_moves(p, false);
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::updateWar` (GAMEOPTION_ALWAYS_WAR: humans declare war on met AI teams).
    pub fn update_war(&mut self) {
        if self.is_option(GameOptionType::GAMEOPTION_ALWAYS_WAR) {
            for t1 in self.team_ids() {
                if self.team_is_alive(t1) && self.team_is_human(t1) {
                    for t2 in self.team_ids() {
                        if self.team_is_alive(t2) && !self.team_is_human(t2) {
                            debug_assert!(t1 != t2);

                            if self.teams[t1].has_met[t2] {
                                if !self.teams[t1].at_war[t2] {
                                    self.team_declare_war(t1, t2, false, None);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::updateTimers` (`CvPlayer::updateTimers` for alive players).
    pub fn update_timers(&mut self) {
        for p in self.player_ids() {
            if self.players[p].alive {
                self.player_update_timers(p);
            }
        }
    }

    /// C++ `CvGame::testAlive` (`CvPlayer::verifyAlive` for every player).
    pub fn test_alive(&mut self) {
        for p in self.player_ids() {
            self.player_verify_alive(p);
        }
    }

    /// C++ `CvGame::verifyCivics` (`CvPlayer::verifyCivics` for alive players).
    pub fn verify_civics(&mut self) {
        for p in self.player_ids() {
            if self.players[p].alive {
                self.player_verify_civics(p);
            }
        }
    }

    /// C++ `CvGameAI::AI_updateAssignWork`: citizen governor update (only cities flagged dirty)
    /// for the alive players of human teams.
    fn ai_update_assign_work(&mut self) {
        for p in self.player_ids() {
            let team = self.players[p].team;
            if self.team_is_human(team) && self.players[p].alive {
                self.player_ai_update_assign_work(p);
            }
        }
    }

    /// C++ `CvGame::setGameTurn(iNewValue)`: on change, building commerce is recomputed and the
    /// score marked dirty.
    pub fn set_game_turn(&mut self, new_value: i32) {
        if self.game_turn() != new_value {
            self.setup.game_turn = new_value;
            debug_assert!(self.game_turn() >= 0);

            self.update_building_commerce();

            self.score_dirty = true;
        }
    }

    /// C++ `CvGame::incrementGameTurn`.
    pub fn increment_game_turn(&mut self) {
        self.set_game_turn(self.game_turn() + 1);
    }

    /// C++ `CvGame::getTurnYear(iGameTurn)`.
    pub fn turn_year(&self, game_turn: i32) -> i32 {
        get_turn_year_for_game(self.d, game_turn, self.start_year, self.calendar(), self.game_speed_type())
    }

    /// C++ `CvGame::getGameTurnYear`.
    pub fn game_turn_year(&self) -> i32 {
        self.turn_year(self.game_turn())
    }

    /// C++ `CvGame::setAIAutoPlay(iNewValue)` (FfH: toggles `setDisableHuman` on the active
    /// player instead of killing it).
    pub fn set_ai_auto_play(&mut self, new_value: i32) {
        let old_value = self.ai_auto_play;

        if old_value != new_value {
            self.ai_auto_play = std::cmp::max(0, new_value);

            // FfH: Modified by Kael 05/29/2008 (for jdog5000's AIAutoPlay)
            // GET_PLAYER(getActivePlayer()).setDisableHuman(getAIAutoPlay() != 0). Headless there
            // is no active player (C++ would index GET_PLAYER(NO_PLAYER)).
            if let Some(active) = self.active_player {
                let disable = self.ai_auto_play != 0;
                self.player_set_disable_human(active, disable);
            }
        }
    }

    /// C++ `CvGame::changeAIAutoPlay(iChange)`.
    pub fn change_ai_auto_play(&mut self, change: i32) {
        self.set_ai_auto_play(self.ai_auto_play + change);

        // FfH: Added by Kael 05/29/2008 (for jdog5000's AIAutoPlay)
        if let Some(active) = self.active_player {
            let disable = self.ai_auto_play != 0;
            self.player_set_disable_human(active, disable);
        }
    }

    // ---------------------------------------------------------------------------------------
    // World updates
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::updatePlotGroups`.
    pub fn update_plot_groups(&mut self) {
        for p in self.player_ids() {
            if self.players[p].alive {
                self.player_update_plot_groups(p);
            }
        }
    }

    /// C++ `CvGame::updateBuildingCommerce`.
    pub fn update_building_commerce(&mut self) {
        for p in self.player_ids() {
            if self.players[p].alive {
                self.player_update_building_commerce(p);
            }
        }
    }

    /// C++ `CvGame::updateCitySight(bIncrement)` (then `updatePlotGroups`).
    pub fn update_city_sight(&mut self, increment: bool) {
        for p in self.player_ids() {
            if self.players[p].alive {
                self.player_update_city_sight(p, increment, false);
            }
        }

        self.update_plot_groups();
    }

    /// C++ `CvGame::updateTradeRoutes`.
    pub fn update_trade_routes(&mut self) {
        for p in self.player_ids() {
            if self.players[p].alive {
                self.player_update_trade_routes(p);
            }
        }
    }

    /// C++ `CvGame::doGlobalWarming`: forests defend; building bad health and nukes warm;
    /// random land plots lose their feature or turn into `GLOBAL_WARMING_TERRAIN`.
    pub fn do_global_warming(&mut self) {
        let d = self.d;

        let mut global_warming_defense = 0;
        for plot in self.map.plot_ids() {
            if !self.plot_is_water(plot) {
                if let Some(feature) = self.map.plots[plot].feature_type {
                    // hack, but we don't want to add new XML field in the patch just for this
                    if d.features[feature].growth_probability > 0 {
                        global_warming_defense += 1;
                    }
                }
            }
        }
        global_warming_defense = global_warming_defense * d.defines.global_warming_forest / std::cmp::max(1, self.map.land_plots);

        let unhealth_weight = d.defines.global_warming_unhealth_weight;
        let mut global_warming_value = 0;
        for p in self.player_ids() {
            if self.players[p].alive {
                for &c in &self.players[p].cities {
                    // getBuildingBadHealth() is <= 0
                    global_warming_value -= self.cities[c].building_bad_health * unhealth_weight;
                }
            }
        }
        // C++ divides by numPlotsINLINE() unchecked (a map always has plots).
        global_warming_value /= self.map.num_plots() as i32;

        global_warming_value += self.nukes_exploded * d.defines.global_warming_nuke_weight / 100;

        // (TerrainTypes)GC.getDefineINT("GLOBAL_WARMING_TERRAIN"): TERRAIN_DESERT in FFH2.
        let warming_terrain = opt_from_i32::<TerrainType>(d.defines.global_warming_terrain);
        let nuke_feature = d.defines.nuke_feature;

        for _ in 0..global_warming_value {
            if self.soren_rand_num(100, "Global Warming") + global_warming_defense < d.defines.global_warming_prob {
                let Some(plot) = self.map_sync_rand_plot(RANDPLOT_LAND | RANDPLOT_NOT_CITY, None, -1, 100) else {
                    continue;
                };

                let mut changed = false;

                if let Some(feature) = self.map.plots[plot].feature_type {
                    if feature.i() != nuke_feature {
                        self.plot_set_feature_type(plot, None);
                        changed = true;
                    }
                } else if self.map.plots[plot].terrain_type != warming_terrain {
                    if self.plot_calculate_total_best_nature_yield(plot, None) > 1 {
                        // (C++ would set NO_TERRAIN if the define were missing; FFH2 defines it.)
                        if let Some(terrain) = warming_terrain {
                            self.plot_set_terrain_type(plot, terrain, true);
                            changed = true;
                        }
                    }
                }

                if changed {
                    self.plot_set_improvement_type(plot, None);

                    // TXT_KEY_MISC_GLOBAL_WARMING_NEAR_CITY to the owner of the nearest city when
                    // the plot is visible to its team: a message only (CvMap::findCity has no side
                    // effects), so it is looked up only when the log is on.
                    if self.log.enabled {
                        let (x, y) = self.map.xy(plot);
                        if let Some(city) = self.map_find_city(x, y, None, None, true, false, None, None, None) {
                            if self.plot_is_visible(plot, self.city_team(city)) {
                                let owner = self.cities[city].owner;
                                let name = self.cities[city].name.clone();
                                let turn = self.game_turn();
                                self.log.msg(turn, Some(owner), || format!("Global warming has struck near {}!", name));
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::changeTradeRoutes(iChange)`.
    pub fn change_trade_routes(&mut self, change: i32) {
        if change != 0 {
            self.trade_routes += change;
            debug_assert!(self.trade_routes >= 0);

            self.update_trade_routes();
        }
    }

    /// C++ `CvGame::isFreeTrade`.
    #[inline]
    pub fn is_free_trade(&self) -> bool {
        self.free_trade_count > 0
    }

    /// C++ `CvGame::changeFreeTradeCount(iChange)`.
    pub fn change_free_trade_count(&mut self, change: i32) {
        if change != 0 {
            let old_free_trade = self.is_free_trade();

            self.free_trade_count += change;
            debug_assert!(self.free_trade_count >= 0);

            if old_free_trade != self.is_free_trade() {
                self.update_trade_routes();
            }
        }
    }

    /// C++ `CvGame::isNoNukes`.
    #[inline]
    pub fn is_no_nukes(&self) -> bool {
        self.no_nukes_count > 0
    }

    /// C++ `CvGame::circumnavigationAvailable`.
    pub fn circumnavigation_available(&self) -> bool {
        if self.circumnavigated {
            return false;
        }

        if self.d.defines.circumnavigate_free_moves == 0 {
            return false;
        }

        let map = &self.map;

        if !map.wrap_x && !map.wrap_y {
            return false;
        }

        if map.land_plots > ((map.num_plots() as i32 * 2) / 3) {
            return false;
        }

        true
    }

    // ---------------------------------------------------------------------------------------
    // Deals
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::implementDeal(eWho, eOtherWho, pOurList, pTheirList, bForce)`.
    pub fn implement_deal(&mut self, who: PlayerId, other_who: PlayerId, our_list: &[TradeData], their_list: &[TradeData], force: bool) {
        debug_assert!(who != other_who, "eWho is not expected to be equal with eOtherWho");

        // addDeal(), then pDeal->init(pDeal->getID(), eWho, eOtherWho)
        let deal = self.deals.insert_with(|id| Deal::new(id, who, other_who));
        self.deal_init(deal, who, other_who);
        self.deal_add_trades(deal, our_list, their_list, !force);
        if self.deals.contains(deal) && self.deals[deal].first_trades.is_empty() && self.deals[deal].second_trades.is_empty() {
            self.deal_kill(deal, true);
        }
    }

    /// C++ `CvGame::verifyDeals`.
    pub fn verify_deals(&mut self) {
        for deal in self.deals.ids() {
            // verify() can cancel (delete) this and other deals
            if self.deals.contains(deal) {
                self.deal_verify(deal);
            }
        }
    }

    /// C++ `CvGame::doDeals` (`verifyDeals`, then `CvDeal::doTurn` for each deal).
    pub fn do_deals(&mut self) {
        self.verify_deals();

        for deal in self.deals.ids() {
            if self.deals.contains(deal) {
                self.deal_do_turn(deal);
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Global queries and counters
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::countCivPlayersAlive`.
    pub fn count_civ_players_alive(&self) -> i32 {
        self.civ_player_ids().filter(|&p| self.players[p].alive).count() as i32
    }

    /// C++ `CvGame::countCivPlayersEverAlive` (players without a parent).
    pub fn count_civ_players_ever_alive(&self) -> i32 {
        self.civ_player_ids()
            .filter(|&p| {
                let pl = &self.players[p];
                pl.ever_alive && pl.parent.is_none()
            })
            .count() as i32
    }

    /// C++ `CvGame::countCivTeamsAlive`.
    pub fn count_civ_teams_alive(&self) -> i32 {
        self.civ_team_ids().filter(|&t| self.team_is_alive(t)).count() as i32
    }

    /// C++ `CvGame::countCivTeamsEverAlive`: teams of the civ players ever alive without a
    /// parent.
    pub fn count_civ_teams_ever_alive(&self) -> i32 {
        let mut teams_ever_alive = BTreeSet::new();

        for p in self.civ_player_ids() {
            let pl = &self.players[p];
            if pl.ever_alive {
                if pl.parent.is_none() {
                    teams_ever_alive.insert(pl.team);
                }
            }
        }

        teams_ever_alive.len() as i32
    }

    /// C++ `CvGame::countHumanPlayersAlive`.
    pub fn count_human_players_alive(&self) -> i32 {
        self.civ_player_ids()
            .filter(|&p| {
                let pl = &self.players[p];
                pl.alive && pl.human
            })
            .count() as i32
    }

    /// C++ `CvGame::countTotalCivPower`.
    pub fn count_total_civ_power(&self) -> i32 {
        let mut count = 0;

        for p in self.civ_player_ids() {
            let pl = &self.players[p];
            if pl.alive {
                count += pl.power;
            }
        }

        count
    }

    /// C++ `CvGame::countTotalNukeUnits`.
    pub fn count_total_nuke_units(&self) -> i32 {
        let mut count = 0;

        for p in self.civ_player_ids() {
            let pl = &self.players[p];
            if pl.alive {
                count += pl.num_nuke_units;
            }
        }

        count
    }

    /// C++ `CvGame::countKnownTechNumTeams(eTech)` (all teams, barbarians included).
    pub fn count_known_tech_num_teams(&self, tech: TechType) -> i32 {
        self.team_ids()
            .filter(|&t| self.team_is_ever_alive(t) && self.team_is_has_tech(t, Some(tech)))
            .count() as i32
    }

    /// C++ `CvGame::countNumHumanGameTurnActive`.
    pub fn count_num_human_game_turn_active(&self) -> i32 {
        self.civ_player_ids()
            .filter(|&p| {
                let pl = &self.players[p];
                pl.human && pl.turn_active
            })
            .count() as i32
    }

    /// C++ `CvGame::getNumCivCities` (all cities minus the barbarians').
    pub fn num_civ_cities(&self) -> i32 {
        self.num_cities - self.players[self.barbarian_player()].num_cities()
    }

    /// C++ `CvGame::isTeamGame` (more civ players than civ teams alive).
    pub fn is_team_game(&self) -> bool {
        debug_assert!(self.count_civ_players_alive() >= self.count_civ_teams_alive());
        self.count_civ_players_alive() > self.count_civ_teams_alive()
    }

    /// C++ `CvGame::isInAdvancedStart`: a human player still has advanced start points (>= 0).
    pub fn is_in_advanced_start(&self) -> bool {
        self.player_ids().any(|p| {
            let pl = &self.players[p];
            pl.advanced_start_points >= 0 && pl.human
        })
    }

    /// C++ `CvGame::getCurrentEra`: average era of the alive players (`None` if none alive).
    pub fn current_era(&self) -> Option<EraType> {
        let mut era = 0;
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                era += self.players[p].current_era.i();
                count += 1;
            }
        }

        if count > 0 {
            return Some(EraType::from_usize((era / count) as usize));
        }

        None
    }

    /// C++ `CvGame::getProductionPerPopulation(eHurry)`.
    pub fn production_per_population(&self, hurry: Option<HurryType>) -> i32 {
        let Some(hurry) = hurry else {
            return 0;
        };
        (self.d.hurries[hurry].production_per_population * 100) / std::cmp::max(1, self.d.game_speeds[self.game_speed_type()].hurry_percent)
    }

    /// C++ `CvGame::goldenAgeLength` (`GOLDEN_AGE_LENGTH` scaled by the speed).
    pub fn golden_age_length(&self) -> i32 {
        let mut length = self.d.defines.golden_age_length;

        length *= self.d.game_speeds[self.game_speed_type()].golden_age_percent;
        length /= 100;

        length
    }

    /// C++ `CvGame::getBestLandUnitCombat` (at least 1).
    pub fn best_land_unit_combat(&self) -> i32 {
        match self.best_land_unit {
            None => 1,
            Some(unit) => std::cmp::max(1, self.d.units[unit].combat),
        }
    }

    /// C++ `CvGame::calculateSyncChecksum` (MP desync check; useful for determinism tests).
    /// C++ `int` arithmetic overflows here by design; MSVC wraps, so every operation wraps.
    pub fn calculate_sync_checksum(&self) -> i32 {
        let mut value: i32 = 0;

        // getSeed() is an unsigned long added to an int: two's complement wrap.
        value = value.wrapping_add(self.map_rand.seed() as i32);
        value = value.wrapping_add(self.soren_rand.seed() as i32);

        value = value.wrapping_add(self.num_cities);
        value = value.wrapping_add(self.total_population);
        value = value.wrapping_add(self.deals.len() as i32);

        value = value.wrapping_add(self.map.owned_plots);
        value = value.wrapping_add(self.map.areas.len() as i32);

        for p in self.player_ids() {
            let pl = &self.players[p];
            if pl.ever_alive {
                let mut multiplier = self.player_score[p];

                match self.turn_slice % 4 {
                    0 => {
                        multiplier = checksum_term(multiplier, pl.total_population, 543271);
                        multiplier = checksum_term(multiplier, pl.total_land, 327382);
                        multiplier = checksum_term(multiplier, pl.gold, 107564);
                        multiplier = checksum_term(multiplier, pl.assets, 327455);
                        multiplier = checksum_term(multiplier, pl.power, 135647);
                        multiplier = checksum_term(multiplier, pl.num_cities(), 436432);
                        multiplier = checksum_term(multiplier, pl.num_units(), 324111);
                        multiplier = checksum_term(multiplier, pl.groups.len() as i32, 215356);
                    }

                    1 => {
                        for y in 0..YieldType::NUM {
                            let total = self.player_calculate_total_yield(p, YieldType::from_usize(y));
                            multiplier = checksum_term(multiplier, total, 432754);
                        }

                        for c in 0..CommerceType::NUM {
                            let rate = self.player_get_commerce_rate(p, CommerceType::from_usize(c));
                            multiplier = checksum_term(multiplier, rate, 432789);
                        }
                    }

                    2 => {
                        for b in self.d.bonuses.keys() {
                            multiplier = checksum_term(multiplier, self.player_get_num_available_bonuses(p, b), 945732);
                            multiplier = checksum_term(multiplier, pl.bonus_import[b], 326443);
                            multiplier = checksum_term(multiplier, pl.bonus_export[b], 932211);
                        }

                        for i in self.d.improvements.keys() {
                            multiplier = checksum_term(multiplier, pl.improvement_count[i], 883422);
                        }

                        for bc in self.d.building_classes.keys() {
                            let count = self.player_get_building_class_count_plus_making(p, bc);
                            multiplier = checksum_term(multiplier, count, 954531);
                        }

                        for uc in self.d.unit_classes.keys() {
                            let count = self.player_get_unit_class_count_plus_making(p, uc);
                            multiplier = checksum_term(multiplier, count, 754843);
                        }

                        for ai in 0..UnitAIType::NUM {
                            // CvPlayerAI::AI_totalUnitAIs(eUnitAI): units in training + existing units
                            let unit_ai = UnitAIType::from_usize(ai);
                            let total = pl.ai_num_train_ai_units[unit_ai] + pl.ai_num_ai_units[unit_ai];
                            multiplier = checksum_term(multiplier, total, 643383);
                        }
                    }

                    3 => {
                        for &u in &pl.units {
                            let unit = &self.units[u];
                            multiplier = checksum_term(multiplier, unit.x, 876543);
                            multiplier = checksum_term(multiplier, unit.y, 985310);
                            multiplier = checksum_term(multiplier, unit.damage, 736373);
                            multiplier = checksum_term(multiplier, unit.experience, 820622);
                            multiplier = checksum_term(multiplier, unit.level, 367291);
                        }
                    }

                    _ => {}
                }

                if multiplier != 0 {
                    value = value.wrapping_mul(multiplier);
                }
            }
        }

        value
    }

    // ---------------------------------------------------------------------------------------
    // Score and victory
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::updateScore(bForce)`: when dirty (or forced) ranks civ players and teams by
    /// score (ties go to the higher index: `>=`) and records score history.
    ///
    /// C++ ranks all `MAX_CIV_PLAYERS` (35) civ slots; this game has `num_civ_players()` of
    /// them, the rest would be empty slots with score 0. Ranking only this game's slots gives
    /// the players with a positive score (every alive player) the same ranks; zero-score
    /// players (dead or never alive) rank right after them instead of after the empty slots.
    /// `cpp_rank_player` translates rank positions to the C++ ones where the challenge options
    /// read them.
    pub fn update_score(&mut self, force: bool) {
        if !self.score_dirty && !force {
            return;
        }

        self.score_dirty = false;

        let turn = self.game_turn();
        let num_civ_players = self.num_civ_players();

        // calculateScore(false) only reads game state, so each player's score is computed
        // once (C++ recomputes it in every pass of the selection sort, with the same result).
        let scores: Vec<i32> = self.civ_player_ids().map(|p| self.player_calculate_score(p, false, false)).collect();
        let mut player_scored = vec![false; num_civ_players];

        for rank in 0..num_civ_players {
            let mut best_score = MIN_INT;
            let mut best_player: Option<PlayerId> = None;

            for j in 0..num_civ_players {
                if !player_scored[j] {
                    let score = scores[j];

                    if score >= best_score {
                        best_score = score;
                        best_player = Some(PlayerId(j as u8));
                    }
                }
            }

            // An unscored slot is left in every pass, and any score is >= MIN_INT.
            let best = best_player.expect("updateScore: an unscored civ player");
            player_scored[best.idx()] = true;

            self.rank_player[rank] = best;
            self.player_rank[best] = rank as i32;
            self.player_score[best] = best_score;
            set_turn_history(&mut self.players[best].score_history, turn, best_score);
        }

        let num_civ_teams = self.num_civ_teams();
        let mut team_scored = vec![false; num_civ_teams];

        for rank in 0..num_civ_teams {
            let mut best_score = MIN_INT;
            let mut best_team: Option<TeamId> = None;

            for j in 0..num_civ_teams {
                if !team_scored[j] {
                    let team = TeamId(j as u8);
                    let mut score = 0;

                    for k in self.civ_player_ids() {
                        if self.players[k].team == team {
                            score += self.player_score[k];
                        }
                    }

                    if score >= best_score {
                        best_score = score;
                        best_team = Some(team);
                    }
                }
            }

            let best = best_team.expect("updateScore: an unscored civ team");
            team_scored[best.idx()] = true;

            self.rank_team[rank] = best;
            self.team_rank[best] = rank as i32;
            self.team_score[best] = best_score;
        }
    }

    /// C++ `getRankPlayer(iRank)` as the C++ game sees it. C++ `updateScore` ranks all
    /// `MAX_CIV_PLAYERS` civ slots: the slots this game does not have are empty, score 0, and
    /// (ties going to the higher index) rank right after the players with a positive score,
    /// ahead of this game's zero-score players. Returns `None` where C++ names one of those
    /// empty slots, else the player at that C++ rank (see `update_score`).
    fn cpp_rank_player(&self, rank: i32) -> Option<PlayerId> {
        debug_assert!(rank >= 0, "iRank is expected to be non-negative (invalid Rank)");
        if rank < 0 {
            return None;
        }
        let rank = rank as usize;
        let num_scored = self.civ_player_ids().filter(|&p| self.player_score[p] > 0).count();
        let num_missing = MAX_CIV_PLAYERS.saturating_sub(self.num_civ_players());

        if rank < num_scored {
            Some(self.rank_player[rank])
        } else if rank < num_scored + num_missing {
            None
        } else {
            self.rank_player.get(rank - num_missing).copied()
        }
    }

    /// C++ `CvGame::testExtendedGame`: GAMESTATE_OVER -> EXTENDED when a human chose to continue.
    pub fn test_extended_game(&mut self) {
        if self.game_state != GameStateType::GAMESTATE_OVER {
            return;
        }

        for p in self.civ_player_ids() {
            let pl = &self.players[p];
            if pl.alive {
                if pl.human {
                    if pl.extended_game {
                        self.set_game_state(GameStateType::GAMESTATE_EXTENDED);
                        break;
                    }
                }
            }
        }
    }

    /// C++ `CvGame::setWinner(eNewWinner, eNewVictory)`. A victory ends the game
    /// (`GAMESTATE_EXTENDED` under AI autoplay or autorun).
    pub fn set_winner(&mut self, new_winner: Option<TeamId>, new_victory: Option<VictoryType>) {
        if self.winner != new_winner || self.victory != new_victory {
            self.winner = new_winner;
            self.victory = new_victory;

            if self.victory.is_some() {
                // (replay message "TXT_KEY_GAME_WON" omitted)
                if self.ai_auto_play > 0 || self.autorun {
                    self.set_game_state(GameStateType::GAMESTATE_EXTENDED);
                } else {
                    self.set_game_state(GameStateType::GAMESTATE_OVER);
                }
            }

            // CvEventReporter::getInstance().victory(eNewWinner, eNewVictory). C++ also reports
            // NO_TEAM / NO_VICTORY; FFH2's onVictory ignores those (iVictory >= 0, team match).
            if let (Some(w), Some(v)) = (new_winner, new_victory) {
                self.ev_victory(w, v);
            }
        }
    }

    /// C++ `CvGame::setGameState(eNewValue)`. (End-game popups and screens are UI.)
    pub fn set_game_state(&mut self, new_value: GameStateType) {
        if self.game_state != new_value {
            self.game_state = new_value;

            if new_value == GameStateType::GAMESTATE_OVER {
                self.ev_game_end();
                // showEndGameSequence() and the "one more turn?" popups are UI.
            }
        }
    }

    /// C++ `CvGame::testVictory(eVictory, eTeam, pbEndScore)`: `(valid, end_score)`.
    pub fn test_victory_team(&self, victory: VictoryType, team: TeamId) -> (bool, bool) {
        debug_assert!(team.idx() < self.num_civ_teams());
        debug_assert!(self.team_is_alive(team));

        let d = self.d;
        let info = &d.victories[victory];

        let mut valid = self.is_victory_valid(victory);
        let mut end_score = false;

        if valid {
            if info.end_score {
                end_score = true;

                if self.max_turns() == 0 {
                    valid = false;
                } else if self.elapsed_game_turns < self.max_turns() {
                    valid = false;
                } else {
                    let mut found = false;

                    for k in self.civ_team_ids() {
                        if self.team_is_alive(k) {
                            if k != team {
                                if self.team_score[k] >= self.team_score[team] {
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }

                    if found {
                        valid = false;
                    }
                }
            }
        }

        if valid {
            if info.target_score {
                if self.target_score() == 0 {
                    valid = false;
                } else if self.team_score[team] < self.target_score() {
                    valid = false;
                } else {
                    let mut found = false;

                    for k in self.civ_team_ids() {
                        if self.team_is_alive(k) {
                            if k != team {
                                if self.team_score[k] >= self.team_score[team] {
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }

                    if found {
                        valid = false;
                    }
                }
            }
        }

        if valid {
            if info.conquest {
                if self.teams[team].num_cities == 0 {
                    valid = false;
                } else {
                    let mut found = false;

                    for k in self.civ_team_ids() {
                        if self.team_is_alive(k) {
                            if k != team && !self.teams[k].vassal[team] {
                                if self.teams[k].num_cities > 0 {
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }

                    if found {
                        valid = false;
                    }
                }
            }
        }

        if valid {
            if info.diplo_vote {
                let mut found = false;

                for vote in d.votes.keys() {
                    if d.votes[vote].victory {
                        // Team votes store the elected team as a PlayerVoteTypes value.
                        if self.vote_outcome[vote].0 == team.i() {
                            found = true;
                            break;
                        }
                    }
                }

                if !found {
                    valid = false;
                }
            }
        }

        if valid {
            if self.adjusted_population_percent(victory) > 0 {
                if 100 * self.team_get_total_population(team, true) < self.total_population * self.adjusted_population_percent(victory) {
                    valid = false;
                }
            }
        }

        if valid {
            if self.adjusted_land_percent(victory) > 0 {
                if 100 * self.team_get_total_land(team, true) < self.map.land_plots * self.adjusted_land_percent(victory) {
                    valid = false;
                }
            }
        }

        if valid {
            if info.religion_percent > 0 {
                let mut found = false;

                if self.num_civ_cities() > (self.count_civ_players_alive() * 2) {
                    for religion in d.religions.keys() {
                        if self.team_has_holy_city(team, religion) {
                            if self.calculate_religion_percent(religion) >= info.religion_percent {
                                found = true;
                                break;
                            }
                        }
                    }
                }

                if !found {
                    valid = false;
                }
            }
        }

        if valid {
            if info.city_culture.is_some() && info.num_culture_cities > 0 {
                let mut count = 0;

                for p in self.civ_player_ids() {
                    let pl = &self.players[p];
                    if pl.alive {
                        if pl.team == team {
                            for &c in &pl.cities {
                                // CultureLevelTypes compare as ints (NO_CULTURELEVEL = None is lowest).
                                if self.cities[c].culture_level >= info.city_culture {
                                    count += 1;
                                }
                            }
                        }
                    }
                }

                if count < info.num_culture_cities {
                    valid = false;
                }
            }
        }

        if valid {
            if info.total_culture_ratio > 0 {
                // C++ int arithmetic (wraps on MSVC for huge culture totals).
                let threshold = self.team_count_total_culture(team).wrapping_mul(100) / info.total_culture_ratio;

                let mut found = false;

                for k in self.civ_team_ids() {
                    if self.team_is_alive(k) {
                        if k != team {
                            if self.team_count_total_culture(k) > threshold {
                                found = true;
                                break;
                            }
                        }
                    }
                }

                if found {
                    valid = false;
                }
            }
        }

        if valid {
            for bc in d.building_classes.keys() {
                if d.building_classes[bc].victory_threshold[victory] > self.teams[team].building_class_count[bc] {
                    valid = false;
                    break;
                }
            }
        }

        if valid {
            for project in d.projects.keys() {
                if d.projects[project].victory_min_threshold[victory] > self.teams[team].project_count[project] {
                    valid = false;
                    break;
                }
            }
        }

        // Python CvGameUtils.isVictory(eVictory) returns True in FFH2: nothing to call.

        (valid, end_score)
    }

    /// C++ `CvGame::testVictory()`: victory countdowns, launch success rolls, tie breaker (soren
    /// rand), max-turns game end.
    pub fn test_victory(&mut self) {
        let mut end_score = false;

        if self.victory.is_some() {
            return;
        }

        if self.game_state == GameStateType::GAMESTATE_EXTENDED {
            return;
        }

        self.update_score(false);

        // Python CvGameUtils.isVictoryTest()
        if !self.py_is_victory_test() {
            return;
        }

        let mut winners: Vec<(TeamId, VictoryType)> = Vec::new();

        for t in self.civ_team_ids() {
            if self.team_is_alive(t) {
                if !self.team_is_minor_civ(t) {
                    for v in self.d.victories.keys() {
                        // C++ quirk: bEndScore is overwritten by every test, so after the loops it
                        // only tells whether the last victory type tested is an end-score victory.
                        let (valid, is_end_score) = self.test_victory_team(v, t);
                        end_score = is_end_score;

                        if valid {
                            if self.teams[t].victory_countdown[v] < 0 {
                                if self.team_get_victory_delay(t, v) == 0 {
                                    self.teams[t].victory_countdown[v] = 0;
                                }
                            }

                            //update victory countdown
                            if self.teams[t].victory_countdown[v] > 0 {
                                // changeVictoryCountdown(-1)
                                self.teams[t].victory_countdown[v] -= 1;
                            }

                            if self.teams[t].victory_countdown[v] == 0 {
                                if self.soren_rand_num(100, "Victory Success") < self.team_get_launch_success_rate(t, v) {
                                    winners.push((t, v));
                                } else {
                                    self.team_reset_victory_progress(t);
                                }
                            }
                        }
                    }
                }
            }
        }

        if !winners.is_empty() {
            let winner = self.soren_rand_num(winners.len() as i32, "Victory tie breaker");
            let (team, victory) = winners[winner as usize];
            self.set_winner(Some(team), Some(victory));
        }

        if self.victory.is_none() {
            if self.max_turns() > 0 {
                if self.elapsed_game_turns >= self.max_turns() {
                    if !end_score {
                        if self.ai_auto_play > 0 || self.autorun {
                            self.set_game_state(GameStateType::GAMESTATE_EXTENDED);
                        } else {
                            self.set_game_state(GameStateType::GAMESTATE_OVER);
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::getAdjustedPopulationPercent(eVictory)`.
    pub fn adjusted_population_percent(&self, victory: VictoryType) -> i32 {
        let lead = self.d.victories[victory].population_percent_lead;

        if lead == 0 {
            return 0;
        }

        if self.total_population == 0 {
            return 100;
        }

        let mut best_population = 0;
        let mut next_best_population = 0;

        for t in self.civ_team_ids() {
            if self.team_is_alive(t) {
                let population = self.team_get_total_population(t, true);

                if population > best_population {
                    next_best_population = best_population;
                    best_population = population;
                } else if population > next_best_population {
                    next_best_population = population;
                }
            }
        }

        std::cmp::min(100, ((next_best_population * 100) / self.total_population) + lead)
    }

    /// C++ `CvGame::getAdjustedLandPercent(eVictory)`.
    pub fn adjusted_land_percent(&self, victory: VictoryType) -> i32 {
        let info = &self.d.victories[victory];

        if info.land_percent == 0 {
            return 0;
        }

        let mut percent = info.land_percent;

        percent -= self.count_civ_teams_ever_alive() * 2;

        std::cmp::max(percent, info.min_land_percent)
    }

    /// C++ `CvGame::victoryDelay(eVictory)` (scaled by the speed's victory delay percent).
    pub fn victory_delay(&self, victory: VictoryType) -> i32 {
        debug_assert!(victory.idx() < self.d.victories.len());

        let mut length = self.d.victories[victory].victory_delay_turns;

        length *= self.d.game_speeds[self.game_speed_type()].victory_delay_percent;
        length /= 100;

        length
    }
}

/// One `iMultiplier += (value * factor)` step of `calculateSyncChecksum`, with the wrapping
/// `int` arithmetic of the C++ (MSVC).
#[inline]
fn checksum_term(multiplier: i32, value: i32, factor: i32) -> i32 {
    multiplier.wrapping_add(value.wrapping_mul(factor))
}

#[cfg(test)]
mod tests {
    use super::checksum_term;
    use crate::defs::MAX_CIV_PLAYERS;
    use crate::game::Game;
    use crate::ids::*;
    use crate::player::Player;
    use crate::setup::GameSetup;
    use ffh_data::*;

    /// A game with `n` civ slots alive (each on its own team) plus the open spare slots.
    fn game(n: usize) -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, n, "WORLDSIZE_DUEL", 3));
        let (n_players, n_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], n_players, n_teams);
            let t = g.players[p].team;
            g.teams[t].num_members = 1;
        }
        for i in 0..n {
            let p = PlayerId(i as u8);
            g.players[p].alive = true;
            g.players[p].ever_alive = true;
            let t = g.players[p].team;
            g.teams[t].alive_count = 1;
            g.teams[t].ever_alive_count = 1;
        }
        g
    }

    #[test]
    fn trade_counters_and_game_state() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3));
        assert!(!g.is_free_trade());
        assert!(!g.is_no_nukes());
        g.no_nukes_count += 1;
        assert!(g.is_no_nukes());
        // no-op changes do not touch the trade route update
        g.change_free_trade_count(0);
        g.change_trade_routes(0);

        g.set_game_state(GameStateType::GAMESTATE_OVER);
        assert_eq!(g.game_state, GameStateType::GAMESTATE_OVER);
        // a victory under AI autoplay extends the game instead of ending it
        g.game_state = GameStateType::GAMESTATE_ON;
        g.ai_auto_play = 5;
        g.set_winner(Some(crate::ids::TeamId(1)), Some(VictoryType(0)));
        assert_eq!(g.game_state, GameStateType::GAMESTATE_EXTENDED);
        assert_eq!(g.winner, Some(crate::ids::TeamId(1)));
        // setting the same winner again changes nothing
        g.game_state = GameStateType::GAMESTATE_ON;
        g.set_winner(Some(crate::ids::TeamId(1)), Some(VictoryType(0)));
        assert_eq!(g.game_state, GameStateType::GAMESTATE_ON);
        // so does a victory under the exe's autorun flag
        g.ai_auto_play = 0;
        g.autorun = true;
        g.set_winner(Some(crate::ids::TeamId(0)), Some(VictoryType(0)));
        assert_eq!(g.game_state, GameStateType::GAMESTATE_EXTENDED);
        g.autorun = false;
        g.game_state = GameStateType::GAMESTATE_ON;
        g.set_winner(Some(crate::ids::TeamId(1)), Some(VictoryType(0)));
        assert_eq!(g.game_state, GameStateType::GAMESTATE_OVER);
    }

    #[test]
    fn counts_over_civ_players_and_teams() {
        let mut g = game(3);
        let (p0, p1, p2) = (PlayerId(0), PlayerId(1), PlayerId(2));
        let barbarian = g.barbarian_player();
        assert_eq!(g.count_civ_players_alive(), 3);
        assert_eq!(g.count_civ_teams_alive(), 3);
        assert!(!g.is_team_game());

        // the barbarians are not a civ player
        g.players[barbarian].alive = true;
        g.players[barbarian].power = 1000;
        g.players[p0].power = 10;
        g.players[p1].power = 20;
        g.players[p2].power = 40;
        g.players[p2].num_nuke_units = 2;
        assert_eq!(g.count_total_civ_power(), 70);
        assert_eq!(g.count_total_nuke_units(), 2);

        g.players[p1].human = true;
        g.players[p1].turn_active = true;
        g.players[p2].human = true;
        assert_eq!(g.count_human_players_alive(), 2);
        assert_eq!(g.count_num_human_game_turn_active(), 1);

        // a dead player is ever alive; a split-off colony (with a parent) is not counted
        g.players[p2].alive = false;
        g.players[p0].parent = Some(p1);
        assert_eq!(g.count_civ_players_alive(), 2);
        assert_eq!(g.count_human_players_alive(), 1);
        assert_eq!(g.count_total_civ_power(), 30);
        assert_eq!(g.count_total_nuke_units(), 0);
        assert_eq!(g.count_civ_players_ever_alive(), 2);
        assert_eq!(g.count_civ_teams_ever_alive(), 2);

        // two alive players sharing a team make a team game
        g.players[p0].parent = None;
        g.players[p2].alive = true;
        g.players[p2].team = TeamId(1);
        g.teams[TeamId(2)].alive_count = 0;
        assert_eq!(g.count_civ_teams_ever_alive(), 2);
        assert!(g.is_team_game());

        g.num_cities = 5;
        assert_eq!(g.num_civ_cities(), 5);

        assert!(!g.is_in_advanced_start());
        g.players[p0].advanced_start_points = 0;
        assert!(!g.is_in_advanced_start());
        g.players[p1].advanced_start_points = 0;
        assert!(g.is_in_advanced_start());
    }

    #[test]
    fn current_era_and_known_techs() {
        let mut g = game(3);
        g.players[PlayerId(0)].current_era = EraType(1);
        g.players[PlayerId(1)].current_era = EraType(2);
        g.players[PlayerId(2)].current_era = EraType(4);
        assert_eq!(g.current_era(), Some(EraType(2)));
        for p in g.player_ids() {
            g.players[p].alive = false;
        }
        assert_eq!(g.current_era(), None);

        let tech = TechType(3);
        assert_eq!(g.count_known_tech_num_teams(tech), 0);
        g.teams[TeamId(0)].has_tech[tech] = true;
        g.teams[TeamId(2)].has_tech[tech] = true;
        // a team that never lived does not count
        g.teams[TeamId(4)].has_tech[tech] = true;
        assert_eq!(g.count_known_tech_num_teams(tech), 2);
    }

    #[test]
    fn speed_scaled_values() {
        let mut g = game(2);
        let d = g.d;
        let speed = &d.game_speeds[g.game_speed_type()];
        assert_eq!(g.golden_age_length(), d.defines.golden_age_length * speed.golden_age_percent / 100);
        for v in d.victories.keys() {
            assert_eq!(g.victory_delay(v), d.victories[v].victory_delay_turns * speed.victory_delay_percent / 100);
        }
        assert_eq!(g.production_per_population(None), 0);
        for h in d.hurries.keys() {
            let expected = d.hurries[h].production_per_population * 100 / std::cmp::max(1, speed.hurry_percent);
            assert_eq!(g.production_per_population(Some(h)), expected);
        }

        assert_eq!(g.best_land_unit_combat(), 1);
        let warrior = d.t::<UnitType>("UNIT_WARRIOR");
        g.best_land_unit = Some(warrior);
        assert_eq!(g.best_land_unit_combat(), std::cmp::max(1, d.units[warrior].combat));
    }

    #[test]
    fn adjusted_victory_percents() {
        let mut g = game(3);
        let d = g.d;
        let domination = d.t::<VictoryType>("VICTORY_DOMINATION");
        let conquest = d.t::<VictoryType>("VICTORY_CONQUEST");
        let info = &d.victories[domination];
        assert!(info.land_percent > 0 && info.population_percent_lead > 0);

        assert_eq!(g.adjusted_land_percent(conquest), 0);
        assert_eq!(
            g.adjusted_land_percent(domination),
            std::cmp::max(info.land_percent - 3 * 2, info.min_land_percent)
        );

        assert_eq!(g.adjusted_population_percent(conquest), 0);
        // no population at all: 100%
        assert_eq!(g.adjusted_population_percent(domination), 100);
        g.teams[TeamId(0)].total_population = 50;
        g.teams[TeamId(1)].total_population = 30;
        g.teams[TeamId(2)].total_population = 20;
        g.total_population = 100;
        assert_eq!(g.adjusted_population_percent(domination), std::cmp::min(100, 30 + info.population_percent_lead));
    }

    #[test]
    fn victory_conditions() {
        let mut g = game(3);
        let d = g.d;
        let time = d.t::<VictoryType>("VICTORY_TIME");
        let score = d.t::<VictoryType>("VICTORY_SCORE");
        let conquest = d.t::<VictoryType>("VICTORY_CONQUEST");
        let t0 = TeamId(0);

        // end score: no max turns -> invalid, but reported as an end-score victory
        assert_eq!(g.test_victory_team(time, t0), (false, true));
        // target score: no target score
        assert_eq!(g.test_victory_team(score, t0), (false, false));
        // conquest: needs cities, and no other alive non-vassal team with cities
        assert_eq!(g.test_victory_team(conquest, t0), (false, false));
        g.teams[t0].num_cities = 2;
        g.teams[TeamId(1)].num_cities = 1;
        assert_eq!(g.test_victory_team(conquest, t0), (false, false));
        g.teams[TeamId(1)].vassal[t0] = true;
        assert_eq!(g.test_victory_team(conquest, t0), (true, false));
        g.setup.victories[conquest] = false;
        assert_eq!(g.test_victory_team(conquest, t0), (false, false));

        // end score: the best team score after max turns
        g.setup.max_turns = 10;
        g.elapsed_game_turns = 10;
        g.team_score[t0] = 100;
        g.team_score[TeamId(1)] = 100;
        assert_eq!(g.test_victory_team(time, t0), (false, true));
        g.team_score[TeamId(1)] = 99;
        assert_eq!(g.test_victory_team(time, t0), (true, true));
    }

    #[test]
    fn score_ranks_ties_to_the_higher_index() {
        let mut g = game(3);
        let n_civ = g.num_civ_players();
        g.estimate_end_turn = 100;
        g.max_population = 100;
        g.players[PlayerId(0)].population_score = 50;
        g.players[PlayerId(1)].population_score = 10;
        g.players[PlayerId(2)].population_score = 50;

        g.update_score(false);
        assert_eq!(g.player_score[PlayerId(0)], 0, "not dirty: nothing ranked");

        g.score_dirty = true;
        g.update_score(false);
        assert!(!g.score_dirty);
        let s0 = g.player_score[PlayerId(0)];
        let s1 = g.player_score[PlayerId(1)];
        assert!(s0 > s1 && s1 > 0);
        assert_eq!(g.player_score[PlayerId(2)], s0);
        // ties go to the higher index; the open (dead) slots score 0, highest index first
        let mut expected = vec![PlayerId(2), PlayerId(0), PlayerId(1)];
        expected.extend((3..n_civ).rev().map(|i| PlayerId(i as u8)));
        assert_eq!(&g.rank_player[..n_civ], &expected[..]);
        for (rank, &p) in expected.iter().enumerate() {
            assert_eq!(g.player_rank[p], rank as i32);
        }
        assert_eq!(crate::player::turn_history(&g.players[PlayerId(1)].score_history, g.game_turn()), s1);
        // one player per team: the team ranking follows
        assert_eq!(&g.rank_team[..3], &[TeamId(2), TeamId(0), TeamId(1)]);
        assert_eq!(g.team_score[TeamId(1)], s1);

        // C++ ranks 35 civ slots: its empty slots come right after the positive scores
        let missing = MAX_CIV_PLAYERS - n_civ;
        assert_eq!(g.cpp_rank_player(0), Some(PlayerId(2)));
        assert_eq!(g.cpp_rank_player(2), Some(PlayerId(1)));
        assert_eq!(g.cpp_rank_player(3), None);
        assert_eq!(g.cpp_rank_player((3 + missing - 1) as i32), None);
        assert_eq!(g.cpp_rank_player((3 + missing) as i32), Some(PlayerId((n_civ - 1) as u8)));
    }

    #[test]
    fn ai_autoplay_toggles_the_active_player() {
        let mut g = game(2);
        let p = PlayerId(0);
        g.set_ai_auto_play(-3);
        assert_eq!(g.ai_auto_play, 0);
        g.change_ai_auto_play(2);
        assert_eq!(g.ai_auto_play, 2);

        g.setup.players[0].human = true;
        g.player_update_human(p);
        g.active_player = Some(p);
        g.change_ai_auto_play(1);
        assert_eq!(g.ai_auto_play, 3);
        assert!(g.players[p].disable_human && !g.players[p].human);
        g.set_ai_auto_play(0);
        assert!(!g.players[p].disable_human && g.players[p].human);
    }

    #[test]
    fn extended_game_needs_a_human_who_continues() {
        let mut g = game(2);
        let p = PlayerId(1);
        g.players[p].human = true;
        g.players[p].extended_game = true;
        g.test_extended_game();
        assert_eq!(g.game_state, GameStateType::GAMESTATE_ON);

        g.game_state = GameStateType::GAMESTATE_OVER;
        g.players[p].extended_game = false;
        g.test_extended_game();
        assert_eq!(g.game_state, GameStateType::GAMESTATE_OVER);
        g.players[p].extended_game = true;
        g.test_extended_game();
        assert_eq!(g.game_state, GameStateType::GAMESTATE_EXTENDED);
    }

    #[test]
    fn circumnavigation_needs_a_wrapping_water_world() {
        let mut g = game(2);
        assert!(g.d.defines.circumnavigate_free_moves != 0);
        assert!(!g.circumnavigation_available());
        g.map.wrap_x = true;
        assert!(g.circumnavigation_available());
        g.circumnavigated = true;
        assert!(!g.circumnavigation_available());
    }

    #[test]
    fn always_war_is_off_by_default() {
        let mut g = game(2);
        g.teams[TeamId(0)].has_met[TeamId(1)] = true;
        g.update_war();
        assert!(!g.teams[TeamId(0)].at_war[TeamId(1)]);
    }

    #[test]
    fn sync_checksum_wraps_like_msvc() {
        let mut g = game(2);
        for p in g.player_ids() {
            g.players[p].ever_alive = false;
        }
        g.map_rand.reset(0xFFFF_FFF0);
        g.soren_rand.reset(7);
        g.num_cities = 3;
        g.total_population = 11;
        g.map.owned_plots = 5;
        // no player ever alive: the sum of the global terms only
        let base = (0xFFFF_FFF0u32 as i32).wrapping_add(7 + 3 + 11 + 5);
        assert_eq!(g.calculate_sync_checksum(), base);

        // slice 0: population, land, gold, assets, power, cities, units and groups
        let p = PlayerId(1);
        g.players[p].ever_alive = true;
        g.player_score[p] = 9;
        g.players[p].gold = 30_000;
        g.players[p].power = 1;
        let mut m = 9;
        m = checksum_term(m, 30_000, 107564);
        m = checksum_term(m, 1, 135647);
        assert_eq!(g.calculate_sync_checksum(), base.wrapping_mul(m));

        // slice 3: units only (none here), the score is the multiplier
        g.turn_slice = 3;
        assert_eq!(g.calculate_sync_checksum(), base.wrapping_mul(9));
        // a zero multiplier leaves the value unchanged
        g.player_score[p] = 0;
        assert_eq!(g.calculate_sync_checksum(), base);
    }
}
