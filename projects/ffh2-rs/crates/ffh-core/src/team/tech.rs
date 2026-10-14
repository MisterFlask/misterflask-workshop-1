//! CvTeam: research and techs (cost, progress, `setHasTech`, `processTech`, tech sharing,
//! no-trade techs), bonuses (obsolete, force-revealed), terrain/river trade, irrigation,
//! water work, extra water sight, improvement yields, building effects, turn processing
//! and circumnavigation.

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{civ_team_ids, is_option};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::utils::get_tech_score;
use ffh_data::*;

/// C++ `GET_PLAYER(getLeaderID())` where the C++ dereferences the leader unchecked (a team
/// always has a member slot; C++ would index `NO_PLAYER` otherwise).
fn leader_of(g: &Game, t: TeamId) -> PlayerId {
    g.team_get_leader_id(t).expect("CvTeam::getLeaderID: team without a player")
}

/// Python `CvGameUtils.doHolyCityTech(argsList)`, which `CvTeam::setHasTech` calls
/// unconditionally for the first team to learn a tech. FFH2 returns False (C++ `lResult` 0),
/// i.e. "found religions and corporations the default way".
fn py_do_holy_city_tech(_g: &Game, _team: TeamId, _player: PlayerId, _tech: TechType, _first: bool) -> bool {
    false
}

/// C++ `CvPlayerAI::AI_chooseFreeTech` (called by `setHasTech` for the free techs of a
/// non-human first discoverer): clear the research queue, pick a tech (Python `AI_chooseTech`
/// / `AI_bestTech` -> the controller's `choose_free_tech`) and grant it.
fn ai_choose_free_tech(g: &mut Game, p: PlayerId) {
    g.player_clear_research_queue(p);

    let best_tech = g.controller(p).choose_free_tech(g, p);

    if let Some(best_tech) = best_tech {
        let team = g.players[p].team;
        g.team_set_has_tech(team, best_tech, true, Some(p), true, true);
    }
}

/// `CvTeam::setHasTech`: `updatePlotGroupBonus(bAdd)` for every plot of team `t` whose bonus
/// is revealed, traded or obsoleted by `tech` (run before and after the tech flag changes).
fn update_tech_bonus_plot_groups(g: &mut Game, t: TeamId, tech: TechType, add: bool) {
    let d = g.d;

    for p in g.map.plot_ids() {
        if let Some(bonus) = g.map.plots[p].bonus_type {
            if g.plot_get_team(p) == Some(t) {
                let info = &d.bonuses[bonus];
                if (info.tech_reveal == Some(tech)) || (info.tech_city_trade == Some(tech)) || (info.tech_obsolete == Some(tech)) {
                    g.plot_update_plot_group_bonus(p, add);
                }
            }
        }
    }
}

impl Game {
    // ------------------------------------------------------------------
    // Turn processing
    // ------------------------------------------------------------------

    /// C++ `CvTeam::doTurn`. Its `AI_doTurnPre()` is `team_ai_do_counter` (bookkeeping);
    /// `AI_doTurnPost()` is `team_ai_update_worst_enemy` + AI-only area strategies and war
    /// decisions (`AI_doWar` -> controller).
    pub fn team_do_turn(&mut self, t: TeamId) {
        let d = self.d;

        debug_assert!(self.team_is_alive(t), "isAlive is expected to be true");

        // AI_doTurnPre(): AI_doCounter(), then early returns (human, barbarian, minor) with
        // nothing after them.
        self.team_ai_do_counter(t);

        if self.team_is_barbarian(t) {
            for tech in d.techs.keys() {
                if !self.team_is_has_tech(t, Some(tech)) {
                    let mut count = 0;
                    let mut possible_count = 0;

                    for o in civ_team_ids(self) {
                        if self.team_is_alive(o) {
                            if self.team_is_has_tech(o, Some(tech)) {
                                count += 1;
                            }

                            possible_count += 1;
                        }
                    }

                    if count > 0 {
                        debug_assert!(possible_count > 0, "iPossibleCount is expected to be greater than 0");

                        let change = (self.team_get_research_cost(t, tech) * ((d.defines.barbarian_free_tech_percent * count) / possible_count)) / 100;
                        let leader = leader_of(self, t);
                        self.team_change_research_progress(t, tech, change, leader);
                    }
                }
            }
        }

        for o in self.team_ids() {
            if self.team_is_alive(o) {
                if self.teams[t].stolen_visibility_timer[o] > 0 {
                    self.team_change_stolen_visibility_timer(t, o, -1);
                }

                // changeCounterespionageTurnsLeftAgainstTeam(-1) / setCounterespionageModAgainstTeam(0):
                // the C++ side effects are UI only.
                if self.teams[t].counterespionage_turns_left_against_team[o] > 0 {
                    self.teams[t].counterespionage_turns_left_against_team[o] -= 1;
                }

                if self.teams[t].counterespionage_turns_left_against_team[o] == 0 {
                    self.teams[t].counterespionage_mod_against_team[o] = 0;
                }
            }
        }

        if !self.is_option(GameOptionType::GAMEOPTION_NO_TECH_BROKERING) {
            for tech in d.techs.keys() {
                self.teams[t].no_trade_tech[tech] = false;
            }
        }

        self.team_do_war_weariness(t);

        self.team_test_circumnavigated(t);

        // AI_doTurnPost():
        self.team_ai_update_worst_enemy(t);
        // AI: AI_updateAreaStragies(false) (area AI types, AI-only state).
        // AI: AI_doWar() for non-human, non-barbarian, non-minor teams: war decisions are the
        // controller's (taken in its `play_turn`).
    }

    /// C++ `CvTeam::updateYield` (updates every alive member player).
    pub fn team_update_yield(&mut self, t: TeamId) {
        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    self.player_update_yield(p);
                }
            }
        }
    }

    /// C++ `CvTeam::updatePowerHealth`.
    pub fn team_update_power_health(&mut self, t: TeamId) {
        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    self.player_update_power_health(p);
                }
            }
        }
    }

    /// C++ `CvTeam::updateCommerce`.
    pub fn team_update_commerce(&mut self, t: TeamId) {
        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    self.player_update_commerce(p);
                }
            }
        }
    }

    /// C++ `CvTeam::processBuilding`: team-wide effects of a building (vote source
    /// eligibility, map centering, enemy war weariness, FfH no civic anger).
    pub fn team_process_building(&mut self, t: TeamId, building: BuildingType, change: i32) {
        let d = self.d;
        let info = &d.buildings[building];

        for vote_source in d.vote_sources.keys() {
            if info.vote_source_type == Some(vote_source) {
                // changeForceTeamVoteEligibilityCount
                self.teams[t].force_team_vote_eligibility_count[vote_source] += if info.force_team_vote_eligible { change } else { 0 };
            }
        }

        if info.map_centering {
            if change > 0 {
                // setMapCentering(true): the C++ side effect is UI only.
                self.teams[t].map_centering = true;
            }
        }

        self.teams[t].enemy_war_weariness_modifier += info.enemy_war_weariness_modifier * change;

        //FfH: Added by Kael 11/25/2007
        if info.no_civic_anger {
            if change > 0 {
                self.teams[t].no_civic_anger = true;
            } else {
                self.teams[t].no_civic_anger = false;
            }
        }
        //FfH: End Add
    }

    /// C++ `CvTeam::testCircumnavigated` (protected).
    pub fn team_test_circumnavigated(&mut self, t: TeamId) {
        if self.team_is_barbarian(t) {
            return;
        }

        if !self.circumnavigation_available() {
            return;
        }

        if self.map.wrap_x {
            for x in 0..self.map.grid_width {
                let mut found_visible = false;

                for y in 0..self.map.grid_height {
                    // C++ plotSorenINLINE: always on the map inside the grid.
                    if self.map.plot_soren(x, y).map_or(false, |p| self.map.plots[p].revealed[t]) {
                        found_visible = true;
                        break;
                    }
                }

                if !found_visible {
                    return;
                }
            }
        }

        if self.map.wrap_y {
            for y in 0..self.map.grid_height {
                let mut found_visible = false;

                for x in 0..self.map.grid_width {
                    if self.map.plot_soren(x, y).map_or(false, |p| self.map.plots[p].revealed[t]) {
                        found_visible = true;
                        break;
                    }
                }

                if !found_visible {
                    return;
                }
            }
        }

        // CvGame::makeCircumnavigated
        self.circumnavigated = true;

        if self.elapsed_game_turns > 0 {
            let free_moves = self.d.defines.circumnavigate_free_moves;
            if free_moves != 0 {
                self.teams[t].extra_moves[DomainType::DOMAIN_SEA.idx()] += free_moves;

                // TXT_KEY_MISC_YOU_CIRC_GLOBE / SOMEONE_CIRC_GLOBE / UNKNOWN_CIRC_GLOBE messages and
                // the replay message.
                let turn = self.game_turn();
                self.log.msg(turn, None, || format!("Team {} has circumnavigated the globe (+{} sea moves)", t, free_moves));
            }
        }
    }

    // ------------------------------------------------------------------
    // Research
    // ------------------------------------------------------------------

    /// C++ `CvTeam::getResearchCost`.
    pub fn team_get_research_cost(&self, t: TeamId, tech: TechType) -> i32 {
        let d = self.d;

        let mut cost = d.techs[tech].research_cost;

        cost *= d.handicaps[self.team_get_handicap_type(t)].research_percent;
        cost /= 100;

        cost *= d.worlds[self.setup.world_size].research_percent;
        cost /= 100;

        cost *= d.game_speeds[self.game_speed_type()].research_percent;
        cost /= 100;

        cost *= d.eras[self.start_era()].research_percent;
        cost /= 100;

        cost *= 0.max((d.defines.tech_cost_extra_team_member_modifier * (self.teams[t].num_members - 1)) + 100);
        cost /= 100;

        1.max(cost)
    }

    /// C++ `CvTeam::getResearchLeft`.
    pub fn team_get_research_left(&self, t: TeamId, tech: TechType) -> i32 {
        (self.team_get_research_cost(t, tech) - self.team_get_research_progress(t, Some(tech))).max(0)
    }

    /// C++ `CvTeam::getResearchProgress`: 0 for NO_TECH (`tech = None`).
    pub fn team_get_research_progress(&self, t: TeamId, tech: Option<TechType>) -> i32 {
        match tech {
            Some(tech) => self.teams[t].research_progress[tech],
            None => 0,
        }
    }

    /// C++ `CvTeam::setResearchProgress` (grants the tech when complete).
    pub fn team_set_research_progress(&mut self, t: TeamId, tech: TechType, value: i32, player: PlayerId) {
        if self.team_get_research_progress(t, Some(tech)) != value {
            self.teams[t].research_progress[tech] = value;
            // FAssert(getResearchProgress(eIndex) >= 0) is disabled in FFH: progress can be
            // negative (Sephi's markets).

            // UI: GameData / Score dirty bits for the active team.

            if self.team_get_research_progress(t, Some(tech)) >= self.team_get_research_cost(t, tech) {
                let overflow = (100 * (self.team_get_research_progress(t, Some(tech)) - self.team_get_research_cost(t, tech)))
                    / 1.max(self.player_calculate_research_modifier(player, Some(tech)));
                // CvPlayer::changeOverflowResearch
                self.players[player].overflow_research += overflow;
                self.team_set_has_tech(t, tech, true, Some(player), true, true);
                // !isMPOption(MPOPTION_SIMULTANEOUS_TURNS) always holds (no multiplayer).
                if !self.is_option(GameOptionType::GAMEOPTION_NO_TECH_BROKERING) {
                    self.teams[t].no_trade_tech[tech] = true;
                }
            }
        }
    }

    /// C++ `CvTeam::changeResearchProgress`.
    pub fn team_change_research_progress(&mut self, t: TeamId, tech: TechType, change: i32, player: PlayerId) {
        let v = self.team_get_research_progress(t, Some(tech)) + change;
        self.team_set_research_progress(t, tech, v, player);
    }

    /// C++ `CvTeam::changeResearchProgressPercent`: returns the beakers added.
    pub fn team_change_research_progress_percent(&mut self, t: TeamId, tech: TechType, percent: i32, player: PlayerId) -> i32 {
        let mut beakers = 0;

        if 0 != percent && !self.team_is_has_tech(t, Some(tech)) {
            if percent > 0 {
                beakers = self
                    .team_get_research_left(t, tech)
                    .min((self.team_get_research_cost(t, tech) * percent) / 100);
            } else {
                beakers = (self.team_get_research_left(t, tech) - self.team_get_research_cost(t, tech))
                    .max((self.team_get_research_cost(t, tech) * percent) / 100);
            }

            self.team_change_research_progress(t, tech, beakers, player);
        }

        beakers
    }

    // ------------------------------------------------------------------
    // Techs
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isHasTech`: true for NO_TECH (`tech = None`).
    pub fn team_is_has_tech(&self, t: TeamId, tech: Option<TechType>) -> bool {
        match tech {
            Some(tech) => self.teams[t].has_tech[tech],
            None => true,
        }
    }

    /// C++ `CvTeam::setHasTech`. C++ returns immediately for NO_TECH, so callers holding an
    /// optional tech skip the call. `player = None` (C++ `NO_PLAYER`) means the team leader.
    ///
    /// For the first team to learn a tech (`first`): FfH religion founding (only religions not
    /// yet founded; the member with the lowest `10 + rand(10) + 10 * religions held` (x10 if not
    /// researching the tech) founds it), corporations, the tech's free great person in the
    /// capital and its free techs (AI: `AI_chooseFreeTech` -> controller; humans:
    /// `player_choose_tech`).
    pub fn team_set_has_tech(&mut self, t: TeamId, tech: TechType, value: bool, player: Option<PlayerId>, first: bool, announce: bool) {
        let d = self.d;

        if self.team_is_has_tech(t, Some(tech)) != value {
            // C++ resolves NO_PLAYER to getLeaderID() before this test (a pure read) and uses
            // ePlayer unchecked from here on.
            let player = player.or_else(|| self.team_get_leader_id(t)).expect("CvTeam::setHasTech: team without a player");

            if d.techs[tech].repeat {
                self.teams[t].tech_count[tech] += 1;

                self.team_set_research_progress(t, tech, 0, player);

                // report event to Python
                let first_time = 1 == self.teams[t].tech_count[tech];
                self.ev_tech_acquired(tech, t, Some(player), announce && first_time);

                if first_time {
                    if announce {
                        if self.final_initialized {
                            // UI: announceTechToPlayers(eIndex) (messages only).
                        }
                    }
                }
            } else {
                update_tech_bonus_plot_groups(self, t, tech, false);

                self.teams[t].has_tech[tech] = value;

                update_tech_bonus_plot_groups(self, t, tech, true);
            }

            self.team_process_tech(t, tech, if value { 1 } else { -1 });

            if self.team_is_has_tech(t, Some(tech)) {
                for p in self.player_ids() {
                    if self.players[p].team == t {
                        // C++ compares with getEra() (-1 for NO_ERA, never greater).
                        if let Some(era) = d.techs[tech].era {
                            if self.players[p].current_era < era {
                                // setCurrentEra: the C++ side effects are graphics only.
                                self.players[p].current_era = era;
                            }
                        }
                    }
                }

                if d.techs[tech].map_visible {
                    self.map_set_revealed_plots(t, true, true);
                }

                for special_building in d.special_buildings.keys() {
                    if Some(tech) == d.special_buildings[special_building].tech_prereq_anyone {
                        self.make_special_building_valid(special_building, announce);
                    }
                }

                // report event to Python, along with some other key state
                self.ev_tech_acquired(tech, t, Some(player), announce);

                // C++ bReligionFounded: read only by the (UI) civic-change popup below.
                let mut _religion_founded = false;
                let mut first_bonus = false;

                if first {
                    if self.count_known_tech_num_teams(tech) == 1 {
                        // C++ `if (lResult != 1)` with the Python result (False -> 0).
                        if !py_do_holy_city_tech(self, t, player, tech, first) {
                            for religion in d.religions.keys() {
                                if d.religions[religion].tech_prereq == Some(tech)
                                    //FfH: Added by Kael 10/10/2008
                                    && !self.is_religion_founded(religion)
                                //FfH: End Add
                                {
                                    let mut best_value = MAX_INT;
                                    let mut best_player = None;

                                    for j in self.player_ids() {
                                        if self.players[j].alive {
                                            if self.players[j].team == t {
                                                let mut v = 10;

                                                v += self.soren_rand_num(10, "Found Religion (Player)");

                                                for k in d.religions.keys() {
                                                    v += self.players[j].has_religion_count[k] * 10;
                                                }

                                                if self.player_get_current_research(j) != Some(tech) {
                                                    v *= 10;
                                                }

                                                if v < best_value {
                                                    best_value = v;
                                                    best_player = Some(j);
                                                }
                                            }
                                        }
                                    }

                                    if let Some(best_player) = best_player {
                                        // CvGame::setReligionSlotTaken
                                        self.religion_slot_taken[religion] = true;

                                        if self.is_option(GameOptionType::GAMEOPTION_PICK_RELIGION) {
                                            // Never true in FFH (CvGame::isOption locks PICK_RELIGION off).
                                            // C++: humans get BUTTONPOPUP_FOUND_RELIGION (their pick is founded in
                                            // this religion's slot), AIs pick with AI_chooseReligion(); both are
                                            // the controller's decision here.
                                            let chosen = self.controller(best_player).choose_religion(self, best_player);
                                            if let Some(chosen) = chosen {
                                                self.player_found_religion(best_player, Some(chosen), religion, true);
                                            }
                                        } else {
                                            self.player_found_religion(best_player, Some(religion), religion, true);
                                        }

                                        _religion_founded = true;
                                        first_bonus = true;
                                    }
                                }
                            }

                            for corporation in d.corporations.keys() {
                                if d.corporations[corporation].tech_prereq == Some(tech) {
                                    if !self.is_corporation_founded(corporation) {
                                        let mut best_value = MAX_INT;
                                        let mut best_player = None;

                                        for j in self.player_ids() {
                                            if self.players[j].alive {
                                                if self.players[j].team == t {
                                                    let mut v = 10;

                                                    v += self.soren_rand_num(10, "Found Corporation (Player)");

                                                    if self.player_get_current_research(j) != Some(tech) {
                                                        v *= 10;
                                                    }

                                                    if v < best_value {
                                                        best_value = v;
                                                        best_player = Some(j);
                                                    }
                                                }
                                            }
                                        }

                                        if let Some(best_player) = best_player {
                                            self.player_found_corporation(best_player, corporation);
                                            first_bonus = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                for p in self.player_ids() {
                    if self.players[p].team == t {
                        if self.players[p].researching_tech[tech] {
                            self.player_pop_research(p, tech);
                        }

                        // AI: AI_nowHasTech(eIndex) (non-human players re-choose production at most
                        // every 10 turns; production is the controller's decision).

                        self.player_invalidate_yield_rank_cache(p, None);
                    }
                }

                if first {
                    if self.count_known_tech_num_teams(tech) == 1 {
                        let free_unit = self.player_get_tech_free_unit(player, tech);
                        if let Some(free_unit) = free_unit {
                            first_bonus = true;

                            if let Some(capital_city) = self.player_get_capital_city(player) {
                                self.city_create_great_people(capital_city, free_unit, false, false);
                            }
                        }

                        let first_free_techs = d.techs[tech].first_free_techs;
                        if first_free_techs > 0 {
                            first_bonus = true;

                            if !self.team_is_human(t) {
                                for _ in 0..first_free_techs {
                                    ai_choose_free_tech(self, player);
                                }
                            } else {
                                // TXT_KEY_MISC_FIRST_TECH_CHOOSE_FREE tech chooser (CvPlayer::chooseTech): a
                                // popup, which CvPlayer::addPopup drops for a non-human ePlayer (an AI
                                // member of a human team gets no free tech).
                                if self.players[player].human {
                                    self.player_choose_tech(player, first_free_techs);
                                }
                            }

                            // TXT_KEY_MISC_SOMEONE_FIRST_TO_TECH / UNKNOWN_FIRST_TO_TECH messages to every
                            // alive player and the replay message.
                            let turn = self.game_turn();
                            self.log.msg(turn, Some(player), || {
                                format!("Player {} was the first to discover {}", player, d.text.name(&d.techs[tech].base.description))
                            });
                        }

                        if first_bonus {
                            for p in self.player_ids() {
                                if self.players[p].alive {
                                    if !self.players[p].human {
                                        if self.players[p].researching_tech[tech] {
                                            self.player_clear_research_queue(p);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if announce {
                    if self.final_initialized {
                        // UI: announceTechToPlayers(eIndex) and TXT_KEY_MISC_YOU_DISCOVERED_BONUS messages
                        // for the team's plots whose bonus this tech reveals.
                        // UI: BUTTONPOPUP_CHANGECIVIC for human members that can revolt and lack a civic
                        // this tech enables (unless they just founded a religion without a state
                        // religion; FfH: only if canDoCivics); the checks are read-only.
                    }
                }

                for o in self.team_ids() {
                    if self.team_is_alive(o) {
                        if o != t {
                            self.team_update_tech_share_tech(o, tech);
                        }
                    }
                }
            }

            if value {
                if announce {
                    if self.final_initialized {
                        if self.player_is_research(player) && self.player_get_current_research(player).is_none() {
                            // TXT_KEY_MISC_WHAT_TO_RESEARCH_NEXT tech chooser (CvPlayer::chooseTech): a
                            // popup, which CvPlayer::addPopup drops for non-human players (AI players
                            // choose their research in their own turn).
                            if self.players[player].human {
                                self.player_choose_tech(player, 0);
                            }
                        }
                    }
                }
            }

            // UI: MiscButtons / SelectionButtons / ResearchButtons / GlobeLayer dirty bits.
        }
    }

    /// C++ `CvTeam::processTech` (protected): apply (`change = 1`) or remove (`-1`) a tech's effects.
    pub fn team_process_tech(&mut self, t: TeamId, tech: TechType, change: i32) {
        let d = self.d;
        let info = &d.techs[tech];

        if info.extra_water_see_from {
            self.team_change_extra_water_see_from_count(t, change);
        }

        if info.map_centering {
            if change > 0 {
                // setMapCentering(true): the C++ side effect is UI only.
                self.teams[t].map_centering = true;
            }
        }

        if info.map_trading {
            self.teams[t].map_trading_count += change;
        }

        if info.tech_trading {
            self.teams[t].tech_trading_count += change;
        }

        if info.gold_trading {
            self.teams[t].gold_trading_count += change;
        }

        if info.open_borders_trading {
            self.teams[t].open_borders_trading_count += change;
        }

        if info.defensive_pact_trading {
            self.teams[t].defensive_pact_trading_count += change;
        }

        if info.permanent_alliance_trading {
            self.teams[t].permanent_alliance_trading_count += change;
        }

        if info.vassal_state_trading {
            self.teams[t].vassal_trading_count += change;
        }

        if info.bridge_building {
            // changeBridgeBuildingCount: the C++ side effect is a graphics refresh only.
            self.teams[t].bridge_building_count += change;
        }

        if info.irrigation {
            self.team_change_irrigation_count(t, change);
        }

        if info.ignore_irrigation {
            self.teams[t].ignore_irrigation_count += change;
        }

        if info.water_work {
            self.team_change_water_work_count(t, change);
        }

        for route in d.routes.keys() {
            self.teams[t].route_change[route] += d.routes[route].tech_movement_change[tech] * change;
        }

        for i in 0..DomainType::NUM {
            self.teams[t].extra_moves[i] += info.domain_extra_moves[DomainType::from_usize(i)] * change;
        }

        for i in 0..CommerceType::NUM {
            if info.commerce_flexible[i] {
                // changeCommerceFlexibleCount: the C++ side effect is UI only.
                self.teams[t].commerce_flexible_count[i] += change;
            }
        }

        for terrain in d.terrains.keys() {
            if info.terrain_trade[terrain] {
                self.team_change_terrain_trade_count(t, terrain, change);
            }
        }

        if info.river_trade {
            self.team_change_river_trade_count(t, change);
        }

        for building in d.buildings.keys() {
            if d.buildings[building].obsolete_tech == Some(tech) {
                self.team_change_obsolete_building_count(t, building, change);
            }

            if let Some(special_building) = d.buildings[building].special_building_type {
                if d.special_buildings[special_building].obsolete_tech == Some(tech) {
                    self.team_change_obsolete_building_count(t, building, change);
                }
            }
        }

        for improvement in d.improvements.keys() {
            for i in 0..YieldType::NUM {
                let yield_change = d.improvements[improvement].tech_yield_changes[tech][i] * change;
                self.team_change_improvement_yield_change(t, improvement, YieldType::from_usize(i), yield_change);
            }
        }

        for p in self.player_ids() {
            if self.players[p].team == t {
                // changeFeatureProductionModifier / changeWorkerSpeedModifier / changeAssets /
                // changePower are plain counters.
                self.players[p].feature_production_modifier += info.feature_production_modifier * change;
                self.players[p].worker_speed_modifier += info.worker_speed_modifier * change;
                self.player_change_trade_routes(p, info.trade_routes * change);
                self.player_change_extra_health(p, info.health * change);
                self.player_change_extra_happiness(p, info.happiness * change);

                self.players[p].assets += info.asset_value * change;
                self.players[p].power += info.power_value * change;
                self.player_change_tech_score(p, get_tech_score(d, tech) * change);
            }
        }

        for p in self.map.plot_ids() {
            if let Some(bonus) = self.map.plots[p].bonus_type {
                if d.bonuses[bonus].tech_reveal == Some(tech) {
                    self.plot_update_yield(p);
                    // UI: setLayoutDirty(true)
                }
            }
        }

        // C++ quirk: the plot loop below reuses the build loop's counter `iI`, so after the
        // first route build enabled by this tech the build loop goes on at index
        // numPlots + 1, i.e. it normally ends there (city routes are updated once).
        let num_builds = d.builds.len() as i32;
        let num_plots = self.map.num_plots() as i32;
        let mut i: i32 = 0;
        while i < num_builds {
            let build = BuildType::from_usize(i as usize);
            if d.builds[build].tech_prereq == Some(tech) {
                if d.builds[build].route.is_some() {
                    i = 0;
                    while i < num_plots {
                        let p = PlotIdx(i as u32);

                        // CvPlot::getPlotCity (NULL for a deleted city)
                        let city = self.map.plots[p].plot_city.filter(|&c| self.cities.contains(c));

                        if let Some(city) = city {
                            if self.city_team(city) == t {
                                self.plot_update_city_route(p, true);
                            }
                        }

                        i += 1;
                    }
                }
            }

            i += 1;
        }

        for p in self.player_ids() {
            if self.players[p].team == t {
                self.player_update_corporation(p);
            }
        }
    }

    /// C++ `CvTeam::shareItems`: on a permanent alliance, take over `other`'s techs, forced
    /// revealed bonuses, espionage points and team-shared buildings.
    pub fn team_share_items(&mut self, t: TeamId, other: TeamId) {
        let d = self.d;

        debug_assert!(other != t);

        for tech in d.techs.keys() {
            if self.team_is_has_tech(other, Some(tech)) {
                self.team_set_has_tech(t, tech, true, None, true, false);
            }
        }

        for bonus in d.bonuses.keys() {
            if self.team_is_force_revealed_bonus(other, bonus) {
                self.team_set_force_revealed_bonus(t, bonus, true);
            }
        }

        for o in self.team_ids() {
            let points = self.teams[other].espionage_points_against_team[o].max(self.teams[t].espionage_points_against_team[o]);
            self.team_set_espionage_points_against_team(t, o, points);
        }

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == other {
                    for c in self.players[p].cities.clone() {
                        for building in d.buildings.keys() {
                            if self.city_num_building(c, building) > 0 {
                                if !self.team_is_obsolete_building(t, building) {
                                    if d.buildings[building].team_share {
                                        for k in self.player_ids() {
                                            if self.players[k].team == t {
                                                let num = self.city_num_building(c, building);
                                                let area = self.city_area(c);
                                                self.player_process_building(k, building, num, area);
                                            }
                                        }
                                    }

                                    let num = self.city_num_building(c, building);
                                    self.team_process_building(t, building, num);
                                }
                            }
                        }
                    }
                }
            }
        }

        // AI_updateBonusValue() for every alive member player of `other` (the bonus value cache
        // read by AI_foundValue).
        for p in self.player_ids() {
            if self.players[p].alive && self.players[p].team == other {
                self.player_ai_update_bonus_value(p);
            }
        }
    }

    /// C++ `CvTeam::isTechShare`.
    pub fn team_is_tech_share(&self, t: TeamId, index: usize) -> bool {
        self.teams[t].tech_share_count[index] > 0
    }

    /// C++ `CvTeam::changeTechShareCount`.
    pub fn team_change_tech_share_count(&mut self, t: TeamId, index: usize, change: i32) {
        if change != 0 {
            self.teams[t].tech_share_count[index] += change;

            if self.team_is_tech_share(t, index) {
                self.team_update_tech_share(t);
            }
        }
    }

    /// C++ `CvTeam::updateTechShare(TechTypes)` (protected overload for one tech).
    pub fn team_update_tech_share_tech(&mut self, t: TeamId, tech: TechType) {
        if self.team_is_has_tech(t, Some(tech)) {
            return;
        }

        let mut best_share = MAX_INT;

        for i in 0..self.num_teams() {
            if self.team_is_tech_share(t, i) {
                best_share = best_share.min(i as i32 + 1);
            }
        }

        if best_share != MAX_INT {
            let mut count = 0;

            for o in civ_team_ids(self) {
                if self.team_is_alive(o) {
                    if self.team_is_has_tech(o, Some(tech)) {
                        if self.teams[t].has_met[o] {
                            count += 1;
                        }
                    }
                }
            }

            if count >= best_share {
                self.team_set_has_tech(t, tech, true, None, true, true);
            }
        }
    }

    /// C++ `CvTeam::updateTechShare()` (protected overload for all techs).
    pub fn team_update_tech_share(&mut self, t: TeamId) {
        for tech in self.d.techs.keys() {
            self.team_update_tech_share_tech(t, tech);
        }
    }

    // ------------------------------------------------------------------
    // Bonuses
    // ------------------------------------------------------------------

    /// C++ `CvTeam::hasBonus` (any member player has the bonus).
    pub fn team_has_bonus(&self, t: TeamId, bonus: BonusType) -> bool {
        for p in self.player_ids() {
            if self.players[p].team == t {
                if self.player_has_bonus(p, bonus) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvTeam::isBonusObsolete`.
    pub fn team_is_bonus_obsolete(&self, t: TeamId, bonus: BonusType) -> bool {
        let obsolete_tech = self.d.bonuses[bonus].tech_obsolete;
        if obsolete_tech.is_some() && self.team_is_has_tech(t, obsolete_tech) {
            return true;
        }
        false
    }

    /// C++ `CvTeam::setForceRevealedBonus`.
    pub fn team_set_force_revealed_bonus(&mut self, t: TeamId, bonus: BonusType, revealed: bool) {
        if self.team_is_force_revealed_bonus(t, bonus) == revealed {
            return;
        }

        for p in self.map.plot_ids() {
            if self.map.plots[p].bonus_type == Some(bonus) {
                if self.plot_get_team(p) == Some(t) {
                    self.plot_update_plot_group_bonus(p, false);
                }
            }
        }

        if revealed {
            self.teams[t].revealed_bonuses.push(bonus);
        } else if let Some(i) = self.teams[t].revealed_bonuses.iter().position(|&b| b == bonus) {
            self.teams[t].revealed_bonuses.remove(i);
        }

        for p in self.map.plot_ids() {
            if self.map.plots[p].bonus_type == Some(bonus) {
                if self.plot_get_team(p) == Some(t) {
                    self.plot_update_plot_group_bonus(p, true);
                }
            }
        }

        for p in self.map.plot_ids() {
            if self.map.plots[p].bonus_type == Some(bonus) {
                self.plot_update_yield(p);
                // UI: setLayoutDirty(true)
            }
        }
    }

    /// C++ `CvTeam::isForceRevealedBonus`.
    pub fn team_is_force_revealed_bonus(&self, t: TeamId, bonus: BonusType) -> bool {
        self.teams[t].revealed_bonuses.contains(&bonus)
    }

    // ------------------------------------------------------------------
    // Terrain, routes, irrigation, sight
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isTerrainTrade`.
    pub fn team_is_terrain_trade(&self, t: TeamId, terrain: TerrainType) -> bool {
        self.teams[t].terrain_trade_count[terrain] > 0
    }

    /// C++ `CvTeam::changeTerrainTradeCount` (updates member players' plot groups).
    pub fn team_change_terrain_trade_count(&mut self, t: TeamId, terrain: TerrainType, change: i32) {
        if change != 0 {
            self.teams[t].terrain_trade_count[terrain] += change;
            debug_assert!(self.teams[t].terrain_trade_count[terrain] >= 0);

            for p in self.player_ids() {
                if self.players[p].alive {
                    if self.players[p].team == t {
                        self.player_update_plot_groups(p);
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::isRiverTrade`.
    pub fn team_is_river_trade(&self, t: TeamId) -> bool {
        self.teams[t].river_trade_count > 0
    }

    /// C++ `CvTeam::changeRiverTradeCount` (updates member players' plot groups).
    pub fn team_change_river_trade_count(&mut self, t: TeamId, change: i32) {
        if change != 0 {
            self.teams[t].river_trade_count += change;
            debug_assert!(self.teams[t].river_trade_count >= 0);

            for p in self.player_ids() {
                if self.players[p].alive {
                    if self.players[p].team == t {
                        self.player_update_plot_groups(p);
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::isBridgeBuilding`. (`changeBridgeBuildingCount` is a plain
    /// `bridge_building_count += n`; its only side effect is a graphics refresh.)
    pub fn team_is_bridge_building(&self, t: TeamId) -> bool {
        self.teams[t].bridge_building_count > 0
    }

    /// C++ `CvTeam::isIrrigation`.
    pub fn team_is_irrigation(&self, t: TeamId) -> bool {
        self.teams[t].irrigation_count > 0
    }

    /// C++ `CvTeam::changeIrrigationCount` (calls `CvMap::updateIrrigated`).
    pub fn team_change_irrigation_count(&mut self, t: TeamId, change: i32) {
        if change != 0 {
            self.teams[t].irrigation_count += change;
            debug_assert!(self.teams[t].irrigation_count >= 0);

            self.map_update_irrigated();
        }
    }

    /// C++ `CvTeam::isIgnoreIrrigation`.
    pub fn team_is_ignore_irrigation(&self, t: TeamId) -> bool {
        self.teams[t].ignore_irrigation_count > 0
    }

    /// C++ `CvTeam::isWaterWork`.
    pub fn team_is_water_work(&self, t: TeamId) -> bool {
        self.teams[t].water_work_count > 0
    }

    /// C++ `CvTeam::changeWaterWorkCount`.
    pub fn team_change_water_work_count(&mut self, t: TeamId, change: i32) {
        if change != 0 {
            self.teams[t].water_work_count += change;

            self.team_ai_make_assign_work_dirty(t);
        }
    }

    /// C++ `CvTeamAI::AI_makeAssignWorkDirty`: mark every alive member player's cities for
    /// citizen re-assignment (governor bookkeeping, see `governor.rs`).
    pub fn team_ai_make_assign_work_dirty(&mut self, t: TeamId) {
        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    self.player_ai_make_assign_work_dirty(p);
                }
            }
        }
    }

    /// C++ `CvTeam::isExtraWaterSeeFrom`.
    pub fn team_is_extra_water_see_from(&self, t: TeamId) -> bool {
        self.teams[t].extra_water_see_from_count > 0
    }

    /// C++ `CvTeam::changeExtraWaterSeeFromCount` (recomputes sight around the change).
    pub fn team_change_extra_water_see_from_count(&mut self, t: TeamId, change: i32) {
        if change != 0 {
            self.map_update_sight(false);

            self.teams[t].extra_water_see_from_count += change;
            debug_assert!(self.teams[t].extra_water_see_from_count >= 0);

            self.map_update_sight(true);
        }
    }

    /// C++ `CvTeam::changeImprovementYieldChange`.
    pub fn team_change_improvement_yield_change(&mut self, t: TeamId, improvement: ImprovementType, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.teams[t].improvement_yield_change[improvement][yield_type.idx()] += change;

            self.team_update_yield(t);
        }
    }

    // ------------------------------------------------------------------
    // Buildings
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isObsoleteBuilding`.
    pub fn team_is_obsolete_building(&self, t: TeamId, building: BuildingType) -> bool {
        self.teams[t].obsolete_building_count[building] > 0
    }

    /// C++ `CvTeam::changeObsoleteBuildingCount` (re-processes the building in member cities).
    pub fn team_change_obsolete_building_count(&mut self, t: TeamId, building: BuildingType, change: i32) {
        if change != 0 {
            let old_obsolete_building = self.team_is_obsolete_building(t, building);

            self.teams[t].obsolete_building_count[building] += change;
            debug_assert!(self.teams[t].obsolete_building_count[building] >= 0);

            if old_obsolete_building != self.team_is_obsolete_building(t, building) {
                for p in self.player_ids() {
                    if self.players[p].alive {
                        if self.players[p].team == t {
                            for c in self.players[p].cities.clone() {
                                if self.city_num_building(c, building) > 0 {
                                    let num = self.city_num_building(c, building);
                                    let num = if self.team_is_obsolete_building(t, building) { -num } else { num };
                                    self.city_process_building(c, building, num, true);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use crate::setup::GameSetup;

    /// A game with `n` alive civs, each on its own team (other slots reset and dead).
    fn game(n: usize) -> Game {
        let d = ffh_data::global();
        let setup = GameSetup::standard(d, n, "WORLDSIZE_DUEL", 7);
        let mut g = Game::new(d, setup);
        let (n_players, n_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], n_players, n_teams);
        }
        for i in 0..n {
            let p = PlayerId(i as u8);
            g.players[p].alive = true;
            g.players[p].ever_alive = true;
            let t = g.players[p].team;
            g.teams[t].num_members += 1;
            g.teams[t].alive_count += 1;
            g.teams[t].ever_alive_count += 1;
        }
        g
    }

    /// Always researches the same tech (`Controller::choose_research`).
    struct Researches(TechType);

    impl crate::controller::Controller for Researches {
        fn choose_research(&self, _g: &Game, _p: PlayerId) -> Option<TechType> {
            Some(self.0)
        }
    }

    #[test]
    fn only_human_players_pick_their_next_research_when_a_tech_arrives() {
        // C++ setHasTech ends with ePlayer's chooseTech(0) when it has nothing to research: a
        // popup, which CvPlayer::addPopup drops for non-human players.
        for human in [false, true] {
            let mut g = game(1);
            let d = g.d;
            let (p, t) = (PlayerId(0), TeamId(0));
            g.setup.players[0].civ = Some(d.t::<CivilizationType>("CIVILIZATION_BANNOR"));
            g.setup.players[0].leader = Some(d.t::<LeaderHeadType>("LEADER_SABATHIEL"));
            g.players[p].human = human;
            g.players[p].founded_first_city = true;
            g.final_initialized = true;
            let startable: Vec<TechType> = d
                .techs
                .iter_enumerated()
                .filter(|&(tech, info)| {
                    info.prereq_and_techs.is_empty() && info.prereq_or_techs.is_empty() && !info.repeat && g.player_can_ever_research(p, tech)
                })
                .map(|(tech, _)| tech)
                .collect();
            let (learned, next) = (startable[0], startable[1]);
            g.controllers[p] = std::sync::Arc::new(Researches(next));

            g.team_set_has_tech(t, learned, true, Some(p), false, true);
            assert!(g.team_is_has_tech(t, Some(learned)));
            assert_eq!(g.player_get_current_research(p), if human { Some(next) } else { None });
        }
    }

    #[test]
    fn building_no_civic_anger_follows_the_last_change() {
        let mut g = game(1);
        let d = g.d;
        let t = TeamId(0);
        let (building, info) = d.buildings.iter_enumerated().find(|(_, b)| b.no_civic_anger).expect("a no-civic-anger building");
        let before = g.teams[t].force_team_vote_eligibility_count.clone();

        g.team_process_building(t, building, 1);
        assert!(g.teams[t].no_civic_anger);
        assert_eq!(g.teams[t].enemy_war_weariness_modifier, info.enemy_war_weariness_modifier);
        g.team_process_building(t, building, 1);
        g.team_process_building(t, building, -1);
        // FfH: a flag, not a count -- removing one copy clears it
        assert!(!g.teams[t].no_civic_anger);
        assert_eq!(g.teams[t].enemy_war_weariness_modifier, info.enemy_war_weariness_modifier);
        if info.vote_source_type.is_none() {
            assert_eq!(g.teams[t].force_team_vote_eligibility_count, before);
        }
    }

    #[test]
    fn bonus_obsolete_follows_the_obsolete_tech() {
        let mut g = game(1);
        let d = g.d;
        let t = TeamId(0);
        for bonus in d.bonuses.keys() {
            assert!(!g.team_is_bonus_obsolete(t, bonus));
        }
        g.teams[t].has_tech.fill(true);
        for (bonus, info) in d.bonuses.iter_enumerated() {
            assert_eq!(g.team_is_bonus_obsolete(t, bonus), info.tech_obsolete.is_some());
        }
    }

    #[test]
    fn force_revealed_bonuses_keep_their_order() {
        // (no map: no plot to update)
        let mut g = game(1);
        let t = TeamId(0);
        let (b1, b2) = (BonusType(1), BonusType(0));
        g.team_set_force_revealed_bonus(t, b1, true);
        g.team_set_force_revealed_bonus(t, b2, true);
        g.team_set_force_revealed_bonus(t, b1, true);
        assert_eq!(g.teams[t].revealed_bonuses, vec![b1, b2]);
        assert!(g.team_is_force_revealed_bonus(t, b2));

        g.team_set_force_revealed_bonus(t, b1, false);
        assert_eq!(g.teams[t].revealed_bonuses, vec![b2]);
        assert!(!g.team_is_force_revealed_bonus(t, b1));
    }

    #[test]
    fn obsolete_building_is_a_count() {
        // (no cities: nothing to re-process)
        let mut g = game(1);
        let t = TeamId(0);
        let building = BuildingType(0);
        g.team_change_obsolete_building_count(t, building, 1);
        g.team_change_obsolete_building_count(t, building, 1);
        assert!(g.team_is_obsolete_building(t, building));
        g.team_change_obsolete_building_count(t, building, -1);
        assert!(g.team_is_obsolete_building(t, building));
        g.team_change_obsolete_building_count(t, building, -1);
        assert!(!g.team_is_obsolete_building(t, building));
    }

    #[test]
    fn irrigation_and_water_work_counts() {
        // (no map, no cities: the irrigation and citizen refreshes have nothing to do)
        let mut g = game(1);
        let t = TeamId(0);
        g.team_change_irrigation_count(t, 1);
        assert!(g.team_is_irrigation(t));
        g.team_change_irrigation_count(t, -1);
        assert!(!g.team_is_irrigation(t));
        g.team_change_water_work_count(t, 1);
        assert!(g.team_is_water_work(t));
    }
}
