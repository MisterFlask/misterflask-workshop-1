//! CvTeam: war & peace, contact, open borders, defensive pacts, force peace, vassals,
//! war weariness; plus the `CvTeamAI` war-plan and contact-counter bookkeeping that rules
//! code reads and writes (`AI_setWarPlan`, `AI_isSneakAttackReady`, `AI_doCounter`, ...).

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{civ_team_ids, is_option};
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

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
        todo!("CvTeam::canDeclareWar")
    }

    /// C++ `CvTeam::declareWar`. `war_plan = None` is C++ `NO_WARPLAN`.
    pub fn team_declare_war(&mut self, t: TeamId, other: TeamId, new_diplo: bool, war_plan: Option<WarPlanType>) {
        todo!("CvTeam::declareWar")
    }

    /// C++ `CvTeam::makePeace` (default `bBumpUnits = true`).
    pub fn team_make_peace(&mut self, t: TeamId, other: TeamId, bump_units: bool) {
        todo!("CvTeam::makePeace")
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
        todo!("CvTeam::changeWarWeariness(TeamTypes, const CvPlot&, int)")
    }

    // ------------------------------------------------------------------
    // Contact
    // ------------------------------------------------------------------

    /// C++ `CvTeam::canContact`.
    pub fn team_can_contact(&self, t: TeamId, other: TeamId) -> bool {
        todo!("CvTeam::canContact")
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
        todo!("CvTeam::makeHasMet")
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
        todo!("CvTeam::signOpenBorders")
    }

    /// C++ `CvTeam::signDefensivePact`.
    pub fn team_sign_defensive_pact(&mut self, t: TeamId, other: TeamId) {
        todo!("CvTeam::signDefensivePact")
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
        todo!("CvTeam::isFreeTrade")
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
        todo!("CvTeam::setOpenBorders")
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
        todo!("CvTeam::cancelDefensivePacts")
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
        todo!("CvTeam::setVassal")
    }

    /// C++ `CvTeam::assignVassal` (`const` in C++ but mutates other teams and deals).
    pub fn team_assign_vassal(&mut self, t: TeamId, vassal: TeamId, surrender: bool) {
        todo!("CvTeam::assignVassal")
    }

    /// C++ `CvTeam::freeVassal` (`const` in C++ but kills deals).
    pub fn team_free_vassal(&mut self, t: TeamId, vassal: TeamId) {
        todo!("CvTeam::freeVassal")
    }

    /// C++ `CvTeam::isParent`.
    pub fn team_is_parent(&self, t: TeamId, other: TeamId) -> bool {
        todo!("CvTeam::isParent")
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
                // AI: AI_makeProductionDirty() on every alive non-human member player.
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
    pub fn team_ai_get_attitude(&self, t: TeamId, other: TeamId, forced: bool) -> AttitudeType {
        todo!("CvTeamAI::AI_getAttitude")
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
        todo!("CvTeamAI::AI_updateWorstEnemy")
    }
}
