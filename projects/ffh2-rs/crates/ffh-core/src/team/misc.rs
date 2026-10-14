//! CvTeam: membership and identity (alive, human, minor, leader, name), aggregates over
//! member players (power, assets, votes, area counts), trading-permission counts,
//! projects, unit/building class limits, victory, espionage, stolen visibility, and team
//! merging on a permanent alliance (`addTeam`, `shareCounters`).

use super::{civ_team_ids, is_option};
use crate::game::Game;
use crate::ids::*;
use crate::utils::{is_team_project, is_team_unit_class, is_team_wonder_class};
use ffh_data::*;

/// The trade items `CvTeam::addTeam` cancels between the merging teams (a deal holding one
/// of them is killed): open borders, defensive pact, peace treaty, vassal, surrender.
fn is_ended_by_alliance(item: TradeableItem) -> bool {
    item == TradeableItem::TRADE_OPEN_BORDERS
        || item == TradeableItem::TRADE_DEFENSIVE_PACT
        || item == TradeableItem::TRADE_PEACE_TREATY
        || item == TradeableItem::TRADE_VASSAL
        || item == TradeableItem::TRADE_SURRENDER
}

impl Game {
    // ------------------------------------------------------------------
    // Team merging (permanent alliances)
    // ------------------------------------------------------------------

    /// C++ `CvTeam::addTeam`: merge team `other` into `t` (permanent alliance). Kills the
    /// deals between the two teams that hold a diplomatic item, shares techs/bonuses/espionage
    /// and team buildings both ways (`shareItems`), unifies contacts, wars, permanent
    /// war/peace, open borders, defensive pacts, forced peace and vassal relations, shares the
    /// counters both ways (`shareCounters`), moves `other`'s players into `t`, merges
    /// visibility and revealed plots, and averages the other teams' counters about the two.
    pub fn team_add_team(&mut self, t: TeamId, other: TeamId) {
        debug_assert!(other != t, "CvTeam::addTeam: eTeam == getID()");

        // UI: TXT_KEY_MISC_PLAYER_PERMANENT_ALLIANCE message to every alive player of a third
        // team that has met both teams, and as a replay message.
        if self.log.enabled {
            let turn = self.game_turn();
            let leader = self.team_get_leader_id(t);
            let text = format!("{} and {} have signed a Permanent Alliance!", self.team_get_name(t), self.team_get_name(other));
            self.log.msg(turn, leader, || text);
        }

        // Deals between the two teams that hold a diplomatic item are cancelled.
        for deal in self.deals.ids() {
            // CvDeal::kill can end (delete) other deals.
            if !self.deals.contains(deal) {
                continue;
            }

            let first_team = self.players[self.deals[deal].first_player].team;
            let second_team = self.players[self.deals[deal].second_player].team;

            if (first_team == t && second_team == other) || (first_team == other && second_team == t) {
                let mut valid = true;

                for trade in &self.deals[deal].first_trades {
                    if is_ended_by_alliance(trade.item_type) {
                        valid = false;
                    }
                }

                for trade in &self.deals[deal].second_trades {
                    if is_ended_by_alliance(trade.item_type) {
                        valid = false;
                    }
                }

                if !valid {
                    self.deal_kill(deal, true);
                }
            }
        }

        self.team_share_items(t, other);
        self.team_share_items(other, t);

        for i in self.team_ids() {
            if i != t && i != other {
                if self.team_is_alive(i) {
                    if self.teams[other].has_met[i] {
                        self.team_meet(t, i, false);
                    } else if self.teams[t].has_met[i] {
                        self.team_meet(other, i, false);
                    }
                }
            }
        }

        for i in self.team_ids() {
            if i != t && i != other {
                if self.team_is_alive(i) {
                    if self.teams[other].at_war[i] {
                        let war_plan = self.teams[other].ai_war_plan[i];
                        self.team_declare_war(t, i, false, war_plan);
                    } else if self.teams[t].at_war[i] {
                        let war_plan = self.teams[t].ai_war_plan[i];
                        self.team_declare_war(other, i, false, war_plan);
                    }
                }
            }
        }

        for i in self.team_ids() {
            if i != t && i != other {
                if self.team_is_alive(i) {
                    // setPermanentWarPeace: plain store.
                    if self.teams[other].permanent_war_peace[i] {
                        self.teams[t].permanent_war_peace[i] = true;
                    } else if self.teams[t].permanent_war_peace[i] {
                        self.teams[other].permanent_war_peace[i] = true;
                    }
                }
            }
        }

        for i in self.team_ids() {
            if i != t && i != other {
                if self.team_is_alive(i) {
                    if self.team_is_open_borders(other, i) {
                        self.team_set_open_borders(t, i, true);
                        self.team_set_open_borders(i, t, true);
                    } else if self.team_is_open_borders(t, i) {
                        self.team_set_open_borders(other, i, true);
                        self.team_set_open_borders(i, other, true);
                    }
                }
            }
        }

        for i in self.team_ids() {
            if i != t && i != other {
                if self.team_is_alive(i) {
                    if self.teams[other].defensive_pact[i] {
                        self.team_set_defensive_pact(t, i, true);
                        self.team_set_defensive_pact(i, t, true);
                    } else if self.teams[t].defensive_pact[i] {
                        self.team_set_defensive_pact(other, i, true);
                        self.team_set_defensive_pact(i, other, true);
                    }
                }
            }
        }

        for i in self.team_ids() {
            if i != t && i != other {
                if self.team_is_alive(i) {
                    if self.teams[other].force_peace[i] {
                        self.team_set_force_peace(t, i, true);
                        self.team_set_force_peace(i, t, true);
                    } else if self.teams[t].force_peace[i] {
                        self.team_set_force_peace(other, i, true);
                        self.team_set_force_peace(i, other, true);
                    }
                }
            }
        }

        for i in self.team_ids() {
            if i != t && i != other {
                if self.team_is_alive(i) {
                    if self.teams[other].vassal[i] {
                        // C++ quirk: passes this team's isCapitulated() (FAsserts that this team
                        // is a vassal), not that of `other`, whose vassal agreement is copied.
                        let capitulated = self.teams[t].capitulated;
                        self.team_set_vassal(t, i, true, capitulated);
                    } else if self.teams[t].vassal[i] {
                        let capitulated = self.teams[t].capitulated;
                        self.team_set_vassal(other, i, true, capitulated);
                    }
                }
            }
        }

        for i in self.team_ids() {
            if i != t && i != other {
                if self.team_is_alive(i) {
                    if self.teams[i].vassal[other] {
                        let capitulated = self.teams[i].capitulated;
                        self.team_set_vassal(i, t, true, capitulated);
                    } else if self.teams[i].vassal[t] {
                        let capitulated = self.teams[i].capitulated;
                        self.team_set_vassal(i, other, true, capitulated);
                    }
                }
            }
        }

        self.team_share_counters(t, other);
        self.team_share_counters(other, t);

        for p in self.player_ids() {
            if self.players[p].team == other {
                self.player_set_team(p, t);
            }
        }

        let d = self.d;
        for p in self.map.plot_ids() {
            let visibility_count = self.map.plots[p].visibility_count[other];
            self.plot_change_visibility_count(p, t, visibility_count, None, false);

            for invisible in d.invisibles.keys() {
                // CvPlot::changeInvisibleVisibilityCount: a plain `+=` apart from UI refreshes.
                let change = self.map.plots[p].invisible_visibility_count[other][invisible];
                self.map.plots[p].invisible_visibility_count[t][invisible] += change;
            }

            if self.map.plots[p].revealed[other] {
                self.plot_set_revealed(p, t, true, false, Some(other), false);
            }
        }

        self.update_plot_groups();

        for i in self.team_ids() {
            if i != t && i != other {
                let war_weariness = (self.teams[i].war_weariness[t] + self.teams[i].war_weariness[other]) / 2;
                self.team_set_war_weariness(i, t, war_weariness);
                let timer = (self.teams[i].stolen_visibility_timer[t] + self.teams[i].stolen_visibility_timer[other]) / 2;
                self.team_set_stolen_visibility_timer(i, t, timer);

                // CvTeamAI counter setters (AI_setAtWarCounter & co.) are plain stores.
                // C++ quirk: the open-borders counter is not averaged.
                let k = &mut self.teams[i];
                k.ai_at_war_counter[t] = (k.ai_at_war_counter[t] + k.ai_at_war_counter[other]) / 2;
                k.ai_at_peace_counter[t] = (k.ai_at_peace_counter[t] + k.ai_at_peace_counter[other]) / 2;
                k.ai_has_met_counter[t] = (k.ai_has_met_counter[t] + k.ai_has_met_counter[other]) / 2;
                k.ai_defensive_pact_counter[t] = (k.ai_defensive_pact_counter[t] + k.ai_defensive_pact_counter[other]) / 2;
                k.ai_share_war_counter[t] = (k.ai_share_war_counter[t] + k.ai_share_war_counter[other]) / 2;
                k.ai_war_success[t] = (k.ai_war_success[t] + k.ai_war_success[other]) / 2;
                k.ai_enemy_peacetime_trade_value[t] = (k.ai_enemy_peacetime_trade_value[t] + k.ai_enemy_peacetime_trade_value[other]) / 2;
                k.ai_enemy_peacetime_grant_value[t] = (k.ai_enemy_peacetime_grant_value[t] + k.ai_enemy_peacetime_grant_value[other]) / 2;

                if self.team_is_alive(i) {
                    self.team_ai_set_war_plan(i, t, None, false);
                    self.team_ai_set_war_plan(i, other, None, false);
                }
            }
        }

        self.team_ai_update_worst_enemy(t);

        // AI: AI_updateAreaStragies() (area AI types, AI-only state).

        self.update_score(true);
    }

    /// C++ `CvTeam::shareCounters`: take the maximum of `other`'s counters (war weariness,
    /// stolen visibility, AI contact counters, war success, ...), clear `other`'s war plans,
    /// add its project/unit/building class counts, and take its research progress.
    pub fn team_share_counters(&mut self, t: TeamId, other: TeamId) {
        for i in self.team_ids() {
            if i != t && i != other {
                if self.teams[other].war_weariness[i] > self.teams[t].war_weariness[i] {
                    let v = self.teams[other].war_weariness[i];
                    self.team_set_war_weariness(t, i, v);
                }

                if self.teams[other].stolen_visibility_timer[i] > self.teams[t].stolen_visibility_timer[i] {
                    let v = self.teams[other].stolen_visibility_timer[i];
                    self.team_set_stolen_visibility_timer(t, i, v);
                }

                // CvTeamAI counter setters (AI_setAtWarCounter & co.) are plain stores.
                let v = self.teams[other].ai_at_war_counter[i];
                if v > self.teams[t].ai_at_war_counter[i] {
                    self.teams[t].ai_at_war_counter[i] = v;
                }

                let v = self.teams[other].ai_at_peace_counter[i];
                if v > self.teams[t].ai_at_peace_counter[i] {
                    self.teams[t].ai_at_peace_counter[i] = v;
                }

                let v = self.teams[other].ai_has_met_counter[i];
                if v > self.teams[t].ai_has_met_counter[i] {
                    self.teams[t].ai_has_met_counter[i] = v;
                }

                let v = self.teams[other].ai_open_borders_counter[i];
                if v > self.teams[t].ai_open_borders_counter[i] {
                    self.teams[t].ai_open_borders_counter[i] = v;
                }

                let v = self.teams[other].ai_defensive_pact_counter[i];
                if v > self.teams[t].ai_defensive_pact_counter[i] {
                    self.teams[t].ai_defensive_pact_counter[i] = v;
                }

                let v = self.teams[other].ai_share_war_counter[i];
                if v > self.teams[t].ai_share_war_counter[i] {
                    self.teams[t].ai_share_war_counter[i] = v;
                }

                let v = self.teams[other].ai_war_success[i];
                if v > self.teams[t].ai_war_success[i] {
                    self.teams[t].ai_war_success[i] = v;
                }

                let v = self.teams[other].ai_enemy_peacetime_trade_value[i];
                if v > self.teams[t].ai_enemy_peacetime_trade_value[i] {
                    self.teams[t].ai_enemy_peacetime_trade_value[i] = v;
                }

                let v = self.teams[other].ai_enemy_peacetime_grant_value[i];
                if v > self.teams[t].ai_enemy_peacetime_grant_value[i] {
                    self.teams[t].ai_enemy_peacetime_grant_value[i] = v;
                }

                self.team_ai_set_war_plan(other, i, None, false);
            }
        }

        let d = self.d;
        for project in d.projects.keys() {
            let extra_projects = self.teams[other].project_count[project] - self.teams[t].project_count[project];
            if extra_projects > 0 {
                self.team_change_project_count(t, project, extra_projects);
                // CvGame::incrementProjectCreatedCount(eProject, -iExtraProjects)
                self.project_created_count[project] -= extra_projects;
            }

            // changeProjectMaking: plain `+=`.
            let making = self.teams[other].project_making[project];
            self.teams[t].project_making[project] += making;
        }

        for unit_class in d.unit_classes.keys() {
            // changeUnitClassCount: plain `+=`.
            let count = self.teams[other].unit_class_count[unit_class];
            self.teams[t].unit_class_count[unit_class] += count;
        }

        for building_class in d.building_classes.keys() {
            // changeBuildingClassCount: plain `+=`.
            let count = self.teams[other].building_class_count[building_class];
            self.teams[t].building_class_count[building_class] += count;
        }

        for tech in d.techs.keys() {
            if self.team_get_research_progress(other, Some(tech)) > self.team_get_research_progress(t, Some(tech)) {
                let progress = self.team_get_research_progress(other, Some(tech));
                // C++ passes getLeaderID() (NO_PLAYER only for a team without members).
                let leader = self.team_get_leader_id(t).expect("CvTeam::shareCounters: team without members");
                self.team_set_research_progress(t, tech, progress, leader);
            }

            // UNOFFICIAL_PATCH 07/06/09 (jdog5000): clear "no trade" for techs the other team
            // has and may trade (instead of copying its "no trade" flags).
            if self.team_is_has_tech(other, Some(tech)) && !self.teams[other].no_trade_tech[tech] {
                self.teams[t].no_trade_tech[tech] = false;
            }
        }
    }

    // ------------------------------------------------------------------
    // Membership and identity
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isAlive` (C++ returns `int`).
    pub fn team_is_alive(&self, t: TeamId) -> bool {
        self.teams[t].alive_count > 0
    }

    /// C++ `CvTeam::changeAliveCount`: when the team dies, its vassals are freed.
    pub fn team_change_alive_count(&mut self, t: TeamId, change: i32) {
        self.teams[t].alive_count += change;

        // free vassals
        if 0 == self.teams[t].alive_count {
            for o in self.team_ids() {
                if o != t {
                    if self.team_is_alive(o) && self.teams[o].vassal[t] {
                        self.team_set_vassal(o, t, false, false);
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::isEverAlive` (C++ returns `int`).
    pub fn team_is_ever_alive(&self, t: TeamId) -> bool {
        self.teams[t].ever_alive_count > 0
    }

    /// C++ `CvTeam::isHuman` (any member player is human).
    pub fn team_is_human(&self, t: TeamId) -> bool {
        for p in self.player_ids() {
            if self.players[p].team == t {
                if self.players[p].human {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvTeam::isBarbarian`.
    pub fn team_is_barbarian(&self, t: TeamId) -> bool {
        t == self.barbarian_team()
    }

    /// C++ `CvTeam::isMinorCiv` (all member players are minor civs; false without members).
    pub fn team_is_minor_civ(&self, t: TeamId) -> bool {
        let mut valid = false;

        for p in self.player_ids() {
            if self.players[p].team == t {
                if self.player_is_minor_civ(p) {
                    valid = true;
                } else {
                    return false;
                }
            }
        }

        valid
    }

    /// C++ `CvTeam::getLeaderID`: first alive member, else first member; `None` (C++
    /// `NO_PLAYER`) if the team has no member at all.
    pub fn team_get_leader_id(&self, t: TeamId) -> Option<PlayerId> {
        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    return Some(p);
                }
            }
        }

        for p in self.player_ids() {
            if self.players[p].team == t {
                return Some(p);
            }
        }

        None
    }

    /// C++ `CvTeam::getSecretaryID`: first alive human member, else the leader.
    pub fn team_get_secretary_id(&self, t: TeamId) -> Option<PlayerId> {
        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].human {
                    if self.players[p].team == t {
                        return Some(p);
                    }
                }
            }
        }

        self.team_get_leader_id(t)
    }

    /// C++ `CvTeam::getHandicapType`: average handicap of the alive members
    /// (STANDARD_HANDICAP if none).
    pub fn team_get_handicap_type(&self, t: TeamId) -> HandicapType {
        let mut game_handicap = 0;
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    game_handicap += opt_to_i32(self.player_get_handicap_type(p));
                    count += 1;
                }
            }
        }

        if count > 0 {
            opt_from_i32(game_handicap / count).expect("(iGameHandicap / iCount) is expected to be non-negative (invalid Index)")
        } else {
            opt_from_i32(self.d.defines.standard_handicap).expect("CvTeam::getHandicapType: STANDARD_HANDICAP is not a handicap")
        }
    }

    /// C++ `CvTeam::getName`: alive member names joined with "/" (for messages and logs).
    pub fn team_get_name(&self, t: TeamId) -> String {
        let mut buffer = String::new();
        let mut first = true;

        for p in self.player_ids() {
            if self.players[p].alive && self.players[p].team == t {
                // setListHelp(szBuffer, L"", getName(), L"/", bFirst)
                if !first {
                    buffer.push('/');
                }
                buffer.push_str(&self.player_get_name(p));
                first = false;
            }
        }

        buffer
    }

    /// C++ `CvTeam::setTurnActive` (simultaneous team turns; C++ default `bTurn = true`).
    pub fn team_set_turn_active(&mut self, t: TeamId, value: bool, do_turn: bool) {
        // C++ FAssert(GC.getGameINLINE().isSimultaneousTeamTurns()) (debug-only).

        for p in self.player_ids() {
            if self.players[p].alive && self.players[p].team == t {
                self.player_set_turn_active(p, value, do_turn);
            }
        }
    }

    /// C++ `CvTeam::isTurnActive` (simultaneous team turns): any member's turn is active.
    pub fn team_is_turn_active(&self, t: TeamId) -> bool {
        // C++ FAssert(GC.getGameINLINE().isSimultaneousTeamTurns()) (debug-only).

        for p in self.player_ids() {
            if self.players[p].team == t {
                if self.players[p].turn_active {
                    return true;
                }
            }
        }

        false
    }

    // ------------------------------------------------------------------
    // Aggregates over member players
    // ------------------------------------------------------------------

    /// C++ `CvTeam::getAssets` (alive members).
    pub fn team_get_assets(&self, t: TeamId) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.players[p].assets;
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getPower`: alive members' power, plus that of the alive players of our
    /// vassals when `include_vassals`.
    pub fn team_get_power(&self, t: TeamId, include_vassals: bool) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            let loop_player = &self.players[p];
            if loop_player.alive {
                if loop_player.team == t || (include_vassals && self.teams[loop_player.team].vassal[t]) {
                    count += loop_player.power;
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getDefensivePower`.
    pub fn team_get_defensive_power(&self, t: TeamId) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) && !self.team_is_a_vassal(o) {
                if t == o || self.teams[t].vassal[o] || self.teams[t].defensive_pact[o] {
                    count += self.team_get_power(o, true);
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getEnemyPower`.
    pub fn team_get_enemy_power(&self, t: TeamId) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if t != o && self.teams[t].at_war[o] {
                    count += self.team_get_power(o, false);
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getNumNukeUnits` (alive members and the alive players of our vassals).
    pub fn team_get_num_nuke_units(&self, t: TeamId) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            let loop_player = &self.players[p];
            if loop_player.alive {
                if loop_player.team == t || self.teams[loop_player.team].vassal[t] {
                    count += loop_player.num_nuke_units;
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getTotalPopulation` (C++ default `bCheckVassals = true`).
    pub fn team_get_total_population(&self, t: TeamId, check_vassals: bool) -> i32 {
        let mut vassal_pop = 0;

        if check_vassals {
            if self.team_is_a_vassal(t) {
                return self.teams[t].total_population / 2;
            }

            for o in self.team_ids() {
                if o != t {
                    if self.team_is_alive(o) && self.teams[o].vassal[t] {
                        vassal_pop += self.team_get_total_population(o, false) / 2;
                    }
                }
            }
        }

        self.teams[t].total_population + vassal_pop
    }

    /// C++ `CvTeam::getTotalLand` (C++ default `bCheckVassals = true`).
    pub fn team_get_total_land(&self, t: TeamId, check_vassals: bool) -> i32 {
        let mut vassal_land = 0;

        if check_vassals {
            if self.team_is_a_vassal(t) {
                return self.teams[t].total_land / 2;
            }

            for o in self.team_ids() {
                if o != t {
                    if self.team_is_alive(o) && self.teams[o].vassal[t] {
                        vassal_land += self.team_get_total_land(o, false) / 2;
                    }
                }
            }
        }

        self.teams[t].total_land + vassal_land
    }

    /// C++ `CvTeam::getUnitClassMaking` (alive members).
    pub fn team_get_unit_class_making(&self, t: TeamId, unit_class: UnitClassType) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.players[p].unit_class_making[unit_class];
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getUnitClassCountPlusMaking`.
    pub fn team_get_unit_class_count_plus_making(&self, t: TeamId, unit_class: UnitClassType) -> i32 {
        self.teams[t].unit_class_count[unit_class] + self.team_get_unit_class_making(t, unit_class)
    }

    /// C++ `CvTeam::getBuildingClassMaking` (alive members).
    pub fn team_get_building_class_making(&self, t: TeamId, building_class: BuildingClassType) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.players[p].building_class_making[building_class];
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getBuildingClassCountPlusMaking`.
    pub fn team_get_building_class_count_plus_making(&self, t: TeamId, building_class: BuildingClassType) -> i32 {
        self.teams[t].building_class_count[building_class] + self.team_get_building_class_making(t, building_class)
    }

    /// C++ `CvTeam::getHasReligionCount` (alive members).
    pub fn team_get_has_religion_count(&self, t: TeamId, religion: ReligionType) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.players[p].has_religion_count[religion];
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getHasCorporationCount` (alive members).
    pub fn team_get_has_corporation_count(&self, t: TeamId, corporation: CorporationType) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.player_get_has_corporation_count(p, corporation);
                }
            }
        }

        count
    }

    /// C++ `CvTeam::countTotalCulture` (alive members; non-const in C++, read-only).
    pub fn team_count_total_culture(&self, t: TeamId) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.player_count_total_culture(p);
                }
            }
        }

        count
    }

    /// C++ `CvTeam::countNumUnitsByArea` (alive members).
    pub fn team_count_num_units_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.map.areas[area].units_per_player[p];
                }
            }
        }

        count
    }

    /// C++ `CvTeam::countNumCitiesByArea` (alive members).
    pub fn team_count_num_cities_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.map.areas[area].cities_per_player[p];
                }
            }
        }

        count
    }

    /// C++ `CvTeam::countTotalPopulationByArea` (alive members).
    pub fn team_count_total_population_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.map.areas[area].population_per_player[p];
                }
            }
        }

        count
    }

    /// C++ `CvTeam::countPowerByArea` (alive members).
    pub fn team_count_power_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.map.areas[area].power[p];
                }
            }
        }

        count
    }

    /// C++ `CvTeam::countEnemyPowerByArea` (BBAI: counts the alive players of every team we
    /// have any war plan against, not only those we are at war with).
    pub fn team_count_enemy_power_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                let player_team = self.players[p].team;
                if player_team != t {
                    // BETTER_BTS_AI_MOD 01/11/09 (jdog5000): count planned wars as well
                    // (original BTS: isAtWar(getTeam())).
                    if self.teams[t].ai_war_plan[player_team].is_some() {
                        count += self.map.areas[area].power[p];
                    }
                }
            }
        }

        count
    }

    /// C++ `CvTeam::countNumAIUnitsByArea` (alive members' units of that UnitAI in the area).
    pub fn team_count_num_ai_units_by_area(&self, t: TeamId, area: AreaId, unit_ai: UnitAIType) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == t {
                    count += self.map.areas[area].num_ai_units[p][unit_ai];
                }
            }
        }

        count
    }

    /// C++ `CvTeam::countEnemyDangerByArea`: enemy defenders visible to our leader on our
    /// plots of the area.
    pub fn team_count_enemy_danger_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        // C++ calls getLeaderID() for each counted plot (the same player every time); a plot of
        // team `t` implies `t` has members, so it is never NO_PLAYER there.
        let leader = opt_to_i32(self.team_get_leader_id(t));
        let mut count = 0;

        for p in self.map.plot_ids() {
            if self.map.plots[p].area == Some(area) {
                if self.plot_get_team(p) == Some(t) {
                    count += self.plot_plot_count(
                        p,
                        Some(Game::puf_can_defend_enemy),
                        leader,
                        false as i32,
                        None,
                        None,
                        Some(Game::puf_is_visible),
                        leader,
                        -1,
                    );
                }
            }
        }

        count
    }

    /// C++ `CvTeam::hasHolyCity`.
    pub fn team_has_holy_city(&self, t: TeamId, religion: ReligionType) -> bool {
        if let Some(holy_city) = self.get_holy_city(religion) {
            return self.city_team(holy_city) == t;
        }

        false
    }

    /// C++ `CvTeam::hasHeadquarters`.
    pub fn team_has_headquarters(&self, t: TeamId, corporation: CorporationType) -> bool {
        if let Some(headquarters) = self.get_headquarters(corporation) {
            return self.city_team(headquarters) == t;
        }

        false
    }

    /// C++ `CvTeam::hasShrine` (`religion = None` is NO_RELIGION -> false; non-const in C++,
    /// read-only): we own the religion's holy city and it has the shrine.
    pub fn team_has_shrine(&self, t: TeamId, religion: Option<ReligionType>) -> bool {
        let mut has_shrine = false;

        if let Some(religion) = religion {
            let holy_city = self.get_holy_city(religion);

            // if the holy city exists, and we own it
            if let Some(holy_city) = holy_city {
                if self.players[self.cities[holy_city].owner].team == t {
                    has_shrine = self.city_has_shrine(holy_city, religion);
                }
            }
        }

        has_shrine
    }

    // ------------------------------------------------------------------
    // Votes
    // ------------------------------------------------------------------

    /// C++ `CvTeam::getVotes` (`vote = None` is C++ `NO_VOTE`; FFH counts alive civ players only).
    pub fn team_get_votes(&self, t: TeamId, vote: Option<VoteType>, vote_source: VoteSourceType) -> i32 {
        let mut count = 0;

        // BUGFIX (Votes for the Councils) Sephi: MAX_CIV_PLAYERS and alive members only
        // (BtS: every MAX_PLAYERS member).
        for p in self.civ_player_ids() {
            if self.players[p].team == t {
                if self.players[p].alive {
                    count += self.player_get_votes(p, vote, vote_source);
                }
            }
        }

        count
    }

    /// C++ `CvTeam::isVotingMember`.
    pub fn team_is_voting_member(&self, t: TeamId, vote_source: VoteSourceType) -> bool {
        self.team_get_votes(t, None, vote_source) > 0
    }

    /// C++ `CvTeam::isFullMember`: force-eligible, or every member player (alive or not) is a
    /// full member.
    pub fn team_is_full_member(&self, t: TeamId, vote_source: VoteSourceType) -> bool {
        if self.team_is_force_team_vote_eligible(t, vote_source) {
            return true;
        }

        for p in self.player_ids() {
            if self.players[p].team == t {
                if !self.player_is_full_member(p, vote_source) {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvTeam::isForceTeamVoteEligible`.
    pub fn team_is_force_team_vote_eligible(&self, t: TeamId, vote_source: VoteSourceType) -> bool {
        self.teams[t].force_team_vote_eligibility_count[vote_source] > 0 && !self.team_is_minor_civ(t)
    }

    // ------------------------------------------------------------------
    // Trading permissions (counts from techs)
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isMapTrading`.
    pub fn team_is_map_trading(&self, t: TeamId) -> bool {
        self.teams[t].map_trading_count > 0
    }

    /// C++ `CvTeam::isTechTrading`.
    pub fn team_is_tech_trading(&self, t: TeamId) -> bool {
        self.teams[t].tech_trading_count > 0
    }

    /// C++ `CvTeam::isGoldTrading`.
    pub fn team_is_gold_trading(&self, t: TeamId) -> bool {
        self.teams[t].gold_trading_count > 0
    }

    /// C++ `CvTeam::isOpenBordersTrading`.
    pub fn team_is_open_borders_trading(&self, t: TeamId) -> bool {
        self.teams[t].open_borders_trading_count > 0
    }

    /// C++ `CvTeam::isDefensivePactTrading`.
    pub fn team_is_defensive_pact_trading(&self, t: TeamId) -> bool {
        self.teams[t].defensive_pact_trading_count > 0
    }

    /// C++ `CvTeam::isPermanentAllianceTrading` (false unless GAMEOPTION_PERMANENT_ALLIANCES).
    pub fn team_is_permanent_alliance_trading(&self, t: TeamId) -> bool {
        if !is_option(self, GameOptionType::GAMEOPTION_PERMANENT_ALLIANCES) {
            return false;
        }

        self.teams[t].permanent_alliance_trading_count > 0
    }

    /// C++ `CvTeam::isVassalStateTrading` (false under GAMEOPTION_NO_VASSAL_STATES).
    pub fn team_is_vassal_state_trading(&self, t: TeamId) -> bool {
        if is_option(self, GameOptionType::GAMEOPTION_NO_VASSAL_STATES) {
            return false;
        }

        self.teams[t].vassal_trading_count > 0
    }

    /// C++ `CvTeam::isCommerceFlexible`. (`changeCommerceFlexibleCount` is a plain
    /// `commerce_flexible_count[c] += n`; its only side effect is a UI refresh.)
    pub fn team_is_commerce_flexible(&self, t: TeamId, commerce: CommerceType) -> bool {
        self.teams[t].commerce_flexible_count[commerce.idx()] > 0
    }

    // ------------------------------------------------------------------
    // Projects, unit and building classes
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isProjectMaxedOut` (C++ default `iExtra = 0`).
    pub fn team_is_project_maxed_out(&self, t: TeamId, project: ProjectType, extra: i32) -> bool {
        if !is_team_project(self.d, project) {
            return false;
        }

        // C++ FAssertMsg(getProjectCount <= getMaxTeamInstances) (debug-only; a team merge
        // can exceed it).

        (self.teams[t].project_count[project] + extra) >= self.d.projects[project].max_team_instances
    }

    /// C++ `CvTeam::changeProjectCount`: global created count, nuke interception, tech share,
    /// spaceship launch eligibility, and on completion the "everyone" special unit/building,
    /// nukes, and production re-choice of AI members whose next project became available.
    /// (The project art-type bookkeeping in the C++ is graphics-only and not ported.)
    pub fn team_change_project_count(&mut self, t: TeamId, project: ProjectType, change: i32) {
        if change != 0 {
            // CvGame::incrementProjectCreatedCount(eIndex, iChange)
            self.project_created_count[project] += change;

            let old_project_count = self.teams[t].project_count[project];

            self.teams[t].project_count[project] += change;
            debug_assert!(self.teams[t].project_count[project] >= 0);

            // m_pavProjectArtTypes push/pop: graphics only.

            let d = self.d;
            let k_project = &d.projects[project];

            // changeNukeInterception: plain `+=`.
            self.teams[t].nuke_interception += k_project.nuke_interception * change;

            // C++ tests `<= MAX_TEAMS` (the size of m_aiTechShareCount); here the array is
            // sized num_teams().
            if k_project.tech_share > 0 && k_project.tech_share <= self.num_teams() as i32 {
                self.team_change_tech_share_count(t, (k_project.tech_share - 1) as usize, change);
            }

            for victory in d.victories.keys() {
                if k_project.victory_threshold[victory] > 0 {
                    // setCanLaunch is a plain store.
                    self.teams[t].can_launch[victory] = self.test_victory_team(victory, t).0;
                }
            }

            if change > 0 {
                if let Some(special_unit) = k_project.everyone_special_unit {
                    // CvGame::makeSpecialUnitValid: plain store.
                    self.special_unit_valid[special_unit] = true;
                }

                if let Some(special_building) = k_project.everyone_special_building {
                    self.make_special_building_valid(special_building, false);
                }

                if k_project.allows_nukes {
                    // CvGame::makeNukesValid(true): plain store.
                    self.nukes_valid = true;
                }

                for p in self.player_ids() {
                    if self.players[p].alive {
                        if self.players[p].team == t {
                            if !self.players[p].human {
                                let mut change_production = false;

                                for other_project in d.projects.keys() {
                                    let needed = d.projects[other_project].projects_needed[project];
                                    if self.teams[t].project_count[project] >= needed && old_project_count < needed {
                                        change_production = true;
                                        break;
                                    }
                                }

                                if change_production {
                                    self.player_ai_make_production_dirty(p);
                                }
                            }
                        }
                    }
                }

                // `!gDLL->GetWorldBuilderMode()`: never in world builder here.
                if self.final_initialized {
                    // UI: TXT_KEY_MISC_COMPLETES_PROJECT replay message and
                    // TXT_KEY_MISC_SOMEONE_HAS_COMPLETED message to every alive player.
                    if self.log.enabled {
                        let turn = self.game_turn();
                        let leader = self.team_get_leader_id(t);
                        let text = format!("{} has completed {}!", self.team_get_name(t), d.text.name(&k_project.base.description));
                        self.log.msg(turn, leader, || text);
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::isUnitClassMaxedOut` (C++ default `iExtra = 0`).
    pub fn team_is_unit_class_maxed_out(&self, t: TeamId, unit_class: UnitClassType, extra: i32) -> bool {
        if !is_team_unit_class(self.d, unit_class) {
            return false;
        }

        // C++ FAssertMsg(getUnitClassCount <= getMaxTeamInstances) (debug-only).

        (self.teams[t].unit_class_count[unit_class] + extra) >= self.d.unit_classes[unit_class].max_team_instances
    }

    /// C++ `CvTeam::isBuildingClassMaxedOut` (C++ default `iExtra = 0`).
    pub fn team_is_building_class_maxed_out(&self, t: TeamId, building_class: BuildingClassType, extra: i32) -> bool {
        if !is_team_wonder_class(self.d, building_class) {
            return false;
        }

        // C++ FAssertMsg(getBuildingClassCount <= getMaxTeamInstances) (debug-only).

        (self.teams[t].building_class_count[building_class] + extra) >= self.d.building_classes[building_class].max_team_instances
    }

    // ------------------------------------------------------------------
    // Victory
    // ------------------------------------------------------------------

    /// C++ `CvTeam::getVictoryDelay`: the game's victory delay, lengthened by each missing
    /// optional project part (-1 if a required minimum is not met).
    pub fn team_get_victory_delay(&self, t: TeamId, victory: VictoryType) -> i32 {
        let d = self.d;
        let mut extra_delay_percent = 0;

        for project in d.projects.keys() {
            let k_project = &d.projects[project];
            let count = self.teams[t].project_count[project];

            if count < k_project.victory_min_threshold[victory] {
                // C++ FAssert(false)
                return -1;
            }

            if count < k_project.victory_threshold[victory] {
                extra_delay_percent += ((k_project.victory_threshold[victory] - count) * k_project.victory_delay_percent)
                    / k_project.victory_threshold[victory];
            }
        }

        (self.victory_delay(victory) * (100 + extra_delay_percent)) / 100
    }

    /// C++ `CvTeam::getLaunchSuccessRate`: 100, minus each missing optional part's success
    /// rate (0 if a required minimum is not met).
    pub fn team_get_launch_success_rate(&self, t: TeamId, victory: VictoryType) -> i32 {
        let d = self.d;
        let mut success_rate = 100;

        for project in d.projects.keys() {
            let k_project = &d.projects[project];
            let count = self.teams[t].project_count[project];

            if count < k_project.victory_min_threshold[victory] {
                return 0;
            }

            if count < k_project.victory_threshold[victory] {
                if k_project.success_rate > 0 {
                    success_rate -= k_project.success_rate * (k_project.victory_threshold[victory] - count);
                }
            }
        }

        success_rate
    }

    /// C++ `CvTeam::resetVictoryProgress`: while the game is on, stops every running victory
    /// countdown and destroys the team's projects counting towards that victory.
    pub fn team_reset_victory_progress(&mut self, t: TeamId) {
        let d = self.d;

        for victory in d.victories.keys() {
            if self.teams[t].victory_countdown[victory] >= 0 && self.game_state == GameStateType::GAMESTATE_ON {
                // setVictoryCountdown: plain store.
                self.teams[t].victory_countdown[victory] = -1;

                for project in d.projects.keys() {
                    if d.projects[project].victory_min_threshold[victory] > 0 {
                        let count = self.teams[t].project_count[project];
                        self.team_change_project_count(t, project, -count);
                    }
                }

                // UI: TXT_KEY_VICTORY_RESET message to every alive player, a popup for the
                // team's members, and a replay message.
                if self.log.enabled {
                    let turn = self.game_turn();
                    let leader = self.team_get_leader_id(t);
                    let text = format!(
                        "{}: progress towards {} has been reset!",
                        self.team_get_name(t),
                        d.text.name(&d.victories[victory].base.description)
                    );
                    self.log.msg(turn, leader, || text);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Espionage and stolen visibility
    // ------------------------------------------------------------------

    /// C++ `CvTeam::setEspionagePointsAgainstTeam`.
    pub fn team_set_espionage_points_against_team(&mut self, t: TeamId, other: TeamId, value: i32) {
        if value != self.teams[t].espionage_points_against_team[other] {
            self.teams[t].espionage_points_against_team[other] = value;

            self.team_verify_spy_units_valid_plot(t);
            self.team_verify_spy_units_valid_plot(other);
        }
    }

    /// C++ `CvTeam::changeEspionagePointsAgainstTeam`.
    pub fn team_change_espionage_points_against_team(&mut self, t: TeamId, other: TeamId, change: i32) {
        let v = self.teams[t].espionage_points_against_team[other] + change;
        self.team_set_espionage_points_against_team(t, other, v);
    }

    /// C++ `CvTeam::verifySpyUnitsValidPlot`: spies of alive members standing in borders
    /// they may not enter jump to the nearest valid plot.
    pub fn team_verify_spy_units_valid_plot(&mut self, t: TeamId) {
        let mut units: Vec<UnitId> = Vec::new();

        for p in self.player_ids() {
            if self.players[p].alive && self.players[p].team == t {
                for &u in &self.players[p].units {
                    let owner = self.map.plots[self.unit_plot(u)].owner;
                    if let Some(owner) = owner {
                        if self.unit_is_spy(u) {
                            if !self.player_can_spies_enter_borders(p, owner) {
                                units.push(u);
                            }
                        }
                    }
                }
            }
        }

        for u in units {
            // C++ uses the collected pointers unchecked; skip a unit an earlier jump destroyed.
            if self.units.contains(u) {
                self.unit_jump_to_nearest_valid_plot(u);
            }
        }
    }

    /// C++ `CvTeam::isStolenVisibility`.
    pub fn team_is_stolen_visibility(&self, t: TeamId, other: TeamId) -> bool {
        self.teams[t].stolen_visibility_timer[other] > 0
    }

    /// C++ `CvTeam::setStolenVisibilityTimer`: when stolen visibility of `other` starts or
    /// ends, every plot `other` sees gains or loses a stolen-visibility count for `t`.
    pub fn team_set_stolen_visibility_timer(&mut self, t: TeamId, other: TeamId, value: i32) {
        if self.teams[t].stolen_visibility_timer[other] != value {
            let old_stolen_visibility = self.team_is_stolen_visibility(t, other);

            self.teams[t].stolen_visibility_timer[other] = value;
            debug_assert!(self.teams[t].stolen_visibility_timer[other] >= 0);

            if old_stolen_visibility != self.team_is_stolen_visibility(t, other) {
                for p in self.map.plot_ids() {
                    if self.plot_is_visible(p, other) {
                        let change = if self.team_is_stolen_visibility(t, other) { 1 } else { -1 };
                        self.plot_change_stolen_visibility_count(p, t, change);
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::changeStolenVisibilityTimer`.
    pub fn team_change_stolen_visibility_timer(&mut self, t: TeamId, other: TeamId, change: i32) {
        let v = self.teams[t].stolen_visibility_timer[other] + change;
        self.team_set_stolen_visibility_timer(t, other, v);
    }
}

#[cfg(test)]
mod tests {
    use crate::game::Game;
    use crate::ids::*;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;
    use ffh_data::*;

    /// A game with `n` civ slots alive (each on its own team) plus the open spare slots, with
    /// real `Player` objects (not `player_init`ed) and no map.
    fn game(n: usize) -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, n, "WORLDSIZE_DUEL", 3));
        let (n_players, n_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], n_players, n_teams);
            let t = g.players[p].team;
            g.teams[t].num_members += 1;
        }
        for i in 0..n {
            let p = PlayerId(i as u8);
            g.players[p].alive = true;
            g.players[p].ever_alive = true;
            let t = g.players[p].team;
            g.teams[t].alive_count += 1;
            g.teams[t].ever_alive_count += 1;
        }
        g
    }

    #[test]
    fn leader_secretary_human_minor_and_name() {
        let mut g = game(3);
        let (p0, p1, p2) = (PlayerId(0), PlayerId(1), PlayerId(2));
        let (t0, t1, t2) = (TeamId(0), TeamId(1), TeamId(2));
        g.setup.players[1].leader_name = "Arendel".into();
        g.setup.players[2].leader_name = "Thessa".into();
        // p2 joins team 1 (raw), team 2 has no member left
        g.players[p2].team = t1;

        assert_eq!(g.team_get_leader_id(t0), Some(p0));
        assert_eq!(g.team_get_leader_id(t1), Some(p1));
        assert_eq!(g.team_get_leader_id(t2), None);
        assert_eq!(g.team_get_name(t1), "Arendel/Thessa");

        // the first alive member leads; without one, the first member
        g.players[p1].alive = false;
        assert_eq!(g.team_get_leader_id(t1), Some(p2));
        assert_eq!(g.team_get_name(t1), "Thessa");
        g.players[p2].alive = false;
        assert_eq!(g.team_get_leader_id(t1), Some(p1));
        assert_eq!(g.team_get_name(t1), "");

        // secretary: the first alive human member, else the leader
        g.players[p1].alive = true;
        g.players[p2].alive = true;
        assert_eq!(g.team_get_secretary_id(t1), Some(p1));
        g.players[p2].human = true;
        assert_eq!(g.team_get_secretary_id(t1), Some(p2));
        assert!(g.team_is_human(t1));
        assert!(!g.team_is_human(t0));
        // a dead human member still makes the team human, but is no secretary
        g.players[p2].alive = false;
        assert!(g.team_is_human(t1));
        assert_eq!(g.team_get_secretary_id(t1), Some(p1));
        assert_eq!(g.team_get_secretary_id(t2), None);

        // no minor civs in this setup; a team without members is not minor either
        assert!(!g.team_is_minor_civ(t0));
        assert!(!g.team_is_minor_civ(t2));

        // turn activity of any member (alive or not)
        assert!(!g.team_is_turn_active(t1));
        g.players[p2].turn_active = true;
        assert!(g.team_is_turn_active(t1));
    }

    #[test]
    fn handicap_is_the_alive_members_average() {
        let mut g = game(3);
        let (p1, p2) = (PlayerId(1), PlayerId(2));
        let (t1, t2) = (TeamId(1), TeamId(2));
        g.players[p2].team = t1;
        g.setup.players[1].handicap = Some(HandicapType(1));
        g.setup.players[2].handicap = Some(HandicapType(4));
        assert_eq!(g.team_get_handicap_type(t1), HandicapType(2));
        g.players[p1].alive = false;
        assert_eq!(g.team_get_handicap_type(t1), HandicapType(4));
        // no alive member: STANDARD_HANDICAP
        let standard = opt_from_i32::<HandicapType>(g.d.defines.standard_handicap).unwrap();
        assert_eq!(g.team_get_handicap_type(t2), standard);
    }

    #[test]
    fn power_assets_and_member_counts() {
        let mut g = game(3);
        let d = g.d;
        let (p0, p1, p2) = (PlayerId(0), PlayerId(1), PlayerId(2));
        let (t0, t1, t2) = (TeamId(0), TeamId(1), TeamId(2));
        g.players[p0].power = 10;
        g.players[p1].power = 20;
        g.players[p2].power = 40;
        g.players[p0].num_nuke_units = 1;
        g.players[p1].num_nuke_units = 2;
        g.players[p2].num_nuke_units = 4;
        // team 1 is a vassal of team 0
        g.teams[t1].vassal[t0] = true;
        assert_eq!(g.team_get_power(t0, false), 10);
        assert_eq!(g.team_get_power(t0, true), 30);
        assert_eq!(g.team_get_power(t1, true), 20);
        assert_eq!(g.team_get_num_nuke_units(t0), 3);
        // the defensive power counts the master with its vassals (getPower(true))
        assert_eq!(g.team_get_defensive_power(t0), 30);
        g.teams[t0].at_war[t2] = true;
        assert_eq!(g.team_get_enemy_power(t0), 40);
        // dead players do not count
        g.players[p1].alive = false;
        assert_eq!(g.team_get_power(t0, true), 10);
        assert_eq!(g.team_get_num_nuke_units(t0), 1);

        g.players[p2].assets = 7;
        assert_eq!(g.team_get_assets(t2), 7);
        assert_eq!(g.team_get_assets(t0), 0);

        let unit_class = UnitClassType(0);
        let building_class = BuildingClassType(0);
        let religion = ReligionType(0);
        g.players[p2].unit_class_making[unit_class] = 2;
        g.players[p2].building_class_making[building_class] = 3;
        g.players[p2].has_religion_count[religion] = 4;
        g.teams[t2].unit_class_count[unit_class] = 1;
        g.teams[t2].building_class_count[building_class] = 1;
        assert_eq!(g.team_get_unit_class_making(t2, unit_class), 2);
        assert_eq!(g.team_get_unit_class_count_plus_making(t2, unit_class), 3);
        assert_eq!(g.team_get_building_class_making(t2, building_class), 3);
        assert_eq!(g.team_get_building_class_count_plus_making(t2, building_class), 4);
        assert_eq!(g.team_get_has_religion_count(t2, religion), 4);
        g.players[p2].alive = false;
        assert_eq!(g.team_get_unit_class_making(t2, unit_class), 0);
        assert_eq!(g.team_get_has_religion_count(t2, religion), 0);

        // FFH has no team-limited unit classes, building classes or projects
        for uc in d.unit_classes.keys() {
            assert!(!g.team_is_unit_class_maxed_out(t0, uc, 100));
        }
        for bc in d.building_classes.keys() {
            assert!(!g.team_is_building_class_maxed_out(t0, bc, 100));
        }
        for project in d.projects.keys() {
            assert!(!g.team_is_project_maxed_out(t0, project, 100));
        }
    }

    #[test]
    fn area_counts() {
        let mut g = game(3);
        let (p0, p1, p2) = (PlayerId(0), PlayerId(1), PlayerId(2));
        let (t0, t1) = (TeamId(0), TeamId(1));
        let area = g.map_add_area(false);
        g.players[p2].team = t1;
        {
            let a = &mut g.map.areas[area];
            a.units_per_player[p1] = 3;
            a.units_per_player[p2] = 4;
            a.cities_per_player[p1] = 1;
            a.cities_per_player[p2] = 2;
            a.population_per_player[p1] = 5;
            a.population_per_player[p2] = 6;
            a.power[p0] = 100;
            a.power[p1] = 7;
            a.power[p2] = 8;
            a.num_ai_units[p1][UnitAIType::UNITAI_ATTACK] = 2;
            a.num_ai_units[p2][UnitAIType::UNITAI_ATTACK] = 1;
        }
        assert_eq!(g.team_count_num_units_by_area(t1, area), 7);
        assert_eq!(g.team_count_num_cities_by_area(t1, area), 3);
        assert_eq!(g.team_count_total_population_by_area(t1, area), 11);
        assert_eq!(g.team_count_power_by_area(t1, area), 15);
        assert_eq!(g.team_count_num_ai_units_by_area(t1, area, UnitAIType::UNITAI_ATTACK), 3);
        // enemy power: every team we have any war plan against (BBAI), not only wars
        assert_eq!(g.team_count_enemy_power_by_area(t0, area), 0);
        g.teams[t0].ai_war_plan[t1] = Some(WarPlanType::WARPLAN_PREPARING_LIMITED);
        assert_eq!(g.team_count_enemy_power_by_area(t0, area), 15);
        g.players[p2].alive = false;
        assert_eq!(g.team_count_enemy_power_by_area(t0, area), 7);
        assert_eq!(g.team_count_num_units_by_area(t1, area), 3);
        // no plots: no danger
        assert_eq!(g.team_count_enemy_danger_by_area(t0, area), 0);
    }

    #[test]
    fn project_count_effects() {
        let mut g = game(2);
        let d = g.d;
        let t0 = TeamId(0);
        let project = ProjectType(0);
        let info = &d.projects[project];

        g.team_change_project_count(t0, project, 2);
        assert_eq!(g.teams[t0].project_count[project], 2);
        assert_eq!(g.project_created_count[project], 2);
        assert_eq!(g.teams[t0].nuke_interception, 2 * info.nuke_interception);
        if let Some(special_unit) = info.everyone_special_unit {
            assert!(g.special_unit_valid[special_unit]);
        }
        assert_eq!(g.nukes_valid, info.allows_nukes);

        // no-op
        g.team_change_project_count(t0, project, 0);
        assert_eq!(g.project_created_count[project], 2);

        g.team_change_project_count(t0, project, -2);
        assert_eq!(g.teams[t0].project_count[project], 0);
        assert_eq!(g.project_created_count[project], 0);
        assert_eq!(g.teams[t0].nuke_interception, 0);
    }

    #[test]
    fn victory_delay_launch_rate_and_reset() {
        let mut g = game(2);
        let d = g.d;
        let t0 = TeamId(0);
        // FFH projects have no victory thresholds: no extra delay, full success rate
        for victory in d.victories.keys() {
            assert_eq!(g.team_get_victory_delay(t0, victory), g.victory_delay(victory));
            assert_eq!(g.team_get_launch_success_rate(t0, victory), 100);
        }

        let victory = VictoryType(0);
        g.teams[t0].victory_countdown[victory] = 5;
        g.game_state = GameStateType::GAMESTATE_EXTENDED;
        g.team_reset_victory_progress(t0);
        assert_eq!(g.teams[t0].victory_countdown[victory], 5);
        g.game_state = GameStateType::GAMESTATE_ON;
        g.team_reset_victory_progress(t0);
        assert_eq!(g.teams[t0].victory_countdown[victory], -1);
    }

    #[test]
    fn share_counters_takes_the_larger_values() {
        let mut g = game(3);
        let (t0, t1, t2) = (TeamId(0), TeamId(1), TeamId(2));
        let project = ProjectType(0);
        let unit_class = UnitClassType(0);
        let building_class = BuildingClassType(0);
        let tech = TechType(0);

        g.teams[t1].war_weariness[t2] = 50;
        g.teams[t0].war_weariness[t2] = 20;
        g.teams[t1].stolen_visibility_timer[t2] = 4;
        g.teams[t1].ai_at_war_counter[t2] = 3;
        g.teams[t0].ai_at_war_counter[t2] = 5;
        g.teams[t1].ai_open_borders_counter[t2] = 6;
        g.teams[t1].ai_war_success[t2] = 9;
        g.teams[t1].ai_enemy_peacetime_grant_value[t2] = 11;
        // not at war: AI_setWarPlan(.., NO_WARPLAN, false) clears it
        g.teams[t1].ai_war_plan[t2] = Some(WarPlanType::WARPLAN_LIMITED);
        g.teams[t1].project_count[project] = 1;
        g.project_created_count[project] = 1;
        g.teams[t1].project_making[project] = 2;
        g.teams[t0].project_making[project] = 1;
        g.teams[t1].unit_class_count[unit_class] = 3;
        g.teams[t0].unit_class_count[unit_class] = 1;
        g.teams[t1].building_class_count[building_class] = 1;
        g.teams[t1].has_tech[tech] = true;
        g.teams[t0].no_trade_tech[tech] = true;

        g.team_share_counters(t0, t1);

        assert_eq!(g.teams[t0].war_weariness[t2], 50);
        assert_eq!(g.teams[t0].stolen_visibility_timer[t2], 4);
        assert_eq!(g.teams[t0].ai_at_war_counter[t2], 5);
        assert_eq!(g.teams[t0].ai_open_borders_counter[t2], 6);
        assert_eq!(g.teams[t0].ai_war_success[t2], 9);
        assert_eq!(g.teams[t0].ai_enemy_peacetime_grant_value[t2], 11);
        assert_eq!(g.teams[t1].ai_war_plan[t2], None);
        // the extra project is added, but the global created count is not raised
        assert_eq!(g.teams[t0].project_count[project], 1);
        assert_eq!(g.project_created_count[project], 1);
        assert_eq!(g.teams[t0].project_making[project], 3);
        assert_eq!(g.teams[t0].unit_class_count[unit_class], 4);
        assert_eq!(g.teams[t0].building_class_count[building_class], 1);
        // UNOFFICIAL_PATCH: a tech the other team may trade is tradeable for us too
        assert!(!g.teams[t0].no_trade_tech[tech]);
        // the other team's own counters are untouched
        assert_eq!(g.teams[t1].war_weariness[t2], 50);
        assert_eq!(g.teams[t1].unit_class_count[unit_class], 3);
    }

    #[test]
    fn stolen_visibility_timer_updates_plots() {
        let mut g = game(2);
        let d = g.d;
        let (n_players, n_teams) = (g.num_players(), g.num_teams());
        g.map = Map::new(d, &MapInitData { grid_w: 3, grid_h: 2, ..Default::default() }, n_players, n_teams);
        let (t0, t1) = (TeamId(0), TeamId(1));
        let seen = g.map.plot_xy(1, 1).unwrap();
        g.map.plots[seen].visibility_count[t1] = 1;

        g.team_set_stolen_visibility_timer(t0, t1, 3);
        assert!(g.team_is_stolen_visibility(t0, t1));
        assert_eq!(g.map.plots[seen].stolen_visibility_count[t0], 1);
        assert!(g.plot_is_visible(seen, t0));
        assert!(g.map.plots[seen].revealed[t0]);
        for p in g.map.plot_ids() {
            if p != seen {
                assert_eq!(g.map.plots[p].stolen_visibility_count[t0], 0);
            }
        }

        // still stealing: plots unchanged
        g.team_change_stolen_visibility_timer(t0, t1, -1);
        assert_eq!(g.map.plots[seen].stolen_visibility_count[t0], 1);

        g.team_set_stolen_visibility_timer(t0, t1, 0);
        assert!(!g.team_is_stolen_visibility(t0, t1));
        assert_eq!(g.map.plots[seen].stolen_visibility_count[t0], 0);
        assert!(!g.plot_is_visible(seen, t0));
        // revealed stays
        assert!(g.map.plots[seen].revealed[t0]);
    }
}
