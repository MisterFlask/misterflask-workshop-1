//! CvPlayer: turn flow (`doTurn`, `doTurnUnits`, `setTurnActive`, the ready/busy/auto unit
//! checks of the turn loop), alive state, identity (civilization/leader/handicap/team/
//! human), names, score and population bookkeeping, space launch, empire splitting
//! (creating a colony player), and the CvPlayerAI hooks rules code calls on players.
//!
//! Turn order (sequential turns, the only mode without networking): a player's turn starts
//! with `player_set_turn_active(p, true)` (units' `doTurn`), the driver lets the player's
//! controller act (`Controller::play_turn`, C++ `AI_unitUpdate` & co.), and the turn ends with
//! `player_set_turn_active(p, false)` (reached through `setAutoMoves(false)` /
//! `setEndTurn(true)`), which runs `doTurn` and activates the next alive player.

#![allow(unused_imports)]

use super::set_turn_history;
use crate::game::Game;
use crate::ids::*;
use crate::setup::SlotStatus;
use crate::structs::*;
use crate::utils::{get_land_plots_asset, get_land_plots_score, get_population_asset, get_population_power, get_population_score};
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------ identity

    /// C++ `CvPlayer::getHandicapType` (`GC.getInitCore().getHandicap(id)`).
    pub fn player_get_handicap_type(&self, p: PlayerId) -> Option<HandicapType> {
        self.setup.players[p.idx()].handicap
    }

    /// C++ `CvPlayer::getCivilizationType` (`GC.getInitCore().getCiv(id)`; None for closed slots).
    pub fn player_get_civilization_type(&self, p: PlayerId) -> Option<CivilizationType> {
        self.setup.players[p.idx()].civ
    }

    /// C++ `CvPlayer::getLeaderType` (`GC.getInitCore().getLeader(id)`).
    pub fn player_get_leader_type(&self, p: PlayerId) -> Option<LeaderHeadType> {
        self.setup.players[p.idx()].leader
    }

    /// C++ `CvPlayer::isBarbarian`.
    pub fn player_is_barbarian(&self, p: PlayerId) -> bool {
        p == self.barbarian_player()
    }

    /// C++ `CvPlayer::isMinorCiv` (`GC.getInitCore().getMinorNationCiv(id)`).
    /// `GameSetup` has no minor-nation slots, so no player is a minor civ.
    pub fn player_is_minor_civ(&self, _p: PlayerId) -> bool {
        false
    }

    /// C++ `CvPlayer::updateHuman`.
    pub fn player_update_human(&mut self, p: PlayerId) {
        let human = if self.players[p].disable_human { false } else { self.setup.players[p.idx()].human };
        self.players[p].human = human;
    }

    /// C++ `CvPlayer::setDisableHuman` (FFH, for AI autoplay).
    pub fn player_set_disable_human(&mut self, p: PlayerId, new_value: bool) {
        self.players[p].disable_human = new_value;
        self.player_update_human(p);
    }

    /// C++ `CvPlayer::updateTeamType`.
    pub fn player_update_team_type(&mut self, p: PlayerId) {
        self.players[p].team = self.setup.players[p.idx()].team;
    }

    /// C++ `CvPlayer::setTeam`: moves the player (and its member/alive/city/population/land
    /// counts) to team `team`.
    pub fn player_set_team(&mut self, p: PlayerId, team: TeamId) {
        let old_team = self.players[p].team;

        self.teams[old_team].num_members -= 1;
        if self.players[p].alive {
            self.team_change_alive_count(old_team, -1);
        }
        if self.players[p].ever_alive {
            self.teams[old_team].ever_alive_count -= 1;
        }
        let (num_cities, total_population, total_land) =
            (self.players[p].num_cities(), self.players[p].total_population, self.players[p].total_land);
        self.teams[old_team].num_cities -= num_cities;
        self.teams[old_team].total_population -= total_population;
        self.teams[old_team].total_land -= total_land;

        // GC.getInitCore().setTeam(getID(), eTeam): updates the player's team when it changes.
        if self.setup.players[p.idx()].team != team {
            self.setup.players[p.idx()].team = team;
            self.player_update_team_type(p);
        }

        let new_team = self.players[p].team;
        self.teams[new_team].num_members += 1;
        if self.players[p].alive {
            self.team_change_alive_count(new_team, 1);
        }
        if self.players[p].ever_alive {
            self.teams[new_team].ever_alive_count += 1;
        }
        let (num_cities, total_population, total_land) =
            (self.players[p].num_cities(), self.players[p].total_population, self.players[p].total_land);
        self.teams[new_team].num_cities += num_cities;
        self.teams[new_team].total_population += total_population;
        self.teams[new_team].total_land += total_land;

        // Score v2 splits a team's knowledge among its members (docs/SCORE.md).
        self.score_dirty = true;
    }

    /// C++ `CvPlayer::getName(uiForm = 0)`: setup leader name, else the leader's description.
    /// (The C++ MPOPTION_ANONYMOUS branch does not apply: no multiplayer.)
    pub fn player_get_name(&self, p: PlayerId) -> String {
        let s = &self.setup.players[p.idx()];
        if !s.leader_name.is_empty() {
            return s.leader_name.clone();
        }
        match self.player_get_leader_type(p) {
            Some(l) => self.d.text.name(&self.d.leader_heads[l].base.description).to_string(),
            None => String::new(),
        }
    }

    /// C++ `CvPlayer::getCivilizationDescription(uiForm = 0)`.
    pub fn player_get_civilization_description(&self, p: PlayerId) -> String {
        let s = &self.setup.players[p.idx()];
        if !s.civ_description.is_empty() {
            return s.civ_description.clone();
        }
        match self.player_get_civilization_type(p) {
            Some(c) => self.d.text.name(&self.d.civilizations[c].base.description).to_string(),
            None => String::new(),
        }
    }

    /// C++ `CvPlayer::getCivilizationShortDescription(uiForm = 0)`. `GameSetup` has no custom
    /// short description (C++ `CvInitCore::getCivShortDesc` is empty), so this is the
    /// civilization's.
    pub fn player_get_civilization_short_description(&self, p: PlayerId) -> String {
        match self.player_get_civilization_type(p) {
            Some(c) => self.d.text.name(&self.d.civilizations[c].short_description_key).to_string(),
            None => String::new(),
        }
    }

    /// C++ `CvPlayer::getCivilizationAdjective(uiForm = 0)`. `GameSetup` has no custom
    /// adjective (C++ `CvInitCore::getCivAdjective` is empty), so this is the civilization's.
    pub fn player_get_civilization_adjective(&self, p: PlayerId) -> String {
        match self.player_get_civilization_type(p) {
            Some(c) => self.d.text.name(&self.d.civilizations[c].adjective_key).to_string(),
            None => String::new(),
        }
    }

    /// C++ `CvPlayer::isPirate` (FFH/BETTER AI: civilization is CIVILIZATION_LANUN).
    pub fn player_is_pirate(&self, p: PlayerId) -> bool {
        opt_to_i32(self.player_get_civilization_type(p)) == self.d.defines.civilization_lanun
    }

    // ------------------------------------------------------------------ turn flow

    /// C++ `CvPlayer::doTurn`: per-turn player processing (deals, timers, gold, research,
    /// espionage, cities, golden age/anarchy, civics, trade routes, war weariness, FFH
    /// timers, events, history). AI parts (`AI_doTurnPre/Post`, cached AI city values,
    /// favorite religion, tower mastery) are left to the controller (`// AI:` notes).
    pub fn player_do_turn(&mut self, p: PlayerId) {
        debug_assert!(self.players[p].alive, "isAlive is expected to be true");
        // C++: FAssertMsg(!hasBusyUnit() || simultaneous turns) -- debug-only check (and
        // hasBusyUnit can kill an empty group), not ported.

        let turn = self.game_turn();
        self.ev_begin_player_turn(turn, p);

        self.player_do_update_cache_on_turn(p);

        // BETTER AI (new Player::DoTurn Functions) Sephi -- for non-human players only:
        // AI: per city AI_calculateNeededPermDefense / AI_calculateNeededPatrol /
        //     AI_calculateNeededCityDefenseProduction / AI_calculateNeededCityPatrolProduction
        //     (AI caches).
        // AI: AI_doFavoriteReligion() -- picks the favorite religion (game RNG) and converts to
        //     it: a decision, left to the controller (`player_ai_do_favorite_religion`).
        // AI: AI_doTowerMastery() -- the arcane tower victory flag, read only by the AI
        //     (`player_ai_do_tower_mastery`).
        // BETTER AI: END

        self.verify_deals();
        // C++ AI_doTurnPre(): of its steps only AI_updateBonusValue feeds rules (through
        // AI_foundValue), so it is kept for every player as in C++.
        self.player_ai_update_bonus_value(p);
        // AI: rest of AI_doTurnPre() -- AI_doCounter (contact timers, memory decay; game RNG),
        // enemy unit data, and for AI players research/commerce/military/civics/religion
        // choices: the controller does this while the player's turn is active.

        if self.players[p].revolution_timer > 0 {
            self.players[p].revolution_timer -= 1;
        }

        if self.players[p].conversion_timer > 0 {
            self.players[p].conversion_timer -= 1;
        }

        // setConscriptCount(0)
        self.players[p].conscript_count = 0;

        self.player_ai_assign_working_plots(p);

        let team = self.players[p].team;
        if 0 == self.team_get_has_met_civ_count(team, true) || self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            self.player_set_commerce_percent(p, CommerceType::COMMERCE_ESPIONAGE, 0);
        }

        self.player_verify_gold_commerce_percent(p);

        self.player_do_gold(p);

        self.player_do_research(p);

        self.player_do_espionage_points(p);

        for c in self.players[p].cities.clone() {
            if self.cities.contains(c) {
                self.city_do_turn(c);
            }
        }

        // Xienwolf Tweak 12/27/08 (Stasis Pauses Golden Ages)
        let pl = &self.players[p];
        if pl.golden_age_turns > 0 && !(pl.disable_production > 0 && pl.disable_research > 0) {
            self.player_change_golden_age_turns(p, -1);
        }

        if self.players[p].anarchy_turns > 0 {
            self.player_change_anarchy_turns(p, -1);
        }

        self.player_verify_civics(p);

        self.player_update_trade_routes(p);

        self.player_update_war_weariness_percent_anger(p);

        // FfH: Added by Kael 11/02/2007
        if self.players[p].temp_player_timer > 0 {
            self.players[p].temp_player_timer -= 1;
            if self.players[p].temp_player_timer == 0 {
                // GC.getInitCore().reassignPlayerAdvanced(getID(), getRealPlayer(), -1); with
                // NO_PLAYER the C++ bounds check makes it a no-op.
                if let Some(real_player) = self.players[p].real_player {
                    reassign_player_advanced(self, p, real_player, -1);
                }
            }
        }
        if self.players[p].sanctuary_timer > 0 {
            // changeSanctuaryTimer(-1): its other effects are graphics (Great Wall art).
            self.players[p].sanctuary_timer -= 1;
        }
        if self.players[p].disable_production > 0 {
            self.players[p].disable_production -= 1;
            self.player_update_maintenance(p);
        }
        if self.players[p].disable_research > 0 {
            self.players[p].disable_research -= 1;
        }
        if self.players[p].disable_spellcasting > 0 {
            self.players[p].disable_spellcasting -= 1;
        }
        // FfH: End Add

        self.player_do_events(p);

        let turn = self.game_turn();
        let economy = self.player_calculate_total_commerce(p);
        set_turn_history(&mut self.players[p].economy_history, turn, economy);
        let industry = self.player_calculate_total_yield(p, YieldType::YIELD_PRODUCTION);
        set_turn_history(&mut self.players[p].industry_history, turn, industry);
        let agriculture = self.player_calculate_total_yield(p, YieldType::YIELD_FOOD);
        set_turn_history(&mut self.players[p].agriculture_history, turn, agriculture);
        let power = self.players[p].power;
        set_turn_history(&mut self.players[p].power_history, turn, power);
        let culture = self.player_count_total_culture(p);
        set_turn_history(&mut self.players[p].culture_history, turn, culture);

        // FfH: Modified by Kael 09/28/2008
        if !self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            let team = self.players[p].team;
            let espionage = self.teams[team].espionage_points_ever;
            set_turn_history(&mut self.players[p].espionage_history, turn, espionage);
        }
        // FfH: End Modify

        // expireMessages(), CityInfo dirty bit: UI.
        // AI: AI_doTurnPost() (AI diplomacy, AI_launch) -- left to the controller.

        let turn = self.game_turn();
        self.ev_end_player_turn(turn, p);
    }

    /// C++ `CvPlayer::doTurnUnits`: kills FFH spell units (SPECIALUNIT_SPELL) of all players,
    /// delayed deaths, then group `doTurn` by domain pass (immobile, air, sea, land).
    pub fn player_do_turn_units(&mut self, p: PlayerId) {
        // AI: AI_doTurnUnitsPre() (found values, area targets; draws "AI Update Area Targets")
        // -- left to the controller.

        // FfH: Added By Kael 09/13/2007
        let spell = self.d.defines.specialunit_spell;
        for other in self.player_ids() {
            if self.players[other].ever_alive {
                for u in self.players[other].units.clone() {
                    if self.units.contains(u) && opt_to_i32(self.unit_get_special_unit_type(u)) == spell {
                        self.unit_kill(u, false, None);
                    }
                }
            }
        }
        // FfH: End Add

        for g in self.players[p].groups.clone() {
            if self.groups.contains(g) {
                self.group_do_delayed_death(g);
            }
        }

        for pass in 0..4 {
            for g in self.players[p].groups.clone() {
                if !self.groups.contains(g) {
                    continue;
                }
                let run = match self.group_get_domain_type(g) {
                    Some(DomainType::DOMAIN_AIR) => pass == 1,
                    Some(DomainType::DOMAIN_SEA) => pass == 2,
                    Some(DomainType::DOMAIN_LAND) => pass == 3,
                    Some(DomainType::DOMAIN_IMMOBILE) => pass == 0,
                    // NO_DOMAIN (an empty group) falls through to the default case.
                    _ => pass == 3,
                };
                if run {
                    self.group_do_turn(g);
                }
            }
        }

        // Interface path finder reset, waypoints/selection/unit info dirty bits: UI.

        // AI: AI_doTurnUnitsPost() (AI promotions, FfH auto-spellcasting of human units with an
        // auto-cast spell, BETTER AI regrouping and mage casting) -- left to the controller.
    }

    /// C++ `CvPlayer::updateTimers`: selection group timers and delayed deaths.
    pub fn player_update_timers(&mut self, p: PlayerId) {
        for g in self.players[p].groups.clone() {
            if self.groups.contains(g) {
                self.group_update_timers(g); // could destroy the selection group...
            }
        }

        // if a unit was busy, perhaps it was not quite deleted yet, give it one more try
        if self.players[p].groups.len() > self.players[p].units.len() {
            for g in self.players[p].groups.clone() {
                if self.groups.contains(g) {
                    self.group_do_delayed_death(g); // could destroy the selection group...
                }
            }
        }
    }

    /// C++ `CvPlayer::hasReadyUnit(bAny = false)`.
    pub fn player_has_ready_unit(&self, p: PlayerId, any: bool) -> bool {
        self.players[p].groups.iter().any(|&g| self.group_ready_to_move(g, any))
    }

    /// C++ `CvPlayer::hasAutoUnit`.
    pub fn player_has_auto_unit(&self, p: PlayerId) -> bool {
        self.players[p].groups.iter().any(|&g| self.group_ready_to_auto(g))
    }

    /// C++ `CvPlayer::hasBusyUnit` (const in C++, but kills a busy group with no units).
    pub fn player_has_busy_unit(&mut self, p: PlayerId) -> bool {
        for i in 0..self.players[p].groups.len() {
            let g = self.players[p].groups[i];
            if self.group_is_busy(g) {
                if self.groups[g].units.is_empty() {
                    self.group_kill(g);
                    return false;
                }

                return true;
            }
        }

        false
    }

    /// C++ `CvPlayer::doUpdateCacheOnTurn` (body commented out in C++).
    pub fn player_do_update_cache_on_turn(&mut self, _p: PlayerId) {
        // C++: "add this back, after testing without it" -- invalidateYieldRankCache();
    }

    /// C++ `CvPlayer::setAlive`: team alive counts, Python event, plot groups and turn
    /// activation when coming alive; research queue/units/cities/deals cleared when dying.
    pub fn player_set_alive(&mut self, p: PlayerId, new_value: bool) {
        if self.players[p].alive != new_value {
            self.players[p].alive = new_value;

            let team = self.players[p].team;
            self.team_change_alive_count(team, if self.players[p].alive { 1 } else { -1 });

            // Report event to Python
            self.ev_set_player_alive(p, new_value);

            if self.players[p].alive {
                if !self.players[p].ever_alive {
                    self.players[p].ever_alive = true;

                    let team = self.players[p].team;
                    self.teams[team].ever_alive_count += 1;
                }

                if self.players[p].num_cities() == 0 {
                    // setFoundedFirstCity(false): plain store (+ UI dirty bits)
                    self.players[p].founded_first_city = false;
                }

                self.player_update_plot_groups(p);

                // MPOPTION_SIMULTANEOUS_TURNS is never set (no multiplayer).
                let team = self.players[p].team;
                if self.num_game_turn_active == 0 || (self.is_simultaneous_team_turns() && self.team_is_turn_active(team)) {
                    self.player_set_turn_active(p, true, true);
                }

                // gDLL->openSlot(getID()): network.
            } else {
                self.player_clear_research_queue(p);

                self.player_kill_units(p);
                self.player_kill_cities(p);
                self.player_kill_all_deals(p);

                self.player_set_turn_active(p, false, true);

                // endMPDiplomacy / endDiplomacy / closeSlot: UI and network.

                if self.elapsed_game_turns > 0 && !self.player_is_barbarian(p) {
                    // TXT_KEY_MISC_CIV_DESTROYED message to every alive player + replay message.
                    if self.log.enabled {
                        let turn = self.game_turn();
                        let adjective = self.player_get_civilization_adjective(p);
                        self.log.msg(turn, Some(p), || format!("The {} civilization has been destroyed", adjective));
                    }
                }
            }

            self.score_dirty = true;
        }
    }

    /// C++ `CvPlayer::verifyAlive`: kills players without cities (and without units or after
    /// founding, unless GAMEOPTION_COMPLETE_KILLS) or past the city-elimination limit.
    pub fn player_verify_alive(&mut self, p: PlayerId) {
        if self.players[p].alive {
            let mut kill = false;

            if !kill && !self.player_is_barbarian(p) {
                let pl = &self.players[p];
                if pl.num_cities() == 0 && pl.advanced_start_points < 0 {
                    if pl.num_units() == 0 || (!self.is_option(GameOptionType::GAMEOPTION_COMPLETE_KILLS) && pl.founded_first_city) {
                        kill = true;
                    }
                }
            }

            if !kill && !self.player_is_barbarian(p) {
                if self.max_city_elimination() > 0 && self.players[p].cities_lost >= self.max_city_elimination() {
                    kill = true;
                }
            }

            if kill {
                self.player_set_alive(p, false);
            }
        } else if self.players[p].num_cities() > 0 || self.players[p].num_units() > 0 {
            self.player_set_alive(p, true);
        }
    }

    /// C++ `CvPlayer::setTurnActive(bNewValue, bDoTurn = true)`: starts/ends the player's
    /// turn (advanced-start AI, `doTurn` in simultaneous mode, `doTurnUnits`, activates the
    /// next player/team when the turn ends).
    ///
    /// Only sequential turns exist here (no MPOPTION_SIMULTANEOUS_TURNS, no PBEM/hot seat/
    /// pitboss): ending a turn runs `doTurn` and activates the next alive player in slot
    /// order. Starting a turn runs the units' turn (`doTurnUnits`) once the game is past its
    /// first turn; the AI then acts through the controller while the turn is active.
    pub fn player_set_turn_active(&mut self, p: PlayerId, new_value: bool, do_turn: bool) {
        if self.players[p].turn_active != new_value {
            self.players[p].turn_active = new_value;

            if self.players[p].turn_active {
                debug_assert!(self.players[p].alive, "isAlive is expected to be true");

                self.player_set_end_turn(p, false);
                // resetTurnTimer(): MP turn timer (not ported). Pitboss reminder e-mails and the
                // hot seat / PBEM active-player switch do not apply.

                self.num_game_turn_active += 1;

                if do_turn {
                    // AI: AI_doAdvancedStart() for an alive non-human, non-barbarian player with
                    // advanced start points >= 0 -- spending the points is an AI decision; the
                    // controller does it while the turn is active.

                    if self.elapsed_game_turns > 0 && self.players[p].alive {
                        // MPOPTION_SIMULTANEOUS_TURNS (doTurn here) is never set.
                        self.player_do_turn_units(p);
                    }

                    // New-turn message/sound for the active player and doWarnings(): UI.
                }

                // Selection cycling / camera for the active player: UI.
            } else {
                // Popups, talking-head messages and DLL profiling for the active player: UI.

                self.num_game_turn_active -= 1;

                // !MPOPTION_SIMULTANEOUS_TURNS: always the case here.
                if do_turn {
                    if self.players[p].alive {
                        self.player_do_turn(p);
                    }

                    // PBEM / hot seat "between turns" state: not applicable.

                    if self.is_simultaneous_team_turns() {
                        let team = self.players[p].team;
                        if !self.team_is_turn_active(team) {
                            for t in (team.idx() + 1)..self.num_teams() {
                                let t = TeamId(t as u8);
                                if self.team_is_alive(t) {
                                    self.team_set_turn_active(t, true, true);
                                    break;
                                }
                            }
                        }
                    } else {
                        for i in (p.idx() + 1)..self.num_players() {
                            let next = PlayerId(i as u8);
                            if self.players[next].alive {
                                // (PBEM human: the turn is e-mailed instead -- no PBEM here.)
                                self.player_set_turn_active(next, true, true);
                                break;
                            }
                        }
                    }
                }
            }

            // Cursor and score dirty bit: UI.
        }
    }

    /// C++ `CvPlayer::setAutoMoves`.
    pub fn player_set_auto_moves(&mut self, p: PlayerId, new_value: bool) {
        if self.players[p].auto_moves != new_value {
            self.players[p].auto_moves = new_value;

            if !new_value {
                if self.players[p].end_turn || !self.players[p].human {
                    self.player_set_turn_active(p, false, true);
                }
                // else: UI only (cycle selection counter for the active player)
            }
        }
    }

    /// C++ `CvPlayer::setEndTurn`.
    pub fn player_set_end_turn(&mut self, p: PlayerId, new_value: bool) {
        if self.players[p].end_turn != new_value {
            debug_assert!(self.players[p].turn_active, "isTurnActive is expected to be true");

            self.players[p].end_turn = new_value;

            if new_value {
                self.player_set_auto_moves(p, true);
            }
        }
    }

    /// C++ `CvPlayer::isTurnDone`: only ever true in PBEM/hotseat games, which this port
    /// does not have.
    pub fn player_is_turn_done(&self, _p: PlayerId) -> bool {
        false
    }

    // ------------------------------------------------------------------ score / population

    /// C++ `CvPlayer::calculateScore(bFinal, bVictory)`, kept for compatibility: score v2 (the
    /// player's share of the world in basis points, [`Game::world_scores`]; a deliberate
    /// deviation, docs/SCORE.md). Dead players and barbarians score 0. `bFinal`/`bVictory` only
    /// fed the Hall of Fame and replay scores, which are not ported, so they are ignored.
    /// Computes every player's score; read `player_score` for the cached value.
    pub fn player_calculate_score(&self, p: PlayerId, _final_score: bool, _victory: bool) -> i32 {
        if !self.players[p].alive || self.player_is_barbarian(p) {
            return 0;
        }

        self.world_scores().get(p.idx()).copied().unwrap_or(0)
    }

    /// FFH2's own score, C++ `CvPlayer::calculateScore(false, false)` (Python `calculateScore`,
    /// CvGameUtils): population, land, tech and wonders as shares of game-wide maxima. Score v2
    /// replaced it as the game score; this is computed on demand for tools only (absolute
    /// progress, which the zero-sum v2 score cannot show). No rule reads it.
    pub fn player_legacy_score(&self, p: PlayerId) -> i32 {
        if !self.players[p].alive {
            return 0;
        }

        if self.teams[self.players[p].team].num_members == 0 {
            return 0;
        }

        self.py_calculate_score(p)
    }

    /// C++ `CvPlayer::getAveragePopulation`.
    pub fn player_get_average_population(&self, p: PlayerId) -> i32 {
        let pl = &self.players[p];
        if pl.num_cities() == 0 {
            return 0;
        }
        (pl.total_population / pl.num_cities()) + 1
    }

    /// C++ `CvPlayer::changeTotalPopulation`: also updates assets, power and population score.
    pub fn player_change_total_population(&mut self, p: PlayerId, change: i32) {
        let old_population = self.players[p].total_population;
        self.players[p].assets -= get_population_asset(old_population);
        self.players[p].power -= get_population_power(old_population);
        self.player_change_pop_score(p, -get_population_score(old_population));

        self.players[p].total_population += change;
        debug_assert!(self.players[p].total_population >= 0);

        let new_population = self.players[p].total_population;
        self.players[p].assets += get_population_asset(new_population);
        self.players[p].power += get_population_power(new_population);
        self.player_change_pop_score(p, get_population_score(new_population));
    }

    /// C++ `CvPlayer::getRealPopulation` (sum of city real populations, capped at MAX_INT).
    pub fn player_get_real_population(&self, p: PlayerId) -> i32 {
        let mut total_population: i64 = 0;

        for &c in &self.players[p].cities {
            total_population += self.city_real_population(c) as i64;
        }

        if total_population > i32::MAX as i64 {
            total_population = i32::MAX as i64;
        }

        total_population as i32
    }

    /// C++ `CvPlayer::changeTotalLandScored`: also updates assets and land score.
    pub fn player_change_total_land_scored(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            let old_land = self.players[p].total_land_scored;
            self.players[p].assets -= get_land_plots_asset(old_land);
            self.player_change_land_score(p, -get_land_plots_score(old_land));

            self.players[p].total_land_scored += change;
            debug_assert!(self.players[p].total_land_scored >= 0);

            let new_land = self.players[p].total_land_scored;
            self.players[p].assets += get_land_plots_asset(new_land);
            self.player_change_land_score(p, get_land_plots_score(new_land));
        }
    }

    /// C++ `CvPlayer::getPopScore(bCheckVassal = true)`.
    pub fn player_get_pop_score(&self, p: PlayerId, check_vassal: bool) -> i32 {
        let team = self.players[p].team;
        if check_vassal && self.team_is_a_vassal(team) {
            return self.players[p].population_score / 2;
        }

        let mut vassal_score = 0;

        if check_vassal {
            for other in self.civ_player_ids() {
                if other != p {
                    let o = &self.players[other];
                    if o.alive && self.teams[o.team].vassal[team] {
                        vassal_score += self.player_get_pop_score(other, false) / 2;
                    }
                }
            }
        }

        self.players[p].population_score + vassal_score / std::cmp::max(1, self.teams[team].num_members)
    }

    /// C++ `CvPlayer::changePopScore` (marks the game score dirty).
    pub fn player_change_pop_score(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].population_score += change;
            // C++ FAssert(getPopScore() >= 0): debug-only, not ported.

            self.score_dirty = true;
        }
    }

    /// C++ `CvPlayer::getLandScore(bCheckVassal = true)`.
    pub fn player_get_land_score(&self, p: PlayerId, check_vassal: bool) -> i32 {
        let team = self.players[p].team;
        if check_vassal && self.team_is_a_vassal(team) {
            return self.players[p].land_score / 2;
        }

        let mut vassal_score = 0;

        if check_vassal {
            for other in self.civ_player_ids() {
                if other != p {
                    let o = &self.players[other];
                    if o.alive && self.teams[o.team].vassal[team] {
                        vassal_score += self.player_get_land_score(other, false) / 2;
                    }
                }
            }
        }

        self.players[p].land_score + vassal_score / std::cmp::max(1, self.teams[team].num_members)
    }

    /// C++ `CvPlayer::changeLandScore` (marks the game score dirty).
    pub fn player_change_land_score(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].land_score += change;
            // C++ FAssert(getLandScore() >= 0): debug-only, not ported.

            self.score_dirty = true;
        }
    }

    /// C++ `CvPlayer::changeWondersScore` (marks the game score dirty).
    pub fn player_change_wonders_score(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].wonders_score += change;
            debug_assert!(self.players[p].wonders_score >= 0);

            self.score_dirty = true;
        }
    }

    /// C++ `CvPlayer::changeTechScore` (marks the game score dirty).
    pub fn player_change_tech_score(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].tech_score += change;
            debug_assert!(self.players[p].tech_score >= 0);

            self.score_dirty = true;
        }
    }

    // ------------------------------------------------------------------ victory

    /// C++ `CvPlayer::launch(eVictory)`: launches the spaceship (team victory countdown).
    pub fn player_launch(&mut self, p: PlayerId, victory: VictoryType) {
        let team = self.players[p].team;

        if !self.teams[team].can_launch[victory] {
            return;
        }

        // kTeam.finalizeProjectArtTypes(): graphics.
        let delay = self.team_get_victory_delay(team, victory);
        self.teams[team].victory_countdown[victory] = delay;

        // AddLaunch: graphics.

        self.teams[team].can_launch[victory] = false;

        // TXT_KEY_VICTORY_YOU_HAVE_LAUNCHED / TXT_KEY_VICTORY_TEAM_HAS_LAUNCHED message to every
        // alive player.
        if self.log.enabled {
            let turn = self.game_turn();
            let civ = self.player_get_civilization_description(p);
            self.log.msg(turn, Some(p), || format!("The {} have launched their spaceship", civ));
        }
    }

    // ------------------------------------------------------------------ CvPlayerAI hooks used by rules

    /// C++ `CvPlayerAI::AI_makeAssignWorkDirty`: flags every city for citizen reassignment
    /// (the governor, see `governor.rs`).
    pub fn player_ai_make_assign_work_dirty(&mut self, p: PlayerId) {
        for i in 0..self.players[p].cities.len() {
            let c = self.players[p].cities[i];
            // CvCityAI::AI_setAssignWorkDirty(true)
            self.cities[c].ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvPlayerAI::AI_assignWorkingPlots`: runs the citizen governor on every city.
    pub fn player_ai_assign_working_plots(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            if self.cities.contains(c) {
                city_ai_assign_working_plots(self, c);
            }
        }
    }

    /// C++ `CvPlayerAI::AI_updateAssignWork`: governor update for every city.
    pub fn player_ai_update_assign_work(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            // CvCityAI::AI_updateAssignWork: reassign only cities flagged dirty.
            if self.cities.contains(c) && self.cities[c].ai_assign_work_dirty {
                city_ai_assign_working_plots(self, c);
            }
        }
    }

    /// C++ `CvPlayerAI::AI_makeProductionDirty`: flags every city to re-choose production.
    pub fn player_ai_make_production_dirty(&mut self, p: PlayerId) {
        // C++ FAssertMsg(!isHuman()): debug-only.
        for i in 0..self.players[p].cities.len() {
            let c = self.players[p].cities[i];
            // CvCityAI::AI_setChooseProductionDirty(true)
            self.cities[c].ai_choose_production_dirty = true;
        }
    }

    /// C++ `CvPlayer::AI_doTowerMastery` (BETTER AI; C++ `doTurn` calls it for AI players):
    /// Python `AI_TowerMastery` updates the arcane tower victory flag (`tower_vic_flag`),
    /// which only the AI reads. `player_do_turn` leaves the call to the controller.
    pub fn player_ai_do_tower_mastery(&mut self, p: PlayerId) {
        // C++ reads GC.getLeaderHeadInfo(getLeaderType()) unchecked.
        let Some(leader) = self.player_get_leader_type(p) else { return };
        if self.d.leader_heads[leader].arcane_tower_victory {
            let flag = self.players[p].tower_vic_flag;
            let result = py_ai_tower_mastery(self, p, flag);
            self.players[p].tower_vic_flag = result;
        }
    }

    // ------------------------------------------------------------------ split empire (colonies): creating a new player

    /// C++ `CvPlayer::getSplitEmpirePlayer(iAreaId)`: slot for a new colony on `area`.
    pub fn player_get_split_empire_player(&self, p: PlayerId, area: AreaId) -> Option<PlayerId> {
        // can't create different derivative civs on the same continent
        for other in self.civ_player_ids() {
            if self.players[other].alive && self.players[other].parent == Some(p) {
                if let Some(capital) = self.player_get_capital_city(other) {
                    if self.city_area(capital) == area {
                        return None;
                    }
                }
            }
        }

        // Try to find a player who's never been in the game before
        let mut new_player = self.civ_player_ids().find(|&i| !self.players[i].ever_alive);

        if new_player.is_none() {
            // Try to recycle a dead player
            new_player = self.civ_player_ids().find(|&i| !self.players[i].alive);
        }

        new_player
    }

    /// C++ `CvPlayer::canSplitEmpire`.
    pub fn player_can_split_empire(&self, p: PlayerId) -> bool {
        if self.is_option(GameOptionType::GAMEOPTION_NO_VASSAL_STATES) {
            return false;
        }

        if self.team_is_a_vassal(self.players[p].team) {
            return false;
        }

        if self.player_get_split_empire_leaders(p).is_empty() {
            return false;
        }

        let mut found_area = false;

        for area in self.map.areas.ids() {
            if self.player_can_split_area(p, area) {
                found_area = true;
                break;
            }
        }

        found_area
    }

    /// C++ `CvPlayer::canSplitArea(iAreaId)`.
    pub fn player_can_split_area(&self, p: PlayerId, area: AreaId) -> bool {
        let Some(capital) = self.player_get_capital_city(p) else { return false };

        let Some(area_info) = self.map.areas.get(area) else { return false };
        if area == self.city_area(capital) {
            return false;
        }

        if 0 == area_info.cities_per_player[p] {
            return false;
        }

        let Some(new_player) = self.player_get_split_empire_player(p, area) else { return false };

        if !self.players[new_player].alive && area_info.cities_per_player[p] <= 1 {
            return false;
        }

        true
    }

    /// C++ `CvPlayer::getSplitEmpireLeaders(aLeaders)`: candidate (civilization, leader)
    /// pairs for a colony; C++ returns `!aLeaders.empty()`.
    pub fn player_get_split_empire_leaders(&self, p: PlayerId) -> Vec<(CivilizationType, LeaderHeadType)> {
        let d = self.d;
        let mut leaders = Vec::new();
        let our_civ = self.player_get_civilization_type(p);

        for civ in d.civilizations.keys() {
            // FfH: Modified by Kael 01/16/2009 -- the "not our own civilization" test is
            // commented out.
            let mut valid = true;

            if valid && (!civ_is_playable(self, civ) || !civ_is_ai_playable(self, civ)) {
                valid = false;
            }

            if valid {
                for other in self.civ_player_ids() {
                    if p != other && self.players[other].ever_alive && self.player_get_civilization_type(other) == Some(civ) {
                        valid = false;
                        break;
                    }
                }
            }

            if valid {
                let civ_info = &d.civilizations[civ];
                for leader in d.leader_heads.keys() {
                    let leader_info = &d.leader_heads[leader];
                    let mut leader_valid = true;
                    if !civ_info.leaders[leader] && !self.is_option(GameOptionType::GAMEOPTION_LEAD_ANY_CIV) {
                        leader_valid = false;
                    }

                    // FfH: Added by Kael 01/16/2008
                    if our_civ == Some(civ) && leader_info.base.graphical_only && leader_info.alignment == self.players[p].alignment {
                        leader_valid = true;
                    }
                    if leader_info.favorite_wonder.is_none() {
                        leader_valid = false;
                    }
                    // FfH: End Add

                    if leader_valid {
                        for other in self.civ_player_ids() {
                            if self.players[other].ever_alive && self.players[other].personality_type == Some(leader) {
                                leader_valid = false;
                            }
                        }
                    }

                    if leader_valid {
                        leaders.push((civ, leader));
                    }
                }
            }
        }

        leaders
    }

    /// C++ `CvPlayer::splitEmpire(iAreaId)`: creates a vassal colony from the cities on `area`.
    pub fn player_split_empire(&mut self, p: PlayerId, area: AreaId) -> bool {
        if !self.player_can_split_empire(p) {
            return false;
        }

        if !self.player_can_split_area(p, area) {
            return false;
        }

        if self.map.areas.get(area).is_none() {
            return false;
        }

        let Some(new_player) = self.player_get_split_empire_player(p, area) else { return false };

        let player_exists = self.team_is_alive(self.players[new_player].team);
        // C++ FAssert(!bPlayerExists)
        if !player_exists {
            let d = self.d;
            let our_civ = self.player_get_civilization_type(p);
            let mut best_value = -1;
            let mut best: Option<(CivilizationType, LeaderHeadType)> = None;

            for (civ, leader) in self.player_get_split_empire_leaders(p) {
                let mut value = 1 + self.soren_rand_num(100, "Choosing Split Personality");

                // FfH: Added by Kael 01/16/2008
                if our_civ == Some(civ) {
                    value += 1000;
                }
                if d.leader_heads[leader].base.graphical_only {
                    value += 1000;
                }
                // FfH: End Add

                if our_civ.and_then(|c| d.civilizations[c].derivative_civ) == Some(civ) {
                    value += 1000;
                }

                if value > best_value {
                    best_value = value;
                    best = Some((civ, leader));
                }
            }

            let Some((best_civ, best_leader)) = best else { return false };

            // TXT_KEY_MISC_EMPIRE_SPLIT message to this player, the new one and the teams that
            // met us, + replay message.
            if self.log.enabled {
                let turn = self.game_turn();
                let name = self.player_get_name(p);
                let civ_name = d.text.name(&d.civilizations[best_civ].short_description_key).to_string();
                let leader_name = d.text.name(&d.leader_heads[best_leader].base.description).to_string();
                self.log.msg(turn, Some(p), || format!("{} splits the empire: {} of the {} takes control of the colony", name, leader_name, civ_name));
            }

            // remove leftover culture from old recycled player
            for plot in self.map.plot_ids() {
                self.plot_set_culture(plot, new_player, 0, false, false);
            }

            self.add_player(new_player, best_leader, best_civ);
            self.players[new_player].parent = Some(p);
            // GC.getInitCore().setLeaderName(eNewPlayer, leader text key): the setup stores
            // the resolved name (what CvInitCore::getLeaderName returns for the key).
            self.setup.players[new_player.idx()].leader_name = d.text.name(&d.leader_heads[best_leader].base.description).to_string();

            let team = self.players[p].team;
            let new_team = self.players[new_player].team;
            for tech in d.techs.keys() {
                if self.team_is_has_tech(team, Some(tech)) {
                    self.team_set_has_tech(new_team, tech, true, Some(new_player), false, false);
                    if self.teams[team].no_trade_tech[tech] || self.is_option(GameOptionType::GAMEOPTION_NO_TECH_BROKERING) {
                        self.teams[new_team].no_trade_tech[tech] = true;
                    }
                }
            }

            for t in self.team_ids() {
                if self.team_is_alive(t) {
                    let points = self.teams[team].espionage_points_against_team[t];
                    self.team_set_espionage_points_against_team(new_team, t, points);
                    let points = self.teams[t].espionage_points_against_team[team];
                    self.team_set_espionage_points_against_team(t, new_team, points);
                }
            }
            self.teams[new_team].espionage_points_ever = self.teams[team].espionage_points_ever;

            self.team_assign_vassal(team, new_team, false);

            // CvPlayerAI::AI_updateBonusValue() (cache read by AI_foundValue).
            self.player_ai_update_bonus_value(p);
        }

        let team = self.players[p].team;
        let new_team = self.players[new_player].team;
        let mut cultures: Vec<(PlotIdx, i32)> = Vec::new();
        for plot in self.map.plot_ids() {
            let mut transfer_plot = false;

            if !transfer_plot && self.map.plots[plot].area == Some(area) {
                transfer_plot = true;
            }

            if !transfer_plot {
                if let Some(working_city) = self.map.plots[plot].working_city {
                    if self.cities[working_city].owner == p && self.city_area(working_city) == area {
                        transfer_plot = true;
                    }
                }
            }

            if !transfer_plot && self.plot_is_water(plot) && self.plot_is_adjacent_to_area(plot, area) {
                transfer_plot = true;
            }

            if transfer_plot {
                let mut culture = self.map.plots[plot].culture[p];

                if player_exists {
                    culture = std::cmp::max(culture, self.map.plots[plot].culture[new_player]);
                }

                cultures.push((plot, culture));
            }

            if self.map.plots[plot].revealed[team] {
                self.plot_set_revealed(plot, new_team, true, false, Some(team), false);
            }
        }

        for c in self.players[p].cities.clone() {
            if !self.cities.contains(c) || self.city_area(c) != area {
                continue;
            }

            // getCultureTimes100(getID())
            let culture = self.cities[c].culture[p];
            let plot = self.city_plot(c);

            self.player_acquire_city(new_player, c, false, true, false);

            let city = self.map.plots[plot].plot_city;
            if let Some(city) = city {
                self.city_set_culture_times100(city, new_player, culture, false, false);
            }

            // FfH: Added by Kael 01/16/2009 (to prevent CtD's when colony free units are dropped
            // on top of hidden nationality units)
            for u in self.map.plots[plot].units.clone() {
                if self.units.contains(u) && self.unit_is_hidden_nationality(u) {
                    self.unit_jump_to_nearest_valid_plot(u);
                }
            }
            // FfH: End Add

            // C++ quirk: the conscription loop dereferences pCity without the NULL check above
            // (a trade acquisition always keeps the city, so it is never NULL in practice).
            if let Some(city) = city {
                for _ in 0..self.d.defines.colony_num_free_defenders {
                    self.city_init_conscripted_unit(city);
                }
            }
        }

        for &(plot, culture) in &cultures {
            self.plot_set_culture(plot, new_player, culture, true, false);
            self.plot_set_culture(plot, p, 0, true, false);

            for t in self.team_ids() {
                // getRevealedOwner / setRevealedOwner (the setter's other effects are UI)
                if self.map.plots[plot].revealed_owner[t] == Some(p) {
                    self.map.plots[plot].revealed_owner[t] = Some(new_player);
                }
            }
        }

        self.update_plot_groups();

        true
    }
}

/// C++ `CvCivilizationInfo::isPlayable()`: FfH makes a graphical-only civilization unplayable
/// once the game is fully initialized.
fn civ_is_playable(g: &Game, civ: CivilizationType) -> bool {
    let info = &g.d.civilizations[civ];
    if g.final_initialized && info.base.graphical_only {
        return false;
    }
    info.playable
}

/// C++ `CvCivilizationInfo::isAIPlayable()` (same FfH rule as [`civ_is_playable`]).
fn civ_is_ai_playable(g: &Game, civ: CivilizationType) -> bool {
    let info = &g.d.civilizations[civ];
    if g.final_initialized && info.base.graphical_only {
        return false;
    }
    info.ai_playable
}

/// C++ `CvCityAI::AI_assignWorkingPlots`: the citizen governor's per-city entry point.
fn city_ai_assign_working_plots(g: &mut Game, c: CityId) {
    g.city_assign_working_plots(c);
}

/// C++ `CvInitCore::reassignPlayerAdvanced(eOldID, eNewID, iTimer)` (FfH; `doTurn` uses it when
/// a temporary player's timer runs out, Python `spellImpersonateLeader` to start one): moves
/// the controlling slot data of `old` to `new` and back (handicap, slot status, human flag),
/// gives `new` the options of `old` (`old`'s are cleared), moves the active player with the
/// slot (CvInitCore.cpp:474-482), refreshes both players' team and human flags, and with a timer
/// makes `new` a temporary player of `old`.
///
/// Network/UI slot data (e-mail, SMTP host, slot claim, playable/minor flags, net id, ready,
/// checksums) is not ported. `PlayerSetup::human` is swapped with the slot status because C++
/// derives `getHuman` from the slot status.
pub(crate) fn reassign_player_advanced(g: &mut Game, old: PlayerId, new: PlayerId, timer: i32) {
    // checkBounds(eOldID, 0, MAX_CIV_PLAYERS) && checkBounds(eNewID, 0, MAX_CIV_PLAYERS)
    let num_civ_players = g.num_civ_players();
    if old.idx() >= num_civ_players || new.idx() >= num_civ_players {
        return;
    }

    let (oi, ni) = (old.idx(), new.idx());
    let handicap = g.setup.players[ni].handicap;
    let slot_status = g.setup.players[ni].slot_status;
    let human = g.setup.players[ni].human;

    g.setup.players[ni].handicap = g.setup.players[oi].handicap;
    // setSlotStatus(eNewID, ..) (its updateHuman is repeated below)
    g.setup.players[ni].slot_status = g.setup.players[oi].slot_status;
    g.setup.players[ni].human = g.setup.players[oi].human;

    g.setup.players[oi].handicap = handicap;
    g.setup.players[oi].slot_status = slot_status;
    g.setup.players[oi].human = human;
    // The controllers are this port's form of the slot claim / net id, which C++ swaps too
    // (whoever played `old` now plays `new`).
    g.controllers.raw.swap(oi, ni);

    for o in 0..PlayerOptionType::NUM {
        let o = PlayerOptionType::from_usize(o);
        let value = g.players[old].options[o];
        g.players[new].options[o] = value;
        g.players[old].options[o] = false;
    }

    // We may have a new active player id... The active player gates rules here (QUICK_MOVES
    // mission timers, the research popup call, `getDeclareWarMove` visibility), so it moves with
    // the slot. `CvGame::setActivePlayer`'s other work is UI (fog and symbol updates, selection)
    // or hot seat / PBEM only (passwords, net ids, `sendPlayerOptions`); `CvInitCore`'s slot
    // claim is not ported.
    if g.active_player == Some(old) {
        g.active_player = Some(new);
    } else if g.active_player == Some(new) {
        g.active_player = Some(old);
    }

    g.player_update_team_type(old);
    g.player_update_team_type(new);
    g.player_update_human(old);
    g.player_update_human(new);

    if timer != -1 {
        // changeTempPlayerTimer(iTimer), setRealPlayer(eOldID)
        g.players[new].temp_player_timer += timer;
        g.players[new].real_player = Some(old);
    }
}

/// Python `CvGameUtils.AI_TowerMastery(argsList)` (FfH2 BETTER AI, called by
/// `CvPlayer::AI_doTowerMastery`): the arcane tower victory flag. With flag 0 it starts a
/// tower plan (1..4 = the first missing tower: Alteration, Divination, Necromancy, the
/// Elements) once the player knows Sorcery, has Metamagic mana and owns at least 4 mana
/// bonuses; with a plan it returns 0 once that tower exists, else keeps the plan.
fn py_ai_tower_mastery(g: &Game, p: PlayerId, flag: i32) -> i32 {
    let d = g.d;
    let pl = &g.players[p];
    let team = pl.team;

    let alteration = d.t::<BuildingClassType>("BUILDINGCLASS_TOWER_OF_ALTERATION");
    let divination = d.t::<BuildingClassType>("BUILDINGCLASS_TOWER_OF_DIVINATION");
    let necromancy = d.t::<BuildingClassType>("BUILDINGCLASS_TOWER_OF_NECROMANCY");
    let elements = d.t::<BuildingClassType>("BUILDINGCLASS_TOWER_OF_THE_ELEMENTS");
    let count = |class: BuildingClassType| pl.building_class_count[class];

    if flag == 0 {
        // gc.getInfoTypeForString returns -1 for an unknown type; isHasTech(NO_TECH) is true.
        if !g.team_is_has_tech(team, d.ty::<TechType>("TECH_SORCERY")) {
            return 0;
        }
        if g.player_get_num_available_bonuses(p, d.t::<BonusType>("BONUS_MANA_METAMAGIC")) == 0 {
            return 0;
        }

        let mana = d.ty::<BonusClassType>("BONUSCLASS_MANA");
        let raw_mana = d.ty::<BonusClassType>("BONUSCLASS_RAWMANA");
        let mut possible_mana = 0;
        for plot in g.map.plot_ids() {
            let pd = &g.map.plots[plot];
            if pd.owner == Some(p) {
                // pPlot.getBonusType(-1): the bonus regardless of tech reveal
                if let Some(bonus) = pd.bonus_type {
                    let class = d.bonuses[bonus].bonus_class_type;
                    if class == mana {
                        possible_mana += 1;
                    }
                    if class == raw_mana {
                        possible_mana += 1;
                    }
                }
            }
        }

        if possible_mana < 4 {
            return 0;
        }

        if count(alteration) == 0 {
            return 1;
        }

        if count(divination) == 0 {
            return 2;
        }

        if count(necromancy) == 0 {
            return 3;
        }

        if count(elements) == 0 {
            return 4;
        }
    }

    if flag == 1 {
        return if count(alteration) > 0 { 0 } else { 1 };
    }

    if flag == 2 {
        return if count(divination) > 0 { 0 } else { 2 };
    }

    if flag == 3 {
        return if count(necromancy) > 0 { 0 } else { 3 };
    }

    if flag == 4 {
        return if count(elements) > 0 { 0 } else { 4 };
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use crate::setup::GameSetup;

    /// A game whose first `n_civs` slots hold freshly reset players (C++ `reset`), alive on
    /// their own teams, with the given civilizations/leaders; other slots stay placeholders.
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
            let t = g.players[p].team;
            g.teams[t].num_members = 1;
            g.teams[t].alive_count = 1;
            g.teams[t].ever_alive_count = 1;
        }
        g
    }

    #[test]
    fn civilization_names_come_from_the_civ_info() {
        let g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let civ = d.t::<CivilizationType>("CIVILIZATION_BANNOR");
        let short = g.player_get_civilization_short_description(PlayerId(0));
        let adjective = g.player_get_civilization_adjective(PlayerId(0));
        assert_eq!(short, d.text.name(&d.civilizations[civ].short_description_key));
        assert_eq!(adjective, d.text.name(&d.civilizations[civ].adjective_key));
        assert!(!short.is_empty() && !adjective.is_empty());
        // a slot without a civilization has no names
        assert_eq!(g.player_get_civilization_short_description(PlayerId(3)), "");
        assert_eq!(g.player_get_civilization_adjective(PlayerId(3)), "");
    }

    #[test]
    fn population_and_land_bookkeeping() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let p = PlayerId(0);
        // Every input of score v2 (people, land, knowledge, alive, team) marks the score dirty;
        // land plots are covered in plot/ownership.rs.
        g.score_dirty = false;
        g.player_change_total_population(p, 5);
        let pl = &g.players[p];
        assert_eq!((pl.total_population, pl.assets, pl.power, pl.population_score), (5, 10, 2, 5));
        assert!(g.score_dirty);
        g.player_change_total_population(p, -2);
        let pl = &g.players[p];
        // power: -(5 / 2) + (3 / 2) = -1
        assert_eq!((pl.total_population, pl.assets, pl.power, pl.population_score), (3, 6, 1, 3));

        // the legacy score's land (plots owned long enough) still marks it too
        g.score_dirty = false;
        g.player_change_total_land_scored(p, 0);
        assert!(!g.score_dirty);
        g.player_change_total_land_scored(p, 7);
        let pl = &g.players[p];
        assert_eq!((pl.total_land_scored, pl.assets, pl.land_score), (7, 13, 7));
        assert!(g.score_dirty);

        g.score_dirty = false;
        g.player_change_tech_score(p, 3);
        assert_eq!(g.players[p].tech_score, 3);
        assert!(g.score_dirty);
        g.score_dirty = false;
        g.player_change_wonders_score(p, 5);
        assert_eq!(g.players[p].wonders_score, 5);
        assert!(g.score_dirty);

        g.score_dirty = false;
        g.player_set_team(p, TeamId(1));
        assert!(g.score_dirty, "a team change moves knowledge between members");

        g.score_dirty = false;
        g.player_set_alive(PlayerId(1), true);
        assert!(g.score_dirty);

        // no cities: no real population
        assert_eq!(g.player_get_real_population(p), 0);
    }

    /// The legacy (FFH2) getters `getPopScore`/`getLandScore`(bCheckVassal) give a vassal half
    /// of its own and every master member half of the vassal's; score v2 instead moves the
    /// vassal's other half to the master's leader, conserving the totals.
    #[test]
    fn pop_and_land_scores_share_vassal_scores() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")]);
        let (master, vassal) = (PlayerId(0), PlayerId(1));
        g.players[master].population_score = 10;
        g.players[master].land_score = 20;
        g.players[vassal].population_score = 7;
        g.players[vassal].land_score = 9;
        assert_eq!(g.player_get_pop_score(master, true), 10);

        // team 1 becomes a vassal of team 0
        g.teams[TeamId(1)].vassal[TeamId(0)] = true;
        // vassal: half of its own (integer division)
        assert_eq!(g.player_get_pop_score(vassal, true), 3);
        assert_eq!(g.player_get_land_score(vassal, true), 4);
        assert_eq!(g.player_get_pop_score(vassal, false), 7);
        // master: own + (vassal's unchecked score / 2) / members
        assert_eq!(g.player_get_pop_score(master, true), 10 + 3);
        assert_eq!(g.player_get_land_score(master, true), 20 + 4);
        assert_eq!(g.player_get_land_score(master, false), 20);

        // score v2: the vassal keeps floor(7 / 2) = 3 people and floor(9 / 2) = 4 plots, the
        // master's leader gets the rest; nothing is created (17 people, 29 plots in all)
        for (pl, pop, land) in [(master, 10, 20), (vassal, 7, 9)] {
            g.players[pl].total_population = pop;
            g.players[pl].total_land = land;
        }
        let shares = g.world_score_shares();
        assert_eq!((shares[0][0], shares[1][0]), (1_000_000 * 14 / 17, 1_000_000 * 3 / 17));
        assert_eq!((shares[0][1], shares[1][1]), (1_000_000 * 25 / 29, 1_000_000 * 4 / 29));
        let scores = g.world_scores();
        assert_eq!(scores[0] + scores[1], 10_000);
        assert!(scores[0] > scores[1] && scores[1] > 0);

        // a dead vassal does not count
        g.players[vassal].alive = false;
        assert_eq!(g.player_get_pop_score(master, true), 10);
    }

    #[test]
    fn set_team_moves_the_counts() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")]);
        let p = PlayerId(0);
        g.players[p].total_population = 4;
        g.players[p].total_land = 12;
        g.teams[TeamId(0)].total_population = 4;
        g.teams[TeamId(0)].total_land = 12;

        g.player_set_team(p, TeamId(1));
        assert_eq!(g.players[p].team, TeamId(1));
        assert_eq!(g.setup.players[0].team, TeamId(1));
        let (old, new) = (&g.teams[TeamId(0)], &g.teams[TeamId(1)]);
        assert_eq!((old.num_members, old.alive_count, old.ever_alive_count), (0, 0, 0));
        assert_eq!((old.total_population, old.total_land), (0, 0));
        assert_eq!((new.num_members, new.alive_count, new.ever_alive_count), (2, 2, 2));
        assert_eq!((new.total_population, new.total_land), (4, 12));
    }

    #[test]
    fn score_unit_checks_and_launch_without_state() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let p = PlayerId(0);
        // alone in the world: all of it
        assert_eq!(g.player_calculate_score(p, false, false), 10_000);
        // a dead player scores 0 (and its legacy score too, with no Python call)
        g.players[p].alive = false;
        assert_eq!(g.player_calculate_score(p, false, false), 0);
        assert_eq!(g.player_legacy_score(p), 0);
        // no groups: nothing ready / automated / busy
        assert!(!g.player_has_ready_unit(p, true));
        assert!(!g.player_has_auto_unit(p));
        assert!(!g.player_has_busy_unit(p));
        g.player_update_timers(p);
        // a team that cannot launch: no countdown
        let v = VictoryType(0);
        g.player_launch(p, v);
        assert_eq!(g.teams[TeamId(0)].victory_countdown[v], -1);
    }

    #[test]
    fn split_empire_leaders_skip_used_civs_and_leaders() {
        let g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")]);
        let d = g.d;
        let leaders = g.player_get_split_empire_leaders(PlayerId(0));
        assert!(!leaders.is_empty());
        let ljosalfar = d.t::<CivilizationType>("CIVILIZATION_LJOSALFAR");
        let sabathiel = d.t::<LeaderHeadType>("LEADER_SABATHIEL");
        let arendel = d.t::<LeaderHeadType>("LEADER_ARENDEL");
        for &(civ, leader) in &leaders {
            // another ever-alive player's civilization is excluded; our own is not
            assert_ne!(civ, ljosalfar);
            // leaders already in play are excluded
            assert!(leader != sabathiel && leader != arendel);
            assert!(d.leader_heads[leader].favorite_wonder.is_some());
            assert!(d.civilizations[civ].playable && d.civilizations[civ].ai_playable);
        }
        // no colony slot without a capital
        assert!(!g.player_can_split_area(PlayerId(0), AreaId(0)));
        // the first never-alive civ slot is the colony slot
        assert_eq!(g.player_get_split_empire_player(PlayerId(0), AreaId(0)), Some(PlayerId(2)));
    }

    #[test]
    fn init_of_an_open_slot_resets_and_joins_the_team() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")]);
        let open = PlayerId(2);
        assert_eq!(g.setup.players[2].slot_status, SlotStatus::Open);
        // other players' entries about the slot, which the reset clears
        let p0 = PlayerId(0);
        g.players[p0].gold_per_turn_by_player[open] = 5;
        g.players[p0].espionage_spending_weight_against_team[TeamId(2)] = 3;
        g.players[p0].ai_memory_count[open][MemoryType(0)] = 4;
        g.players[p0].ai_first_contact[open] = true;
        g.players[p0].ai_peacetime_trade_value[open] = 9;
        g.players[PlayerId(1)].ai_attitude_extra[open] = -2;
        g.players[PlayerId(1)].ai_gold_traded_to[open] = 8;
        let seed = g.soren_rand.seed();

        g.player_init(open);
        let pl = &g.players[open];
        assert_eq!((pl.id, pl.team), (open, TeamId(2)));
        assert!(!pl.alive && !pl.ever_alive);
        assert_eq!(pl.gold_per_turn_by_player.len(), g.num_players());
        assert_eq!((pl.ai_civic_timer, pl.ai_religion_timer), (0, 0));
        assert_eq!(g.teams[TeamId(2)].num_members, 1);
        assert_eq!(g.players[p0].gold_per_turn_by_player[open], 0);
        assert_eq!(g.players[p0].espionage_spending_weight_against_team[TeamId(2)], 0);
        assert_eq!(g.players[p0].ai_memory_count[open][MemoryType(0)], 0);
        assert!(!g.players[p0].ai_first_contact[open]);
        assert_eq!(g.players[p0].ai_peacetime_trade_value[open], 0);
        assert_eq!(g.players[PlayerId(1)].ai_attitude_extra[open], 0);
        assert_eq!(g.players[PlayerId(1)].ai_gold_traded_to[open], 0);
        // an open slot draws nothing (the peace weight draw is for taken/computer slots)
        assert_eq!(g.soren_rand.seed(), seed);
        // the global counter limit only grows for taken/computer slots
        assert_eq!(g.global_counter_limit, g.d.defines.global_counter_limit_default);
    }

    #[test]
    fn turn_activation_counts_active_players() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let p = PlayerId(0);
        g.num_game_turn_active = 0;
        // elapsed turns 0: starting the turn does not run the units' turn
        g.player_set_turn_active(p, true, true);
        assert!(g.players[p].turn_active && !g.players[p].end_turn);
        assert_eq!(g.num_game_turn_active, 1);
        g.player_set_turn_active(p, true, true);
        assert_eq!(g.num_game_turn_active, 1);
        // ending without bDoTurn: no doTurn, no next player
        g.player_set_turn_active(p, false, false);
        assert!(!g.players[p].turn_active);
        assert_eq!(g.num_game_turn_active, 0);
        assert!(!g.player_is_turn_done(p));
    }

    #[test]
    fn tower_mastery_keeps_the_plan_until_the_tower_exists() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        let alteration = d.t::<BuildingClassType>("BUILDINGCLASS_TOWER_OF_ALTERATION");
        let elements = d.t::<BuildingClassType>("BUILDINGCLASS_TOWER_OF_THE_ELEMENTS");
        assert_eq!(py_ai_tower_mastery(&g, p, 1), 1);
        assert_eq!(py_ai_tower_mastery(&g, p, 4), 4);
        assert_eq!(py_ai_tower_mastery(&g, p, 5), 0);
        g.players[p].building_class_count[alteration] = 1;
        g.players[p].building_class_count[elements] = 1;
        assert_eq!(py_ai_tower_mastery(&g, p, 1), 0);
        assert_eq!(py_ai_tower_mastery(&g, p, 4), 0);
        assert_eq!(py_ai_tower_mastery(&g, p, 2), 2);
    }

    #[test]
    fn reassign_player_advanced_moves_control() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")]);
        let (old, new) = (PlayerId(0), PlayerId(1));
        g.setup.players[0].slot_status = SlotStatus::Taken;
        g.setup.players[0].human = true;
        g.setup.players[0].handicap = Some(HandicapType(1));
        g.setup.players[1].handicap = Some(HandicapType(4));
        g.player_update_human(old);
        let option = PlayerOptionType(2);
        g.players[old].options[option] = true;
        g.active_player = Some(old);
        assert!(g.players[old].human && !g.players[new].human);

        reassign_player_advanced(&mut g, old, new, 6);
        assert!(!g.players[old].human && g.players[new].human);
        // The active player moves with the slot (CvInitCore.cpp:474-482).
        assert_eq!(g.active_player, Some(new));
        assert_eq!(g.setup.players[1].slot_status, SlotStatus::Taken);
        assert_eq!(g.setup.players[0].slot_status, SlotStatus::Computer);
        assert_eq!(g.player_get_handicap_type(new), Some(HandicapType(1)));
        assert_eq!(g.player_get_handicap_type(old), Some(HandicapType(4)));
        assert!(g.players[new].options[option] && !g.players[old].options[option]);
        assert_eq!((g.players[new].temp_player_timer, g.players[new].real_player), (6, Some(old)));

        // timer -1: no temporary player; the barbarian slot is out of bounds (no-op)
        reassign_player_advanced(&mut g, new, old, -1);
        assert!(g.players[old].human && !g.players[new].human);
        assert_eq!(g.active_player, Some(old));
        assert_eq!(g.players[old].temp_player_timer, 0);
        let barbarian = g.barbarian_player();
        reassign_player_advanced(&mut g, old, barbarian, -1);
        assert!(g.players[old].human);
        assert_eq!(g.active_player, Some(old));

        // The active player is the target slot: it moves to the old one (the `else if`).
        reassign_player_advanced(&mut g, new, old, -1);
        assert!(!g.players[old].human && g.players[new].human);
        assert_eq!(g.active_player, Some(new));
        // Neither slot is active (headless: no active player): nothing moves.
        g.active_player = None;
        reassign_player_advanced(&mut g, new, old, -1);
        assert_eq!(g.active_player, None);
    }
}
