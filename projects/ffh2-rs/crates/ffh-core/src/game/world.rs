//! CvGame: world content. Religions (founding, holy cities, `doHolyCity`), corporations
//! (`doHeadquarters`), world unit/building/project limits, forced civics, special
//! units/buildings, the shrine and culture-victory caches (`doUpdateCacheOnTurn`), plot extra
//! yields/costs, "ever active" queries, destroyed city / great person names; barbarians
//! (`createBarbarianCities`, `createBarbarianUnits`, `createAnimals`, FfH `foundBarbarianCity`)
//! and the other FfH additions: the global (Armageddon) counter, trophies, disabled religions.
//! (FfH's crime, challenge and scenario counters are plain fields.)

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use crate::utils::range;
use ffh_data::*;

impl Game {
    // ---------------------------------------------------------------------------------------
    // Religions and corporations
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::doHolyCity`: founds religions whose slot is free (random religion offset,
    /// team then player chosen by fewest religions + soren rand; AI players +10).
    pub fn do_holy_city(&mut self) {
        todo!("CvGame::doHolyCity")
    }

    /// C++ `CvGame::doHeadquarters`: founds corporations (no corporations in FFH2 data).
    pub fn do_headquarters(&mut self) {
        todo!("CvGame::doHeadquarters")
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
        todo!("CvGame::setHolyCity")
    }

    /// C++ `CvGame::getHeadquarters(eIndex)` (`None` once the city is gone).
    #[inline]
    pub fn get_headquarters(&self, corporation: CorporationType) -> Option<CityId> {
        self.headquarters[corporation].filter(|&c| self.cities.contains(c))
    }

    /// C++ `CvGame::setHeadquarters(eIndex, pNewValue, bAnnounce)`.
    pub fn set_headquarters(&mut self, corporation: CorporationType, new_value: Option<CityId>, announce: bool) {
        todo!("CvGame::setHeadquarters")
    }

    /// C++ `CvGame::isCompetingCorporation(eCorporation1, eCorporation2)`.
    pub fn is_competing_corporation(&self, corporation1: CorporationType, corporation2: CorporationType) -> bool {
        todo!("CvGame::isCompetingCorporation")
    }

    /// C++ `CvGame::countReligionLevels(eReligion)`.
    pub fn count_religion_levels(&self, religion: ReligionType) -> i32 {
        todo!("CvGame::countReligionLevels")
    }

    /// C++ `CvGame::calculateReligionPercent(eReligion)`.
    pub fn calculate_religion_percent(&self, religion: ReligionType) -> i32 {
        todo!("CvGame::calculateReligionPercent")
    }

    /// C++ `CvGame::countCorporationLevels(eCorporation)`.
    pub fn count_corporation_levels(&self, corporation: CorporationType) -> i32 {
        todo!("CvGame::countCorporationLevels")
    }

    /// C++ `CvGame::replaceCorporation(eCorporation1, eCorporation2)`.
    pub fn replace_corporation(&mut self, corporation1: CorporationType, corporation2: CorporationType) {
        todo!("CvGame::replaceCorporation")
    }

    // ---------------------------------------------------------------------------------------
    // Limits, forced civics, special units/buildings
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::isUnitClassMaxedOut(eIndex, iExtra)`.
    pub fn is_unit_class_maxed_out(&self, unit_class: UnitClassType, extra: i32) -> bool {
        todo!("CvGame::isUnitClassMaxedOut")
    }

    /// C++ `CvGame::isBuildingClassMaxedOut(eIndex, iExtra)`.
    pub fn is_building_class_maxed_out(&self, building_class: BuildingClassType, extra: i32) -> bool {
        todo!("CvGame::isBuildingClassMaxedOut")
    }

    /// C++ `CvGame::isProjectMaxedOut(eIndex, iExtra)`.
    pub fn is_project_maxed_out(&self, project: ProjectType, extra: i32) -> bool {
        todo!("CvGame::isProjectMaxedOut")
    }

    /// C++ `CvGame::isForceCivic`.
    #[inline]
    pub fn is_force_civic(&self, civic: CivicType) -> bool {
        self.force_civic_count[civic] > 0
    }

    /// C++ `CvGame::isForceCivicOption(eCivicOption)`.
    pub fn is_force_civic_option(&self, civic_option: CivicOptionType) -> bool {
        todo!("CvGame::isForceCivicOption")
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

    /// C++ `CvGame::canTrainNukes`.
    pub fn can_train_nukes(&self) -> bool {
        todo!("CvGame::canTrainNukes")
    }

    /// C++ `CvGame::getNumFreeBonuses(eBuilding)` (-1 in XML = world size default).
    pub fn num_free_bonuses(&self, building: BuildingType) -> i32 {
        todo!("CvGame::getNumFreeBonuses")
    }

    /// C++ `CvGame::getImprovementUpgradeTime(eImprovement)` (scaled by speed and start era).
    pub fn improvement_upgrade_time(&self, improvement: ImprovementType) -> i32 {
        todo!("CvGame::getImprovementUpgradeTime")
    }

    /// C++ `CvGame::processBuilding(eBuilding, iChange)`: diplo vote sources of the building.
    pub fn process_building(&mut self, building: BuildingType, change: i32) {
        todo!("CvGame::processBuilding")
    }

    // ---------------------------------------------------------------------------------------
    // Caches: shrines and culture victory
    // ---------------------------------------------------------------------------------------

    /// C++ `CvGame::doUpdateCacheOnTurn`: rebuilds the shrine cache (buildings with a holy
    /// city religion) and the strictest valid culture victory.
    pub fn do_update_cache_on_turn(&mut self) {
        todo!("CvGame::doUpdateCacheOnTurn")
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
        debug_assert!(remove || (self.shrine_building_count as usize) < self.d.buildings.len(), "trying to add too many buildings to CvGame::changeShrineBuilding");

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
        todo!("CvGame::getCultureThreshold")
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
        todo!("CvGame::setPlotExtraYield")
    }

    /// C++ `CvGame::removePlotExtraYield(iX, iY)` (then `CvPlot::updateYield`).
    pub fn remove_plot_extra_yield(&mut self, x: i32, y: i32) {
        todo!("CvGame::removePlotExtraYield")
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
        todo!("CvGame::isCivEverActive")
    }

    /// C++ `CvGame::isLeaderEverActive(eLeader)`.
    pub fn is_leader_ever_active(&self, leader: LeaderHeadType) -> bool {
        todo!("CvGame::isLeaderEverActive")
    }

    /// C++ `CvGame::isUnitEverActive(eUnit)`.
    pub fn is_unit_ever_active(&self, unit: UnitType) -> bool {
        todo!("CvGame::isUnitEverActive")
    }

    /// C++ `CvGame::isBuildingEverActive(eBuilding)`.
    pub fn is_building_ever_active(&self, building: BuildingType) -> bool {
        todo!("CvGame::isBuildingEverActive")
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
        todo!("CvGame::createBarbarianCities")
    }

    /// C++ `CvGame::createBarbarianUnits`: animals early (or with few cities), else barbarian
    /// units per area (FfH: animals do not count, `/6 + 1` spawn rate, no world-unique classes).
    pub fn create_barbarian_units(&mut self) {
        todo!("CvGame::createBarbarianUnits")
    }

    /// C++ `CvGame::createAnimals` (FfH: also in water areas; GAMEOPTION_DOUBLE_ANIMALS; animals
    /// get `HIDDEN_NATIONALITY_PROMOTION`).
    pub fn create_animals(&mut self) {
        todo!("CvGame::createAnimals")
    }

    /// C++ `CvGame::foundBarbarianCity` (FfH, GAMEOPTION_BARBARIAN_WORLD): best plot by path
    /// distance to the starting plots, `AI_foundValue`, owned tiles and soren rand.
    pub fn found_barbarian_city(&mut self) {
        todo!("CvGame::foundBarbarianCity")
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
        g.plot_extra_yields.push(crate::structs::PlotExtraYield { x: 1, y: 1, extra_yield: [2, 0, 1] });
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
}
