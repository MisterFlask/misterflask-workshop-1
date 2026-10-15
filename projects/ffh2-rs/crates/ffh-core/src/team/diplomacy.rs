//! CvTeam: war & peace, contact, open borders, defensive pacts, force peace, vassals,
//! war weariness; plus the `CvTeamAI` war-plan and contact-counter bookkeeping that rules
//! code reads and writes (`AI_setWarPlan`, `AI_isSneakAttackReady`, `AI_doCounter`, ...).

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{civ_team_ids, is_option};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use crate::utils::set_trade_item;
use ffh_data::*;

/// C++ `GET_PLAYER(getLeaderID())` where the C++ dereferences the leader unchecked (a team
/// always has a member slot; C++ would index `NO_PLAYER` otherwise).
fn leader_of(g: &Game, t: TeamId) -> PlayerId {
    g.team_get_leader_id(t).expect("CvTeam::getLeaderID: team without a player")
}

/// C++ walk over a deal's `CLinkList<TradeData>`: does `list` hold an item of one of `items`?
fn has_trade_item(list: &[TradeData], items: &[TradeableItem]) -> bool {
    list.iter().any(|trade| items.contains(&trade.item_type))
}

/// Python `CvGameUtils.canDeclareWar(argsList)`, called by `CvTeam::canDeclareWar` only when
/// `USE_CAN_DECLARE_WAR_CALLBACK` is set (FFH2's PythonCallbackDefines.xml sets it to 0).
/// FFH2 returns True.
fn py_can_declare_war(_g: &Game, _team: TeamId, _other: TeamId) -> bool {
    true
}

/// C++ `CvGameAI::AI_makeAssignWorkDirty` (no CvGame method for it yet): every alive player's
/// cities are flagged for citizen reassignment (governor bookkeeping).
fn game_ai_make_assign_work_dirty(g: &mut Game) {
    for p in g.player_ids() {
        if g.players[p].alive {
            g.player_ai_make_assign_work_dirty(p);
        }
    }
}

/// One `changeNumOutsideUnits` pass of `CvTeam::setVassal` over the units of `master`'s alive
/// players (C++ `eIndex`): every unit outside its own team's and its vassals' territory counts
/// `change` (FfH: summons, i.e. units with a duration, are never counted).
fn change_master_outside_units(g: &mut Game, master: TeamId, change: i32) {
    for p in g.player_ids() {
        if g.players[p].alive && g.players[p].team == master {
            for i in 0..g.players[p].units.len() {
                let u = g.players[p].units[i];
                let unit_team = g.unit_get_team(u);
                let plot_team = g.plot_get_team(g.unit_plot(u));
                if Some(unit_team) != plot_team && plot_team.map_or(true, |pt| !g.teams[pt].vassal[unit_team]) {
                    //FfH: Modified by Kael 04/19/2009
                    if g.units[u].duration == 0 {
                        // CvPlayer::changeNumOutsideUnits (the C++ side effect is UI only)
                        g.players[p].num_outside_units += change;
                    }
                    //FfH: End Modify
                }
            }
        }
    }
}

impl Game {
    // ------------------------------------------------------------------
    // War and peace
    // ------------------------------------------------------------------

    /// C++ `CvTeam::canChangeWarPeace` (default `bAllowVassal = false`).
    pub fn team_can_change_war_peace(&self, t: TeamId, other: TeamId, allow_vassal: bool) -> bool {
        if is_option(self, GameOptionType::GAMEOPTION_NO_CHANGING_WAR_PEACE) {
            return false;
        }

        if other == t {
            return false;
        }

        if self.teams[t].permanent_war_peace[other] || self.teams[other].permanent_war_peace[t] {
            return false;
        }

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                let k = &self.teams[o];
                if k.vassal[t] && k.permanent_war_peace[other] {
                    return false;
                }

                if k.vassal[other] && k.permanent_war_peace[t] {
                    return false;
                }
            }
        }

        if self.team_is_a_vassal(t) {
            return false;
        }

        if allow_vassal {
            if self.teams[other].vassal[t] {
                return false;
            }
        } else if self.team_is_a_vassal(other) {
            return false;
        }

        true
    }

    /// C++ `CvTeam::canDeclareWar` (includes the `canDeclareWar` Python callback when
    /// `USE_CAN_DECLARE_WAR_CALLBACK` is set).
    pub fn team_can_declare_war(&self, t: TeamId, other: TeamId) -> bool {
        if other == t {
            return false;
        }

        if !self.team_is_alive(t) || !self.team_is_alive(other) {
            return false;
        }

        if self.teams[t].at_war[other] {
            return false;
        }

        if !self.teams[t].has_met[other] {
            return false;
        }

        if self.teams[t].force_peace[other] {
            return false;
        }

        for o in self.team_ids() {
            if o != other && o != t && self.teams[other].vassal[o] {
                if self.teams[t].force_peace[o] {
                    return false;
                }
            }
        }

        if !self.team_can_change_war_peace(t, other, true) {
            return false;
        }

        if self.is_option(GameOptionType::GAMEOPTION_ALWAYS_PEACE) {
            return false;
        }

        if self.d.defines.use_can_declare_war_callback != 0 {
            // C++ `if (lResult == 0) return false;`
            if !py_can_declare_war(self, t, other) {
                return false;
            }
        }

        true
    }

    /// C++ `CvTeam::declareWar`. `war_plan = None` is C++ `NO_WARPLAN`.
    pub fn team_declare_war(&mut self, t: TeamId, other: TeamId, new_diplo: bool, war_plan: Option<WarPlanType>) {
        debug_assert!(other != t, "eTeam is not expected to be equal with getID()");

        if !self.teams[t].at_war[other] {
            //FfH: Added by Kael 11/29/2007
            for p in self.player_ids() {
                if self.players[p].team == t {
                    self.players[p].declaring_war = true;
                }
            }
            //FfH: End Add

            for d in self.deals.ids() {
                // C++ nextDeal never visits a deal deleted meanwhile (killing a deal can end the
                // two teams' other deals of the same kind).
                if !self.deals.contains(d) {
                    continue;
                }

                let first_team = self.players[self.deals[d].first_player].team;
                let second_team = self.players[self.deals[d].second_player].team;
                if (first_team == t && second_team == other) || (first_team == other && second_team == t) {
                    self.deal_kill(d, true);
                }
            }

            for p in self.player_ids() {
                if self.players[p].team == t || self.players[p].team == other {
                    self.player_update_plunder(p, -1, false);
                }
            }

            self.teams[t].at_war[other] = true;
            self.teams[other].at_war[t] = true;

            for p in self.player_ids() {
                if self.players[p].team == t || self.players[p].team == other {
                    self.player_update_plunder(p, 1, false);
                }
            }

            self.team_meet(t, other, false);

            self.teams[t].ai_at_peace_counter[other] = 0;
            self.teams[other].ai_at_peace_counter[t] = 0;

            self.teams[t].ai_share_war_counter[other] = 0;
            self.teams[other].ai_share_war_counter[t] = 0;

            let attacked_plan = if self.team_is_barbarian(t) || self.team_is_minor_civ(t) {
                WarPlanType::WARPLAN_ATTACKED
            } else {
                WarPlanType::WARPLAN_ATTACKED_RECENT
            };
            self.team_ai_set_war_plan(other, t, Some(attacked_plan), true);

            for o in self.team_ids() {
                if self.team_is_alive(o) {
                    if !self.teams[other].at_war[o] && self.team_ai_is_chosen_war(other, o) {
                        self.team_ai_set_war_plan(other, o, None, true);
                    }
                }
            }

            if war_plan.is_some() {
                self.team_ai_set_war_plan(t, other, war_plan, true);
            }

            // C++ FAssert(!AI_isSneakAttackPreparing(eTeam)) (debug check only; handled below).
            if self.teams[t].ai_war_plan[other].is_none() || self.team_ai_is_sneak_attack_preparing(t, other) {
                if self.team_is_human(t) {
                    self.team_ai_set_war_plan(t, other, Some(WarPlanType::WARPLAN_TOTAL), true);
                } else if self.team_is_minor_civ(t) || self.team_is_barbarian(t) || (self.team_get_at_war_count(other, true) == 1) {
                    self.team_ai_set_war_plan(t, other, Some(WarPlanType::WARPLAN_LIMITED), true);
                } else {
                    self.team_ai_set_war_plan(t, other, Some(WarPlanType::WARPLAN_DOGPILE), true);
                }
            }

            self.map_verify_unit_valid_plot();

            for p in self.player_ids() {
                if self.players[p].team == t {
                    self.player_verify_unit_stacks_valid(p);
                }
            }

            game_ai_make_assign_work_dirty(self);

            // UI: Score / CityInfo dirty bits for the active team.

            for i in self.player_ids() {
                if self.players[i].alive {
                    for j in self.player_ids() {
                        if self.players[j].alive {
                            if (self.players[i].team == t) && (self.players[j].team == other) {
                                // AI_setFirstContact (AI bookkeeping)
                                self.players[i].ai_first_contact[j] = true;
                                self.players[j].ai_first_contact[i] = true;
                            }
                        }
                    }
                }
            }

            for i in self.player_ids() {
                if self.players[i].alive {
                    for j in self.player_ids() {
                        if self.players[j].alive {
                            if self.players[i].team == t {
                                let j_team = self.players[j].team;
                                if j_team == other {
                                    // AI_changeMemoryCount (AI bookkeeping)
                                    self.players[j].ai_memory_count[i][MemoryType::MEMORY_DECLARED_WAR] += 1;
                                } else if j_team != t {
                                    if self.teams[j_team].has_met[other] {
                                        if (self.team_ai_get_attitude(j_team, other, true) >= AttitudeType::ATTITUDE_PLEASED) && !self.teams[j_team].at_war[other] {
                                            self.players[j].ai_memory_count[i][MemoryType::MEMORY_DECLARED_WAR_ON_FRIEND] += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            for p in self.player_ids() {
                if self.players[p].alive {
                    if (self.players[p].team == t) || (self.players[p].team == other) {
                        self.player_update_war_weariness_percent_anger(p);
                    }
                }
            }

            for p in self.player_ids() {
                if self.players[p].alive {
                    if (self.players[p].team == t) || (self.players[p].team == other) {
                        self.player_update_plot_groups(p);
                    }
                }
            }

            for p in self.player_ids() {
                if self.players[p].alive {
                    if (self.players[p].team == t) || (self.players[p].team == other) {
                        self.player_update_trade_routes(p);
                    }
                }
            }

            // C++ `isFinalInitialized() && !GetWorldBuilderMode()` (no world builder here).
            if self.final_initialized {
                // UI: with bNewDiplo, a non-human declaring team opens the
                // AI_DIPLOCOMMENT_DECLARE_WAR diplomacy screen with every human member of `other`
                // its leader can contact (no game-state effect).

                // TXT_KEY_MISC_YOU_DECLARED_WAR_ON / DECLARED_WAR_ON_YOU / SOMEONE_DECLARED_WAR messages
                // and the replay message, between non-barbarian, non-minor teams only.
                if self.log.enabled
                    && !self.team_is_barbarian(t)
                    && !self.team_is_barbarian(other)
                    && !self.team_is_minor_civ(t)
                    && !self.team_is_minor_civ(other)
                {
                    let turn = self.game_turn();
                    self.log.msg(turn, None, || format!("Team {} declared war on team {}", t, other));
                }
            }

            if !self.team_is_minor_civ(other) {
                for i in self.player_ids() {
                    if self.players[i].alive {
                        if self.players[i].team == t {
                            for d in self.deals.ids() {
                                if !self.deals.contains(d) {
                                    continue;
                                }

                                let mut cancel_deal = false;

                                let deal = &self.deals[d];
                                if (deal.first_player == i) || (deal.second_player == i) {
                                    if has_trade_item(&deal.first_trades, &[TradeableItem::TRADE_DEFENSIVE_PACT]) {
                                        cancel_deal = true;
                                    }

                                    if has_trade_item(&deal.second_trades, &[TradeableItem::TRADE_DEFENSIVE_PACT]) {
                                        cancel_deal = true;
                                    }
                                }

                                if cancel_deal {
                                    self.deal_kill(d, true);
                                }
                            }
                        }
                    }
                }
            }

            self.ev_change_war(true, t, other);

            self.team_cancel_defensive_pacts(t);

            for o in self.team_ids() {
                if self.team_is_alive(o) {
                    if self.teams[o].defensive_pact[other] {
                        self.team_declare_war(o, t, new_diplo, Some(WarPlanType::WARPLAN_DOGPILE));
                    }
                }
            }

            self.team_cancel_defensive_pacts(other);

            for o in self.team_ids() {
                if o != t && o != other {
                    if self.team_is_alive(o) {
                        if self.teams[o].vassal[other] || self.teams[other].vassal[o] {
                            let plan = self.teams[t].ai_war_plan[other];
                            self.team_declare_war(t, o, new_diplo, plan);
                        } else if self.teams[o].vassal[t] || self.teams[t].vassal[o] {
                            self.team_declare_war(o, other, new_diplo, Some(WarPlanType::WARPLAN_DOGPILE));
                        }
                    }
                }
            }

            //FfH: Added by Kael 11/29/2007
            for p in self.player_ids() {
                if self.players[p].team == t {
                    self.players[p].declaring_war = false;
                }
            }
            //FfH: End Add
        }
    }

    /// C++ `CvTeam::makePeace` (default `bBumpUnits = true`).
    pub fn team_make_peace(&mut self, t: TeamId, other: TeamId, bump_units: bool) {
        debug_assert!(other != t, "eTeam is not expected to be equal with getID()");

        if self.teams[t].at_war[other] {
            for p in self.player_ids() {
                if self.players[p].team == t || self.players[p].team == other {
                    self.player_update_plunder(p, -1, false);
                }
            }

            self.teams[t].at_war[other] = false;
            self.teams[other].at_war[t] = false;

            for p in self.player_ids() {
                if self.players[p].team == t || self.players[p].team == other {
                    self.player_update_plunder(p, 1, false);
                }
            }

            self.teams[t].ai_at_war_counter[other] = 0;
            self.teams[other].ai_at_war_counter[t] = 0;

            self.teams[t].ai_war_success[other] = 0;
            self.teams[other].ai_war_success[t] = 0;

            self.team_ai_set_war_plan(t, other, None, true);
            self.team_ai_set_war_plan(other, t, None, true);

            if bump_units {
                self.map_verify_unit_valid_plot();
            }

            game_ai_make_assign_work_dirty(self);

            // UI: Score / CityInfo dirty bits for the active team.

            for p in self.player_ids() {
                if self.players[p].alive {
                    if (self.players[p].team == t) || (self.players[p].team == other) {
                        self.player_update_war_weariness_percent_anger(p);
                    }
                }
            }

            for p in self.player_ids() {
                if self.players[p].alive {
                    if (self.players[p].team == t) || (self.players[p].team == other) {
                        self.player_update_plot_groups(p);
                    }
                }
            }

            for p in self.player_ids() {
                if self.players[p].alive {
                    if (self.players[p].team == t) || (self.players[p].team == other) {
                        self.player_update_trade_routes(p);
                    }
                }
            }

            // TXT_KEY_MISC_YOU_MADE_PEACE_WITH / SOMEONE_MADE_PEACE messages and the replay message.
            let turn = self.game_turn();
            self.log.msg(turn, None, || format!("Team {} made peace with team {}", t, other));

            self.ev_change_war(false, t, other);

            for o in self.team_ids() {
                if o != t && o != other {
                    if self.team_is_alive(o) {
                        if self.teams[o].vassal[other] {
                            self.team_make_peace(o, t, bump_units);
                        } else if self.teams[o].vassal[t] {
                            self.team_make_peace(o, other, bump_units);
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::getAtWarCount`.
    pub fn team_get_at_war_count(&self, t: TeamId, ignore_minors: bool) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if !ignore_minors || !self.team_is_minor_civ(o) {
                    if self.teams[t].at_war[o] {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getWarPlanCount`.
    pub fn team_get_war_plan_count(&self, t: TeamId, war_plan: WarPlanType, ignore_minors: bool) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if !ignore_minors || !self.team_is_minor_civ(o) {
                    if self.teams[t].ai_war_plan[o] == Some(war_plan) {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getAnyWarPlanCount`.
    pub fn team_get_any_war_plan_count(&self, t: TeamId, ignore_minors: bool) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if !ignore_minors || !self.team_is_minor_civ(o) {
                    if self.teams[t].ai_war_plan[o].is_some() {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getChosenWarCount`.
    pub fn team_get_chosen_war_count(&self, t: TeamId, ignore_minors: bool) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if !ignore_minors || !self.team_is_minor_civ(o) {
                    if self.team_ai_is_chosen_war(t, o) {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// C++ `CvTeam::doWarWeariness` (protected; called from `doTurn`).
    pub fn team_do_war_weariness(&mut self, t: TeamId) {
        for o in self.team_ids() {
            if self.teams[t].war_weariness[o] > 0 {
                self.team_change_war_weariness(t, o, 100 * self.d.defines.ww_decay_rate);

                if !self.team_is_alive(o)
                    || !self.teams[t].at_war[o]
                    || is_option(self, GameOptionType::GAMEOPTION_ALWAYS_WAR)
                    || is_option(self, GameOptionType::GAMEOPTION_NO_CHANGING_WAR_PEACE)
                {
                    let ww = (self.teams[t].war_weariness[o] * self.d.defines.ww_decay_peace_percent) / 100;
                    self.team_set_war_weariness(t, o, ww);
                }
            }
        }
    }

    /// C++ `CvTeam::setWarWeariness` (clamps at 0).
    pub fn team_set_war_weariness(&mut self, t: TeamId, other: TeamId, value: i32) {
        self.teams[t].war_weariness[other] = value.max(0);
    }

    /// C++ `CvTeam::changeWarWeariness(TeamTypes, int)`.
    pub fn team_change_war_weariness(&mut self, t: TeamId, other: TeamId, change: i32) {
        let v = self.teams[t].war_weariness[other] + change;
        self.team_set_war_weariness(t, other, v);
    }

    /// C++ `CvTeam::changeWarWeariness(TeamTypes eOtherTeam, const CvPlot& kPlot, int iFactor)`
    /// (overload: weighted by the plot's culture split).
    pub fn team_change_war_weariness_at_plot(&mut self, t: TeamId, other: TeamId, plot: PlotIdx, factor: i32) {
        let our_culture = self.plot_count_friendly_culture(plot, t);
        let their_culture = self.plot_count_friendly_culture(plot, other);

        let mut ratio = 100;
        if 0 != our_culture + their_culture {
            ratio = (100 * their_culture) / (our_culture + their_culture);
        }

        self.team_change_war_weariness(t, other, ratio * factor);
    }

    // ------------------------------------------------------------------
    // Contact
    // ------------------------------------------------------------------

    /// C++ `CvTeam::canContact`.
    pub fn team_can_contact(&self, t: TeamId, other: TeamId) -> bool {
        for i in self.player_ids() {
            if self.players[i].alive {
                if self.players[i].team == t {
                    for j in self.player_ids() {
                        if self.players[j].alive {
                            if self.players[j].team == other {
                                if self.player_can_contact(i, j) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// C++ `CvTeam::meet`.
    pub fn team_meet(&mut self, t: TeamId, other: TeamId, new_diplo: bool) {
        if !self.teams[t].has_met[other] {
            self.team_make_has_met(t, other, new_diplo);
            self.team_make_has_met(other, t, new_diplo);
        }
    }

    /// C++ `CvTeam::makeHasMet`.
    pub fn team_make_has_met(&mut self, t: TeamId, other: TeamId, new_diplo: bool) {
        if !self.teams[t].has_met[other] {
            self.teams[t].has_met[other] = true;

            self.team_update_tech_share(t);

            if self.team_is_human(other) {
                for p in self.player_ids() {
                    if self.players[p].alive {
                        if self.players[p].team == t {
                            if !self.players[p].human {
                                self.player_clear_research_queue(p);
                                self.player_ai_make_production_dirty(p);
                            }
                        }
                    }
                }
            }

            for o in civ_team_ids(self) {
                if self.team_is_alive(o) {
                    if self.teams[o].vassal[t] || self.teams[t].vassal[o] {
                        self.team_meet(o, other, new_diplo);
                    }
                }
            }

            // UI: Score dirty bit for the active team.

            if self.is_option(GameOptionType::GAMEOPTION_ALWAYS_WAR) {
                if self.team_is_human(t) && t != other {
                    self.team_declare_war(t, other, false, None);
                }
            } else {
                // UI: once final-initialized, with bNewDiplo, a non-human team at peace with `other`
                // opens the AI_DIPLOCOMMENT_FIRST_CONTACT diplomacy screen with every human member
                // of `other` its leader can contact (no game-state effect).
            }

            // report event to Python, along with some other key state
            self.ev_first_contact(t, other);
        }
    }

    /// C++ `CvTeam::getHasMetCivCount`.
    pub fn team_get_has_met_civ_count(&self, t: TeamId, ignore_minors: bool) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if o != t {
                    if !ignore_minors || !self.team_is_minor_civ(o) {
                        if self.teams[t].has_met[o] {
                            count += 1;
                        }
                    }
                }
            }
        }

        count
    }

    /// C++ `CvTeam::hasMetHuman`.
    pub fn team_has_met_human(&self, t: TeamId) -> bool {
        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if o != t {
                    if self.team_is_human(o) {
                        if self.teams[t].has_met[o] {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    // ------------------------------------------------------------------
    // Open borders, defensive pacts, force peace
    // ------------------------------------------------------------------

    /// C++ `CvTeam::signOpenBorders`.
    pub fn team_sign_open_borders(&mut self, t: TeamId, other: TeamId) {
        self.team_sign_dual_item(t, other, TradeableItem::TRADE_OPEN_BORDERS);
    }

    /// C++ `CvTeam::signDefensivePact`.
    pub fn team_sign_defensive_pact(&mut self, t: TeamId, other: TeamId) {
        self.team_sign_dual_item(t, other, TradeableItem::TRADE_DEFENSIVE_PACT);
    }

    /// The common body of C++ `CvTeam::signOpenBorders` / `signDefensivePact`: when at peace
    /// and both leaders can trade the item, implement the dual deal between the leaders.
    fn team_sign_dual_item(&mut self, t: TeamId, other: TeamId, item_type: TradeableItem) {
        debug_assert!(other != t);

        if !self.teams[t].at_war[other] && (t != other) {
            let item = set_trade_item(item_type, 0);

            let leader = leader_of(self, t);
            let other_leader = leader_of(self, other);

            if self.player_can_trade_item(leader, other_leader, &item, false) && self.player_can_trade_item(other_leader, leader, &item, false) {
                let our_list = vec![item.clone()];
                let their_list = vec![item];

                self.implement_deal(leader, other_leader, &our_list, &their_list, false);
            }
        }
    }

    /// C++ `CvTeam::canSignDefensivePact` (non-const in C++, read-only).
    pub fn team_can_sign_defensive_pact(&self, t: TeamId, other: TeamId) -> bool {
        for o in civ_team_ids(self) {
            if o != t && o != other {
                let k = &self.teams[o];
                if k.permanent_war_peace[other] != k.permanent_war_peace[t] {
                    return false;
                }

                if self.teams[t].permanent_war_peace[o] != self.teams[other].permanent_war_peace[o] {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvTeam::getDefensivePactCount` (`other = None` is C++ `NO_TEAM`: count all).
    pub fn team_get_defensive_pact_count(&self, t: TeamId, other: Option<TeamId>) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if o != t {
                    if self.teams[t].defensive_pact[o] {
                        if other.map_or(true, |e| self.teams[e].has_met[o]) {
                            count += 1;
                        }
                    }
                }
            }
        }

        count
    }

    /// C++ `CvTeam::isFreeTrade`.
    pub fn team_is_free_trade(&self, t: TeamId, other: TeamId) -> bool {
        if self.teams[t].at_war[other] {
            return false;
        }

        if !self.teams[t].has_met[other] {
            return false;
        }

        self.team_is_open_borders(t, other) || self.is_free_trade()
    }

    /// C++ `CvTeam::isOpenBorders`. FFH: always true under GAMEOPTION_ALWAYS_OPEN_BORDERS,
    /// so never read `Team::open_borders` directly for rules.
    pub fn team_is_open_borders(&self, t: TeamId, other: TeamId) -> bool {
        //FfH: Added by Kael 11/17/2008
        if is_option(self, GameOptionType::GAMEOPTION_ALWAYS_OPEN_BORDERS) {
            return true;
        }
        //FfH: End Add

        self.teams[t].open_borders[other]
    }

    /// C++ `CvTeam::setOpenBorders`.
    pub fn team_set_open_borders(&mut self, t: TeamId, other: TeamId, value: bool) {
        // C++ quirk: the test reads isOpenBorders, which FFH answers true for every pair under
        // GAMEOPTION_ALWAYS_OPEN_BORDERS; then only closing borders (`value = false`) gets past
        // it and clears the raw flag (with the side effects below).
        if self.team_is_open_borders(t, other) != value {
            let old_free_trade = self.team_is_free_trade(t, other);

            self.teams[t].open_borders[other] = value;

            // AI_setOpenBordersCounter (AI bookkeeping)
            self.teams[t].ai_open_borders_counter[other] = 0;

            self.map_verify_unit_valid_plot();

            // UI: Score dirty bit for the active team.

            if old_free_trade != self.team_is_free_trade(t, other) {
                for p in self.player_ids() {
                    if self.players[p].alive {
                        if self.players[p].team == t {
                            self.player_update_trade_routes(p);
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::setDefensivePact`.
    pub fn team_set_defensive_pact(&mut self, t: TeamId, other: TeamId, value: bool) {
        if self.teams[t].defensive_pact[other] != value {
            self.teams[t].defensive_pact[other] = value;

            // UI: Score_DIRTY_BIT for the active team.
            // UI: when signing (value && !GET_TEAM(other).isDefensivePact(t)): replay message and
            // addMessage TXT_KEY_MISC_PLAYERS_SIGN_DEFENSIVE_PACT to players who met both teams.
        }
    }

    /// C++ `CvTeam::setForcePeace`.
    pub fn team_set_force_peace(&mut self, t: TeamId, other: TeamId, value: bool) {
        self.teams[t].force_peace[other] = value;

        if self.teams[t].force_peace[other] {
            if self.team_ai_is_sneak_attack_preparing(t, other) {
                self.team_ai_set_war_plan(t, other, None, true);
            }

            for o in self.team_ids() {
                if self.teams[o].vassal[other] {
                    if self.team_ai_is_sneak_attack_preparing(t, o) {
                        self.team_ai_set_war_plan(t, o, None, true);
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::cancelDefensivePacts` (protected).
    pub fn team_cancel_defensive_pacts(&mut self, t: TeamId) {
        for d in self.deals.ids() {
            // C++ nextDeal never visits a deal deleted meanwhile (killing a deal ends the
            // teams' other defensive pact deals).
            if !self.deals.contains(d) {
                continue;
            }

            let mut cancel_deal = false;

            let deal = &self.deals[d];
            if (self.players[deal.first_player].team == t) || (self.players[deal.second_player].team == t) {
                if has_trade_item(&deal.first_trades, &[TradeableItem::TRADE_DEFENSIVE_PACT]) {
                    cancel_deal = true;
                }

                if !cancel_deal {
                    if has_trade_item(&deal.second_trades, &[TradeableItem::TRADE_DEFENSIVE_PACT]) {
                        cancel_deal = true;
                    }
                }
            }

            if cancel_deal {
                self.deal_kill(d, true);
            }
        }
    }

    /// C++ `CvTeam::isFriendlyTerritory` (`other = None` is C++ `NO_TEAM`).
    pub fn team_is_friendly_territory(&self, t: TeamId, other: Option<TeamId>) -> bool {
        let Some(other) = other else {
            return false;
        };

        if other == t {
            return true;
        }

        if self.teams[other].vassal[t] {
            return true;
        }

        if self.teams[t].vassal[other] && self.team_is_open_borders(t, other) {
            return true;
        }

        false
    }

    // ------------------------------------------------------------------
    // Vassals
    // ------------------------------------------------------------------

    /// C++ `CvTeam::getVassalCount` (`other = None` is C++ `NO_TEAM`: count all).
    pub fn team_get_vassal_count(&self, t: TeamId, other: Option<TeamId>) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if o != t {
                    if self.teams[o].vassal[t] {
                        if other.map_or(true, |e| self.teams[e].has_met[o]) {
                            count += 1;
                        }
                    }
                }
            }
        }

        count
    }

    /// C++ `CvTeam::isAVassal`.
    pub fn team_is_a_vassal(&self, t: TeamId) -> bool {
        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if o != t {
                    if self.teams[t].vassal[o] {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// C++ `CvTeam::canVassalRevolt`.
    pub fn team_can_vassal_revolt(&self, t: TeamId, master: TeamId) -> bool {
        let defs = &self.d.defines;

        if self.teams[t].vassal[master] {
            if 100 * self.team_get_total_land(t, false) < defs.vassal_revolt_own_losses_factor * self.teams[t].vassal_power {
                return true;
            }

            if 100 * self.team_get_total_land(master, true) < defs.vassal_revolt_master_losses_factor * self.teams[t].master_power {
                return true;
            }
        }

        if defs.free_vassal_land_percent < 0
            || 100 * self.team_get_total_land(t, false) < self.team_get_total_land(master, false) * defs.free_vassal_land_percent
        {
            return false;
        }

        if defs.free_vassal_population_percent < 0
            || 100 * self.team_get_total_population(t, false)
                < self.team_get_total_population(master, false) * defs.free_vassal_population_percent
        {
            return false;
        }

        true
    }

    /// C++ `CvTeam::setVassal(eIndex, bNewValue, bCapitulated)`: make `t` a vassal of
    /// `master` (or free it).
    pub fn team_set_vassal(&mut self, t: TeamId, master: TeamId, value: bool, capitulated: bool) {
        debug_assert!(!value || !self.team_is_a_vassal(master), "can't become a vassal of a vassal");

        if self.teams[t].vassal[master] != value {
            for p in self.player_ids() {
                if self.players[p].team == t {
                    self.player_update_city_sight(p, false, false);
                }
            }

            change_master_outside_units(self, master, -1);

            self.teams[t].vassal[master] = value;
            // Score v2 moves half of a vassal's people and land to its master (docs/SCORE.md).
            self.score_dirty = true;

            change_master_outside_units(self, master, 1);

            for p in self.player_ids() {
                if self.players[p].team == t {
                    self.player_update_city_sight(p, true, false);
                }
            }

            for p in self.map.plot_ids() {
                let plot_team = self.plot_get_team(p);
                if (plot_team == Some(t)) || (plot_team == Some(master)) {
                    self.plot_update_culture(p, true, false);
                }
            }

            self.update_plot_groups();

            if self.teams[t].vassal[master] {
                self.teams[t].capitulated = capitulated;

                for d in self.deals.ids() {
                    // C++ nextDeal never visits a deal deleted meanwhile.
                    if !self.deals.contains(d) {
                        continue;
                    }

                    let deal = &self.deals[d];
                    if (self.players[deal.first_player].team == t) || (self.players[deal.second_player].team == t) {
                        let ended = [TradeableItem::TRADE_DEFENSIVE_PACT, TradeableItem::TRADE_PEACE_TREATY];

                        // (C++ also checks that the trade lists are not NULL; they always exist.)
                        let mut valid = true;

                        if has_trade_item(&deal.first_trades, &ended) {
                            valid = false;
                        }

                        if valid && has_trade_item(&deal.second_trades, &ended) {
                            valid = false;
                        }

                        if !valid {
                            self.deal_kill(d, true);
                        }
                    }
                }

                self.team_set_force_peace(t, master, false);
                self.team_set_force_peace(master, t, false);

                // Declare war on teams you should be at war with
                for o in self.team_ids() {
                    if (o != t) && (o != master) {
                        if self.team_is_alive(o) {
                            if self.teams[master].has_met[o] {
                                self.team_meet(t, o, true);
                            }

                            if self.teams[t].has_met[o] {
                                self.team_meet(master, o, true);
                            }

                            if self.teams[master].at_war[o] {
                                self.team_declare_war(t, o, false, Some(WarPlanType::WARPLAN_DOGPILE));
                            } else if self.teams[t].at_war[o] {
                                if capitulated {
                                    self.team_make_peace(t, o, true);
                                } else {
                                    self.team_declare_war(master, o, false, Some(WarPlanType::WARPLAN_DOGPILE));
                                }
                            }
                        }
                    }
                }

                for o in self.team_ids() {
                    if self.team_is_alive(o) {
                        if !self.teams[o].at_war[t] {
                            self.team_ai_set_war_plan(o, t, None, true);
                            self.team_ai_set_war_plan(t, o, None, true);
                        }

                        if !self.teams[o].at_war[master] {
                            self.team_ai_set_war_plan(o, master, None, true);
                        }
                    }
                }

                // All our vassals now become free
                for o in self.team_ids() {
                    if (o != t) && (o != master) {
                        if self.team_is_alive(o) && self.teams[o].vassal[t] {
                            self.team_free_vassal(t, o);
                        }
                    }
                }

                self.teams[t].master_power = self.team_get_total_land(master, true);
                self.teams[t].vassal_power = self.team_get_total_land(t, false);

                // C++ `isFinalInitialized() && !GetWorldBuilderMode()` (no world builder here).
                if self.final_initialized {
                    // TXT_KEY_MISC_CAPITULATE_AGREEMENT / TXT_KEY_MISC_VASSAL_AGREEMENT replay message,
                    // also sent to every alive player who has met both teams.
                    let kind = if capitulated { "capitulated to" } else { "became a vassal of" };
                    let turn = self.game_turn();
                    self.log.msg(turn, None, || format!("Team {} {} team {}", t, kind, master));
                }
            } else {
                self.teams[t].master_power = 0;
                self.teams[t].vassal_power = 0;

                if self.final_initialized && self.team_is_alive(t) && self.team_is_alive(master) {
                    // TXT_KEY_MISC_SURRENDER_REVOLT / TXT_KEY_MISC_VASSAL_REVOLT (by the old
                    // m_bCapitulated) replay message and messages.
                    let kind = if self.teams[t].capitulated { "revolted against its surrender to" } else { "revolted against its master" };
                    let turn = self.game_turn();
                    self.log.msg(turn, None, || format!("Team {} {} team {}", t, kind, master));
                }

                self.teams[t].capitulated = false;
            }

            for p in self.player_ids() {
                if self.players[p].team == master {
                    self.player_update_maintenance(p);
                }
            }

            if self.final_initialized {
                self.ev_vassal_state(master, t, value);
            }
        }
    }

    /// C++ `CvTeam::assignVassal` (`const` in C++ but mutates other teams and deals).
    pub fn team_assign_vassal(&mut self, t: TeamId, vassal: TeamId, surrender: bool) {
        self.team_set_vassal(vassal, t, true, surrender);

        let mut item = set_trade_item(if surrender { TradeableItem::TRADE_SURRENDER } else { TradeableItem::TRADE_VASSAL }, 0);
        item.data = 1;

        for p in self.player_ids() {
            if self.players[p].team == vassal && self.players[p].alive {
                let our_list: Vec<TradeData> = Vec::new();
                let their_list = vec![item.clone()];

                for j in self.player_ids() {
                    if self.players[j].team == t {
                        self.implement_deal(j, p, &our_list, &their_list, true);
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::freeVassal` (`const` in C++ but kills deals).
    pub fn team_free_vassal(&mut self, t: TeamId, vassal: TeamId) {
        let vassal_items = [TradeableItem::TRADE_VASSAL, TradeableItem::TRADE_SURRENDER];

        for d in self.deals.ids() {
            // C++ nextDeal never visits a deal deleted meanwhile.
            if !self.deals.contains(d) {
                continue;
            }

            let mut valid = true;

            let deal = &self.deals[d];
            let first_team = self.players[deal.first_player].team;
            let second_team = self.players[deal.second_player].team;

            if (first_team == vassal) && (second_team == t) {
                if has_trade_item(&deal.first_trades, &vassal_items) {
                    valid = false;
                }
            }

            if valid {
                if (first_team == t) && (second_team == vassal) {
                    if has_trade_item(&deal.second_trades, &vassal_items) {
                        valid = false;
                    }
                }
            }

            if !valid {
                self.deal_kill(d, true);
            }
        }
    }

    /// C++ `CvTeam::isParent`.
    pub fn team_is_parent(&self, t: TeamId, other: TeamId) -> bool {
        if self.teams[other].vassal[t] {
            for p in self.player_ids() {
                if self.players[p].alive && self.players[p].team == other {
                    if let Some(parent) = self.players[p].parent {
                        if self.players[parent].team == t {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    // ------------------------------------------------------------------
    // CvTeamAI bookkeeping read/written by rules code
    // ------------------------------------------------------------------

    /// C++ `CvTeamAI::AI_setWarPlan` (C++ default `bWar = true`). `plan = None` is `NO_WARPLAN`.
    pub fn team_ai_set_war_plan(&mut self, t: TeamId, other: TeamId, plan: Option<WarPlanType>, war: bool) {
        if self.teams[t].ai_war_plan[other] != plan {
            if war || !self.teams[t].at_war[other] {
                self.teams[t].ai_war_plan[other] = plan;

                // AI_setWarPlanStateCounter(eIndex, 0)
                self.teams[t].ai_war_plan_state_counter[other] = 0;

                // AI: AI_updateAreaStragies() (area AI types, AI-only state).

                for p in self.player_ids() {
                    if self.players[p].alive {
                        if self.players[p].team == t {
                            if !self.players[p].human {
                                // AI_makeProductionDirty (city bookkeeping flag)
                                self.player_ai_make_production_dirty(p);
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvTeamAI::AI_isChosenWar`.
    pub fn team_ai_is_chosen_war(&self, t: TeamId, other: TeamId) -> bool {
        match self.teams[t].ai_war_plan[other] {
            Some(WarPlanType::WARPLAN_ATTACKED_RECENT) | Some(WarPlanType::WARPLAN_ATTACKED) => false,
            Some(WarPlanType::WARPLAN_PREPARING_LIMITED)
            | Some(WarPlanType::WARPLAN_PREPARING_TOTAL)
            | Some(WarPlanType::WARPLAN_LIMITED)
            | Some(WarPlanType::WARPLAN_TOTAL)
            | Some(WarPlanType::WARPLAN_DOGPILE) => true,
            _ => false,
        }
    }

    /// C++ `CvTeamAI::AI_isSneakAttackPreparing`.
    pub fn team_ai_is_sneak_attack_preparing(&self, t: TeamId, other: TeamId) -> bool {
        let plan = self.teams[t].ai_war_plan[other];
        plan == Some(WarPlanType::WARPLAN_PREPARING_LIMITED) || plan == Some(WarPlanType::WARPLAN_PREPARING_TOTAL)
    }

    /// C++ `CvTeamAI::AI_isSneakAttackReady`.
    pub fn team_ai_is_sneak_attack_ready(&self, t: TeamId, other: TeamId) -> bool {
        self.team_ai_is_chosen_war(t, other) && !self.team_ai_is_sneak_attack_preparing(t, other)
    }

    /// C++ `CvTeamAI::AI_shareWar`.
    pub fn team_ai_share_war(&self, t: TeamId, other: TeamId) -> bool {
        for o in civ_team_ids(self) {
            if self.team_is_alive(o) && !self.team_is_minor_civ(o) {
                if o != t && o != other {
                    if self.teams[t].at_war[o] && self.teams[other].at_war[o] {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// C++ `CvTeamAI::AI_getAttitude` (team average of the member players' attitudes;
    /// C++ default `bForced = true`). Read by rules (declareWar memory, CvUnit, CvPlayer).
    ///
    /// Same averaging as the C++: over every pair of an alive member `i` of `t` and another
    /// alive player `j` (`i != j`) of `other` or of a team that is `other`'s vassal or master,
    /// the sum of `i`'s attitudes towards `j` as `AttitudeTypes` values (0 = FURIOUS ..
    /// 4 = FRIENDLY), integer-divided by the number of pairs; CAUTIOUS without pairs. The
    /// player attitude (C++ `CvPlayerAI::AI_getAttitude`) is the controller's
    /// (`Controller::attitude`, the new AI's relations).
    /// `forced` applies the C++ `bForced` overrides (`Game::player_ai_get_attitude`).
    pub fn team_ai_get_attitude(&self, t: TeamId, other: TeamId, forced: bool) -> AttitudeType {
        debug_assert!(other != t, "shouldn't call this function on ourselves");

        let mut attitude = 0;
        let mut count = 0;

        for i in self.player_ids() {
            if self.players[i].alive {
                if self.players[i].team == t {
                    for j in self.player_ids() {
                        if self.players[j].alive && i != j {
                            let team_loop = self.players[j].team;
                            if team_loop == other || self.teams[team_loop].vassal[other] || self.teams[other].vassal[team_loop] {
                                attitude += self.player_ai_get_attitude(i, j, forced).i();
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        if count > 0 {
            return AttitudeType::from_usize((attitude / count) as usize);
        }

        AttitudeType::ATTITUDE_CAUTIOUS
    }

    /// C++ `CvTeamAI::AI_getAttitudeVal` (C++ default `bForced = true`): the average, over every
    /// pair of an alive member of `t` and an alive member of `other`, of the member's attitude
    /// value towards the other player (0 without pairs). Read by `team_ai_update_worst_enemy`.
    ///
    /// Deliberate deviation: C++ averages `CvPlayerAI::AI_getAttitudeVal`, the raw attitude
    /// score (roughly -100..100, the source of the AttitudeTypes thresholds); the controller only
    /// gives an `AttitudeType`, so its index (0 = FURIOUS .. 4 = FRIENDLY) is the value here.
    /// `forced`: see `team_ai_get_attitude`.
    pub fn team_ai_get_attitude_val(&self, t: TeamId, other: TeamId, forced: bool) -> i32 {
        debug_assert!(other != t, "shouldn't call this function on ourselves");

        let mut attitude_val = 0;
        let mut count = 0;

        for i in self.player_ids() {
            if self.players[i].alive {
                if self.players[i].team == t {
                    for j in self.player_ids() {
                        if self.players[j].alive {
                            if self.players[j].team == other {
                                attitude_val += self.player_ai_get_attitude(i, j, forced).i();
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        if count > 0 {
            return attitude_val / count;
        }

        0
    }

    /// C++ `CvTeamAI::AI_doCounter` (the bookkeeping part of `AI_doTurnPre`, called from
    /// `CvTeam::doTurn`): advances the per-team contact counters read by rules code.
    pub fn team_ai_do_counter(&mut self, t: TeamId) {
        for o in self.team_ids() {
            if self.team_is_alive(o) {
                if o != t {
                    // AI_changeWarPlanStateCounter
                    self.teams[t].ai_war_plan_state_counter[o] += 1;

                    if self.teams[t].at_war[o] {
                        self.teams[t].ai_at_war_counter[o] += 1;
                    } else {
                        self.teams[t].ai_at_peace_counter[o] += 1;
                    }

                    if self.teams[t].has_met[o] {
                        self.teams[t].ai_has_met_counter[o] += 1;
                    }

                    if self.team_is_open_borders(t, o) {
                        self.teams[t].ai_open_borders_counter[o] += 1;
                    }

                    if self.teams[t].defensive_pact[o] {
                        self.teams[t].ai_defensive_pact_counter[o] += 1;
                    } else if self.teams[t].ai_defensive_pact_counter[o] > 0 {
                        self.teams[t].ai_defensive_pact_counter[o] -= 1;
                    }

                    if self.teams[t].has_met[o] {
                        if self.team_ai_share_war(t, o) {
                            self.teams[t].ai_share_war_counter[o] += 1;
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvTeamAI::AI_updateWorstEnemy` (called from `CvTeam::addTeam` and the
    /// `AI_doTurnPost` part of `CvTeam::doTurn`; the result is read by CvPlayer events/trades).
    pub fn team_ai_update_worst_enemy(&mut self, t: TeamId) {
        let mut best_team = None;
        let mut best_value = MAX_INT;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if o != t && !self.teams[o].vassal[t] {
                    if self.teams[t].has_met[o] {
                        if self.team_ai_get_attitude(t, o, true) < AttitudeType::ATTITUDE_CAUTIOUS {
                            let value = self.team_ai_get_attitude_val(t, o, true);
                            if value < best_value {
                                best_value = value;
                                best_team = Some(o);
                            }
                        }
                    }
                }
            }
        }

        self.teams[t].ai_worst_enemy = best_team;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::Controller;
    use crate::deal::Deal;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;
    use std::sync::Arc;

    /// A game whose first `teams.len()` player slots are alive civs on the given teams (the
    /// other slots are reset, dead players on their own teams).
    fn game(teams: &[u8]) -> Game {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, teams.len(), "WORLDSIZE_DUEL", 7);
        for (i, &t) in teams.iter().enumerate() {
            setup.players[i].team = TeamId(t);
        }
        let mut g = Game::new(d, setup);
        let (n_players, n_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], n_players, n_teams);
        }
        for i in 0..teams.len() {
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

    fn meet(g: &mut Game, a: TeamId, b: TeamId) {
        g.teams[a].has_met[b] = true;
        g.teams[b].has_met[a] = true;
    }

    /// Player attitudes from a table: `0[p][towards]` is an `AttitudeType` index.
    struct FixedAttitudes(Vec<Vec<u16>>);

    impl Controller for FixedAttitudes {
        fn attitude(&self, _g: &Game, p: PlayerId, towards: PlayerId) -> AttitudeType {
            AttitudeType(self.0[p.idx()][towards.idx()])
        }
    }

    /// Every player gets a `FixedAttitudes` controller; attitudes not in `entries` are CAUTIOUS.
    fn set_attitudes(g: &mut Game, entries: &[(u8, u8, AttitudeType)]) {
        let n = g.num_players();
        let mut table = vec![vec![AttitudeType::ATTITUDE_CAUTIOUS.0; n]; n];
        for &(p, towards, attitude) in entries {
            table[p as usize][towards as usize] = attitude.0;
        }
        let controller: Arc<dyn Controller> = Arc::new(FixedAttitudes(table));
        for p in g.player_ids() {
            g.controllers[p] = controller.clone();
        }
    }

    #[test]
    fn declaring_war_needs_contact_and_no_forced_peace() {
        let mut g = game(&[0, 1, 2]);
        let (a, b, c) = (TeamId(0), TeamId(1), TeamId(2));
        assert!(!g.team_can_declare_war(a, a));
        assert!(!g.team_can_declare_war(a, b), "not met");

        meet(&mut g, a, b);
        assert!(g.team_can_declare_war(a, b));

        g.teams[a].force_peace[b] = true;
        assert!(!g.team_can_declare_war(a, b));
        g.teams[a].force_peace[b] = false;

        // forced peace with the target's master
        g.teams[b].vassal[c] = true;
        g.teams[a].force_peace[c] = true;
        assert!(!g.team_can_declare_war(a, b));
        g.teams[a].force_peace[c] = false;
        assert!(g.team_can_declare_war(a, b), "a vassal can be attacked");
        g.teams[b].vassal[c] = false;

        // a vassal cannot change war and peace itself
        meet(&mut g, c, a);
        g.teams[c].vassal[b] = true;
        assert!(!g.team_can_declare_war(c, a));
        g.teams[c].vassal[b] = false;
        assert!(g.team_can_declare_war(c, a));

        g.teams[a].at_war[b] = true;
        g.teams[b].at_war[a] = true;
        assert!(!g.team_can_declare_war(a, b));
        g.teams[a].at_war[b] = false;
        g.teams[b].at_war[a] = false;

        g.teams[a].permanent_war_peace[b] = true;
        assert!(!g.team_can_declare_war(a, b));
        g.teams[a].permanent_war_peace[b] = false;

        g.setup.options[GameOptionType::GAMEOPTION_ALWAYS_PEACE] = true;
        assert!(!g.team_can_declare_war(a, b));
        g.setup.options[GameOptionType::GAMEOPTION_ALWAYS_PEACE] = false;

        g.teams[b].alive_count = 0;
        assert!(!g.team_can_declare_war(a, b), "dead target");
    }

    #[test]
    fn free_trade_needs_contact_peace_and_open_borders() {
        let mut g = game(&[0, 1]);
        let (a, b) = (TeamId(0), TeamId(1));
        g.teams[a].open_borders[b] = true;
        assert!(!g.team_is_free_trade(a, b), "not met");

        meet(&mut g, a, b);
        assert!(g.team_is_free_trade(a, b));
        g.teams[a].open_borders[b] = false;
        assert!(!g.team_is_free_trade(a, b));

        // CvGame free trade (vote effect) instead of open borders
        g.free_trade_count = 1;
        assert!(g.team_is_free_trade(a, b));

        g.teams[a].at_war[b] = true;
        g.teams[b].at_war[a] = true;
        assert!(!g.team_is_free_trade(a, b), "at war");
    }

    #[test]
    fn open_borders_reset_the_counter_and_keep_the_always_open_borders_quirk() {
        let mut g = game(&[0, 1]);
        let (a, b) = (TeamId(0), TeamId(1));
        // (not met: free trade does not change, so no trade route update)
        g.teams[a].ai_open_borders_counter[b] = 7;
        g.team_set_open_borders(a, b, true);
        assert!(g.teams[a].open_borders[b] && g.team_is_open_borders(a, b));
        assert!(!g.teams[b].open_borders[a], "one side only");
        assert_eq!(g.teams[a].ai_open_borders_counter[b], 0);

        g.teams[a].ai_open_borders_counter[b] = 3;
        g.team_set_open_borders(a, b, true);
        assert_eq!(g.teams[a].ai_open_borders_counter[b], 3, "no change, no reset");

        // FFH GAMEOPTION_ALWAYS_OPEN_BORDERS: isOpenBorders is always true, so only closing
        // gets through and it clears the raw flag.
        g.setup.options[GameOptionType::GAMEOPTION_ALWAYS_OPEN_BORDERS] = true;
        g.team_set_open_borders(a, b, false);
        assert!(!g.teams[a].open_borders[b]);
        assert!(g.team_is_open_borders(a, b));
        assert_eq!(g.teams[a].ai_open_borders_counter[b], 0);
        g.team_set_open_borders(a, b, true);
        assert!(!g.teams[a].open_borders[b]);
    }

    #[test]
    fn war_weariness_at_a_plot_follows_the_culture_split() {
        let mut g = game(&[0, 1]);
        let (a, b) = (TeamId(0), TeamId(1));
        let init = MapInitData { grid_w: 2, grid_h: 2, ..Default::default() };
        g.map = Map::new(g.d, &init, g.num_players(), g.num_teams());
        let plot = PlotIdx(0);

        // nobody's culture: the full factor
        g.team_change_war_weariness_at_plot(a, b, plot, 2);
        assert_eq!(g.teams[a].war_weariness[b], 200);

        g.map.plots[plot].culture[PlayerId(0)] = 30;
        g.map.plots[plot].culture[PlayerId(1)] = 10;
        g.team_change_war_weariness_at_plot(a, b, plot, 2);
        assert_eq!(g.teams[a].war_weariness[b], 200 + (100 * 10 / 40) * 2);
        assert_eq!(g.teams[b].war_weariness[a], 0);
    }

    #[test]
    fn parent_is_the_master_of_a_split_vassal() {
        let mut g = game(&[0, 1]);
        let (a, b) = (TeamId(0), TeamId(1));
        g.players[PlayerId(1)].parent = Some(PlayerId(0));
        assert!(!g.team_is_parent(a, b), "not a vassal");
        g.teams[b].vassal[a] = true;
        assert!(g.team_is_parent(a, b));
        assert!(!g.team_is_parent(b, a));
        g.players[PlayerId(1)].alive = false;
        assert!(!g.team_is_parent(a, b), "dead members do not count");
    }

    #[test]
    fn team_attitude_averages_member_attitudes_including_vassals() {
        // players 0 and 1 on team 0, player 2 on team 2, player 3 on team 3 (vassal of team 2)
        let mut g = game(&[0, 0, 2, 3]);
        let (t0, t2, t3) = (TeamId(0), TeamId(2), TeamId(3));
        g.teams[t3].vassal[t2] = true;
        set_attitudes(
            &mut g,
            &[
                (0, 2, AttitudeType::ATTITUDE_FURIOUS),
                (0, 3, AttitudeType::ATTITUDE_ANNOYED),
                (1, 2, AttitudeType::ATTITUDE_ANNOYED),
                (1, 3, AttitudeType::ATTITUDE_ANNOYED),
                (2, 0, AttitudeType::ATTITUDE_PLEASED),
                (2, 1, AttitudeType::ATTITUDE_FRIENDLY),
            ],
        );

        // pairs (0,2) (0,3) (1,2) (1,3): (0 + 1 + 1 + 1) / 4 = 0 (integer division)
        assert_eq!(g.team_ai_get_attitude(t0, t2, true), AttitudeType::ATTITUDE_FURIOUS);
        // towards the vassal: its master's members count too
        assert_eq!(g.team_ai_get_attitude(t0, t3, true), AttitudeType::ATTITUDE_FURIOUS);
        // (3 + 4) / 2 = 3
        assert_eq!(g.team_ai_get_attitude(t2, t0, true), AttitudeType::ATTITUDE_PLEASED);
        // no member to judge: CAUTIOUS
        assert_eq!(g.team_ai_get_attitude(t0, TeamId(1), true), AttitudeType::ATTITUDE_CAUTIOUS);

        // the attitude value only pairs with the team's own members: (0 + 1) / 2
        assert_eq!(g.team_ai_get_attitude_val(t0, t2, true), 0);
        assert_eq!(g.team_ai_get_attitude_val(t0, t3, true), 1);
        assert_eq!(g.team_ai_get_attitude_val(t0, TeamId(1), true), 0);
    }

    #[test]
    fn worst_enemy_is_the_met_team_we_like_least() {
        let mut g = game(&[0, 1, 2]);
        let (a, b, c) = (TeamId(0), TeamId(1), TeamId(2));
        set_attitudes(&mut g, &[(0, 1, AttitudeType::ATTITUDE_ANNOYED), (0, 2, AttitudeType::ATTITUDE_FURIOUS)]);

        g.team_ai_update_worst_enemy(a);
        assert_eq!(g.teams[a].ai_worst_enemy, None, "nobody met");

        meet(&mut g, a, b);
        meet(&mut g, a, c);
        g.team_ai_update_worst_enemy(a);
        assert_eq!(g.teams[a].ai_worst_enemy, Some(c));

        // our own vassals are never the worst enemy
        g.teams[c].vassal[a] = true;
        g.team_ai_update_worst_enemy(a);
        assert_eq!(g.teams[a].ai_worst_enemy, Some(b));

        // only teams below CAUTIOUS qualify
        set_attitudes(&mut g, &[(0, 2, AttitudeType::ATTITUDE_FURIOUS)]);
        g.team_ai_update_worst_enemy(a);
        assert_eq!(g.teams[a].ai_worst_enemy, None);
    }

    fn add_deal(g: &mut Game, first: u8, second: u8, first_trades: &[TradeableItem], second_trades: &[TradeableItem]) -> DealId {
        let d = g.deals.insert_with(|id| Deal::new(id, PlayerId(first), PlayerId(second)));
        g.deals[d].first_trades = first_trades.iter().map(|&i| set_trade_item(i, 1)).collect();
        g.deals[d].second_trades = second_trades.iter().map(|&i| set_trade_item(i, 1)).collect();
        d
    }

    #[test]
    fn cancelling_defensive_pacts_kills_only_the_pact_deals() {
        let mut g = game(&[0, 1, 2]);
        let (a, b) = (TeamId(0), TeamId(1));
        let dp = [TradeableItem::TRADE_DEFENSIVE_PACT];
        let pact = add_deal(&mut g, 0, 1, &dp, &dp);
        g.teams[a].defensive_pact[b] = true;
        g.teams[b].defensive_pact[a] = true;
        // a pact between two other players' teams survives
        let other_pact = add_deal(&mut g, 1, 2, &dp, &dp);
        // a deal of team `a` without a pact survives
        let pp = [TradeableItem::TRADE_PEACE_TREATY];
        let peace = add_deal(&mut g, 2, 0, &pp, &pp);

        g.team_cancel_defensive_pacts(a);
        assert!(!g.deals.contains(pact));
        assert!(!g.teams[a].defensive_pact[b] && !g.teams[b].defensive_pact[a]);
        assert!(g.deals.contains(other_pact));
        assert!(g.deals.contains(peace));
    }

    #[test]
    fn freeing_a_vassal_kills_the_vassal_deals_it_gives() {
        let mut g = game(&[0, 1, 2]);
        let (a, b) = (TeamId(0), TeamId(1));
        let vassal = [TradeableItem::TRADE_VASSAL];
        let surrender = [TradeableItem::TRADE_SURRENDER];
        // the vassal (team b) gives the vassal item to its master (team a), as either party
        let given_first = add_deal(&mut g, 1, 0, &vassal, &[]);
        let given_second = add_deal(&mut g, 0, 1, &[], &surrender);
        // the master giving such an item, or a vassal item with a third team, is not freed
        let master_gives = add_deal(&mut g, 0, 1, &vassal, &[]);
        let third_team = add_deal(&mut g, 1, 2, &vassal, &[]);

        g.team_free_vassal(a, b);
        assert!(!g.deals.contains(given_first));
        assert!(!g.deals.contains(given_second));
        assert!(g.deals.contains(master_gives));
        assert!(g.deals.contains(third_team));
    }
}
