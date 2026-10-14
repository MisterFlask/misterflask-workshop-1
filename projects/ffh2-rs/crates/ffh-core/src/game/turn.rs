//! CvGame: the update loop and turn processing (`update`, `doTurn`, turn bookkeeping,
//! `updateMoves`, `updateWar`, timers, `testAlive`), world updates (plot groups, building
//! commerce, city sight, trade routes, global warming), deals, global queries/counters,
//! score ranking (`updateScore`) and victory (`testVictory`, `setWinner`, `setGameState`).

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::game::Game;
use crate::ids::*;
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
    /// governor), `testAlive`, game over when no human is alive (unless AI autoplay /
    /// extended), `turn_slice += 1`.
    pub fn update(&mut self) {
        todo!("CvGame::update")
    }

    /// C++ `CvGame::doTurn`: end of game turn. Score, deals, `CvTeam::doTurn`, `CvMap::doTurn`,
    /// barbarian cities/units, global warming, holy cities, headquarters, diplo votes, AI
    /// autoplay countdown, `incrementGameTurn`/`incrementElapsedGameTurns`, turn activation
    /// (sequential: first alive player), FfH challenge options (cut losers, high to low,
    /// increasing/flexible difficulty, scaled by the speed's growth percent), `testVictory`.
    /// Python `beginGameTurn`/`endGameTurn` events -> crate::ffh.
    pub fn do_turn(&mut self) {
        todo!("CvGame::doTurn")
    }

    /// C++ `CvGame::updateMoves`: for each alive player with an active turn, unit updates
    /// (`AI_unitUpdate` -> controller), auto moves (`autoMission` of every group), and FfH's
    /// end-of-turn AI spell casting (Sephi BETTER AI) for non-human players.
    pub fn update_moves(&mut self) {
        todo!("CvGame::updateMoves")
    }

    /// C++ `CvGame::updateWar` (GAMEOPTION_ALWAYS_WAR: humans declare war on met AI teams).
    pub fn update_war(&mut self) {
        todo!("CvGame::updateWar")
    }

    /// C++ `CvGame::updateTimers` (`CvPlayer::updateTimers` for alive players).
    pub fn update_timers(&mut self) {
        todo!("CvGame::updateTimers")
    }

    /// C++ `CvGame::testAlive` (`CvPlayer::verifyAlive` for every player).
    pub fn test_alive(&mut self) {
        todo!("CvGame::testAlive")
    }

    /// C++ `CvGame::verifyCivics` (`CvPlayer::verifyCivics` for alive players).
    pub fn verify_civics(&mut self) {
        todo!("CvGame::verifyCivics")
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
        todo!("CvGame::setAIAutoPlay")
    }

    /// C++ `CvGame::changeAIAutoPlay(iChange)`.
    pub fn change_ai_auto_play(&mut self, change: i32) {
        todo!("CvGame::changeAIAutoPlay")
    }

    // ---------------------------------------------------------------------------------------
    // World updates
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::updatePlotGroups`.
    pub fn update_plot_groups(&mut self) {
        todo!("CvGame::updatePlotGroups")
    }

    /// C++ `CvGame::updateBuildingCommerce`.
    pub fn update_building_commerce(&mut self) {
        todo!("CvGame::updateBuildingCommerce")
    }

    /// C++ `CvGame::updateCitySight(bIncrement)` (then `updatePlotGroups`).
    pub fn update_city_sight(&mut self, increment: bool) {
        todo!("CvGame::updateCitySight")
    }

    /// C++ `CvGame::updateTradeRoutes`.
    pub fn update_trade_routes(&mut self) {
        todo!("CvGame::updateTradeRoutes")
    }

    /// C++ `CvGame::doGlobalWarming`: forests defend; building bad health and nukes warm;
    /// random land plots lose their feature or turn into `GLOBAL_WARMING_TERRAIN`.
    pub fn do_global_warming(&mut self) {
        todo!("CvGame::doGlobalWarming")
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
        todo!("CvGame::circumnavigationAvailable")
    }

    // ---------------------------------------------------------------------------------------
    // Deals
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::implementDeal(eWho, eOtherWho, pOurList, pTheirList, bForce)`.
    pub fn implement_deal(&mut self, who: PlayerId, other_who: PlayerId, our_list: &[TradeData], their_list: &[TradeData], force: bool) {
        todo!("CvGame::implementDeal")
    }

    /// C++ `CvGame::verifyDeals`.
    pub fn verify_deals(&mut self) {
        todo!("CvGame::verifyDeals")
    }

    /// C++ `CvGame::doDeals` (`verifyDeals`, then `CvDeal::doTurn` for each deal).
    pub fn do_deals(&mut self) {
        todo!("CvGame::doDeals")
    }

    // ---------------------------------------------------------------------------------------
    // Global queries and counters
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::countCivPlayersAlive`.
    pub fn count_civ_players_alive(&self) -> i32 {
        todo!("CvGame::countCivPlayersAlive")
    }

    /// C++ `CvGame::countCivPlayersEverAlive` (players without a parent).
    pub fn count_civ_players_ever_alive(&self) -> i32 {
        todo!("CvGame::countCivPlayersEverAlive")
    }

    /// C++ `CvGame::countCivTeamsAlive`.
    pub fn count_civ_teams_alive(&self) -> i32 {
        todo!("CvGame::countCivTeamsAlive")
    }

    /// C++ `CvGame::countCivTeamsEverAlive`.
    pub fn count_civ_teams_ever_alive(&self) -> i32 {
        todo!("CvGame::countCivTeamsEverAlive")
    }

    /// C++ `CvGame::countHumanPlayersAlive`.
    pub fn count_human_players_alive(&self) -> i32 {
        todo!("CvGame::countHumanPlayersAlive")
    }

    /// C++ `CvGame::countTotalCivPower`.
    pub fn count_total_civ_power(&self) -> i32 {
        todo!("CvGame::countTotalCivPower")
    }

    /// C++ `CvGame::countTotalNukeUnits`.
    pub fn count_total_nuke_units(&self) -> i32 {
        todo!("CvGame::countTotalNukeUnits")
    }

    /// C++ `CvGame::countKnownTechNumTeams(eTech)`.
    pub fn count_known_tech_num_teams(&self, tech: TechType) -> i32 {
        todo!("CvGame::countKnownTechNumTeams")
    }

    /// C++ `CvGame::countNumHumanGameTurnActive`.
    pub fn count_num_human_game_turn_active(&self) -> i32 {
        todo!("CvGame::countNumHumanGameTurnActive")
    }

    /// C++ `CvGame::getNumCivCities` (all cities minus the barbarians').
    pub fn num_civ_cities(&self) -> i32 {
        todo!("CvGame::getNumCivCities")
    }

    /// C++ `CvGame::isTeamGame` (more civ players than civ teams alive).
    pub fn is_team_game(&self) -> bool {
        todo!("CvGame::isTeamGame")
    }

    /// C++ `CvGame::isInAdvancedStart`: a human player still has advanced start points (>= 0).
    pub fn is_in_advanced_start(&self) -> bool {
        todo!("CvGame::isInAdvancedStart")
    }

    /// C++ `CvGame::getCurrentEra`: average era of the alive players (`None` if none alive).
    pub fn current_era(&self) -> Option<EraType> {
        todo!("CvGame::getCurrentEra")
    }

    /// C++ `CvGame::getProductionPerPopulation(eHurry)`.
    pub fn production_per_population(&self, hurry: Option<HurryType>) -> i32 {
        todo!("CvGame::getProductionPerPopulation")
    }

    /// C++ `CvGame::goldenAgeLength` (`GOLDEN_AGE_LENGTH` scaled by the speed).
    pub fn golden_age_length(&self) -> i32 {
        todo!("CvGame::goldenAgeLength")
    }

    /// C++ `CvGame::getBestLandUnitCombat` (at least 1).
    pub fn best_land_unit_combat(&self) -> i32 {
        todo!("CvGame::getBestLandUnitCombat")
    }

    /// C++ `CvGame::calculateSyncChecksum` (MP desync check; useful for determinism tests).
    pub fn calculate_sync_checksum(&self) -> i32 {
        todo!("CvGame::calculateSyncChecksum")
    }

    // ---------------------------------------------------------------------------------------
    // Score and victory
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::updateScore(bForce)`: when dirty (or forced) ranks civ players and teams by
    /// score (ties go to the higher index: `>=`) and records score history.
    pub fn update_score(&mut self, force: bool) {
        todo!("CvGame::updateScore")
    }

    /// C++ `CvGame::testExtendedGame`: GAMESTATE_OVER -> EXTENDED when a human chose to continue.
    pub fn test_extended_game(&mut self) {
        todo!("CvGame::testExtendedGame")
    }

    /// C++ `CvGame::setWinner(eNewWinner, eNewVictory)`. A victory ends the game
    /// (`GAMESTATE_EXTENDED` under AI autoplay).
    pub fn set_winner(&mut self, new_winner: Option<TeamId>, new_victory: Option<VictoryType>) {
        if self.winner != new_winner || self.victory != new_victory {
            self.winner = new_winner;
            self.victory = new_victory;

            if self.victory.is_some() {
                // (replay message "TXT_KEY_GAME_WON" omitted)
                // C++: (getAIAutoPlay() > 0) || gDLL->GetAutorun()
                if self.ai_auto_play > 0 {
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
        todo!("CvGame::testVictory(VictoryTypes, TeamTypes, bool*)")
    }

    /// C++ `CvGame::testVictory()`: victory countdowns, launch success rolls, tie breaker (soren
    /// rand), max-turns game end.
    pub fn test_victory(&mut self) {
        todo!("CvGame::testVictory")
    }

    /// C++ `CvGame::getAdjustedPopulationPercent(eVictory)`.
    pub fn adjusted_population_percent(&self, victory: VictoryType) -> i32 {
        todo!("CvGame::getAdjustedPopulationPercent")
    }

    /// C++ `CvGame::getAdjustedLandPercent(eVictory)`.
    pub fn adjusted_land_percent(&self, victory: VictoryType) -> i32 {
        todo!("CvGame::getAdjustedLandPercent")
    }

    /// C++ `CvGame::victoryDelay(eVictory)` (scaled by the speed's victory delay percent).
    pub fn victory_delay(&self, victory: VictoryType) -> i32 {
        todo!("CvGame::victoryDelay")
    }
}

#[cfg(test)]
mod tests {
    use crate::game::Game;
    use crate::setup::GameSetup;
    use ffh_data::*;

    #[test]
    fn trade_counters_and_game_state() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3));
        assert!(!g.is_free_trade());
        assert!(!g.is_no_nukes());
        g.no_nukes_count += 1;
        assert!(g.is_no_nukes());
        // no-op changes do not touch the (not yet ported) trade route update
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
    }
}
