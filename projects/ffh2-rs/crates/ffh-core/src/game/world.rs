//! CvGame: world content. Religions (founding, holy cities, `doHolyCity`), corporations
//! (`doHeadquarters`), world unit/building/project limits, forced civics, special
//! units/buildings, the shrine and culture-victory caches (`doUpdateCacheOnTurn`), plot extra
//! yields/costs, "ever active" queries, destroyed city / great person names; barbarians
//! (`createBarbarianCities`, `createBarbarianUnits`, `createAnimals`, FfH `foundBarbarianCity`)
//! and the other FfH additions: the global (Armageddon) counter, trophies, disabled religions.
//! (FfH's crime, challenge and scenario counters are plain fields.)
//!
//! Trivial CvGame accessors of this topic are fields (docs/porting/game.md, "Field access"):
//! `incrementUnitCreatedCount` = `unit_created_count[u] += 1`, `incrementUnitClassCreatedCount`,
//! `incrementBuildingClassCreatedCount`, `incrementProjectCreatedCount(e, iExtra)`,
//! `makeSpecialUnitValid`, `makeNukesValid`, `get/setBestLandUnit`, `getTrueGlobalCounter`, ...

use crate::defs::{RANDPLOT_ADJACENT_LAND, RANDPLOT_NOT_VISIBLE_TO_CIV, RANDPLOT_PASSIBLE};
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use crate::utils::{is_world_project, is_world_unit_class, is_world_wonder_class, range};
use ffh_data::*;

/// C++ `GET_PLAYER(p).getCivilizationType()` where the C++ indexes the civilization info with
/// it (alive players and the barbarians always have a civilization).
fn civ_of(g: &Game, p: PlayerId) -> CivilizationType {
    g.player_get_civilization_type(p).expect("CvPlayer::getCivilizationType() is NO_CIVILIZATION")
}

/// C++ `CvGameAI::AI_makeAssignWorkDirty`: every alive player's cities are flagged for citizen
/// re-assignment (governor bookkeeping, `CvPlayerAI::AI_makeAssignWorkDirty`).
fn ai_make_assign_work_dirty(g: &mut Game) {
    for p in g.player_ids() {
        if g.players[p].alive {
            g.player_ai_make_assign_work_dirty(p);
        }
    }
}

impl Game {
    // ---------------------------------------------------------------------------------------
    // Religions and corporations
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::doHolyCity`: founds religions whose slot is free (random religion offset,
    /// team then player chosen by fewest religions + soren rand; AI players +10).
    pub fn do_holy_city(&mut self) {
        // Python doHolyCity(): FFH2's CvGameUtils returns False, so the C++ default runs.

        if self.elapsed_game_turns < 5 && !self.is_option(GameOptionType::GAMEOPTION_ADVANCED_START) {
            return;
        }

        let d = self.d;
        let num_religions = d.religions.len() as i32;

        let rand_offset = self.soren_rand_num(num_religions, "Holy City religion offset");
        for i_loop in 0..num_religions {
            let religion = ReligionType::from_usize(((i_loop + rand_offset) % num_religions) as usize);

            if !self.religion_slot_taken[religion] {
                let mut best_value = i32::MAX;
                let mut best_team = None;

                for t in self.team_ids() {
                    if self.team_is_alive(t) {
                        if self.team_is_has_tech(t, d.religions[religion].tech_prereq) {
                            if self.teams[t].num_cities > 0 {
                                let mut value = self.soren_rand_num(10, "Found Religion (Team)");

                                for k in d.religions.keys() {
                                    let religion_count = self.team_get_has_religion_count(t, k);

                                    if religion_count > 0 {
                                        value += religion_count * 20;
                                    }
                                }

                                if value < best_value {
                                    best_value = value;
                                    best_team = Some(t);
                                }
                            }
                        }
                    }
                }

                if let Some(best_team) = best_team {
                    let mut best_value = i32::MAX;
                    let mut best_player = None;

                    for p in self.player_ids() {
                        if self.players[p].alive {
                            if self.players[p].team == best_team {
                                if self.players[p].num_cities() > 0 {
                                    let mut value = self.soren_rand_num(10, "Found Religion (Player)");

                                    if !self.players[p].human {
                                        value += 10;
                                    }

                                    for k in d.religions.keys() {
                                        let religion_count = self.players[p].has_religion_count[k];

                                        if religion_count > 0 {
                                            value += religion_count * 20;
                                        }
                                    }

                                    if value < best_value {
                                        best_value = value;
                                        best_player = Some(p);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(best_player) = best_player {
                        let mut found_religion = Some(religion);

                        if self.is_option(GameOptionType::GAMEOPTION_PICK_RELIGION) {
                            // C++ AI_chooseReligion() -> the controller (FfH locks the option
                            // off, see `is_option`).
                            found_religion = self.controller(best_player).choose_religion(self, best_player);
                        }

                        if let Some(found_religion) = found_religion {
                            self.player_found_religion(best_player, Some(found_religion), religion, false);
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvGame::doHeadquarters`: founds corporations (no corporations in FFH2 data).
    pub fn do_headquarters(&mut self) {
        // Python doHeadquarters(): FFH2's CvGameInterface.py does not define it (the call fails
        // and lResult stays 0), so the C++ default runs.

        if self.elapsed_game_turns < 5 {
            return;
        }

        let d = self.d;
        for corporation in d.corporations.keys() {
            let info = &d.corporations[corporation];
            if !self.is_corporation_founded(corporation) {
                let mut best_value = i32::MAX;
                let mut best_team = None;

                for t in self.team_ids() {
                    if self.team_is_alive(t) {
                        if info.tech_prereq.is_some() && self.team_is_has_tech(t, info.tech_prereq) {
                            if self.teams[t].num_cities > 0 {
                                // (`prereq_bonus` lists only the non-NONE slots of
                                // NUM_CORPORATION_PREREQ_BONUSES.)
                                let has_bonus = info.prereq_bonus.iter().any(|&b| self.team_has_bonus(t, b));

                                if has_bonus {
                                    let mut value = self.soren_rand_num(10, "Found Corporation (Team)");

                                    for k in d.corporations.keys() {
                                        // C++ quirk: GET_PLAYER((PlayerTypes)iJ) with the team
                                        // index iJ (team slot = player slot in sized arrays).
                                        let corporation_count = self.player_get_has_corporation_count(PlayerId(t.0), k);

                                        if corporation_count > 0 {
                                            value += corporation_count * 20;
                                        }
                                    }

                                    if value < best_value {
                                        best_value = value;
                                        best_team = Some(t);
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(best_team) = best_team {
                    let mut best_value = i32::MAX;
                    let mut best_player = None;

                    for p in self.player_ids() {
                        if self.players[p].alive {
                            if self.players[p].team == best_team {
                                if self.players[p].num_cities() > 0 {
                                    let has_bonus = info.prereq_bonus.iter().any(|&b| self.player_has_bonus(p, b));

                                    if has_bonus {
                                        // C++ quirk: the log string says "Religion".
                                        let mut value = self.soren_rand_num(10, "Found Religion (Player)");

                                        if !self.players[p].human {
                                            value += 10;
                                        }

                                        for k in d.corporations.keys() {
                                            let corporation_count = self.player_get_has_corporation_count(p, k);

                                            if corporation_count > 0 {
                                                value += corporation_count * 20;
                                            }
                                        }

                                        if value < best_value {
                                            best_value = value;
                                            best_player = Some(p);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(best_player) = best_player {
                        self.player_found_corporation(best_player, corporation);
                    }
                }
            }
        }
    }

    /// C++ `CvGame::isReligionFounded`.
    #[inline]
    pub fn is_religion_founded(&self, religion: ReligionType) -> bool {
        self.religion_game_turn_founded[religion] != -1
    }

    /// C++ `CvGame::makeReligionFounded(eIndex, ePlayer)`.
    pub fn make_religion_founded(&mut self, religion: ReligionType, player: PlayerId) {
        if !self.is_religion_founded(religion) {
            debug_assert!(self.game_turn() != -1, "getGameTurn() is not expected to be equal with -1");
            self.religion_game_turn_founded[religion] = self.game_turn();

            self.ev_religion_founded(religion, player);
        }
    }

    /// C++ `CvGame::isCorporationFounded`.
    #[inline]
    pub fn is_corporation_founded(&self, corporation: CorporationType) -> bool {
        self.corporation_game_turn_founded[corporation] != -1
    }

    /// C++ `CvGame::makeCorporationFounded(eIndex, ePlayer)`.
    pub fn make_corporation_founded(&mut self, corporation: CorporationType, player: PlayerId) {
        if !self.is_corporation_founded(corporation) {
            debug_assert!(self.game_turn() != -1, "getGameTurn() is not expected to be equal with -1");
            self.corporation_game_turn_founded[corporation] = self.game_turn();

            self.ev_corporation_founded(corporation, player);
        }
    }

    /// C++ `CvGame::getHolyCity(eIndex)` (`getCity(IDInfo)`: `None` once the city is gone).
    #[inline]
    pub fn get_holy_city(&self, religion: ReligionType) -> Option<CityId> {
        self.holy_city[religion].filter(|&c| self.cities.contains(c))
    }

    /// C++ `CvGame::setHolyCity(eIndex, pNewValue, bAnnounce)`: FfH also changes the
    /// Armageddon counter by the religion's `iGlobalCounterFound`; moves the holy city
    /// influence and religion commerce; announcement messages.
    pub fn set_holy_city(&mut self, religion: ReligionType, new_value: Option<CityId>, announce: bool) {
        let d = self.d;
        let old_value = self.get_holy_city(religion);

        if old_value != new_value {
            // religion visibility now part of espionage
            //updateCitySight(false, true);

            if let Some(new_city) = new_value {
                self.holy_city[religion] = Some(new_city);

                // FfH: Added by Kael 08/13/2007
                self.change_global_counter(d.religions[religion].global_counter_found);
            } else {
                self.holy_city[religion] = None;

                // FfH: Added by Kael 08/13/2007
                self.change_global_counter(-d.religions[religion].global_counter_found);
            }
            // C++ quirk: moving a holy city from one city to another raises the counter again
            // (only the change to "no holy city" lowers it).

            // religion visibility now part of espionage
            //updateCitySight(true, true);

            let influence = d.defines.holy_city_influence;

            if let Some(old_city) = old_value {
                // changeReligionInfluence
                self.cities[old_city].religion_influence[religion] += -influence;
                debug_assert!(self.cities[old_city].religion_influence[religion] >= 0);

                self.city_update_religion_commerces(old_city);
                // setInfoDirty(true): UI.
            }

            if let Some(holy_city) = self.get_holy_city(religion) {
                self.city_set_has_religion(holy_city, religion, true, announce, true);
                // changeReligionInfluence
                self.cities[holy_city].religion_influence[religion] += influence;
                debug_assert!(self.cities[holy_city].religion_influence[religion] >= 0);

                self.city_update_religion_commerces(holy_city);
                // setInfoDirty(true): UI.

                if announce {
                    // (gDLL->GetWorldBuilderMode() is never set.)
                    if self.final_initialized {
                        // C++: replay message, and TXT_KEY_MISC_REL_FOUNDED to every alive player
                        // that has revealed the city and can see the religion (FfH
                        // canSeeReligion), TXT_KEY_MISC_REL_FOUNDED_UNKNOWN to the others.
                        let turn = self.game_turn();
                        let cities = &self.cities;
                        self.log.msg(turn, Some(cities[holy_city].owner), || {
                            format!(
                                "{} has been founded in {}",
                                d.text.name(&d.religions[religion].base.description),
                                cities[holy_city].name
                            )
                        });
                    }
                }
            }

            ai_make_assign_work_dirty(self);
        }
    }

    /// C++ `CvGame::getHeadquarters(eIndex)` (`None` once the city is gone).
    #[inline]
    pub fn get_headquarters(&self, corporation: CorporationType) -> Option<CityId> {
        self.headquarters[corporation].filter(|&c| self.cities.contains(c))
    }

    /// C++ `CvGame::setHeadquarters(eIndex, pNewValue, bAnnounce)`.
    pub fn set_headquarters(&mut self, corporation: CorporationType, new_value: Option<CityId>, announce: bool) {
        let old_value = self.get_headquarters(corporation);

        if old_value != new_value {
            self.headquarters[corporation] = new_value;

            if let Some(old_city) = old_value {
                self.city_update_corporation(old_city);
                // setInfoDirty(true): UI.
            }

            if let Some(headquarters) = self.get_headquarters(corporation) {
                self.city_set_has_corporation(headquarters, corporation, true, announce, true);
                self.city_update_corporation(headquarters);
                // setInfoDirty(true): UI.

                if announce {
                    // (gDLL->GetWorldBuilderMode() is never set.)
                    if self.final_initialized {
                        // C++: replay message, and TXT_KEY_MISC_CORPORATION_FOUNDED to every alive
                        // player that has revealed the city (..._UNKNOWN to the others).
                        let d = self.d;
                        let turn = self.game_turn();
                        let cities = &self.cities;
                        self.log.msg(turn, Some(cities[headquarters].owner), || {
                            format!(
                                "{} has been founded in {}",
                                d.text.name(&d.corporations[corporation].base.description),
                                cities[headquarters].name
                            )
                        });
                    }
                }
            }

            ai_make_assign_work_dirty(self);
        }
    }

    /// C++ `CvGame::isCompetingCorporation(eCorporation1, eCorporation2)`: the two share a
    /// prerequisite bonus. (C++ also tests a `bShareResources` flag that is never set.)
    pub fn is_competing_corporation(&self, corporation1: CorporationType, corporation2: CorporationType) -> bool {
        let d = self.d;
        // (`prereq_bonus` lists only the non-NONE slots of NUM_CORPORATION_PREREQ_BONUSES.)
        for &bonus1 in &d.corporations[corporation1].prereq_bonus {
            for &bonus2 in &d.corporations[corporation2].prereq_bonus {
                if bonus1 == bonus2 {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvGame::countReligionLevels(eReligion)`: cities with the religion, all alive players.
    pub fn count_religion_levels(&self, religion: ReligionType) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                count += self.players[p].has_religion_count[religion];
            }
        }

        count
    }

    /// C++ `CvGame::calculateReligionPercent(eReligion)`: share of the world population following
    /// the religion (a city's population split evenly between its religions, rounded).
    pub fn calculate_religion_percent(&self, religion: ReligionType) -> i32 {
        if self.total_population == 0 {
            return 0;
        }

        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                for &c in &self.players[p].cities {
                    if self.cities[c].has_religion[religion] {
                        let religion_count = self.city_religion_count(c);
                        count += (self.cities[c].population + (religion_count / 2)) / religion_count;
                    }
                }
            }
        }

        (count * 100) / self.total_population
    }

    /// C++ `CvGame::countCorporationLevels(eCorporation)`.
    pub fn count_corporation_levels(&self, corporation: CorporationType) -> i32 {
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                count += self.player_get_has_corporation_count(p, corporation);
            }
        }

        count
    }

    /// C++ `CvGame::replaceCorporation(eCorporation1, eCorporation2)`: every city with the first
    /// corporation gets the second instead; units spreading the first are killed.
    pub fn replace_corporation(&mut self, corporation1: CorporationType, corporation2: CorporationType) {
        let d = self.d;

        for p in self.player_ids() {
            if self.players[p].alive {
                for c in self.players[p].cities.clone() {
                    if self.cities.contains(c) && self.cities[c].has_corporation[corporation1] {
                        self.city_set_has_corporation(c, corporation1, false, false, false);
                        self.city_set_has_corporation(c, corporation2, true, true, true);
                    }
                }

                for u in self.players[p].units.clone() {
                    if self.units.contains(u) && d.units[self.units[u].unit_type].corporation_spreads[corporation1] > 0 {
                        self.unit_kill(u, false, None);
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Limits, forced civics, special units/buildings
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::isUnitClassMaxedOut(eIndex, iExtra)` (world units only).
    pub fn is_unit_class_maxed_out(&self, unit_class: UnitClassType, extra: i32) -> bool {
        if !is_world_unit_class(self.d, unit_class) {
            return false;
        }

        // (C++ keeps the "created <= max" assert commented out here.)
        (self.unit_class_created_count[unit_class] + extra) >= self.d.unit_classes[unit_class].max_global_instances
    }

    /// C++ `CvGame::isBuildingClassMaxedOut(eIndex, iExtra)` (world wonders only).
    pub fn is_building_class_maxed_out(&self, building_class: BuildingClassType, extra: i32) -> bool {
        if !is_world_wonder_class(self.d, building_class) {
            return false;
        }

        // C++ quirk: FAssertMsg(getBuildingClassCreatedCount(e) <= getMaxGlobalInstances()) is
        // reachable, so it is not ported as an assertion. FfH equipment wonders (Syliven's Perfect
        // Lyre) leave a city through a take spell (Python `setNumRealBuilding(i, 0)`, which does
        // not lower the count) and come back through a `CreateBuildingType` drop spell
        // (`setNumRealBuilding(e, true)`, CvUnit.cpp:14589), and every drop counts the wonder as
        // created again (CvCity.cpp:10619). A debug DLL only shows an assert dialog; the class
        // stays maxed out.

        (self.building_class_created_count[building_class] + extra) >= self.d.building_classes[building_class].max_global_instances
    }

    /// C++ `CvGame::isProjectMaxedOut(eIndex, iExtra)` (world projects only).
    pub fn is_project_maxed_out(&self, project: ProjectType, extra: i32) -> bool {
        if !is_world_project(self.d, project) {
            return false;
        }

        debug_assert!(
            self.project_created_count[project] <= self.d.projects[project].max_global_instances,
            "Index is expected to be within maximum bounds (invalid Index)"
        );

        (self.project_created_count[project] + extra) >= self.d.projects[project].max_global_instances
    }

    /// C++ `CvGame::isForceCivic`.
    #[inline]
    pub fn is_force_civic(&self, civic: CivicType) -> bool {
        self.force_civic_count[civic] > 0
    }

    /// C++ `CvGame::isForceCivicOption(eCivicOption)`.
    pub fn is_force_civic_option(&self, civic_option: CivicOptionType) -> bool {
        for civic in self.d.civics.keys() {
            if self.d.civics[civic].civic_option_type == Some(civic_option) {
                if self.is_force_civic(civic) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvGame::changeForceCivicCount(eIndex, iChange)`: players re-verify their civics
    /// when a civic becomes (un)forced.
    pub fn change_force_civic_count(&mut self, civic: CivicType, change: i32) {
        if change != 0 {
            let old_force_civic = self.is_force_civic(civic);

            self.force_civic_count[civic] += change;
            debug_assert!(self.force_civic_count[civic] >= 0);

            if old_force_civic != self.is_force_civic(civic) {
                self.verify_civics();
            }
        }
    }

    /// C++ `CvGame::makeSpecialBuildingValid(eIndex, bAnnounce)`.
    pub fn make_special_building_valid(&mut self, special_building: SpecialBuildingType, announce: bool) {
        if !self.special_building_valid[special_building] {
            self.special_building_valid[special_building] = true;

            if announce {
                // C++: TXT_KEY_SPECIAL_BUILDING_VALID message to every alive player.
                let d = self.d;
                let turn = self.game_turn();
                self.log.msg(turn, None, || {
                    format!("{} can now be built", d.text.name(&d.special_buildings[special_building].base.description))
                });
            }
        }
    }

    /// C++ `CvGame::canTrainNukes`: some alive player can train a nuke (`iNukeRange != -1`).
    pub fn can_train_nukes(&self) -> bool {
        let d = self.d;

        for p in self.player_ids() {
            if self.players[p].alive {
                let civ = civ_of(self, p);
                for unit_class in d.unit_classes.keys() {
                    if let Some(unit) = d.civ_unit(civ, unit_class) {
                        if d.units[unit].nuke_range != -1 {
                            if self.player_can_train(p, unit, false, false, false) {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// C++ `CvGame::getNumFreeBonuses(eBuilding)` (-1 in XML = world size default).
    pub fn num_free_bonuses(&self, building: BuildingType) -> i32 {
        let num_free_bonuses = self.d.buildings[building].num_free_bonuses;
        if num_free_bonuses == -1 {
            self.d.worlds[self.setup.world_size].num_free_building_bonuses
        } else {
            num_free_bonuses
        }
    }

    /// C++ `CvGame::getImprovementUpgradeTime(eImprovement)` (scaled by speed and start era).
    pub fn improvement_upgrade_time(&self, improvement: ImprovementType) -> i32 {
        let d = self.d;
        let mut time = d.improvements[improvement].upgrade_time;

        time *= d.game_speeds[self.game_speed_type()].improvement_percent;
        time /= 100;

        time *= d.eras[self.start_era()].improvement_percent;
        time /= 100;

        time
    }

    /// C++ `CvGame::processBuilding(eBuilding, iChange)`: diplo vote sources of the building.
    pub fn process_building(&mut self, building: BuildingType, change: i32) {
        let d = self.d;
        for vote_source in d.vote_sources.keys() {
            if d.buildings[building].vote_source_type == Some(vote_source) {
                self.change_diplo_vote(vote_source, change);
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Caches: shrines and culture victory
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::doUpdateCacheOnTurn`: rebuilds the shrine cache (buildings with a holy
    /// city religion) and the strictest valid culture victory.
    pub fn do_update_cache_on_turn(&mut self) {
        let d = self.d;

        // reset shrine count
        self.shrine_building_count = 0;

        for building in d.buildings.keys() {
            let info = &d.buildings[building];

            // if it is for holy city, then its a shrine-thing, add it
            if info.holy_city.is_some() {
                self.change_shrine_building(building, info.religion_type, false);
            }
        }

        // reset cultural victories
        self.num_culture_victory_cities = 0;
        for victory in d.victories.keys() {
            if self.is_victory_valid(victory) {
                let info = &d.victories[victory];
                // C++ `getCityCulture() > 0` (CULTURELEVEL_NONE is 0, NO_CULTURELEVEL -1)
                if info.city_culture.is_some_and(|level| level.i() > 0) {
                    let num_culture_cities = info.num_culture_cities;
                    if num_culture_cities > self.num_culture_victory_cities {
                        self.num_culture_victory_cities = num_culture_cities;
                        self.culture_victory_culture_level = info.city_culture;
                    }
                }
            }
        }
    }

    /// C++ `CvGame::getShrineBuildingCount(eReligion)` (`None` = all religions).
    pub fn get_shrine_building_count(&self, religion: Option<ReligionType>) -> i32 {
        match religion {
            None => self.shrine_building_count,
            Some(r) => (0..self.shrine_building_count as usize).filter(|&i| self.shrine_religion[i] == Some(r)).count() as i32,
        }
    }

    /// C++ `CvGame::getShrineBuilding(eIndex, eReligion)`: the index-th shrine (of `religion`).
    pub fn get_shrine_building(&self, index: i32, religion: Option<ReligionType>) -> Option<BuildingType> {
        debug_assert!(index >= 0 && index < self.shrine_building_count, "invalid index to CvGame::getShrineBuilding");

        let mut building = None;
        if index >= 0 && index < self.shrine_building_count {
            match religion {
                None => building = self.shrine_building[index as usize],
                Some(r) => {
                    let mut religious_building = 0;
                    for i in 0..self.shrine_building_count as usize {
                        if self.shrine_religion[i] == Some(r) {
                            if religious_building == index {
                                // found it
                                building = self.shrine_building[i];
                                break;
                            }
                            religious_building += 1;
                        }
                    }
                }
            }
        }
        building
    }

    /// C++ `CvGame::changeShrineBuilding(eBuilding, eReligion, bRemove)`.
    pub fn change_shrine_building(&mut self, building: BuildingType, religion: Option<ReligionType>, remove: bool) {
        debug_assert!(
            remove || (self.shrine_building_count as usize) < self.d.buildings.len(),
            "trying to add too many buildings to CvGame::changeShrineBuilding"
        );

        if remove {
            let mut found = false;
            let mut i = 0;
            while i < self.shrine_building_count {
                let iu = i as usize;
                if !found {
                    // note, the religion is not important if we are removing, since each
                    // building is always one religion
                    if self.shrine_building[iu] == Some(building) {
                        found = true;
                    }
                }

                if found {
                    let to_move = i + 1;
                    if to_move < self.shrine_building_count {
                        self.shrine_building[iu] = self.shrine_building[to_move as usize];
                        self.shrine_religion[iu] = self.shrine_religion[to_move as usize];
                    } else {
                        self.shrine_building[iu] = None;
                        self.shrine_religion[iu] = None;
                    }
                }

                // C++ quirk: this decrement sits inside the loop, so the count drops once per
                // remaining iteration after the match (trailing entries are lost).
                if found {
                    self.shrine_building_count -= 1;
                }
                i += 1;
            }
        } else if (self.shrine_building_count as usize) < self.d.buildings.len() {
            // add this item to the end
            let n = self.shrine_building_count as usize;
            self.shrine_building[n] = Some(building);
            self.shrine_religion[n] = religion;
            self.shrine_building_count += 1;
        }
    }

    /// C++ `CvGame::culturalVictoryValid`.
    #[inline]
    pub fn cultural_victory_valid(&self) -> bool {
        self.num_culture_victory_cities > 0
    }

    /// C++ `CvGame::culturalVictoryCultureLevel`.
    #[inline]
    pub fn cultural_victory_culture_level(&self) -> Option<CultureLevelType> {
        if self.num_culture_victory_cities > 0 {
            self.culture_victory_culture_level
        } else {
            None
        }
    }

    /// C++ `CvGame::getCultureThreshold(eLevel)` (speed threshold; FfH NO_ESPIONAGE modifier).
    pub fn culture_threshold(&self, level: CultureLevelType) -> i32 {
        let mut threshold = self.d.culture_levels[level].speed_threshold[self.game_speed_type()];
        if self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            threshold *= 100 + self.d.defines.no_espionage_culture_level_modifier;
            threshold /= 100;
        }
        threshold
    }

    // ---------------------------------------------------------------------------------------
    // Plot extra yields and costs
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::getPlotExtraYield(iX, iY, eYield)`.
    pub fn plot_extra_yield(&self, x: i32, y: i32, yield_type: YieldType) -> i32 {
        for e in &self.plot_extra_yields {
            if e.x == x && e.y == y {
                return e.extra_yield[yield_type.idx()];
            }
        }
        0
    }

    /// C++ `CvGame::setPlotExtraYield(iX, iY, eYield, iExtraYield)` (adds to an existing entry;
    /// then `CvPlot::updateYield`).
    pub fn set_plot_extra_yield(&mut self, x: i32, y: i32, yield_type: YieldType, extra_yield: i32) {
        let mut found = false;

        for e in &mut self.plot_extra_yields {
            if e.x == x && e.y == y {
                e.extra_yield[yield_type.idx()] += extra_yield;
                found = true;
                break;
            }
        }

        if !found {
            let mut extra = [0; YieldType::NUM];
            extra[yield_type.idx()] = extra_yield;
            self.plot_extra_yields.push(PlotExtraYield { x, y, extra_yield: extra });
        }

        if let Some(p) = self.map.plot_xy(x, y) {
            self.plot_update_yield(p);
        }
    }

    /// C++ `CvGame::removePlotExtraYield(iX, iY)` (then `CvPlot::updateYield`).
    pub fn remove_plot_extra_yield(&mut self, x: i32, y: i32) {
        if let Some(i) = self.plot_extra_yields.iter().position(|e| e.x == x && e.y == y) {
            self.plot_extra_yields.remove(i);
        }

        if let Some(p) = self.map.plot_xy(x, y) {
            self.plot_update_yield(p);
        }
    }

    /// C++ `CvGame::getPlotExtraCost(iX, iY)`.
    pub fn plot_extra_cost(&self, x: i32, y: i32) -> i32 {
        for e in &self.plot_extra_costs {
            if e.x == x && e.y == y {
                return e.cost;
            }
        }
        0
    }

    /// C++ `CvGame::changePlotExtraCost(iX, iY, iCost)`.
    pub fn change_plot_extra_cost(&mut self, x: i32, y: i32, cost: i32) {
        for e in &mut self.plot_extra_costs {
            if e.x == x && e.y == y {
                e.cost += cost;
                return;
            }
        }
        self.plot_extra_costs.push(PlotExtraCost { x, y, cost });
    }

    /// C++ `CvGame::removePlotExtraCost(iX, iY)`.
    pub fn remove_plot_extra_cost(&mut self, x: i32, y: i32) {
        if let Some(i) = self.plot_extra_costs.iter().position(|e| e.x == x && e.y == y) {
            self.plot_extra_costs.remove(i);
        }
    }

    // ---------------------------------------------------------------------------------------
    // "Ever active" queries, names
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::isCivEverActive(eCivilization)`.
    pub fn is_civ_ever_active(&self, civilization: CivilizationType) -> bool {
        for p in self.player_ids() {
            if self.players[p].ever_alive {
                if self.player_get_civilization_type(p) == Some(civilization) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvGame::isLeaderEverActive(eLeader)`.
    pub fn is_leader_ever_active(&self, leader: LeaderHeadType) -> bool {
        for p in self.player_ids() {
            if self.players[p].ever_alive {
                if self.player_get_leader_type(p) == Some(leader) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvGame::isUnitEverActive(eUnit)`: the unit of a civilization that was ever alive.
    pub fn is_unit_ever_active(&self, unit: UnitType) -> bool {
        let d = self.d;

        for civ in d.civilizations.keys() {
            if self.is_civ_ever_active(civ) {
                let unit_class = d.units[unit].unit_class_type.expect("CvUnitInfo without a unit class");
                if d.civ_unit(civ, unit_class) == Some(unit) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvGame::isBuildingEverActive(eBuilding)`.
    pub fn is_building_ever_active(&self, building: BuildingType) -> bool {
        let d = self.d;

        for civ in d.civilizations.keys() {
            if self.is_civ_ever_active(civ) {
                let building_class = d.buildings[building].building_class_type.expect("CvBuildingInfo without a building class");
                if d.civ_building(civ, building_class) == Some(building) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvGame::isDestroyedCityName` (`addDestroyedCityName` = `destroyed_cities.push`).
    pub fn is_destroyed_city_name(&self, name: &str) -> bool {
        self.destroyed_cities.iter().any(|n| n == name)
    }

    /// C++ `CvGame::isGreatPersonBorn` (`addGreatPersonBornName` = `great_people_born.push`).
    pub fn is_great_person_born(&self, name: &str) -> bool {
        self.great_people_born.iter().any(|n| n == name)
    }
}

impl Game {
    // ---------------------------------------------------------------------------------------
    // Barbarians and animals
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::createBarbarianCities` (handicap creation turn/probability, unowned tiles
    /// per barbarian city, `AI_foundValue` of the barbarian player + soren rand).
    pub fn create_barbarian_cities(&mut self) {
        if self.max_city_elimination() > 0 {
            return;
        }

        if self.is_option(GameOptionType::GAMEOPTION_NO_BARBARIANS) {
            return;
        }

        // Python createBarbarianCities(): FFH2's CvGameUtils returns False, so the C++ default
        // runs.

        let d = self.d;
        let current_era = self.current_era().expect("CvGame::getCurrentEra() is NO_ERA");
        if d.eras[current_era].no_barb_cities {
            return;
        }

        let handicap = &d.handicaps[self.handicap];
        if handicap.unowned_tiles_per_barbarian_city <= 0 {
            return;
        }

        if self.num_civ_cities() < (self.count_civ_players_alive() * 2) {
            return;
        }

        let barb_percent = d.game_speeds[self.game_speed_type()].barb_percent;
        if self.elapsed_game_turns < (((handicap.barbarian_city_creation_turns_elapsed * barb_percent) / 100) / std::cmp::max(self.start_era().i() + 1, 1)) {
            return;
        }

        if self.soren_rand_num(100, "Barb City Creation") >= handicap.barbarian_city_creation_prob {
            return;
        }

        let barbarian = self.barbarian_player();

        let mut best_value = 0;
        let mut best_plot = None;

        let mut target_cities_multiplier = 100;
        {
            let target_barb_cities = (self.num_civ_cities() * 5 * handicap.barbarian_city_creation_prob) / 100;
            let barb_cities = self.players[barbarian].num_cities();
            if barb_cities < target_barb_cities {
                target_cities_multiplier += (300 * (target_barb_cities - barb_cities)) / target_barb_cities;
            }

            if self.is_option(GameOptionType::GAMEOPTION_RAGING_BARBARIANS) {
                target_cities_multiplier *= 3;
                target_cities_multiplier /= 2;
            }
        }

        for p in self.map.plot_ids() {
            if !self.plot_is_water(p) {
                if !self.plot_is_visible_to_civ_team(p) {
                    let area = self.plot_area(p).expect("CvPlot::area() is NULL");
                    let mut target_cities = self.area_get_num_unowned_tiles(area);

                    if self.map.areas[area].num_cities == self.map.areas[area].cities_per_player[barbarian] {
                        target_cities *= 3;
                    }

                    let unowned_tiles_threshold = handicap.unowned_tiles_per_barbarian_city;

                    if self.map.areas[area].num_tiles < (unowned_tiles_threshold / 3) {
                        target_cities *= target_cities_multiplier;
                        target_cities /= 100;
                    }

                    target_cities /= std::cmp::max(1, unowned_tiles_threshold);

                    if self.map.areas[area].cities_per_player[barbarian] < target_cities {
                        let (x, y) = self.map.xy(p);
                        let mut value = self.player_ai_found_value(barbarian, x, y, d.defines.min_barbarian_city_starting_distance, false, &[]);
                        if target_cities_multiplier > 100 {
                            value *= self.map.areas[area].num_owned_tiles;
                        }

                        value += 100 + self.soren_rand_num(50, "Barb City Found");
                        value /= 100;

                        if value > best_value {
                            best_value = value;
                            best_plot = Some(p);
                        }
                    }
                }
            }
        }

        if let Some(best_plot) = best_plot {
            let (x, y) = self.map.xy(best_plot);
            self.player_found(barbarian, x, y);
        }
    }

    /// C++ `CvGame::createBarbarianUnits`: animals early (or with few cities), else barbarian
    /// units per area (FfH: animals do not count, `/6 + 1` spawn rate, no world-unique classes).
    pub fn create_barbarian_units(&mut self) {
        if self.is_option(GameOptionType::GAMEOPTION_NO_BARBARIANS) {
            return;
        }

        // Python createBarbarianUnits(): FFH2's CvGameUtils returns False, so the C++ default
        // runs.

        let d = self.d;
        let mut animals = false;

        let current_era = self.current_era().expect("CvGame::getCurrentEra() is NO_ERA");
        if d.eras[current_era].no_barb_units {
            animals = true;
        }

        if self.num_civ_cities() < ((self.count_civ_players_alive() * 3) / 2) && !self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) {
            animals = true;
        }

        // FfH: Added by Kael 11/27/2007
        if self.is_option(GameOptionType::GAMEOPTION_DOUBLE_ANIMALS) && self.num_civ_cities() < self.count_civ_players_alive() * 3 {
            animals = true;
        }
        // FfH: End Add

        let handicap = &d.handicaps[self.handicap];
        if self.elapsed_game_turns < ((handicap.barbarian_creation_turns_elapsed * d.game_speeds[self.game_speed_type()].barb_percent) / 100) {
            animals = true;
        }

        if animals {
            self.create_animals();
        } else {
            let barbarian = self.barbarian_player();
            let barbarian_team = self.barbarian_team();
            let barbarian_civ = civ_of(self, barbarian);

            for area in self.map.areas.ids() {
                let water = self.map.areas[area].water;

                let (barb_unit_ai, mut divisor) = if water {
                    (UnitAIType::UNITAI_ATTACK_SEA, handicap.unowned_water_tiles_per_barbarian_unit)
                } else {
                    (UnitAIType::UNITAI_ATTACK, handicap.unowned_tiles_per_barbarian_unit)
                };

                if self.is_option(GameOptionType::GAMEOPTION_RAGING_BARBARIANS) {
                    divisor = std::cmp::max(1, divisor / 2);
                }

                if divisor > 0 {
                    // FfH: Modified by Kael 08/27/2007 (so that animals arent considered for barb
                    // spawn rates, and barbs spawn a little slower)
                    let barb_units = self.map.areas[area].units_per_player[barbarian] - self.map.areas[area].animals_per_player[barbarian];
                    let mut needed_barbs = (self.area_get_num_unowned_tiles(area) / divisor) - barb_units;
                    if needed_barbs > 0 {
                        needed_barbs = (needed_barbs / 6) + 1;
                        // FfH: End Modify

                        for _ in 0..needed_barbs {
                            let plot = self.map_sync_rand_plot(
                                RANDPLOT_NOT_VISIBLE_TO_CIV | RANDPLOT_ADJACENT_LAND | RANDPLOT_PASSIBLE,
                                Some(area),
                                d.defines.min_barbarian_starting_distance,
                                100,
                            );

                            if let Some(plot) = plot {
                                let mut best_unit = None;
                                let mut best_value = 0;

                                for unit_class in d.unit_classes.keys() {
                                    let Some(loop_unit) = d.civ_unit(barbarian_civ, unit_class) else {
                                        continue;
                                    };
                                    let unit = &d.units[loop_unit];

                                    let mut valid = unit.combat > 0 && !unit.only_defensive;

                                    // FfH: Added by Kael 08/14/2007
                                    if d.unit_classes[unit_class].max_global_instances == 1 {
                                        valid = false;
                                    }
                                    // FfH: End Add

                                    if valid {
                                        if water && unit.domain_type != Some(DomainType::DOMAIN_SEA) {
                                            valid = false;
                                        } else if !water && unit.domain_type != Some(DomainType::DOMAIN_LAND) {
                                            valid = false;
                                        }
                                    }

                                    if valid {
                                        if !self.player_can_train(barbarian, loop_unit, false, false, false) {
                                            valid = false;
                                        }
                                    }

                                    if valid {
                                        if let Some(bonus) = unit.prereq_and_bonus {
                                            if !self.team_is_has_tech(barbarian_team, d.bonuses[bonus].tech_city_trade) {
                                                valid = false;
                                            }
                                        }
                                    }

                                    if valid {
                                        let mut found = false;
                                        let mut requires = false;
                                        // (`prereq_or_bonuses` lists only the non-NONE slots.)
                                        for &bonus in &unit.prereq_or_bonuses {
                                            if let Some(tech) = d.bonuses[bonus].tech_city_trade {
                                                requires = true;

                                                if self.team_is_has_tech(barbarian_team, Some(tech)) {
                                                    found = true;
                                                    break;
                                                }
                                            }
                                        }

                                        if requires && !found {
                                            valid = false;
                                        }
                                    }

                                    if valid {
                                        let mut value = 1 + self.soren_rand_num(1000, "Barb Unit Selection");

                                        if unit.unit_ai_type[barb_unit_ai] {
                                            value += 200;
                                        }

                                        if value > best_value {
                                            best_unit = Some(loop_unit);
                                            best_value = value;
                                        }
                                    }
                                }

                                if let Some(best_unit) = best_unit {
                                    let (x, y) = self.map.xy(plot);
                                    self.player_init_unit(barbarian, best_unit, x, y, Some(barb_unit_ai), None);
                                }
                            }
                        }
                    }
                }
            }

            // FfH: Modified by Kael 08/02/2007 (so animals are never deleted): the BTS loop that
            // killed one barbarian animal per turn is commented out.
        }
    }

    /// C++ `CvGame::createAnimals` (FfH: also in water areas; GAMEOPTION_DOUBLE_ANIMALS; animals
    /// get `HIDDEN_NATIONALITY_PROMOTION`).
    pub fn create_animals(&mut self) {
        let d = self.d;

        let current_era = self.current_era().expect("CvGame::getCurrentEra() is NO_ERA");
        if d.eras[current_era].no_animals {
            return;
        }

        let handicap = &d.handicaps[self.handicap];
        if handicap.unowned_tiles_per_game_animal <= 0 {
            return;
        }

        if self.num_civ_cities() < self.count_civ_players_alive() {
            return;
        }

        if self.elapsed_game_turns < 5 {
            return;
        }

        // FfH: Modified by Kael 08/27/2007 (So that water animals spawn): every area, water areas
        // with a fifth of the animals (none below 20 unowned tiles or above 3 barbarian units).
        let barbarian = self.barbarian_player();
        let barbarian_civ = civ_of(self, barbarian);

        for area in self.map.areas.ids() {
            let unowned_tiles = self.area_get_num_unowned_tiles(area);
            let barb_units = self.map.areas[area].units_per_player[barbarian];

            let mut needed_animals = (unowned_tiles / handicap.unowned_tiles_per_game_animal) - barb_units;
            if self.map.areas[area].water {
                needed_animals /= 5;
                if unowned_tiles < 20 {
                    needed_animals = 0;
                }
                if barb_units > 3 {
                    needed_animals = 0;
                }
            }

            if needed_animals > 0 {
                needed_animals = (needed_animals / 5) + 1;
                if self.is_option(GameOptionType::GAMEOPTION_DOUBLE_ANIMALS) {
                    needed_animals *= 2;
                }

                for _ in 0..needed_animals {
                    let plot = self.map_sync_rand_plot(
                        RANDPLOT_NOT_VISIBLE_TO_CIV | RANDPLOT_PASSIBLE,
                        Some(area),
                        d.defines.min_animal_starting_distance,
                        100,
                    );

                    if let Some(plot) = plot {
                        let mut best_unit = None;
                        let mut best_value = 0;

                        let feature = self.map.plots[plot].feature_type;
                        let terrain = self.map.plots[plot].terrain_type;

                        for unit_class in d.unit_classes.keys() {
                            if let Some(loop_unit) = d.civ_unit(barbarian_civ, unit_class) {
                                let unit = &d.units[loop_unit];
                                if unit.unit_ai_type[UnitAIType::UNITAI_ANIMAL] {
                                    let native = match feature {
                                        Some(feature) => unit.feature_native[feature],
                                        None => unit.terrain_native[terrain.expect("plot without a terrain")],
                                    };

                                    if native {
                                        let value = 1 + self.soren_rand_num(1000, "Animal Unit Selection");

                                        if value > best_value {
                                            best_unit = Some(loop_unit);
                                            best_value = value;
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(best_unit) = best_unit {
                            let (x, y) = self.map.xy(plot);
                            let unit = self.player_init_unit(barbarian, best_unit, x, y, Some(UnitAIType::UNITAI_ANIMAL), None);
                            // (HIDDEN_NATIONALITY_PROMOTION is always set in FFH2's GlobalDefines.)
                            if let Some(promotion) = opt_from_i32::<PromotionType>(d.defines.hidden_nationality_promotion) {
                                self.unit_set_has_promotion(unit, promotion, true);
                            }
                        }
                    }
                }
            }
            // FfH: End Modify
        }
    }

    /// C++ `CvGame::foundBarbarianCity` (FfH, GAMEOPTION_BARBARIAN_WORLD): best plot by path
    /// distance to the starting plots, `AI_foundValue`, owned tiles and soren rand.
    pub fn found_barbarian_city(&mut self) {
        let d = self.d;
        let barbarian = self.barbarian_player();
        let bonus_mana = d.defines.bonus_mana;

        let mut best_value = 0;
        let mut best_plot = None;

        for p in self.map.plot_ids() {
            let mut valid = true;
            let mut value = 0;

            if self.plot_is_water(p) {
                valid = false;
            } else if self.plot_is_impassable(p) {
                valid = false;
            } else if self.plot_is_city(p, false, None) {
                valid = false;
            } else if self.map.plots[p].improvement_type.is_some() {
                valid = false;
            } else if self.map.plots[p].found_disabled {
                valid = false;
            } else if bonus_mana != -1 {
                if opt_to_i32(self.plot_get_bonus_type(p, None)) == bonus_mana {
                    valid = false;
                }
            }

            if valid {
                for i in self.civ_player_ids() {
                    if self.players[i].alive {
                        if let Some(start) = self.player_get_starting_plot(i) {
                            // (no break once invalid: C++ measures every starting plot)
                            let dist = self.map_calculate_path_distance(start, p);
                            if dist == -1 {
                                value += 100;
                            } else if dist < 5 {
                                valid = false;
                            } else {
                                value += dist * 10;
                            }
                        }
                    }
                }

                if valid {
                    let (x, y) = self.map.xy(p);
                    value += self.player_ai_found_value(barbarian, x, y, d.defines.min_barbarian_city_starting_distance, false, &[]);
                    let area = self.plot_area(p).expect("CvPlot::area() is NULL");
                    value += self.map.areas[area].num_owned_tiles + 10;
                    value += self.soren_rand_num(250, "Barb City Found");

                    if value > best_value {
                        best_value = value;
                        best_plot = Some(p);
                    }
                }
            }
        }

        if let Some(best_plot) = best_plot {
            let (x, y) = self.map.xy(best_plot);
            self.player_found(barbarian, x, y);
        }
    }

    // ---------------------------------------------------------------------------------------
    // FfH global (Armageddon) counter
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::getGlobalCounter`: the Armageddon counter as a percentage of its limit.
    /// (`getTrueGlobalCounter` = `self.global_counter`.)
    #[inline]
    pub fn get_global_counter(&self) -> i32 {
        self.global_counter * 100 / self.global_counter_limit
    }

    /// C++ `CvGame::getMaxGlobalCounter`: the highest counter reached, as a percentage.
    /// (`getTrueMaxGlobalCounter` = `self.max_global_counter`.)
    #[inline]
    pub fn get_max_global_counter(&self) -> i32 {
        self.max_global_counter * 100 / self.global_counter_limit
    }

    /// C++ `CvGame::changeGlobalCounter(iChange)`: no effect with GAMEOPTION_NO_GLOBAL_COUNTER,
    /// doubled with GAMEOPTION_DOUBLE_GLOBAL_COUNTER, clamped to `0..=limit`; tracks the maximum.
    pub fn change_global_counter(&mut self, change: i32) {
        if change != 0 && !self.is_option(GameOptionType::GAMEOPTION_NO_GLOBAL_COUNTER) {
            let mut change = change;
            if self.is_option(GameOptionType::GAMEOPTION_DOUBLE_GLOBAL_COUNTER) {
                change *= 2;
            }
            let mut new_value = self.global_counter + change;
            if new_value < 0 {
                new_value = 0;
            }
            if new_value > self.global_counter_limit {
                new_value = self.global_counter_limit;
            }
            self.global_counter = new_value;
            if self.max_global_counter < new_value {
                self.max_global_counter = new_value;
            }
            // AS2D_ARMAGEDDON_INCREASE / _DECREASE sounds are UI.
        }
    }

    // ---------------------------------------------------------------------------------------
    // Trophies (C++: Python CvDataStorageInterface, persistent across games)
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::getTrophyValue(szName)` (0 in multiplayer or when unset).
    pub fn trophy_value(&self, name: &str) -> i32 {
        if self.is_game_multi_player() {
            return 0;
        }
        range(self.trophies.get(name).copied().unwrap_or(0), 0, i32::MAX)
    }

    /// C++ `CvGame::isHasTrophy(szName)`.
    pub fn is_has_trophy(&self, name: &str) -> bool {
        self.trophy_value(name) > 0
    }

    /// C++ `CvGame::setTrophyValue(szName, iNewValue)` (ignored in multiplayer).
    pub fn set_trophy_value(&mut self, name: &str, new_value: i32) {
        if !self.is_game_multi_player() {
            let value = range(new_value, 0, i32::MAX);
            self.trophies.insert(name.to_string(), value);
        }
    }

    /// C++ `CvGame::changeTrophyValue(szName, iChange)`.
    pub fn change_trophy_value(&mut self, name: &str, change: i32) {
        let v = self.trophy_value(name) + change;
        self.set_trophy_value(name, v);
    }

    /// C++ `CvGame::isReligionDisabled(iReligion)`: GAMEOPTION_NO_RELIGION_0..6 by religion index.
    pub fn is_religion_disabled(&self, religion: ReligionType) -> bool {
        const OPTIONS: [GameOptionType; 7] = [
            GameOptionType::GAMEOPTION_NO_RELIGION_0,
            GameOptionType::GAMEOPTION_NO_RELIGION_1,
            GameOptionType::GAMEOPTION_NO_RELIGION_2,
            GameOptionType::GAMEOPTION_NO_RELIGION_3,
            GameOptionType::GAMEOPTION_NO_RELIGION_4,
            GameOptionType::GAMEOPTION_NO_RELIGION_5,
            GameOptionType::GAMEOPTION_NO_RELIGION_6,
        ];
        match OPTIONS.get(religion.idx()) {
            Some(&option) => self.is_option(option),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::Game;
    use crate::setup::GameSetup;
    use ffh_data::*;

    fn game() -> Game {
        let d = ffh_data::global();
        Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3))
    }

    #[test]
    fn shrine_cache() {
        let mut g = game();
        let (b1, b2, b3) = (BuildingType(10), BuildingType(11), BuildingType(12));
        let (r0, r1) = (ReligionType(0), ReligionType(1));
        g.change_shrine_building(b1, Some(r0), false);
        g.change_shrine_building(b2, Some(r1), false);
        g.change_shrine_building(b3, Some(r0), false);
        assert_eq!(g.get_shrine_building_count(None), 3);
        assert_eq!(g.get_shrine_building_count(Some(r0)), 2);
        assert_eq!(g.get_shrine_building(1, None), Some(b2));
        assert_eq!(g.get_shrine_building(1, Some(r0)), Some(b3));
        assert_eq!(g.get_shrine_building(0, Some(r1)), Some(b2));
        assert_eq!(g.get_shrine_building(1, Some(r1)), None);
        // C++ quirk: removing the first of three entries decrements the count per remaining
        // iteration: i=0 (match, shift, 3->2), i=1 (shift, 2->1), loop ends.
        g.change_shrine_building(b1, Some(r0), true);
        assert_eq!(g.shrine_building_count, 1);
        assert_eq!(g.get_shrine_building(0, None), Some(b2));
        // removing the last entry behaves
        g.change_shrine_building(b2, Some(r1), true);
        assert_eq!(g.shrine_building_count, 0);
    }

    #[test]
    fn plot_extra_costs_and_names() {
        let mut g = game();
        assert_eq!(g.plot_extra_cost(3, 4), 0);
        g.change_plot_extra_cost(3, 4, 10);
        g.change_plot_extra_cost(3, 4, 5);
        g.change_plot_extra_cost(1, 1, 7);
        assert_eq!(g.plot_extra_cost(3, 4), 15);
        assert_eq!(g.plot_extra_costs.len(), 2);
        g.remove_plot_extra_cost(3, 4);
        assert_eq!(g.plot_extra_cost(3, 4), 0);
        assert_eq!(g.plot_extra_cost(1, 1), 7);
        assert_eq!(g.plot_extra_yield(1, 1, YieldType::YIELD_FOOD), 0);
        g.plot_extra_yields.push(crate::structs::PlotExtraYield {
            x: 1,
            y: 1,
            extra_yield: [2, 0, 1],
        });
        assert_eq!(g.plot_extra_yield(1, 1, YieldType::YIELD_COMMERCE), 1);

        g.destroyed_cities.push("Ruins".to_string());
        assert!(g.is_destroyed_city_name("Ruins"));
        assert!(!g.is_great_person_born("Ruins"));
    }

    #[test]
    fn founding_and_forced_civics() {
        let mut g = game();
        let r = ReligionType(2);
        assert!(!g.is_religion_founded(r));
        g.setup.game_turn = 17;
        g.make_religion_founded(r, crate::ids::PlayerId(0));
        assert!(g.is_religion_founded(r));
        assert_eq!(g.religion_game_turn_founded[r], 17);
        g.setup.game_turn = 20;
        g.make_religion_founded(r, crate::ids::PlayerId(1));
        assert_eq!(g.religion_game_turn_founded[r], 17);
        assert_eq!(g.get_holy_city(r), None);
        g.holy_city[r] = Some(crate::ids::CityId(5)); // a city that does not exist
        assert_eq!(g.get_holy_city(r), None);

        assert!(!g.cultural_victory_valid());
        assert_eq!(g.cultural_victory_culture_level(), None);

        let c = CivicType(0);
        assert!(!g.is_force_civic(c));
        g.change_force_civic_count(c, 0);
        assert!(!g.is_force_civic(c));
    }

    #[test]
    fn special_building_valid() {
        let mut g = game();
        g.log.enabled = true;
        let sb = SpecialBuildingType(0);
        g.make_special_building_valid(sb, true);
        assert!(g.special_building_valid[sb]);
        assert_eq!(g.log.entries.len(), 1);
        g.make_special_building_valid(sb, true);
        assert_eq!(g.log.entries.len(), 1);
    }

    #[test]
    fn world_class_limits() {
        let mut g = game();
        let d = g.d;

        // world unit class (heroes: iMaxGlobalInstances 1)
        let uc = d.unit_classes.keys().find(|&c| d.unit_classes[c].max_global_instances > 0).unwrap();
        let max = d.unit_classes[uc].max_global_instances;
        assert!(!g.is_unit_class_maxed_out(uc, 0));
        assert!(g.is_unit_class_maxed_out(uc, max));
        g.unit_class_created_count[uc] = max;
        assert!(g.is_unit_class_maxed_out(uc, 0));
        let unlimited = d.unit_classes.keys().find(|&c| d.unit_classes[c].max_global_instances == -1).unwrap();
        g.unit_class_created_count[unlimited] = 1000;
        assert!(!g.is_unit_class_maxed_out(unlimited, 0));

        // world wonder
        let bc = d.building_classes.keys().find(|&c| d.building_classes[c].max_global_instances > 0).unwrap();
        let max = d.building_classes[bc].max_global_instances;
        assert!(!g.is_building_class_maxed_out(bc, max - 1));
        assert!(g.is_building_class_maxed_out(bc, max));
        g.building_class_created_count[bc] = max;
        assert!(g.is_building_class_maxed_out(bc, 0));
        let national = d.building_classes.keys().find(|&c| d.building_classes[c].max_global_instances == -1).unwrap();
        assert!(!g.is_building_class_maxed_out(national, 1000));

        // world project
        let project = d.t::<ProjectType>("PROJECT_BANE_DIVINE");
        assert_eq!(d.projects[project].max_global_instances, 1);
        assert!(!g.is_project_maxed_out(project, 0));
        g.project_created_count[project] += 1;
        assert!(g.is_project_maxed_out(project, 0));
    }

    #[test]
    fn forced_civic_options_and_scaled_values() {
        let mut g = game();
        let d = g.d;

        let liberty = d.t::<CivicType>("CIVIC_LIBERTY");
        let option = d.civics[liberty].civic_option_type.unwrap();
        assert!(!g.is_force_civic_option(option));
        g.force_civic_count[liberty] = 1;
        assert!(g.is_force_civic_option(option));
        let other = d.civic_options.keys().find(|&o| o != option).unwrap();
        assert!(!g.is_force_civic_option(other));

        // getImprovementUpgradeTime: scaled by the speed, then by the start era
        let cottage = d.t::<ImprovementType>("IMPROVEMENT_COTTAGE");
        let expected = d.improvements[cottage].upgrade_time * d.game_speeds[g.game_speed_type()].improvement_percent / 100
            * d.eras[g.start_era()].improvement_percent
            / 100;
        assert_eq!(d.improvements[cottage].upgrade_time, 10);
        assert_eq!(g.improvement_upgrade_time(cottage), expected);

        // getCultureThreshold: FfH always has NO_ESPIONAGE
        let level = CultureLevelType(2);
        let threshold = d.culture_levels[level].speed_threshold[g.game_speed_type()];
        assert_eq!(g.culture_threshold(level), threshold * (100 + d.defines.no_espionage_culture_level_modifier) / 100);

        // getNumFreeBonuses: the building's own count (-1 = the world size's)
        for b in d.buildings.keys() {
            let own = d.buildings[b].num_free_bonuses;
            let expected = if own == -1 { d.worlds[g.setup.world_size].num_free_building_bonuses } else { own };
            assert_eq!(g.num_free_bonuses(b), expected);
        }
    }

    #[test]
    fn update_cache_on_turn() {
        let mut g = game();
        let d = g.d;
        g.do_update_cache_on_turn();

        let shrines: Vec<BuildingType> = d.buildings.keys().filter(|&b| d.buildings[b].holy_city.is_some()).collect();
        assert!(!shrines.is_empty());
        assert_eq!(g.get_shrine_building_count(None), shrines.len() as i32);
        assert_eq!(g.get_shrine_building(0, None), Some(shrines[0]));
        let r = d.buildings[shrines[0]].religion_type.unwrap();
        assert_eq!(g.get_shrine_building(0, Some(r)), Some(shrines[0]));

        // the next turn rebuilds the same cache
        g.do_update_cache_on_turn();
        assert_eq!(g.get_shrine_building_count(None), shrines.len() as i32);

        // the culture victory with the most culture cities (every victory is valid here)
        let best = d.victories.keys().filter(|&v| d.victories[v].city_culture.is_some_and(|c| c.0 > 0)).max_by_key(|&v| d.victories[v].num_culture_cities);
        match best {
            Some(v) => {
                assert_eq!(g.num_culture_victory_cities, d.victories[v].num_culture_cities);
                assert!(g.cultural_victory_valid());
                assert_eq!(g.cultural_victory_culture_level(), d.victories[v].city_culture);
            }
            None => assert!(!g.cultural_victory_valid()),
        }
    }

    #[test]
    fn plot_extra_yields() {
        let mut g = game();
        let d = g.d;
        let init = crate::map::MapInitData { grid_w: 4, grid_h: 4, ..Default::default() };
        g.map = crate::map::Map::new(d, &init, g.num_players(), g.num_teams());

        g.set_plot_extra_yield(1, 2, YieldType::YIELD_FOOD, 2);
        g.set_plot_extra_yield(1, 2, YieldType::YIELD_FOOD, 1);
        g.set_plot_extra_yield(1, 2, YieldType::YIELD_COMMERCE, 4);
        g.set_plot_extra_yield(3, 3, YieldType::YIELD_PRODUCTION, 1);
        assert_eq!(g.plot_extra_yields.len(), 2);
        assert_eq!(g.plot_extra_yield(1, 2, YieldType::YIELD_FOOD), 3);
        assert_eq!(g.plot_extra_yield(1, 2, YieldType::YIELD_PRODUCTION), 0);
        assert_eq!(g.plot_extra_yield(1, 2, YieldType::YIELD_COMMERCE), 4);

        g.remove_plot_extra_yield(1, 2);
        assert_eq!(g.plot_extra_yield(1, 2, YieldType::YIELD_FOOD), 0);
        assert_eq!(g.plot_extra_yield(3, 3, YieldType::YIELD_PRODUCTION), 1);
        g.remove_plot_extra_yield(1, 2);
        assert_eq!(g.plot_extra_yields.len(), 1);
    }

    #[test]
    fn ever_active_and_religion_counts() {
        let mut g = game();
        let d = g.d;
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = crate::player::Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        // (`GameSetup::standard` gives a civilization and a leader to the barbarian slot only)
        let barbarian = g.barbarian_player();
        let civ = g.player_get_civilization_type(barbarian).unwrap();
        let leader = g.player_get_leader_type(barbarian).unwrap();
        let warrior = d.t::<UnitClassType>("UNITCLASS_WARRIOR");
        let unit = d.civ_unit(civ, warrior).unwrap();

        assert!(!g.is_civ_ever_active(civ));
        assert!(!g.is_leader_ever_active(leader));
        assert!(!g.is_unit_ever_active(unit));
        g.players[barbarian].ever_alive = true;
        assert!(g.is_civ_ever_active(civ));
        assert!(g.is_leader_ever_active(leader));
        assert!(g.is_unit_ever_active(unit));

        // a building no ever-active civilization builds
        let foreign = d
            .buildings
            .keys()
            .find(|&b| {
                let class = d.buildings[b].building_class_type.unwrap();
                !g.player_ids().any(|p| g.players[p].ever_alive && g.player_get_civilization_type(p).is_some_and(|c| d.civ_building(c, class) == Some(b)))
            })
            .unwrap();
        assert!(!g.is_building_ever_active(foreign));

        // countReligionLevels counts alive players only
        let (p0, p1) = (crate::ids::PlayerId(0), crate::ids::PlayerId(1));
        let r = ReligionType(1);
        g.players[p0].has_religion_count[r] = 3;
        g.players[p1].has_religion_count[r] = 2;
        assert_eq!(g.count_religion_levels(r), 0);
        g.players[p0].alive = true;
        assert_eq!(g.count_religion_levels(r), 3);

        // calculateReligionPercent: 0 without world population
        assert_eq!(g.calculate_religion_percent(r), 0);
    }
}

#[cfg(test)]
mod ffh_tests {
    use crate::game::Game;
    use crate::setup::GameSetup;
    use ffh_data::*;

    fn game_with(options: &[GameOptionType]) -> Game {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3);
        for &o in options {
            setup.options[o] = true;
        }
        Game::new(d, setup)
    }

    #[test]
    fn global_counter() {
        let mut g = game_with(&[]);
        let limit = g.global_counter_limit;
        assert!(limit > 0);
        g.change_global_counter(limit / 4);
        assert_eq!(g.global_counter, limit / 4);
        assert_eq!(g.get_global_counter(), (limit / 4) * 100 / limit);
        g.change_global_counter(-2 * limit);
        assert_eq!(g.global_counter, 0);
        assert_eq!(g.max_global_counter, limit / 4);
        g.change_global_counter(3 * limit);
        assert_eq!(g.global_counter, limit);
        assert_eq!(g.get_max_global_counter(), 100);

        let mut doubled = game_with(&[GameOptionType::GAMEOPTION_DOUBLE_GLOBAL_COUNTER]);
        doubled.change_global_counter(3);
        assert_eq!(doubled.global_counter, 6);

        let mut off = game_with(&[GameOptionType::GAMEOPTION_NO_GLOBAL_COUNTER]);
        off.change_global_counter(3);
        assert_eq!(off.global_counter, 0);
    }

    #[test]
    fn trophies_and_religions() {
        let mut g = game_with(&[GameOptionType::GAMEOPTION_NO_RELIGION_2]);
        assert_eq!(g.trophy_value("TROPHY_X"), 0);
        assert!(!g.is_has_trophy("TROPHY_X"));
        g.change_trophy_value("TROPHY_X", 2);
        g.change_trophy_value("TROPHY_X", 1);
        assert_eq!(g.trophy_value("TROPHY_X"), 3);
        g.set_trophy_value("TROPHY_X", -5);
        assert_eq!(g.trophy_value("TROPHY_X"), 0);

        assert!(g.is_religion_disabled(ReligionType(2)));
        assert!(!g.is_religion_disabled(ReligionType(1)));
        assert!(!g.is_religion_disabled(ReligionType(9)));
    }

    /// Only the barbarian player is alive (as after `init`): no civ cities.
    fn barbarian_game(options: &[GameOptionType]) -> Game {
        let mut g = game_with(options);
        let barbarian = g.barbarian_player();
        g.players[barbarian].alive = true;
        g
    }

    #[test]
    fn barbarians_early_outs_draw_nothing() {
        // GAMEOPTION_NO_BARBARIANS: nothing, not even a random draw
        let mut g = barbarian_game(&[GameOptionType::GAMEOPTION_NO_BARBARIANS]);
        let rng = g.soren_rand.clone();
        g.create_barbarian_cities();
        g.create_barbarian_units();
        assert_eq!(g.soren_rand, rng);

        // a city elimination limit disables barbarian cities
        let mut g = barbarian_game(&[]);
        g.setup.max_city_elimination = 3;
        let rng = g.soren_rand.clone();
        g.create_barbarian_cities();
        assert_eq!(g.soren_rand, rng);

        // early turns: animals only, and none before the 5th turn
        let mut g = barbarian_game(&[]);
        let rng = g.soren_rand.clone();
        g.create_barbarian_units();
        g.create_animals();
        assert_eq!(g.soren_rand, rng);
        assert!(g.units.is_empty());
    }

    #[test]
    fn holy_city_draws() {
        // before the 5th turn (no advanced start): nothing
        let mut g = barbarian_game(&[]);
        let rng = g.soren_rand.clone();
        g.do_holy_city();
        g.do_headquarters();
        assert_eq!(g.soren_rand, rng);

        // later: the religion offset is drawn; a team needs the tech and a city to draw more
        g.elapsed_game_turns = 5;
        let mut expected = g.soren_rand.clone();
        expected.get_i(g.d.religions.len() as i32, "Holy City religion offset");
        g.do_holy_city();
        assert_eq!(g.soren_rand, expected);
    }
}
