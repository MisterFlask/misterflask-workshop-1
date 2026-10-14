//! CvPlayer: civics (revolution, anarchy, upkeep, civic effects), state religion and
//! conversion, holy cities and religion founding, corporations, vote sources (resolutions,
//! membership, votes), and FFH2 leader traits (`setHasTrait`) and hidden civics/religions.
//!
//! Porting notes:
//! - `GC.getGameINLINE().AI_makeAssignWorkDirty()` (CvGameAI) has no `Game` method yet; it is
//!   ported here as the private `game_ai_make_assign_work_dirty`.
//! - Python `CvGameUtils.canDoCivic` (only called when `USE_CAN_DO_CIVIC_CALLBACK` is set, which
//!   FFH2 leaves off) is the private stub `py_can_do_civic`.
//! - `addMessage` / `addReplayMessage` calls go to the game log (one entry per event, built only
//!   when the log is enabled); diplomacy screen and interface dirty bits are UI and omitted.
//! - C++ indexes info arrays with `getCivicOptionType()` unchecked; every FFH2 civic belongs to a
//!   civic option (`civic_option_of`).

#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use crate::utils::range;
use ffh_data::*;

/// C++ `(CivicOptionTypes)GC.getCivicInfo(eCivic).getCivicOptionType()`.
fn civic_option_of(d: &GameData, civic: CivicType) -> CivicOptionType {
    d.civics[civic].civic_option_type.expect("civic without a civic option (C++ would index with -1)")
}

/// C++ `CvGameAI::AI_makeAssignWorkDirty`: flags the cities of every alive player for citizen
/// reassignment (`CvPlayerAI::AI_makeAssignWorkDirty`).
fn game_ai_make_assign_work_dirty(g: &mut Game) {
    for p in g.player_ids() {
        if g.players[p].alive {
            g.player_ai_make_assign_work_dirty(p);
        }
    }
}

/// Python `CvGameUtils.canDoCivic(argsList)`, called by `CvPlayer::canDoCivics` only when
/// `USE_CAN_DO_CIVIC_CALLBACK` is set (FFH2 leaves it off). FFH2 returns False.
fn py_can_do_civic(_g: &Game, _p: PlayerId, _civic: CivicType) -> bool {
    false
}

/// `GC.getGameINLINE().getUnitClassCreatedCount(eClass) == 0` for a hero class read from a
/// religion info, which may be NO_UNITCLASS. C++ quirk: `AI_doFavoriteReligion` reads the counts
/// of every religion's heroes without checking them for NO_UNITCLASS (the Empyrean and the Council
/// of Esus have no second hero), i.e. `m_paiUnitClassCreatedCount[-1]`, out of bounds; here a
/// missing class is never "not created yet" (a missing type is "no match").
fn hero_not_created(g: &Game, unit_class: Option<UnitClassType>) -> bool {
    match unit_class {
        Some(unit_class) => g.unit_class_created_count[unit_class] == 0,
        None => false,
    }
}

impl Game {
    // ------------------------------------------------------------------ civics

    /// C++ `CvPlayer::isCivic(eCivic)`.
    pub fn player_is_civic(&self, p: PlayerId, civic: CivicType) -> bool {
        self.players[p].civics.iter().any(|&c| c == Some(civic))
    }

    /// C++ `CvPlayer::canDoCivics(eCivic)` (true for NO_CIVIC; forced civics, Python
    /// `canDoCivic`/`cannotDoCivic`, FFH prereqs: war, alignment, civilization, religion).
    pub fn player_can_do_civics(&self, p: PlayerId, civic: Option<CivicType>) -> bool {
        let d = self.d;

        // UNOFFICIAL_PATCH 10/05/09 (jdog5000): circumvents a crash in simultaneous turns MP games.
        let Some(civic) = civic else {
            return true;
        };

        let info = &d.civics[civic];
        let civic_option = civic_option_of(d, civic);

        //FfH: Added by Kael 11/23/2007 (forced civics only bind full members of vote source 0)
        if self.player_is_full_member(p, VoteSourceType(0)) {
            if self.is_force_civic_option(civic_option) {
                return self.is_force_civic(civic);
            }
        }

        if d.defines.use_can_do_civic_callback != 0 {
            if py_can_do_civic(self, p, civic) {
                return true;
            }
        }

        let team = self.players[p].team;
        if !self.player_is_has_civic_option(p, civic_option) && !self.team_is_has_tech(team, info.tech_prereq) {
            return false;
        }

        //FfH Civics: Added by Kael 08/11/2007
        if info.prereq_war {
            if self.team_get_at_war_count(team, true) == 0 {
                return false;
            }
        }
        if info.block_alignment.is_some() {
            if self.players[p].alignment == info.block_alignment {
                return false;
            }
        }
        if info.prereq_alignment.is_some() {
            if self.players[p].alignment != info.prereq_alignment {
                return false;
            }
        }
        if info.prereq_civilization.is_some() {
            if self.player_get_civilization_type(p) != info.prereq_civilization {
                return false;
            }
        }
        if info.prereq_religion.is_some() {
            if self.player_get_state_religion(p) != info.prereq_religion {
                return false;
            }
        }
        //FfH: End Add

        if d.defines.use_cannot_do_civic_callback != 0 {
            if self.py_cannot_do_civic(p, civic) {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlayer::canRevolution(paeNewCivics)` (None = any civic change possible).
    pub fn player_can_revolution(&self, p: PlayerId, new_civics: Option<&IdxVec<CivicOptionType, Option<CivicType>>>) -> bool {
        let d = self.d;

        if self.player_is_anarchy(p) {
            return false;
        }

        if self.players[p].revolution_timer > 0 {
            return false;
        }

        match new_civics {
            None => {
                // XXX is this necessary?
                for civic in d.civics.keys() {
                    if self.player_can_do_civics(p, Some(civic)) {
                        if self.players[p].civics[civic_option_of(d, civic)] != Some(civic) {
                            return true;
                        }
                    }
                }
            }
            Some(new_civics) => {
                for civic_option in d.civic_options.keys() {
                    //FfH: Added by Kael 12/30/2007 (forced civics only bind full members)
                    if self.player_is_full_member(p, VoteSourceType(0)) {
                        if self.is_force_civic_option(civic_option) {
                            // C++ `isForceCivic(paeNewCivics[iI])`; a NO_CIVIC entry (never passed
                            // by callers) would index -1: here it is not forced.
                            if !new_civics[civic_option].is_some_and(|c| self.is_force_civic(c)) {
                                return false;
                            }
                        }
                    }

                    if self.players[p].civics[civic_option] != new_civics[civic_option] {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// C++ `CvPlayer::revolution(paeNewCivics, bForce = false)`.
    pub fn player_revolution(&mut self, p: PlayerId, new_civics: &IdxVec<CivicOptionType, Option<CivicType>>, force: bool) {
        let d = self.d;

        if !force && !self.player_can_revolution(p, Some(new_civics)) {
            return;
        }

        let anarchy_length = self.player_get_civic_anarchy_length(p, new_civics);

        if anarchy_length > 0 {
            self.player_change_anarchy_turns(p, anarchy_length);

            for civic_option in d.civic_options.keys() {
                self.player_set_civics(p, civic_option, new_civics[civic_option]);
            }
        } else {
            for civic_option in d.civic_options.keys() {
                self.player_set_civics(p, civic_option, new_civics[civic_option]);
            }
        }

        // setRevolutionTimer
        let anarchy_modifier = self.players[p].anarchy_modifier;
        self.players[p].revolution_timer =
            std::cmp::max(1, ((100 + anarchy_modifier) * d.defines.min_revolution_turns) / 100) + anarchy_length;

        // UI: Popup_DIRTY_BIT for the active player (civic chooser popup).
    }

    /// C++ `CvPlayer::verifyCivics`: replaces civics that can no longer be run.
    pub fn player_verify_civics(&mut self, p: PlayerId) {
        let d = self.d;

        if !self.player_is_anarchy(p) {
            for civic_option in d.civic_options.keys() {
                if !self.player_can_do_civics(p, self.players[p].civics[civic_option]) {
                    for civic in d.civics.keys() {
                        if d.civics[civic].civic_option_type == Some(civic_option) {
                            if self.player_can_do_civics(p, Some(civic)) {
                                self.player_set_civics(p, civic_option, Some(civic));
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvPlayer::setCivics(eIndex, eNewValue)`: processes the old and new civic.
    pub fn player_set_civics(&mut self, p: PlayerId, civic_option: CivicOptionType, civic: Option<CivicType>) {
        let d = self.d;

        let old_civic = self.players[p].civics[civic_option];

        if old_civic != civic {
            self.players[p].civics[civic_option] = civic;

            if let Some(old_civic) = old_civic {
                self.player_process_civics(p, old_civic, -1);
            }
            if let Some(new_civic) = self.players[p].civics[civic_option] {
                self.player_process_civics(p, new_civic, 1);
            }

            self.update_secretary_general();

            game_ai_make_assign_work_dirty(self);

            if self.final_initialized {
                // gDLL->isDiplomacy() / updateDiplomacyAttitude: diplomacy screen (UI).

                if !self.player_is_barbarian(p) {
                    if let Some(new_civic) = self.players[p].civics[civic_option] {
                        // TXT_KEY_MISC_PLAYER_ADOPTED_CIVIC to every alive player that has met us
                        // (FfH: and `canSeeCivic(eIndex)`; C++ quirk: it passes the civic option
                        // index as a civic), plus the replay message: logged once.
                        if self.log.enabled {
                            let initial_civic = self
                                .player_get_civilization_type(p)
                                .and_then(|civ| d.civilizations[civ].civilization_initial_civics[civic_option]);
                            if Some(new_civic) != initial_civic {
                                let turn = self.game_turn();
                                let name = self.player_get_name(p);
                                self.log.msg(turn, Some(p), || {
                                    format!("{} has adopted {}", name, d.text.name(&d.civics[new_civic].base.description))
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvPlayer::processCivics(eCivic, iChange)`: applies (+1) or removes (-1) a civic's effects.
    pub fn player_process_civics(&mut self, p: PlayerId, civic: CivicType, change: i32) {
        let d = self.d;
        let info = &d.civics[civic];

        // changeGreatPeopleRateModifier .. changeStateReligionGreatPeopleRateModifier: plain changes.
        self.players[p].great_people_rate_modifier += info.great_people_rate_modifier * change;
        self.players[p].great_general_rate_modifier += info.great_general_rate_modifier * change;
        self.players[p].domestic_great_general_rate_modifier += info.domestic_great_general_rate_modifier * change;
        self.players[p].state_religion_great_people_rate_modifier += info.state_religion_great_people_rate_modifier * change;
        self.player_change_distance_maintenance_modifier(p, info.distance_maintenance_modifier * change);
        self.player_change_num_cities_maintenance_modifier(p, info.num_cities_maintenance_modifier * change);
        self.player_change_corporation_maintenance_modifier(p, info.corporation_maintenance_modifier * change);
        self.player_change_extra_health(p, info.extra_health * change);
        // changeFreeExperience .. changeGoldPerMilitaryUnit: plain changes (+ UI dirty bits).
        self.players[p].free_experience += info.free_experience * change;
        self.players[p].worker_speed_modifier += info.worker_speed_modifier * change;
        self.players[p].improvement_upgrade_rate_modifier += info.improvement_upgrade_rate_modifier * change;
        self.players[p].military_production_modifier += info.military_production_modifier * change;
        self.players[p].base_free_units += info.base_free_units * change;
        self.players[p].base_free_military_units += info.base_free_military_units * change;
        self.players[p].free_units_population_percent += info.free_units_population_percent * change;
        self.players[p].free_military_units_population_percent += info.free_military_units_population_percent * change;
        self.players[p].gold_per_unit += info.gold_per_unit * change;
        self.players[p].gold_per_military_unit += info.gold_per_military_unit * change;
        self.player_change_happy_per_military_unit(p, info.happy_per_military_unit * change);
        // changeMilitaryFoodProductionCount (+ UI dirty bit)
        self.players[p].military_food_production_count += if info.military_food_production { change } else { 0 };
        // changeMaxConscript
        let max_conscript = self.get_world_size_max_conscript(civic);
        self.players[p].max_conscript += max_conscript * change;
        debug_assert!(self.players[p].max_conscript >= 0);
        self.player_change_no_unhealthy_population_count(p, if info.no_unhealthy_population { change } else { 0 });
        self.player_change_building_only_healthy_count(p, if info.building_only_healthy { change } else { 0 });
        self.player_change_largest_city_happiness(p, info.largest_city_happiness * change);
        self.player_change_war_weariness_modifier(p, info.war_weariness_modifier * change);
        self.player_change_free_specialist(p, info.free_specialist * change);
        self.player_change_trade_routes(p, info.trade_routes * change);
        self.player_change_no_foreign_trade_count(p, info.no_foreign_trade as i32 * change);
        self.player_change_no_corporations_count(p, info.no_corporations as i32 * change);
        self.player_change_no_foreign_corporations_count(p, info.no_foreign_corporations as i32 * change);
        self.player_change_state_religion_count(p, if info.state_religion { change } else { 0 });
        // changeNoNonStateReligionSpreadCount
        self.players[p].no_non_state_religion_spread_count += if info.no_non_state_religion_spread { change } else { 0 };
        debug_assert!(self.players[p].no_non_state_religion_spread_count >= 0);
        self.player_change_state_religion_happiness(p, info.state_religion_happiness * change);
        self.player_change_non_state_religion_happiness(p, info.non_state_religion_happiness * change);
        // changeStateReligionUnitProductionModifier .. changeExpInBorderModifier: plain changes.
        self.players[p].state_religion_unit_production_modifier += info.state_religion_unit_production_modifier * change;
        self.players[p].state_religion_building_production_modifier += info.state_religion_building_production_modifier * change;
        self.players[p].state_religion_free_experience += info.state_religion_free_experience * change;
        let exp_in_border_change = info.exp_in_border_modifier * change;
        if exp_in_border_change != 0 {
            self.players[p].exp_in_border_modifier += exp_in_border_change;
            debug_assert!(self.players[p].exp_in_border_modifier >= 0);
        }

        //FfH Civics: Added by Kael 08/11/2007
        self.player_change_coastal_trade_routes(p, info.coastal_trade_routes * change);
        // changeEnslavementChance, changeNoDiplomacyWithEnemies: plain changes.
        self.players[p].enslavement_chance += info.enslavement_chance * change;
        self.players[p].no_diplomacy_with_enemies += info.no_diplomacy_with_enemies as i32 * change;
        if change == -1 {
            for vote_source in d.vote_sources.keys() {
                if d.vote_sources[vote_source].civic == Some(civic) {
                    if self.secretary_general(vote_source) == Some(self.players[p].team) {
                        self.clear_secretary_general(vote_source);
                    }
                }
            }
        }
        //FfH: End Add

        for y in 0..YieldType::NUM {
            let yield_type = YieldType::from_usize(y);
            self.player_change_yield_rate_modifier(p, yield_type, info.yield_modifier[y] * change);
            self.player_change_capital_yield_rate_modifier(p, yield_type, info.capital_yield_modifier[y] * change);
            self.player_change_trade_yield_modifier(p, yield_type, info.trade_yield_modifier[y] * change);
        }

        for c in 0..CommerceType::NUM {
            let commerce = CommerceType::from_usize(c);
            self.player_change_commerce_rate_modifier(p, commerce, info.commerce_modifier[c] * change);
            self.player_change_capital_commerce_rate_modifier(p, commerce, info.capital_commerce_modifier[c] * change);
            self.player_change_specialist_extra_commerce(p, commerce, info.specialist_extra_commerce[c] * change);
        }

        let civ = self.player_get_civilization_type(p).expect("CvPlayer::processCivics: player without a civilization");
        for building_class in d.building_classes.keys() {
            if let Some(our_building) = d.civ_building(civ, building_class) {
                self.player_change_extra_building_happiness(p, our_building, info.building_happiness_changes[building_class] * change);
                self.player_change_extra_building_health(p, our_building, info.building_health_changes[building_class] * change);
            }
        }

        for feature in d.features.keys() {
            self.player_change_feature_happiness(p, feature, info.feature_happiness_changes[feature] * change);
        }

        for hurry in d.hurries.keys() {
            self.player_change_hurry_count(p, hurry, if info.hurry[hurry] { change } else { 0 });
        }

        for special_building in d.special_buildings.keys() {
            // changeSpecialBuildingNotRequiredCount
            self.players[p].special_building_not_required_count[special_building] +=
                if info.special_building_not_required[special_building] { change } else { 0 };
            debug_assert!(self.players[p].special_building_not_required_count[special_building] >= 0);
        }

        for specialist in d.specialists.keys() {
            self.player_change_specialist_valid_count(p, specialist, if info.specialist_valid[specialist] { change } else { 0 });
        }

        for improvement in d.improvements.keys() {
            for y in 0..YieldType::NUM {
                self.player_change_improvement_yield_change(
                    p,
                    improvement,
                    YieldType::from_usize(y),
                    info.improvement_yield_changes[improvement][y] * change,
                );
            }
        }
    }

    /// C++ `CvPlayer::getCivicPercentAnger(eCivic, bIgnore = false)`.
    pub fn player_get_civic_percent_anger(&self, p: PlayerId, civic: CivicType, ignore: bool) -> i32 {
        let d = self.d;
        let team = self.players[p].team;

        //FfH: Added by Kael 11/25/2007
        if self.teams[team].no_civic_anger {
            return 0;
        }
        //FfH: End Add

        let info = &d.civics[civic];
        if info.civic_percent_anger == 0 {
            return 0;
        }

        let civic_option = civic_option_of(d, civic);
        if !ignore && self.players[p].civics[civic_option] == Some(civic) {
            return 0;
        }

        let mut count = 0;
        let mut possible_count = 0;

        for other in self.civ_player_ids() {
            let other = &self.players[other];
            if other.alive {
                if other.team != team {
                    if other.civics[civic_option] == Some(civic) {
                        count += other.num_cities();
                    }

                    possible_count += other.num_cities();
                }
            }
        }

        if possible_count == 0 {
            return 0;
        }

        (info.civic_percent_anger * count) / possible_count
    }

    /// C++ `CvPlayer::getCivicAnarchyLength(paeNewCivics)`.
    pub fn player_get_civic_anarchy_length(&self, p: PlayerId, new_civics: &IdxVec<CivicOptionType, Option<CivicType>>) -> i32 {
        let d = self.d;
        let pl = &self.players[p];

        if pl.max_anarchy_turns == 0 {
            return 0;
        }

        if self.player_is_golden_age(p) {
            return 0;
        }

        let mut anarchy_length = 0;

        let mut change = false;

        for civic_option in d.civic_options.keys() {
            if new_civics[civic_option] != pl.civics[civic_option] {
                // C++ reads `GC.getCivicInfo(paeNewCivics[iI])` unchecked; a NO_CIVIC entry
                // (never passed by callers) adds nothing here.
                if let Some(new_civic) = new_civics[civic_option] {
                    anarchy_length += d.civics[new_civic].anarchy_length;
                }

                change = true;
            }
        }

        if change {
            anarchy_length += d.defines.base_civic_anarchy_length;

            anarchy_length += (pl.num_cities() * d.worlds[self.setup.world_size].num_cities_anarchy_percent) / 100;
        }

        anarchy_length = (anarchy_length * std::cmp::max(0, pl.anarchy_modifier + 100)) / 100;

        if anarchy_length == 0 {
            return 0;
        }

        anarchy_length *= d.game_speeds[self.game_speed_type()].anarchy_percent;
        anarchy_length /= 100;

        anarchy_length *= d.eras[self.start_era()].anarchy_percent;
        anarchy_length /= 100;

        range(anarchy_length, 1, pl.max_anarchy_turns)
    }

    /// C++ `CvPlayer::getSingleCivicUpkeep(eCivic, bIgnoreAnarchy = false)`.
    pub fn player_get_single_civic_upkeep(&self, p: PlayerId, civic: Option<CivicType>, ignore_anarchy: bool) -> i32 {
        let d = self.d;

        let Some(civic) = civic else {
            return 0;
        };

        let info = &d.civics[civic];
        let civic_option = civic_option_of(d, civic);

        if self.player_is_no_civic_upkeep(p, civic_option) {
            return 0;
        }

        let Some(upkeep) = info.upkeep else {
            return 0;
        };

        let pl = &self.players[p];

        if !ignore_anarchy {
            if self.player_is_anarchy(p) {
                return 0;
            }

            //FfH: Added by Kael 10/03/2008
            if pl.disable_production > 0 {
                return 0;
            }
            //FfH: End Add
        }

        let upkeep_info = &d.upkeeps[upkeep];

        let mut upkeep_value = 0;

        upkeep_value += (std::cmp::max(0, pl.total_population + d.defines.upkeep_population_offset - civic_option.i())
            * upkeep_info.population_percent)
            / 100;

        //FfH: Modified by Kael 11/18/2007 (settlements are not counted)
        upkeep_value += (std::cmp::max(
            0,
            (pl.num_cities() - self.player_get_num_settlements(p)) + d.defines.upkeep_city_offset + civic_option.i()
                - (d.civic_options.len() as i32 / 2),
        ) * upkeep_info.city_percent)
            / 100;
        //FfH: End Modify

        upkeep_value *= std::cmp::max(0, pl.upkeep_modifier + 100);
        upkeep_value /= 100;

        let handicap = self.player_get_handicap_type(p).expect("CvPlayer::getSingleCivicUpkeep: player without a handicap");
        upkeep_value *= d.handicaps[handicap].civic_upkeep_percent;
        upkeep_value /= 100;

        if !pl.human && !self.player_is_barbarian(p) {
            let game_handicap = &d.handicaps[self.handicap];

            upkeep_value *= game_handicap.ai_civic_upkeep_percent;
            upkeep_value /= 100;

            upkeep_value *= std::cmp::max(0, (game_handicap.ai_per_era_modifier * pl.current_era.i()) + 100);
            upkeep_value /= 100;
        }

        std::cmp::max(0, upkeep_value)
    }

    /// C++ `CvPlayer::getCivicUpkeep(paeCivics = NULL, bIgnoreAnarchy = false)` (None = current civics).
    pub fn player_get_civic_upkeep(&self, p: PlayerId, civics: Option<&IdxVec<CivicOptionType, Option<CivicType>>>, ignore_anarchy: bool) -> i32 {
        let civics = civics.unwrap_or(&self.players[p].civics);
        let mut total_upkeep = 0;
        for civic_option in self.d.civic_options.keys() {
            total_upkeep += self.player_get_single_civic_upkeep(p, civics[civic_option], ignore_anarchy);
        }
        total_upkeep
    }

    /// C++ `CvPlayer::isHasCivicOption`.
    pub fn player_is_has_civic_option(&self, p: PlayerId, civic_option: CivicOptionType) -> bool {
        self.players[p].has_civic_option_count[civic_option] > 0
    }

    /// C++ `CvPlayer::isNoCivicUpkeep`.
    pub fn player_is_no_civic_upkeep(&self, p: PlayerId, civic_option: CivicOptionType) -> bool {
        self.players[p].no_civic_upkeep_count[civic_option] > 0
    }

    /// C++ `CvPlayer::canSeeCivic(iCivic)` (FFH: hidden civics are only visible to their holder).
    pub fn player_can_see_civic(&self, p: PlayerId, civic: CivicType) -> bool {
        let d = self.d;

        if d.civics[civic].hidden {
            if self.players[p].civics[civic_option_of(d, civic)] == Some(civic) {
                return true;
            }
            return false;
        }
        true
    }

    // ------------------------------------------------------------------ anarchy

    /// C++ `CvPlayer::isAnarchy`.
    pub fn player_is_anarchy(&self, p: PlayerId) -> bool {
        self.players[p].anarchy_turns > 0
    }

    /// C++ `CvPlayer::changeAnarchyTurns`: commerce, maintenance, trade routes and
    /// corporations are recomputed when anarchy starts or ends.
    pub fn player_change_anarchy_turns(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            let old_anarchy = self.player_is_anarchy(p);

            self.players[p].anarchy_turns += change;
            debug_assert!(self.players[p].anarchy_turns >= 0);

            if old_anarchy != self.player_is_anarchy(p) {
                self.player_update_commerce(p);
                self.player_update_maintenance(p);
                self.player_update_trade_routes(p);
                self.player_update_corporation(p);

                self.player_ai_make_assign_work_dirty(p);

                let turn = self.game_turn();
                if self.player_is_anarchy(p) {
                    // TXT_KEY_MISC_REVOLUTION_HAS_BEGUN
                    self.log.msg(turn, Some(p), || "The revolution has begun!".to_string());
                } else {
                    // TXT_KEY_MISC_REVOLUTION_OVER
                    self.log.msg(turn, Some(p), || "The revolution is over!".to_string());
                }

                // UI: MiscButtons / CityInfo dirty bits for the active player/team.
            }

            // UI: GameData dirty bit for the active player.
        }
    }

    /// C++ `CvPlayer::updateMaxAnarchyTurns` (MAX_ANARCHY_TURNS lowered by traits).
    pub fn player_update_max_anarchy_turns(&mut self, p: PlayerId) {
        let d = self.d;

        let mut best_value = d.defines.max_anarchy_turns;

        debug_assert!(!d.traits.is_empty(), "GC.getNumTraitInfos() is expected to be larger than zero");
        for trait_type in d.traits.keys() {
            if self.players[p].traits[trait_type] {
                let max_anarchy = d.traits[trait_type].max_anarchy;
                if max_anarchy >= 0 {
                    if max_anarchy < best_value {
                        best_value = max_anarchy;
                    }
                }
            }
        }

        self.players[p].max_anarchy_turns = best_value;
        debug_assert!(self.players[p].max_anarchy_turns >= 0);
    }

    /// C++ `CvPlayer::changeAnarchyModifier`: also rescales the running revolution and
    /// conversion timers.
    pub fn player_change_anarchy_modifier(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            let pl = &mut self.players[p];
            pl.anarchy_modifier += change;
            pl.revolution_timer = 0.max(((100 + change) * pl.revolution_timer) / 100);
            pl.conversion_timer = 0.max(((100 + change) * pl.conversion_timer) / 100);
        }
    }

    /// C++ `CvPlayer::getReligionAnarchyLength`.
    pub fn player_get_religion_anarchy_length(&self, p: PlayerId) -> i32 {
        let d = self.d;
        let pl = &self.players[p];

        if pl.max_anarchy_turns == 0 {
            return 0;
        }

        if self.player_is_golden_age(p) {
            return 0;
        }

        let mut anarchy_length = d.defines.base_religion_anarchy_length;

        anarchy_length += (pl.num_cities() * d.worlds[self.setup.world_size].num_cities_anarchy_percent) / 100;

        anarchy_length = (anarchy_length * std::cmp::max(0, pl.anarchy_modifier + 100)) / 100;

        if anarchy_length == 0 {
            return 0;
        }

        anarchy_length *= d.game_speeds[self.game_speed_type()].anarchy_percent;
        anarchy_length /= 100;

        anarchy_length *= d.eras[self.start_era()].anarchy_percent;
        anarchy_length /= 100;

        range(anarchy_length, 1, pl.max_anarchy_turns)
    }

    // ------------------------------------------------------------------ state religion

    /// C++ `CvPlayer::isStateReligion`.
    pub fn player_is_state_religion(&self, p: PlayerId) -> bool {
        self.players[p].state_religion_count > 0
    }

    /// C++ `CvPlayer::getStateReligion`.
    pub fn player_get_state_religion(&self, p: PlayerId) -> Option<ReligionType> {
        if self.player_is_state_religion(p) {
            self.players[p].last_state_religion
        } else {
            None
        }
    }

    /// C++ `CvPlayer::changeStateReligionCount`: maintenance, religion happiness/commerce and
    /// the game's citizen assignment are updated.
    pub fn player_change_state_religion_count(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            // religion visibility now part of espionage (C++ comments out updateCitySight here)

            self.players[p].state_religion_count += change;
            debug_assert!(self.players[p].state_religion_count >= 0);

            self.player_update_maintenance(p);
            self.player_update_religion_happiness(p);
            self.player_update_religion_commerce(p);

            game_ai_make_assign_work_dirty(self);

            // UI: Score dirty bit.
        }
    }

    /// C++ `CvPlayer::setLastStateReligion`: city sight, maintenance, religion happiness and
    /// commerce, secretary general, messages and the Python `playerChangeStateReligion` event.
    pub fn player_set_last_state_religion(&mut self, p: PlayerId, religion: Option<ReligionType>) {
        let d = self.d;

        if self.players[p].last_state_religion != religion {
            // religion visibility now part of espionage (BtS updateCitySight(false, true) removed)

            //FfH: Added by Kael 11/03/2007
            self.update_city_sight(false);
            //FfH: End Add

            let old_religion = self.players[p].last_state_religion;
            self.players[p].last_state_religion = religion;

            //FfH: Added by Kael 11/03/2007
            self.update_city_sight(true);
            //FfH: End Add

            self.player_update_maintenance(p);
            self.player_update_religion_happiness(p);
            self.player_update_religion_commerce(p);

            self.update_secretary_general();

            game_ai_make_assign_work_dirty(self);

            // UI: Score dirty bit.

            if self.final_initialized {
                // gDLL->isDiplomacy() / updateDiplomacyAttitude: diplomacy screen (UI).

                if !self.player_is_barbarian(p) {
                    if let Some(new_religion) = self.players[p].last_state_religion {
                        // TXT_KEY_MISC_PLAYER_CONVERT_RELIGION to every alive player that has met
                        // us, plus the replay message: logged once.
                        if self.log.enabled {
                            let turn = self.game_turn();
                            let name = self.player_get_name(p);
                            self.log.msg(turn, Some(p), || {
                                format!("{} has converted to {}", name, d.text.name(&d.religions[new_religion].base.description))
                            });
                        }
                    }
                }

                // Python Event
                self.ev_player_change_state_religion(p, religion, old_religion);
            }
        }
    }

    /// C++ `CvPlayer::isNoNonStateReligionSpread`.
    pub fn player_is_no_non_state_religion_spread(&self, p: PlayerId) -> bool {
        self.players[p].no_non_state_religion_spread_count > 0
    }

    /// C++ `CvPlayer::changeStateReligionHappiness`.
    pub fn player_change_state_religion_happiness(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].state_religion_happiness += change;
            self.player_update_religion_happiness(p);
        }
    }

    /// C++ `CvPlayer::changeNonStateReligionHappiness`.
    pub fn player_change_non_state_religion_happiness(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].non_state_religion_happiness += change;
            self.player_update_religion_happiness(p);
        }
    }

    /// C++ `CvPlayer::canDoReligion(eReligion)` (FFH: agnostics and leader religion weights).
    pub fn player_can_do_religion(&self, p: PlayerId, religion: ReligionType) -> bool {
        let d = self.d;

        let team = self.players[p].team;
        if self.team_get_has_religion_count(team, religion) == 0 {
            return false;
        }

        //FfH Traits: Added by Kael 08/02/2007
        if self.players[p].agnostic {
            return false;
        }
        let personality = self.players[p].personality_type.expect("CvPlayer::canDoReligion: getPersonalityType() is NO_LEADER");
        let weight = d.leader_heads[personality].religion_weight_modifier[religion];
        if weight <= -100 {
            return false;
        }
        //FfH: End Add

        // BETTER AI (favorite Religions) Sephi
        if !self.players[p].human {
            if weight < 0 {
                return false;
            }
        }
        // BETTER AI: END

        true
    }

    /// C++ `CvPlayer::canChangeReligion`.
    pub fn player_can_change_religion(&self, p: PlayerId) -> bool {
        for religion in self.d.religions.keys() {
            if self.player_can_convert(p, Some(religion)) {
                return true;
            }
        }
        false
    }

    /// C++ `CvPlayer::canConvert(eReligion)` (None = drop the state religion).
    pub fn player_can_convert(&self, p: PlayerId, religion: Option<ReligionType>) -> bool {
        let d = self.d;
        let pl = &self.players[p];

        // BETTER AI (favorite Religions) Sephi
        if !pl.human {
            if let Some(religion) = religion {
                let leader = &d.leader_heads[self.player_get_leader_type(p).expect("CvPlayer::canConvert: player without a leader")];

                if let Some(favorite) = pl.favorite_religion {
                    if Some(favorite) == pl.last_state_religion && leader.religion_weight_modifier[favorite] > 50 {
                        return false;
                    }
                }

                if leader.religion_weight_modifier[religion] < 0 {
                    return false;
                }
            }
        }
        // BETTER AI: END

        if self.player_is_barbarian(p) {
            return false;
        }

        if self.player_is_anarchy(p) {
            return false;
        }

        if pl.conversion_timer > 0 {
            return false;
        }

        if !self.player_is_state_religion(p) {
            return false;
        }

        if pl.last_state_religion == religion {
            return false;
        }

        if let Some(religion) = religion {
            if !self.player_can_do_religion(p, religion) {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlayer::convert(eReligion)` (FFH: alignment pulled into the religion's range).
    pub fn player_convert(&mut self, p: PlayerId, religion: Option<ReligionType>) {
        let d = self.d;

        if !self.player_can_convert(p, religion) {
            return;
        }

        let anarchy_length = self.player_get_religion_anarchy_length(p);

        self.player_change_anarchy_turns(p, anarchy_length);

        self.player_set_last_state_religion(p, religion);

        // setConversionTimer
        let anarchy_modifier = self.players[p].anarchy_modifier;
        self.players[p].conversion_timer =
            std::cmp::max(1, ((100 + anarchy_modifier) * d.defines.min_conversion_turns) / 100) + anarchy_length;

        //FfH Religion: Added by Kael 08/09/2007
        // Alignments compare as ints in C++ (good < neutral < evil, NO_ALIGNMENT = -1 lowest),
        // which is the `Option<AlignmentType>` ordering. setAlignment: plain store (+ UI dirty bit).
        if let Some(religion) = religion {
            let info = &d.religions[religion];
            if self.players[p].alignment < info.alignment_best {
                self.players[p].alignment = info.alignment_best;
            }
            if self.players[p].alignment > info.alignment_worst {
                self.players[p].alignment = info.alignment_worst;
            }
        }
        //FfH: End Add
    }

    /// C++ `CvPlayer::canSeeReligion(iReligion, pCity)` (FFH: hidden religions).
    pub fn player_can_see_religion(&self, p: PlayerId, religion: ReligionType, city: Option<CityId>) -> bool {
        if self.d.religions[religion].hidden {
            if self.player_get_state_religion(p) == Some(religion) {
                return true;
            }
            if let Some(city) = city {
                if self.cities[city].owner == p {
                    return true;
                }
            }
            return false;
        }
        true
    }

    /// C++ `CvPlayer::AI_doFavoriteReligion` (BETTER AI, called from `doTurn` for AI players):
    /// picks a favorite religion (game RNG) and converts to it when possible.
    pub fn player_ai_do_favorite_religion(&mut self, p: PlayerId) {
        let d = self.d;

        for trait_type in d.traits.keys() {
            if self.players[p].traits[trait_type] {
                if d.traits[trait_type].agnostic {
                    return;
                }
            }
        }

        let leader = &d.leader_heads[self.player_get_leader_type(p).expect("CvPlayer::AI_doFavoriteReligion: player without a leader")];

        // Pick a Favorite Religion
        if self.players[p].favorite_religion.is_none() {
            for religion in d.religions.keys() {
                if self.soren_rand_num(100, "Choose Religion") < leader.religion_weight_modifier[religion] {
                    self.players[p].favorite_religion = Some(religion);
                    break;
                }
            }
            // If still have none, pick randomly
            let mut hack = 0;
            while self.players[p].favorite_religion.is_none() {
                let new_religion = ReligionType::from_usize(self.soren_rand_num(d.religions.len() as i32, "Choose Religion") as usize);
                if leader.religion_weight_modifier[new_religion] >= 0 {
                    self.players[p].favorite_religion = Some(new_religion);
                }
                hack += 1;
                if hack == 30 {
                    break;
                }
            }
        }

        // Convert to Favorite Religion if possible
        if let Some(favorite) = self.players[p].favorite_religion {
            if self.player_can_convert(p, Some(favorite)) {
                self.player_convert(p, Some(favorite));
            }
        }

        // Choose new Religion for Opportunists
        if leader.religion_opportunist {
            if let Some(favorite) = self.players[p].favorite_religion {
                let favorite_info = &d.religions[favorite];
                let mut valid = true;

                if let Some(hero1) = favorite_info.religion_hero1 {
                    if self.unit_class_created_count[hero1] == 0 {
                        valid = false;
                    }
                }

                if let Some(hero2) = favorite_info.religion_hero2 {
                    if self.unit_class_created_count[hero2] == 0 {
                        valid = false;
                    }
                }

                if let Some(hero1) = favorite_info.religion_hero1 {
                    if self.player_get_unit_class_count_plus_making(p, hero1) > 0 {
                        valid = false;
                    }
                }

                if let Some(hero2) = favorite_info.religion_hero2 {
                    if self.player_get_unit_class_count_plus_making(p, hero2) > 0 {
                        valid = false;
                    }
                }

                if valid {
                    for religion in d.religions.keys() {
                        let info = &d.religions[religion];
                        // C++ quirk: tests the favorite religion's heroes for NO_UNITCLASS, then reads
                        // the created counts of religion `religion`'s heroes (see `hero_not_created`).
                        if (favorite_info.religion_hero1.is_some() && hero_not_created(self, info.religion_hero1))
                            || (favorite_info.religion_hero2.is_some() && hero_not_created(self, info.religion_hero2))
                        {
                            if leader.religion_weight_modifier[religion] >= 0 {
                                self.players[p].favorite_religion = Some(religion);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------ religions in the empire

    /// C++ `CvPlayer::hasHolyCity(eReligion)`.
    pub fn player_has_holy_city(&self, p: PlayerId, religion: ReligionType) -> bool {
        match self.get_holy_city(religion) {
            Some(holy_city) => self.cities[holy_city].owner == p,
            None => false,
        }
    }

    /// C++ `CvPlayer::countHolyCities`.
    pub fn player_count_holy_cities(&self, p: PlayerId) -> i32 {
        let mut count = 0;
        for religion in self.d.religions.keys() {
            if self.player_has_holy_city(p, religion) {
                count += 1;
            }
        }
        count
    }

    /// C++ `CvPlayer::foundReligion(eReligion, eSlotReligion, bAward)` (no-op for NO_RELIGION,
    /// disabled or already founded religions).
    pub fn player_found_religion(&mut self, p: PlayerId, religion: Option<ReligionType>, slot_religion: ReligionType, award: bool) {
        let d = self.d;

        let Some(religion) = religion else {
            return;
        };

        //FfH: Added by Kael 10/29/2007
        if self.is_religion_disabled(religion) {
            return;
        }
        //FfH: End Add

        if self.is_religion_founded(religion) {
            //FfH: Modified by Kael 12/20/2008 -- the BtS re-choice of an already founded religion
            // (popup for humans, `foundReligion(AI_chooseReligion(), ..)` for the AI) is commented
            // out: nothing happens.
            return;
        }

        // setReligionSlotTaken
        self.religion_slot_taken[slot_religion] = true;

        let starting = match d.religions[slot_religion].tech_prereq {
            None => true,
            Some(tech) => d.techs[tech].era < Some(self.start_era()),
        };

        let mut best_value = 0;
        let mut best_city: Option<CityId> = None;

        for c in self.players[p].cities.clone() {
            if !starting || !self.city_is_any_holy_city(c) {
                let mut value = 10;
                value += self.cities[c].population;
                value += self.soren_rand_num(d.defines.found_religion_city_rand, "Found Religion");

                value /= self.city_religion_count(c) + 1;

                if self.city_is_capital(c) {
                    value /= 8;
                }

                //FfH: Added by Kael 11/18/2007
                if self.cities[c].settlement {
                    value /= 8;
                }
                //FfH: End Add

                value = std::cmp::max(1, value);

                if value > best_value {
                    best_value = value;
                    best_city = Some(c);
                }
            }
        }

        if let Some(best_city) = best_city {
            // The city's plot is read for the free units below (cities never move).
            let (x, y) = (self.cities[best_city].x, self.cities[best_city].y);

            self.set_holy_city(religion, Some(best_city), true);

            if award {
                if d.religions[slot_religion].num_free_units > 0 {
                    // C++ `getCivilizationUnits(GC.getReligionInfo(eReligion).getFreeUnitClass())`
                    // (eReligion's class, eSlotReligion's count); no class (C++ would index -1)
                    // gives no unit here.
                    let civ = self.player_get_civilization_type(p).expect("CvPlayer::foundReligion: player without a civilization");
                    let free_unit = d.religions[religion].free_unit_class.and_then(|unit_class| d.civ_unit(civ, unit_class));

                    if let Some(free_unit) = free_unit {
                        for _ in 0..d.religions[slot_religion].num_free_units {
                            self.player_init_unit(p, free_unit, x, y, None, None);
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvPlayer::hasShrine(eReligion)`.
    pub fn player_has_shrine(&self, p: PlayerId, religion: Option<ReligionType>) -> bool {
        let mut has_shrine = false;

        if let Some(religion) = religion {
            let holy_city = self.get_holy_city(religion);

            // if the holy city exists, and we own it
            if let Some(holy_city) = holy_city {
                if self.cities[holy_city].owner == p {
                    has_shrine = self.city_has_shrine(holy_city, religion);
                }
            }
        }
        has_shrine
    }

    /// C++ `CvPlayer::countTotalHasReligion`.
    pub fn player_count_total_has_religion(&self, p: PlayerId) -> i32 {
        self.players[p].has_religion_count.iter().sum()
    }

    /// C++ `CvPlayer::findHighestHasReligionCount`.
    pub fn player_find_highest_has_religion_count(&self, p: PlayerId) -> i32 {
        let mut best_value = 0;
        for &value in self.players[p].has_religion_count.iter() {
            if value > best_value {
                best_value = value;
            }
        }
        best_value
    }

    /// C++ `CvPlayer::changeHasReligionCount` (updates building commerce and citizen
    /// assignment game-wide).
    pub fn player_change_has_religion_count(&mut self, p: PlayerId, religion: ReligionType, change: i32) {
        if change != 0 {
            self.players[p].has_religion_count[religion] += change;
            debug_assert!(self.players[p].has_religion_count[religion] >= 0);

            self.update_building_commerce();

            game_ai_make_assign_work_dirty(self);
        }
    }

    /// C++ `CvPlayer::getReligionPopulation(eReligion)`.
    pub fn player_get_religion_population(&self, p: PlayerId, religion: ReligionType) -> i32 {
        let mut population = 0;
        for &c in &self.players[p].cities {
            let city = &self.cities[c];
            if city.has_religion[religion] {
                population += city.population;
            }
        }

        population
    }

    // ------------------------------------------------------------------ corporations

    /// C++ `CvPlayer::hasHeadquarters(eCorporation)`.
    pub fn player_has_headquarters(&self, p: PlayerId, corporation: CorporationType) -> bool {
        match self.get_headquarters(corporation) {
            Some(headquarters) => self.cities[headquarters].owner == p,
            None => false,
        }
    }

    /// C++ `CvPlayer::countHeadquarters`.
    pub fn player_count_headquarters(&self, p: PlayerId) -> i32 {
        let mut count = 0;
        for corporation in self.d.corporations.keys() {
            if self.player_has_headquarters(p, corporation) {
                count += 1;
            }
        }
        count
    }

    /// C++ `CvPlayer::countCorporations(eCorporation)` (cities with the corporation).
    pub fn player_count_corporations(&self, p: PlayerId, corporation: CorporationType) -> i32 {
        let mut count = 0;

        for &c in &self.players[p].cities {
            if self.cities[c].has_corporation[corporation] {
                count += 1;
            }
        }

        count
    }

    /// C++ `CvPlayer::foundCorporation(eCorporation)`.
    pub fn player_found_corporation(&mut self, p: PlayerId, corporation: CorporationType) {
        let d = self.d;

        if self.is_corporation_founded(corporation) {
            return;
        }

        let info = &d.corporations[corporation];

        let starting = match info.tech_prereq {
            None => true,
            Some(tech) => d.techs[tech].era < Some(self.start_era()),
        };

        let mut best_value = 0;
        let mut best_city: Option<CityId> = None;

        for c in self.players[p].cities.clone() {
            if !starting || !self.city_is_any_headquarters(c) {
                let mut value = 10;
                value += self.cities[c].population;

                // C++ loops the NUM_CORPORATION_PREREQ_BONUSES slots and skips NO_BONUS ones;
                // `prereq_bonus` holds the set slots in order.
                for &bonus in &info.prereq_bonus {
                    value += 10 * self.city_num_bonuses(c, bonus);
                }

                value += self.soren_rand_num(d.defines.found_corporation_city_rand, "Found Corporation");

                value /= self.city_corporation_count(c) + 1;

                value = std::cmp::max(1, value);

                if value > best_value {
                    best_value = value;
                    best_city = Some(c);
                }
            }
        }

        if let Some(best_city) = best_city {
            self.city_set_headquarters(best_city, corporation);
        }
    }

    /// C++ `CvPlayer::isActiveCorporation(eIndex)`.
    pub fn player_is_active_corporation(&self, p: PlayerId, corporation: CorporationType) -> bool {
        if self.player_is_no_corporations(p) {
            return false;
        }

        if self.player_is_no_foreign_corporations(p) && !self.player_has_headquarters(p, corporation) {
            return false;
        }

        true
    }

    /// C++ `CvPlayer::getHasCorporationCount(eIndex)` (0 unless the corporation is active).
    pub fn player_get_has_corporation_count(&self, p: PlayerId, corporation: CorporationType) -> i32 {
        if !self.player_is_active_corporation(p, corporation) {
            return 0;
        }
        self.players[p].has_corporation_count[corporation]
    }

    /// C++ `CvPlayer::countTotalHasCorporation`.
    pub fn player_count_total_has_corporation(&self, p: PlayerId) -> i32 {
        let mut count = 0;
        for corporation in self.d.corporations.keys() {
            count += self.player_get_has_corporation_count(p, corporation);
        }
        count
    }

    /// C++ `CvPlayer::changeHasCorporationCount` (updates building commerce and citizen
    /// assignment game-wide).
    pub fn player_change_has_corporation_count(&mut self, p: PlayerId, corporation: CorporationType, change: i32) {
        if change != 0 {
            self.players[p].has_corporation_count[corporation] += change;
            // C++ FAssert(getHasCorporationCount(eIndex) >= 0) (the active-corporation getter).
            debug_assert!(self.player_get_has_corporation_count(p, corporation) >= 0);

            self.update_building_commerce();

            game_ai_make_assign_work_dirty(self);
        }
    }

    // ------------------------------------------------------------------ vote sources

    /// C++ `CvPlayer::getVote(iId)` (`NO_PLAYER_VOTE` if this player has not voted).
    pub fn player_get_vote(&self, p: PlayerId, id: VoteId) -> PlayerVote {
        for &(vote_id, vote) in &self.players[p].votes {
            if vote_id == id {
                return vote;
            }
        }
        PlayerVote::NO_PLAYER_VOTE
    }

    /// C++ `CvPlayer::setVote(iId, ePlayerVote)` (`NO_PLAYER_VOTE` removes the entry).
    pub fn player_set_vote(&mut self, p: PlayerId, id: VoteId, vote: PlayerVote) {
        let votes = &mut self.players[p].votes;
        if let Some(i) = votes.iter().position(|&(vote_id, _)| vote_id == id) {
            if vote == PlayerVote::NO_PLAYER_VOTE {
                votes.remove(i);
            } else {
                votes[i].1 = vote;
            }
            return;
        }

        if vote != PlayerVote::NO_PLAYER_VOTE {
            votes.push((id, vote));
        }
    }

    /// C++ `CvPlayer::setLoyalMember(eVoteSource, bNewValue)`.
    pub fn player_set_loyal_member(&mut self, p: PlayerId, vote_source: VoteSourceType, new_value: bool) {
        if self.players[p].loyal_member[vote_source] != new_value {
            self.player_process_vote_source_bonus(p, vote_source, false);
            self.players[p].loyal_member[vote_source] = new_value;
            self.player_process_vote_source_bonus(p, vote_source, true);

            self.update_secretary_general();
        }
    }

    /// C++ `CvPlayer::processVoteSourceBonus(eVoteSource, bActive)` (per city).
    pub fn player_process_vote_source_bonus(&mut self, p: PlayerId, vote_source: VoteSourceType, active: bool) {
        for c in self.players[p].cities.clone() {
            self.city_process_vote_source_bonus(c, vote_source, active);
        }
    }

    /// C++ `CvPlayer::getVotes(eVote, eVoteSource)` (`vote` None = C++ NO_VOTE).
    pub fn player_get_votes(&self, p: PlayerId, vote: Option<VoteType>, vote_source: VoteSourceType) -> i32 {
        let d = self.d;
        let pl = &self.players[p];

        let mut votes;

        let religion = self.vote_source_religions[vote_source];

        //FfH: Added by Kael 11/15/2007
        if let Some(civic) = d.vote_sources[vote_source].civic {
            if !self.player_is_civic(p, civic) {
                return 0;
            }
        }
        //FfH: End Add

        match vote {
            None => {
                votes = match religion {
                    Some(religion) => self.player_get_religion_population(p, religion),
                    None => pl.total_population,
                };
            }
            Some(vote) => {
                let vote_info = &d.votes[vote];

                if !vote_info.vote_source_type[vote_source] {
                    return 0;
                }

                votes = 0;

                if vote_info.civ_voting {
                    if religion.is_none_or(|religion| pl.has_religion_count[religion] > 0) {
                        votes = 1;
                    }
                } else if vote_info.city_voting {
                    votes = match religion {
                        Some(religion) => pl.has_religion_count[religion],
                        None => pl.num_cities(),
                    };
                } else {
                    votes = match religion {
                        None => pl.total_population,
                        Some(religion) => self.player_get_religion_population(p, religion),
                    };
                }

                if religion.is_some() && self.player_get_state_religion(p) == religion {
                    votes *= 100 + vote_info.state_religion_vote_percent;
                    votes /= 100;
                }
            }
        }

        //FfH: Added by Kael 11/14/2007 (diplomat units carry a vote)
        for &u in &pl.units {
            if d.units[self.units[u].unit_type].diplo_vote_type.is_some() {
                votes += 1;
            }
        }
        //FfH: End Add

        votes
    }

    /// C++ `CvPlayer::canDoResolution(eVoteSource, kData)`.
    pub fn player_can_do_resolution(&self, p: PlayerId, vote_source: VoteSourceType, data: &VoteSelectionSubData) -> bool {
        let d = self.d;
        let our_team = self.players[p].team;

        if let Some(player) = data.player {
            if !self.teams[our_team].has_met[self.players[player].team] {
                return false;
            }
        }

        let vote_info = &d.votes[data.vote];

        if vote_info.open_borders {
            for team2 in self.civ_team_ids() {
                if self.team_is_voting_member(team2, vote_source) {
                    if !self.team_is_open_borders_trading(our_team) && !self.team_is_open_borders_trading(team2) {
                        return false;
                    }

                    if self.teams[our_team].at_war[team2] {
                        return false;
                    }
                }
            }
        } else if vote_info.defensive_pact {
            for team2 in self.civ_team_ids() {
                if self.team_is_voting_member(team2, vote_source) {
                    if !self.team_is_defensive_pact_trading(our_team) && !self.team_is_defensive_pact_trading(team2) {
                        return false;
                    }

                    if self.team_get_at_war_count(our_team, true) > 0 || self.team_get_at_war_count(team2, true) > 0 {
                        return false;
                    }

                    if !self.team_can_sign_defensive_pact(our_team, team2) {
                        return false;
                    }
                }
            }
        } else if vote_info.force_peace {
            let player = data.player.expect("CvPlayer::canDoResolution: force peace vote without a player");
            let their_team = self.players[player].team;

            if their_team != our_team {
                if self.teams[our_team].at_war[their_team] {
                    let mut master = our_team;
                    for team in self.civ_team_ids() {
                        // C++ quirk: `iMaster != getID()` compares the team with the player id.
                        if team.idx() != p.idx() && self.teams[our_team].vassal[team] {
                            if self.team_is_voting_member(team, vote_source) {
                                master = team;
                                break;
                            }
                        }
                    }

                    if !self.team_can_contact(master, their_team) {
                        return false;
                    }
                }
            }
        } else if vote_info.force_war {
            let player = data.player.expect("CvPlayer::canDoResolution: force war vote without a player");
            let their_team = self.players[player].team;

            if !self.teams[our_team].at_war[their_team] {
                let mut master = our_team;
                for team in self.civ_team_ids() {
                    // C++ quirk: `iMaster != getID()` compares the team with the player id.
                    if team.idx() != p.idx() && self.teams[our_team].vassal[team] {
                        if self.team_is_voting_member(team, vote_source) {
                            master = team;
                            break;
                        }
                    }
                }

                if !self.team_can_declare_war(master, their_team) {
                    return false;
                }
            }
        } else if vote_info.force_no_trade {
            let player = data.player.expect("CvPlayer::canDoResolution: force no trade vote without a player");

            if !self.player_can_stop_trading_with_team(p, self.players[player].team, true) {
                return false;
            }
        } else if vote_info.assign_city {
            let player = data.player.expect("CvPlayer::canDoResolution: assign city vote without a player");
            let other_player = data.other_player.expect("CvPlayer::canDoResolution: assign city vote without another player");
            if self.teams[self.players[other_player].team].vassal[self.players[player].team] {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlayer::canDefyResolution(eVoteSource, kData)`.
    pub fn player_can_defy_resolution(&self, p: PlayerId, vote_source: VoteSourceType, data: &VoteSelectionSubData) -> bool {
        let d = self.d;
        let our_team = self.players[p].team;

        if self.secretary_general(vote_source) == Some(our_team) {
            return false;
        }

        let vote_info = &d.votes[data.vote];

        if vote_info.open_borders {
            for team in self.civ_team_ids() {
                // C++ `(PlayerTypes)iTeam != getTeam()` (same values).
                if team != our_team {
                    if self.team_is_voting_member(team, vote_source) {
                        if !self.team_is_open_borders(team, our_team) {
                            return true;
                        }
                    }
                }
            }
        } else if vote_info.defensive_pact {
            for team in self.civ_team_ids() {
                if team != our_team {
                    if self.team_is_voting_member(team, vote_source) {
                        if !self.teams[team].defensive_pact[our_team] {
                            return true;
                        }
                    }
                }
            }
        } else if vote_info.force_no_trade {
            return true;
        } else if vote_info.force_war {
            let player = data.player.expect("CvPlayer::canDefyResolution: force war vote without a player");
            if !self.at_war(our_team, self.players[player].team) {
                return true;
            }
        } else if vote_info.force_peace {
            let player = data.player.expect("CvPlayer::canDefyResolution: force peace vote without a player");
            let their_team = self.players[player].team;
            if their_team == our_team {
                return true;
            }

            if self.at_war(our_team, their_team) {
                return true;
            }
        } else if vote_info.assign_city {
            if data.player == Some(p) {
                return true;
            }
        } else if !self.is_team_vote(data.vote) {
            return true;
        }

        false
    }

    /// C++ `CvPlayer::setDefiedResolution(eVoteSource, kData)`: defiance anger in cities,
    /// loses loyal membership.
    pub fn player_set_defied_resolution(&mut self, p: PlayerId, vote_source: VoteSourceType, data: &VoteSelectionSubData) {
        // C++ FAssert(canDefyResolution(eVoteSource, kData)): debug-only check, not ported.

        // cities get unhappiness
        for c in self.players[p].cities.clone() {
            let religion = self.vote_source_religions[vote_source];

            if religion.is_none_or(|religion| self.cities[c].has_religion[religion]) {
                let mut anger_length = self.city_flat_defy_resolution_anger_length(c);
                if religion.is_some_and(|religion| self.cities[c].has_religion[religion]) {
                    anger_length /= std::cmp::max(1, self.city_religion_count(c));
                }

                self.city_change_defy_resolution_anger_timer(c, anger_length);
            }
        }

        self.player_set_loyal_member(p, vote_source, false);
    }

    /// C++ `CvPlayer::setEndorsedResolution(eVoteSource, kData)`.
    pub fn player_set_endorsed_resolution(&mut self, p: PlayerId, vote_source: VoteSourceType, data: &VoteSelectionSubData) {
        self.player_set_loyal_member(p, vote_source, true);
    }

    /// C++ `CvPlayer::isFullMember(eVoteSource)`.
    pub fn player_is_full_member(&self, p: PlayerId, vote_source: VoteSourceType) -> bool {
        if let Some(religion) = self.vote_source_religions[vote_source] {
            if self.player_get_state_religion(p) != Some(religion) {
                return false;
            }
        }

        if let Some(civic) = self.d.vote_sources[vote_source].civic {
            if !self.player_is_civic(p, civic) {
                return false;
            }
        }

        if !self.players[p].loyal_member[vote_source] {
            return false;
        }

        self.player_is_voting_member(p, vote_source)
    }

    /// C++ `CvPlayer::isVotingMember(eVoteSource)` (FFH: vote source civic required).
    pub fn player_is_voting_member(&self, p: PlayerId, vote_source: VoteSourceType) -> bool {
        //FfH: Added by Kael 11/15/2007
        if let Some(civic) = self.d.vote_sources[vote_source].civic {
            if !self.player_is_civic(p, civic) {
                return false;
            }
        }
        //FfH: End Add

        self.player_get_votes(p, None, vote_source) > 0
    }

    /// C++ `CvPlayer::isGamblingRing` (FFH: full member of a vote source with the gambling ring resolution).
    pub fn player_is_gambling_ring(&self, p: PlayerId) -> bool {
        for vote_source in self.d.vote_sources.keys() {
            if self.player_is_full_member(p, vote_source) {
                if self.gambling_ring[vote_source] {
                    return true;
                }
            }
        }
        false
    }

    /// C++ `CvPlayer::isSlaveTrade` (FFH vote source resolution).
    pub fn player_is_slave_trade(&self, p: PlayerId) -> bool {
        for vote_source in self.d.vote_sources.keys() {
            if self.player_is_full_member(p, vote_source) {
                if self.slave_trade[vote_source] {
                    return true;
                }
            }
        }
        false
    }

    /// C++ `CvPlayer::isSmugglingRing` (FFH vote source resolution).
    pub fn player_is_smuggling_ring(&self, p: PlayerId) -> bool {
        for vote_source in self.d.vote_sources.keys() {
            if self.player_is_full_member(p, vote_source) {
                if self.smuggling_ring[vote_source] {
                    return true;
                }
            }
        }
        false
    }

    // ------------------------------------------------------------------ traits (FFH)

    /// C++ `CvPlayer::setHasTrait(eTrait, bNewValue)` (FFH scriptable leader traits): applies
    /// or removes the trait's health, happiness, modifiers, commerce, civic upkeep, FFH flags
    /// (adaptive, agnostic, barbarian ally, ...), max cities and gold/summon modifiers.
    pub fn player_set_has_trait(&mut self, p: PlayerId, trait_type: TraitType, new_value: bool) {
        let d = self.d;

        let change = if new_value { 1 } else { -1 };

        if self.players[p].traits[trait_type] == new_value {
            return;
        }

        self.players[p].traits[trait_type] = new_value;

        let info = &d.traits[trait_type];

        self.player_change_extra_health(p, info.health * change);
        self.player_change_extra_happiness(p, info.happiness * change);

        for building in d.buildings.keys() {
            self.player_change_extra_building_happiness(p, building, d.buildings[building].happiness_traits[trait_type] * change);
        }

        // changeUpkeepModifier .. changeMaxPlayerBuildingProductionModifier: plain changes.
        let pl = &mut self.players[p];
        pl.upkeep_modifier += info.upkeep_modifier * change;
        pl.level_experience_modifier += info.level_experience_modifier * change;
        pl.great_people_rate_modifier += info.great_people_rate_modifier * change;
        pl.great_general_rate_modifier += info.great_general_rate_modifier * change;
        pl.domestic_great_general_rate_modifier += info.domestic_great_general_rate_modifier * change;

        pl.max_global_building_production_modifier += info.max_global_building_production_modifier * change;
        pl.max_team_building_production_modifier += info.max_team_building_production_modifier * change;
        pl.max_player_building_production_modifier += info.max_player_building_production_modifier * change;

        for y in 0..YieldType::NUM {
            self.player_change_trade_yield_modifier(p, YieldType::from_usize(y), info.trade_yield_modifier[y] * change);
        }

        for c in 0..CommerceType::NUM {
            let commerce = CommerceType::from_usize(c);
            self.player_change_free_city_commerce(p, commerce, info.commerce_change[c] * change);
            self.player_change_commerce_rate_modifier(p, commerce, info.commerce_modifier[c] * change);
        }

        for civic_option in d.civic_options.keys() {
            if d.civic_options[civic_option].trait_no_upkeep[trait_type] {
                // changeNoCivicUpkeepCount (+ UI dirty bit)
                self.players[p].no_civic_upkeep_count[civic_option] += change;
                debug_assert!(self.players[p].no_civic_upkeep_count[civic_option] >= 0);
            }
        }

        // setAdaptive .. setSprawling, CvTeam::setBarbarianAlly: plain stores.
        if info.adaptive {
            self.players[p].adaptive = new_value;
        }
        if info.agnostic {
            self.players[p].agnostic = new_value;
        }
        if info.assimilation {
            self.players[p].assimilation = new_value;
        }
        if info.barbarian_ally {
            let team = self.players[p].team;
            self.teams[team].barbarian_ally = new_value;
        }
        if info.ignore_food {
            self.players[p].ignore_food = new_value;
        }
        if info.insane {
            self.players[p].insane = new_value;
        }
        if info.sprawling {
            self.players[p].sprawling = new_value;
        }
        // changeFreeXPFromCombat, setMaxCities, changePillagingGold .. changeUpgradeCostModifier:
        // plain changes.
        self.players[p].free_xp_from_combat += info.free_xp_from_combat * change;
        if info.max_cities != -1 {
            if change == 1 {
                self.players[p].max_cities = info.max_cities + d.worlds[self.setup.world_size].max_cities_mod;
            } else {
                self.players[p].max_cities = -1;
            }
        }
        let pl = &mut self.players[p];
        pl.pillaging_gold += info.pillaging_gold * change;
        pl.starting_gold += info.starting_gold * change;
        pl.summon_duration += info.summon_duration * change;
        pl.upgrade_cost_modifier += info.upgrade_cost_modifier * change;

        for y in 0..YieldType::NUM {
            self.player_update_extra_yield_threshold(p, YieldType::from_usize(y));
        }

        self.player_update_max_anarchy_turns(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use crate::rng::Rng;
    use crate::setup::GameSetup;

    /// A game whose first slots hold freshly reset players (C++ `reset`), alive on their own
    /// teams, with the given civilizations/leaders; other slots stay placeholders.
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

    fn set_civic(g: &mut Game, p: PlayerId, civic: CivicType) {
        let option = g.d.civics[civic].civic_option_type.unwrap();
        g.players[p].civics[option] = Some(civic);
    }

    #[test]
    fn max_anarchy_turns_are_lowered_by_traits() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        g.player_update_max_anarchy_turns(p);
        assert_eq!(g.players[p].max_anarchy_turns, d.defines.max_anarchy_turns);
        // TRAIT_CHARISMATIC has iMaxAnarchy -1 (no limit)
        g.players[p].traits[d.t::<TraitType>("TRAIT_CHARISMATIC")] = true;
        g.player_update_max_anarchy_turns(p);
        assert_eq!(g.players[p].max_anarchy_turns, d.defines.max_anarchy_turns);
        g.players[p].traits[d.t::<TraitType>("TRAIT_SPIRITUAL")] = true;
        g.player_update_max_anarchy_turns(p);
        assert_eq!(g.players[p].max_anarchy_turns, 0);
    }

    #[test]
    fn anarchy_lengths() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        g.player_update_max_anarchy_turns(p);
        let speed = d.game_speeds[g.game_speed_type()].anarchy_percent;
        let era = d.eras[g.start_era()].anarchy_percent;
        let scaled = |len: i32| range(len * speed / 100 * era / 100, 1, d.defines.max_anarchy_turns);

        // religion: BASE_RELIGION_ANARCHY_LENGTH (no cities, no anarchy modifier)
        assert_eq!(g.player_get_religion_anarchy_length(p), scaled(d.defines.base_religion_anarchy_length));

        // civics: the new civic's anarchy length + BASE_CIVIC_ANARCHY_LENGTH; nothing if unchanged
        let slavery = d.t::<CivicType>("CIVIC_SLAVERY");
        let unchanged = g.players[p].civics.clone();
        assert_eq!(g.player_get_civic_anarchy_length(p, &unchanged), 0);
        let mut new_civics = unchanged.clone();
        new_civics[d.civics[slavery].civic_option_type.unwrap()] = Some(slavery);
        let expected = scaled(d.civics[slavery].anarchy_length + d.defines.base_civic_anarchy_length);
        assert_eq!(g.player_get_civic_anarchy_length(p, &new_civics), expected);

        // -100% anarchy modifier: no anarchy
        g.players[p].anarchy_modifier = -100;
        assert_eq!(g.player_get_civic_anarchy_length(p, &new_civics), 0);
        assert_eq!(g.player_get_religion_anarchy_length(p), 0);
        g.players[p].anarchy_modifier = 0;

        // golden ages and spiritual leaders (max anarchy 0) skip anarchy
        g.players[p].golden_age_turns = 3;
        assert_eq!(g.player_get_civic_anarchy_length(p, &new_civics), 0);
        assert_eq!(g.player_get_religion_anarchy_length(p), 0);
        g.players[p].golden_age_turns = 0;
        g.players[p].max_anarchy_turns = 0;
        assert_eq!(g.player_get_civic_anarchy_length(p, &new_civics), 0);
        assert_eq!(g.player_get_religion_anarchy_length(p), 0);
    }

    #[test]
    fn can_do_civics_checks_tech_alignment_and_religion() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        let team = g.players[p].team;

        // UNOFFICIAL_PATCH: NO_CIVIC is always allowed
        assert!(g.player_can_do_civics(p, None));
        assert!(g.player_can_do_civics(p, Some(d.t::<CivicType>("CIVIC_DESPOTISM"))));

        // tech prerequisite (or the whole civic option)
        let city_states = d.t::<CivicType>("CIVIC_CITY_STATES");
        assert!(!g.player_can_do_civics(p, Some(city_states)));
        g.teams[team].has_tech[d.t::<TechType>("TECH_CARTOGRAPHY")] = true;
        assert!(g.player_can_do_civics(p, Some(city_states)));
        let republic = d.t::<CivicType>("CIVIC_REPUBLIC");
        assert!(!g.player_can_do_civics(p, Some(republic)));
        g.players[p].has_civic_option_count[d.civics[republic].civic_option_type.unwrap()] = 1;
        assert!(g.player_can_do_civics(p, Some(republic)));

        // FfH: Slavery is blocked for good civilizations
        let slavery = d.t::<CivicType>("CIVIC_SLAVERY");
        g.teams[team].has_tech[d.t::<TechType>("TECH_WAY_OF_THE_WICKED")] = true;
        assert!(g.player_can_do_civics(p, Some(slavery)));
        g.players[p].alignment = Some(d.t::<AlignmentType>("ALIGNMENT_GOOD"));
        assert!(!g.player_can_do_civics(p, Some(slavery)));
        g.players[p].alignment = Some(d.t::<AlignmentType>("ALIGNMENT_EVIL"));
        assert!(g.player_can_do_civics(p, Some(slavery)));

        // FfH: Sacrifice the Weak needs the Ashen Veil as state religion
        let sacrifice = d.t::<CivicType>("CIVIC_SACRIFICE_THE_WEAK");
        let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
        g.teams[team].has_tech[d.t::<TechType>("TECH_INFERNAL_PACT")] = true;
        assert!(!g.player_can_do_civics(p, Some(sacrifice)));
        g.players[p].last_state_religion = Some(veil);
        assert!(!g.player_can_do_civics(p, Some(sacrifice)), "last state religion is not a state religion");
        g.players[p].state_religion_count = 1;
        assert!(g.player_can_do_civics(p, Some(sacrifice)));
    }

    #[test]
    fn can_revolution() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        let despotism = d.t::<CivicType>("CIVIC_DESPOTISM");

        let same = g.players[p].civics.clone();
        let mut new_civics = same.clone();
        new_civics[d.civics[despotism].civic_option_type.unwrap()] = Some(despotism);
        assert!(g.player_can_revolution(p, None));
        assert!(g.player_can_revolution(p, Some(&new_civics)));
        assert!(!g.player_can_revolution(p, Some(&same)));

        g.players[p].revolution_timer = 2;
        assert!(!g.player_can_revolution(p, Some(&new_civics)));
        g.players[p].revolution_timer = 0;
        g.players[p].anarchy_turns = 1;
        assert!(!g.player_can_revolution(p, None));
    }

    #[test]
    fn civic_percent_anger_counts_foreign_cities() {
        let mut g = game(&[
            ("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"),
            ("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"),
            ("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"),
        ]);
        let d = g.d;
        let republic = d.t::<CivicType>("CIVIC_REPUBLIC");
        let anger = d.civics[republic].civic_percent_anger;
        assert!(anger > 0);
        // only city counts matter here
        g.players[PlayerId(1)].cities = vec![CityId(100), CityId(101)];
        g.players[PlayerId(2)].cities = vec![CityId(102)];

        let p = PlayerId(0);
        assert_eq!(g.player_get_civic_percent_anger(p, republic, false), 0);
        set_civic(&mut g, PlayerId(1), republic);
        assert_eq!(g.player_get_civic_percent_anger(p, republic, false), anger * 2 / 3);

        // no anger for the civic we run, unless asked to ignore it
        set_civic(&mut g, p, republic);
        assert_eq!(g.player_get_civic_percent_anger(p, republic, false), 0);
        assert_eq!(g.player_get_civic_percent_anger(p, republic, true), anger * 2 / 3);

        // FfH: no civic anger for the team
        let team = g.players[p].team;
        g.teams[team].no_civic_anger = true;
        assert_eq!(g.player_get_civic_percent_anger(p, republic, true), 0);
    }

    #[test]
    fn hidden_civics_and_religions() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);

        let undercouncil = d.t::<CivicType>("CIVIC_UNDERCOUNCIL");
        assert!(d.civics[undercouncil].hidden);
        assert!(!g.player_can_see_civic(p, undercouncil));
        set_civic(&mut g, p, undercouncil);
        assert!(g.player_can_see_civic(p, undercouncil));
        assert!(g.player_can_see_civic(p, d.t::<CivicType>("CIVIC_DESPOTISM")));

        let esus = d.t::<ReligionType>("RELIGION_COUNCIL_OF_ESUS");
        assert!(d.religions[esus].hidden);
        assert!(!g.player_can_see_religion(p, esus, None));
        g.players[p].last_state_religion = Some(esus);
        assert!(!g.player_can_see_religion(p, esus, None));
        g.players[p].state_religion_count = 1;
        assert!(g.player_can_see_religion(p, esus, None));
        assert!(g.player_can_see_religion(p, d.t::<ReligionType>("RELIGION_THE_ORDER"), None));
    }

    #[test]
    fn votes_and_membership_need_the_vote_source_civic() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        let overcouncil = d.t::<VoteSourceType>("DIPLOVOTE_OVERCOUNCIL");
        let civic = d.vote_sources[overcouncil].civic.unwrap();
        g.players[p].total_population = 7;

        assert_eq!(g.player_get_votes(p, None, overcouncil), 0);
        assert!(!g.player_is_voting_member(p, overcouncil));
        assert!(!g.player_is_full_member(p, overcouncil));

        set_civic(&mut g, p, civic);
        assert_eq!(g.player_get_votes(p, None, overcouncil), 7);
        assert!(g.player_is_voting_member(p, overcouncil));
        assert!(g.player_is_full_member(p, overcouncil));
        // civ voting: one vote per civilization
        let secretary_general = d.t::<VoteType>("VOTE_SECRETARY_GENERAL");
        assert_eq!(g.player_get_votes(p, Some(secretary_general), overcouncil), 1);
        // a vote of the other vote source
        let undercouncil = d.t::<VoteSourceType>("DIPLOVOTE_UNDERCOUNCIL");
        assert_eq!(g.player_get_votes(p, Some(secretary_general), undercouncil), 0);

        g.players[p].loyal_member[overcouncil] = false;
        assert!(g.player_is_voting_member(p, overcouncil));
        assert!(!g.player_is_full_member(p, overcouncil));
        g.players[p].loyal_member[overcouncil] = true;

        // a vote source religion: only the religion's population votes, members need it as
        // state religion
        let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
        g.vote_source_religions[overcouncil] = Some(order);
        assert_eq!(g.player_get_votes(p, None, overcouncil), 0);
        assert!(!g.player_is_full_member(p, overcouncil));
        assert_eq!(g.player_get_votes(p, Some(secretary_general), overcouncil), 0);
        g.players[p].has_religion_count[order] = 2;
        assert_eq!(g.player_get_votes(p, Some(secretary_general), overcouncil), 1);
    }

    #[test]
    fn can_convert_early_outs() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        let order = d.t::<ReligionType>("RELIGION_THE_ORDER");
        let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
        let leaves = d.t::<ReligionType>("RELIGION_FELLOWSHIP_OF_LEAVES");

        // no state religion civic
        assert!(!g.player_can_convert(p, None));
        g.players[p].state_religion_count = 1;
        g.players[p].last_state_religion = Some(leaves);
        assert!(g.player_can_convert(p, None));
        assert!(!g.player_can_convert(p, Some(leaves)), "already the state religion");

        // BETTER AI: an AI never converts to a religion its leader dislikes, nor away from a
        // favorite it likes more than 50
        assert!(!g.player_can_convert(p, Some(veil)));
        g.players[p].favorite_religion = Some(order);
        g.players[p].last_state_religion = Some(order);
        assert!(g.player_can_convert(p, None), "dropping the state religion skips the BETTER AI checks");
        assert!(!g.player_can_convert(p, Some(leaves)));

        g.players[p].last_state_religion = Some(leaves);
        g.players[p].conversion_timer = 1;
        assert!(!g.player_can_convert(p, None));
        g.players[p].conversion_timer = 0;
        g.players[p].anarchy_turns = 1;
        assert!(!g.player_can_convert(p, None));
    }

    #[test]
    fn favorite_religion_draws() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        let leader = g.player_get_leader_type(p).unwrap();
        let weights = &d.leader_heads[leader].religion_weight_modifier;

        // agnostics never pick one (and draw nothing)
        let agnostic = d.traits.keys().find(|&t| d.traits[t].agnostic).unwrap();
        g.players[p].traits[agnostic] = true;
        let seed = g.soren_rand.seed();
        g.player_ai_do_favorite_religion(p);
        assert_eq!(g.players[p].favorite_religion, None);
        assert_eq!(g.soren_rand.seed(), seed);
        g.players[p].traits[agnostic] = false;

        // one "Choose Religion" draw per religion until one beats its weight (Sabathiel: The
        // Order, weight 100), no conversion without a state religion
        let mut rng = Rng::new(seed);
        let mut expected = None;
        for r in d.religions.keys() {
            if rng.get_i(100, "Choose Religion") < weights[r] {
                expected = Some(r);
                break;
            }
        }
        g.player_ai_do_favorite_religion(p);
        assert_eq!(g.players[p].favorite_religion, expected);
        assert_eq!(expected, Some(d.t::<ReligionType>("RELIGION_THE_ORDER")));
        assert_eq!(g.soren_rand, rng);
        assert_eq!(g.players[p].last_state_religion, None);

        // a favorite is kept (no more draws)
        g.player_ai_do_favorite_religion(p);
        assert_eq!(g.soren_rand, rng);
    }

    #[test]
    fn single_civic_upkeep_early_outs() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        let slavery = d.t::<CivicType>("CIVIC_SLAVERY");
        assert!(d.civics[slavery].upkeep.is_some());

        assert_eq!(g.player_get_single_civic_upkeep(p, None, false), 0);
        g.players[p].anarchy_turns = 1;
        assert_eq!(g.player_get_single_civic_upkeep(p, Some(slavery), false), 0);
        g.players[p].anarchy_turns = 0;
        g.players[p].disable_production = 1;
        assert_eq!(g.player_get_single_civic_upkeep(p, Some(slavery), false), 0);
        g.players[p].disable_production = 0;
        g.players[p].no_civic_upkeep_count[d.civics[slavery].civic_option_type.unwrap()] = 1;
        assert_eq!(g.player_get_single_civic_upkeep(p, Some(slavery), true), 0);
    }
}
