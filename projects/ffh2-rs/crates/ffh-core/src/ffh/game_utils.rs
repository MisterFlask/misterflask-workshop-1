//! CvGameUtils.py rules callbacks. FFH2 0.41 enables (PythonCallbackDefines.xml):
//! USE_CANNOT_DO_CIVIC, USE_CANNOT_CONSTRUCT, USE_CANNOT_RESEARCH,
//! USE_GET_BUILDING_COST_MOD, USE_CAN_BUILD, USE_CANNOT_TRAIN, USE_ON_UNIT_CREATED.
//! The DLL also calls some CvGameUtils functions unconditionally; those whose FFH2
//! Python contains rules logic are ported here too: doPillageGold, doCityCaptureGold,
//! getExperienceNeeded, cannotCreate, calculateScore, isVictoryTest. All remaining
//! hooks are disabled or return "use default" in FFH2, so the C++ default behaviour
//! applies and they are not called.
//!
//! AI-only branches (`if not pPlayer.isHuman(): ...` build restrictions in cannotTrain,
//! cannotConstruct, cannotCreate, cannotResearch, canBuild) are AI policy, not rules:
//! they are omitted here and left to the new AI (docs/PORTING.md, "Changes from FFH2").
//!
//! NOTE: Python 2 `/` on ints is floor division; use `py_div` for faithful results.

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    /// `cannotDoCivic(ePlayer, eCivic)`: FFH2 always returns False.
    pub fn py_cannot_do_civic(&self, _p: PlayerId, _civic: CivicType) -> bool {
        false
    }

    /// `cannotConstruct(pCity, eBuilding, bContinue, bTestVisible, bIgnoreCost)`.
    /// The AI-only branch (`BUILDING_PROPHECY_OF_RAGNAROK` for non-evil AIs) is omitted (AI policy).
    pub fn py_cannot_construct(&self, c: CityId, building: BuildingType, _continue: bool, _test_visible: bool, _ignore_cost: bool) -> bool {
        let d = self.d;
        let owner = self.cities[c].owner;
        let player = &self.players[owner];
        let t = |name: &str| d.ty::<BuildingType>(name) == Some(building);
        if d.ty::<TraitType>("TRAIT_AGNOSTIC").map_or(false, |tr| player.traits[tr]) {
            for name in [
                "BUILDING_TEMPLE_OF_LEAVES",
                "BUILDING_TEMPLE_OF_KILMORPH",
                "BUILDING_TEMPLE_OF_THE_EMPYREAN",
                "BUILDING_TEMPLE_OF_THE_OVERLORDS",
                "BUILDING_TEMPLE_OF_THE_VEIL",
                "BUILDING_TEMPLE_OF_THE_ORDER",
            ] {
                if t(name) {
                    return true;
                }
            }
        }
        let crusade = d.ty::<CivicType>("CIVIC_CRUSADE");
        let cultural_values = d.ty::<CivicOptionType>("CIVICOPTION_CULTURAL_VALUES");
        if let (Some(cv), Some(cr)) = (cultural_values, crusade) {
            if player.civics[cv] == Some(cr) {
                for name in [
                    "BUILDING_ELDER_COUNCIL",
                    "BUILDING_MARKET",
                    "BUILDING_MONUMENT",
                    "BUILDING_MONEYCHANGER",
                    "BUILDING_THEATRE",
                    "BUILDING_AQUEDUCT",
                    "BUILDING_PUBLIC_BATHS",
                    "BUILDING_HERBALIST",
                    "BUILDING_CARNIVAL",
                    "BUILDING_COURTHOUSE",
                    "BUILDING_GAMBLING_HOUSE",
                    "BUILDING_GRANARY",
                    "BUILDING_SMOKEHOUSE",
                    "BUILDING_LIBRARY",
                    "BUILDING_HARBOR",
                    "BUILDING_ALCHEMY_LAB",
                    "BUILDING_BREWERY",
                ] {
                    if t(name) {
                        return true;
                    }
                }
            }
        }
        if t("BUILDING_SHRINE_OF_THE_CHAMPION") {
            let hero = self.cf_get_hero(owner);
            let hero = match hero {
                None => return true,
                Some(h) => h,
            };
            if !self.is_unit_class_maxed_out(hero, 0) {
                return true;
            }
            if player.unit_class_count[hero] > 0 {
                return true;
            }
        }
        if t("BUILDING_MERCURIAN_GATE") {
            if let Some(o) = d.ty::<GameOptionType>("GAMEOPTION_NO_HYBOREM_OR_BASIUM") {
                if self.is_option(o) {
                    return true;
                }
            }
            if self.player_get_state_religion(owner) == d.ty::<ReligionType>("RELIGION_THE_ASHEN_VEIL") && d.ty::<ReligionType>("RELIGION_THE_ASHEN_VEIL").is_some() {
                return true;
            }
            if self.city_is_capital(c) {
                return true;
            }
            // AI-only branch (evil AIs may not build it) omitted: AI policy.
        }
        let altars: Vec<Option<BuildingType>> = [
            "BUILDING_ALTAR_OF_THE_LUONNOTAR",
            "BUILDING_ALTAR_OF_THE_LUONNOTAR_ANOINTED",
            "BUILDING_ALTAR_OF_THE_LUONNOTAR_BLESSED",
            "BUILDING_ALTAR_OF_THE_LUONNOTAR_CONSECRATED",
            "BUILDING_ALTAR_OF_THE_LUONNOTAR_DIVINE",
            "BUILDING_ALTAR_OF_THE_LUONNOTAR_EXALTED",
            "BUILDING_ALTAR_OF_THE_LUONNOTAR_FINAL",
        ]
        .iter()
        .map(|n| d.ty::<BuildingType>(n))
        .collect();
        if let Some(i) = altars.iter().position(|a| *a == Some(building)) {
            if player.alignment.is_some() && player.alignment == d.ty::<AlignmentType>("ALIGNMENT_EVIL") {
                return true;
            }
            // Each altar stage is blocked once a later stage exists.
            for later in altars.iter().skip(i + 1).flatten() {
                if self.player_count_num_buildings(owner, *later) > 0 {
                    return true;
                }
            }
        }
        if t("BUILDING_SMUGGLERS_PORT") && !self.player_is_smuggling_ring(owner) {
            return true;
        }
        false
    }

    /// `cannotResearch(ePlayer, eTech, bTrade)`. AI-only branches (religion techs disliked by
    /// the leader, Stirrups for some civs) are omitted (AI policy). Scenario hooks are out of scope.
    pub fn py_cannot_research(&self, p: PlayerId, tech: TechType, _trade: bool) -> bool {
        let d = self.d;
        let player = &self.players[p];
        let is = |name: &str| d.ty::<TechType>(name) == Some(tech);
        let agnostic = d.ty::<TraitType>("TRAIT_AGNOSTIC").map_or(false, |tr| player.traits[tr]);
        let opt = |name: &str| d.ty::<GameOptionType>(name).map_or(false, |o| self.is_option(o));
        let religion_techs: [(&str, Option<&str>); 7] = [
            ("TECH_ORDERS_FROM_HEAVEN", Some("GAMEOPTION_NO_RELIGION_1")),
            ("TECH_WAY_OF_THE_EARTHMOTHER", Some("GAMEOPTION_NO_RELIGION_3")),
            ("TECH_WAY_OF_THE_FORESTS", Some("GAMEOPTION_NO_RELIGION_0")),
            ("TECH_MESSAGE_FROM_THE_DEEP", Some("GAMEOPTION_NO_RELIGION_2")),
            ("TECH_CORRUPTION_OF_SPIRIT", Some("GAMEOPTION_NO_RELIGION_4")),
            ("TECH_HONOR", None),
            ("TECH_DECEPTION", None),
        ];
        for (tname, option) in religion_techs {
            if is(tname) {
                if let Some(o) = option {
                    if opt(o) {
                        return true;
                    }
                }
                if agnostic {
                    return true;
                }
            }
        }
        if is("TECH_SEAFARING") && self.player_get_civilization_type(p) != d.ty::<CivilizationType>("CIVILIZATION_LANUN") {
            return true;
        }
        false
    }

    /// `getBuildingCostMod(ePlayer, iCityID, eBuilding)`: -1 = no override.
    pub fn py_get_building_cost_mod(&self, p: PlayerId, _c: CityId, building: BuildingType) -> i32 {
        if self.d.ty::<BuildingType>("BUILDING_GAMBLING_HOUSE") == Some(building) && self.player_is_gambling_ring(p) {
            return py_div(self.d.buildings[building].production_cost, 4);
        }
        -1
    }

    /// `canBuild(pPlot, iBuild, iTestVisible, iPlayer)`: FFH2's body only restricts AI players
    /// (AI policy, omitted), so it always answers -1 ("use default").
    pub fn py_can_build(&self, _plot: PlotIdx, _build: BuildType, _test_visible: bool, _p: PlayerId) -> i32 {
        -1
    }

    /// `cannotTrain(pCity, eUnit, bContinue, bTestVisible, bIgnoreCost, bIgnoreUpgrades)`.
    /// The large AI-only block (scout/settler/worker/adept limits, Airship, Lightbringer, ...)
    /// is omitted (AI policy).
    pub fn py_cannot_train(&self, c: CityId, unit: UnitType, _continue: bool, _test_visible: bool, _ignore_cost: bool, _ignore_upgrades: bool) -> bool {
        let d = self.d;
        let owner = self.cities[c].owner;
        let is = |name: &str| d.ty::<UnitType>(name) == Some(unit);
        if is("UNIT_BEAST_OF_AGARES") && self.cities[c].population <= 5 {
            return true;
        }
        if let (Some(cv), Some(cr)) = (d.ty::<CivicOptionType>("CIVICOPTION_CULTURAL_VALUES"), d.ty::<CivicType>("CIVIC_CRUSADE")) {
            if self.players[owner].civics[cv] == Some(cr) && (is("UNIT_WORKER") || is("UNIT_SETTLER") || is("UNIT_WORKBOAT")) {
                return true;
            }
        }
        if is("UNIT_ACHERON") && d.ty::<GameOptionType>("GAMEOPTION_NO_ACHERON").map_or(false, |o| self.is_option(o)) {
            return true;
        }
        if is("UNIT_DUIN") && d.ty::<GameOptionType>("GAMEOPTION_NO_DUIN").map_or(false, |o| self.is_option(o)) {
            return true;
        }
        false
    }

    /// `doPillageGold(pPlot, pUnit)` (always called by `CvUnit::pillage`).
    pub fn py_do_pillage_gold(&mut self, plot: PlotIdx, u: UnitId) -> i32 {
        let imp = match self.map.plots[plot].improvement_type {
            Some(i) => i,
            None => return 0,
        };
        let pg = self.d.improvements[imp].pillage_gold;
        let mut gold = self.soren_rand_num(pg, "Pillage Gold 1");
        gold += self.soren_rand_num(pg, "Pillage Gold 2");
        gold += py_div(self.units[u].pillage_change * gold, 100);
        gold
    }

    /// `doCityCaptureGold(pOldCity)` (always called by `CvPlayer::acquireCity`).
    pub fn py_do_city_capture_gold(&mut self, old_city: CityId) -> i32 {
        let df = &self.d.defines;
        let (base, per_pop, rand1, rand2, max_turns) =
            (df.base_capture_gold, df.capture_gold_per_population, df.capture_gold_rand1, df.capture_gold_rand2, df.capture_gold_max_turns);
        let mut gold = base;
        gold += self.cities[old_city].population * per_pop;
        gold += self.soren_rand_num(rand1, "Capture Gold 1");
        gold += self.soren_rand_num(rand2, "Capture Gold 2");
        if max_turns > 0 {
            let turns = (self.game_turn() - self.cities[old_city].game_turn_acquired).clamp(0, max_turns);
            gold *= turns;
            gold = py_div(gold, max_turns);
        }
        gold
    }

    /// `getExperienceNeeded(iLevel, iOwner)` (always called by `CvUnit::experienceNeeded`).
    pub fn py_get_experience_needed(&self, level: i32, owner: PlayerId) -> i32 {
        let mut needed = level * level + 1;
        let modifier = self.players[owner].level_experience_modifier;
        if modifier != 0 {
            needed += py_div(needed * modifier + 99, 100);
        }
        needed
    }

    /// `cannotCreate(pCity, eProject, bContinue, bTestVisible)` (always called by `CvCity::canCreate`).
    /// AI-only branches (Purge the Unfaithful, Samhain, early-game production limits) are omitted.
    pub fn py_cannot_create(&self, c: CityId, project: ProjectType, _continue: bool, _test_visible: bool) -> bool {
        let d = self.d;
        let owner = self.cities[c].owner;
        let is = |name: &str| d.ty::<ProjectType>(name) == Some(project);
        if is("PROJECT_PURGE_THE_UNFAITHFUL") && self.player_get_state_religion(owner).is_none() {
            return true;
        }
        if is("PROJECT_BIRTHRIGHT_REGAINED") && !self.players[owner].feat_accomplished[FeatType::FEAT_GLOBAL_SPELL] {
            return true;
        }
        if is("PROJECT_STIR_FROM_SLUMBER") && self.players[owner].players_killed == 0 {
            return true;
        }
        if is("PROJECT_GENESIS") && self.player_get_civilization_type(owner) == d.ty::<CivilizationType>("CIVILIZATION_ILLIANS") {
            return true;
        }
        false
    }

    /// `calculateScore(ePlayer, False, False)`: FFH2's score, now only the legacy score
    /// (`player_legacy_score`); the game score is score v2 (`Game::world_scores`). The final and
    /// victory variants (`bFinal`/`bVictory`) only fed the Hall of Fame and replays, which are
    /// not ported, and are dropped.
    pub fn py_calculate_score(&self, p: PlayerId) -> i32 {
        let df = &self.d.defines;
        let pop = self.score_component(self.player_get_pop_score(p, true), self.max_population, df.score_population_factor);
        let land = self.score_component(self.player_get_land_score(p, true), self.max_land, df.score_land_factor);
        let tech = self.score_component(self.players[p].tech_score, self.max_tech, df.score_tech_factor);
        let wonders = self.score_component(self.players[p].wonders_score, self.max_wonders, df.score_wonder_factor);
        pop + land + wonders + tech
    }

    /// `CvUtil.getScoreComponent(iRawScore, iInitial, iMax, iFactor, bExponential, False, False)`
    /// (`iInitial` and `bExponential` only matter for a final victory score).
    fn score_component(&self, raw: i32, max: i32, factor: i32) -> i32 {
        if self.estimate_end_turn == 0 {
            return 0;
        }
        let df = &self.d.defines;
        let free = py_div(df.score_free_percent * max, 100);
        if free + max != 0 {
            py_div(factor * (raw + free), free + max)
        } else {
            factor
        }
    }

    /// `isVictoryTest()` (always called by `CvGame::testVictory`).
    pub fn py_is_victory_test(&self) -> bool {
        self.elapsed_game_turns > 10
    }
}

/// Python 2 integer division (floor), for faithful ports of Python arithmetic.
#[inline]
pub fn py_div(a: i32, b: i32) -> i32 {
    let q = a / b;
    if (a % b != 0) && ((a < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}

#[cfg(test)]
mod tests {
    use super::py_div;

    #[test]
    fn floor_division() {
        assert_eq!(py_div(7, 2), 3);
        assert_eq!(py_div(-7, 2), -4);
        assert_eq!(py_div(7, -2), -4);
        assert_eq!(py_div(-150, 100), -2);
        assert_eq!(py_div(-200, 100), -2);
    }
}
