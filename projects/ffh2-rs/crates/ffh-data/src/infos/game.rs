//! Info classes: CommerceInfo, GameSpeedInfo, WorldInfo, ClimateInfo, SeaLevelInfo, EraInfo,
//! HandicapInfo, GameOptionInfo, GoodyInfo, HurryInfo, UpkeepInfo, CultureLevelInfo,
//! VictoryInfo, ProcessInfo, EmphasizeInfo.
//! Port of the corresponding `Cv*Info::read(CvXMLLoadUtility*)` functions in CvInfos.cpp.
//! None of these classes has a `readPass2`/`readPass3`.
//!
//! Not ported (no gameplay or AI effect):
//! - members read() never sets: `CvCommerceInfo::m_iChar` (font glyph, set at runtime),
//!   `CvGameSpeedInfo::m_szGameSpeedName`, `CvHandicapInfo::m_szHandicapName`;
//! - audio/visual members: `CvGoodyInfo::m_szSound`, `CvVictoryInfo::m_szMovie`, and the
//!   `CvEraInfo` soundtrack/soundscape members (`iSoundtrackSpace`, `bFirstSoundtrackFirst`,
//!   `EraInfoSoundtracks`, `CitySoundscapes`, `AudioUnitVictoryScript`, `AudioUnitDefeatScript`).
//!
//! The XML also carries `iCulturePercent` in GameSpeed and Era infos; the FFH2 0.41 DLL never
//! reads it, so it is ignored here as well.

use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

// ---------------------------------------------------------------------------------------------
// CommerceInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvCommerceInfo` (Gameinfo/CIV4CommerceInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct CommerceInfo {
    pub base: InfoBase,
    pub initial_percent: i32,
    pub initial_happiness: i32,
    /// C++ `getAIWeightPercent()`
    pub ai_weight_percent: i32,
    /// C++ `isFlexiblePercent()`
    pub flexible_percent: bool,
}

/// C++ `CvCommerceInfo::read`.
pub fn read_commerce(rd: &Rd) -> CommerceInfo {
    CommerceInfo {
        base: InfoBase::read(rd),
        initial_percent: rd.int("iInitialPercent"),
        initial_happiness: rd.int("iInitialHappiness"),
        ai_weight_percent: rd.int("iAIWeightPercent"),
        flexible_percent: rd.bool("bFlexiblePercent"),
    }
}

// ---------------------------------------------------------------------------------------------
// GameSpeedInfo
// ---------------------------------------------------------------------------------------------

/// C++ `GameTurnInfo` (CvStructs.h): one calendar increment of a game speed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameTurnInfo {
    /// C++ `iMonthIncrement`
    pub month_increment: i32,
    /// C++ `iNumGameTurnsPerIncrement` (XML `iTurnsPerIncrement`)
    pub num_game_turns_per_increment: i32,
}

/// C++ `CvGameSpeedInfo` (Gameinfo/CIV4GameSpeedInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct GameSpeedInfo {
    pub base: InfoBase,
    pub growth_percent: i32,
    pub train_percent: i32,
    pub construct_percent: i32,
    pub create_percent: i32,
    pub research_percent: i32,
    pub build_percent: i32,
    pub improvement_percent: i32,
    pub great_people_percent: i32,
    pub anarchy_percent: i32,
    pub barb_percent: i32,
    pub feature_production_percent: i32,
    pub unit_discover_percent: i32,
    pub unit_hurry_percent: i32,
    pub unit_trade_percent: i32,
    pub unit_great_work_percent: i32,
    pub golden_age_percent: i32,
    pub hurry_percent: i32,
    pub hurry_conscript_anger_percent: i32,
    pub inflation_offset: i32,
    pub inflation_percent: i32,
    pub victory_delay_percent: i32,
    /// C++ `getGameTurnInfo(i)` for `i < getNumTurnIncrements()`, in XML order
    /// (`m_pGameTurnInfo` / `m_iNumTurnIncrements`).
    pub game_turn_info: Vec<GameTurnInfo>,
}

impl GameSpeedInfo {
    /// C++ `getNumTurnIncrements()`.
    pub fn num_turn_increments(&self) -> i32 {
        self.game_turn_info.len() as i32
    }
}

/// C++ `CvGameSpeedInfo::read`.
pub fn read_game_speed(rd: &Rd) -> GameSpeedInfo {
    GameSpeedInfo {
        base: InfoBase::read(rd),
        growth_percent: rd.int("iGrowthPercent"),
        train_percent: rd.int("iTrainPercent"),
        construct_percent: rd.int("iConstructPercent"),
        create_percent: rd.int("iCreatePercent"),
        research_percent: rd.int("iResearchPercent"),
        build_percent: rd.int("iBuildPercent"),
        improvement_percent: rd.int("iImprovementPercent"),
        great_people_percent: rd.int("iGreatPeoplePercent"),
        anarchy_percent: rd.int("iAnarchyPercent"),
        barb_percent: rd.int("iBarbPercent"),
        feature_production_percent: rd.int("iFeatureProductionPercent"),
        unit_discover_percent: rd.int("iUnitDiscoverPercent"),
        unit_hurry_percent: rd.int("iUnitHurryPercent"),
        unit_trade_percent: rd.int("iUnitTradePercent"),
        unit_great_work_percent: rd.int("iUnitGreatWorkPercent"),
        golden_age_percent: rd.int("iGoldenAgePercent"),
        hurry_percent: rd.int("iHurryPercent"),
        hurry_conscript_anger_percent: rd.int("iHurryConscriptAngerPercent"),
        inflation_offset: rd.int("iInflationOffset"),
        inflation_percent: rd.int("iInflationPercent"),
        victory_delay_percent: rd.int("iVictoryDelayPercent"),
        game_turn_info: read_game_turn_infos(rd),
    }
}

/// The `GameTurnInfos` block of `CvGameSpeedInfo::read`: one increment per child element, whose
/// `iMonthIncrement` / `iTurnsPerIncrement` are looked up by name (default 0). No block -> none.
fn read_game_turn_infos(rd: &Rd) -> Vec<GameTurnInfo> {
    match rd.child("GameTurnInfos") {
        Some(list) => list
            .children()
            .map(|t| GameTurnInfo {
                month_increment: t.int("iMonthIncrement"),
                num_game_turns_per_increment: t.int("iTurnsPerIncrement"),
            })
            .collect(),
        None => Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------------
// WorldInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvWorldInfo` (Gameinfo/CIV4WorldInfo.xml), indexed by `WorldSizeType`.
#[derive(Debug, Clone, Default)]
pub struct WorldInfo {
    pub base: InfoBase,
    pub default_players: i32,
    pub unit_name_modifier: i32,
    pub target_num_cities: i32,
    pub num_free_building_bonuses: i32,
    pub building_class_prereq_modifier: i32,
    pub max_conscript_modifier: i32,
    pub war_weariness_modifier: i32,
    pub grid_width: i32,
    pub grid_height: i32,
    pub terrain_grain_change: i32,
    pub feature_grain_change: i32,
    pub research_percent: i32,
    pub trade_profit_percent: i32,
    pub distance_maintenance_percent: i32,
    pub num_cities_maintenance_percent: i32,
    pub colony_maintenance_percent: i32,
    pub corporation_maintenance_percent: i32,
    pub num_cities_anarchy_percent: i32,
    pub advanced_start_points_mod: i32,
    /// FfH: C++ `getMaxCitiesMod()`
    pub max_cities_mod: i32,
    /// FfH: C++ `getUniqueFeatureChance()`
    pub unique_feature_chance: i32,
}

/// C++ `CvWorldInfo::read`.
pub fn read_world(rd: &Rd) -> WorldInfo {
    WorldInfo {
        base: InfoBase::read(rd),
        default_players: rd.int("iDefaultPlayers"),
        unit_name_modifier: rd.int("iUnitNameModifier"),
        target_num_cities: rd.int("iTargetNumCities"),
        num_free_building_bonuses: rd.int("iNumFreeBuildingBonuses"),
        building_class_prereq_modifier: rd.int("iBuildingClassPrereqModifier"),
        max_conscript_modifier: rd.int("iMaxConscriptModifier"),
        war_weariness_modifier: rd.int("iWarWearinessModifier"),
        grid_width: rd.int("iGridWidth"),
        grid_height: rd.int("iGridHeight"),
        terrain_grain_change: rd.int("iTerrainGrainChange"),
        feature_grain_change: rd.int("iFeatureGrainChange"),
        research_percent: rd.int("iResearchPercent"),
        trade_profit_percent: rd.int("iTradeProfitPercent"),
        distance_maintenance_percent: rd.int("iDistanceMaintenancePercent"),
        num_cities_maintenance_percent: rd.int("iNumCitiesMaintenancePercent"),
        colony_maintenance_percent: rd.int("iColonyMaintenancePercent"),
        corporation_maintenance_percent: rd.int("iCorporationMaintenancePercent"),
        num_cities_anarchy_percent: rd.int("iNumCitiesAnarchyPercent"),
        advanced_start_points_mod: rd.int("iAdvancedStartPointsMod"),
        max_cities_mod: rd.int("iMaxCitiesMod"),
        unique_feature_chance: rd.int("iUniqueFeatureChance"),
    }
}

// ---------------------------------------------------------------------------------------------
// ClimateInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvClimateInfo` (Gameinfo/CIV4ClimateInfo.xml). The latitude changes are C++ `float`s
/// (used by map generation), so they stay `f32`.
#[derive(Debug, Clone, Default)]
pub struct ClimateInfo {
    pub base: InfoBase,
    pub desert_percent_change: i32,
    pub jungle_latitude: i32,
    pub hill_range: i32,
    pub peak_percent: i32,
    pub snow_latitude_change: f32,
    pub tundra_latitude_change: f32,
    pub grass_latitude_change: f32,
    pub desert_bottom_latitude_change: f32,
    pub desert_top_latitude_change: f32,
    pub ice_latitude: f32,
    pub rand_ice_latitude: f32,
}

/// C++ `CvClimateInfo::read`.
pub fn read_climate(rd: &Rd) -> ClimateInfo {
    ClimateInfo {
        base: InfoBase::read(rd),
        desert_percent_change: rd.int("iDesertPercentChange"),
        jungle_latitude: rd.int("iJungleLatitude"),
        hill_range: rd.int("iHillRange"),
        peak_percent: rd.int("iPeakPercent"),
        snow_latitude_change: rd.float("fSnowLatitudeChange"),
        tundra_latitude_change: rd.float("fTundraLatitudeChange"),
        grass_latitude_change: rd.float("fGrassLatitudeChange"),
        desert_bottom_latitude_change: rd.float("fDesertBottomLatitudeChange"),
        desert_top_latitude_change: rd.float("fDesertTopLatitudeChange"),
        ice_latitude: rd.float("fIceLatitude"),
        rand_ice_latitude: rd.float("fRandIceLatitude"),
    }
}

// ---------------------------------------------------------------------------------------------
// SeaLevelInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvSeaLevelInfo` (Gameinfo/CIV4SeaLevelInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct SeaLevelInfo {
    pub base: InfoBase,
    pub sea_level_change: i32,
}

/// C++ `CvSeaLevelInfo::read`.
pub fn read_sea_level(rd: &Rd) -> SeaLevelInfo {
    SeaLevelInfo { base: InfoBase::read(rd), sea_level_change: rd.int("iSeaLevelChange") }
}

// ---------------------------------------------------------------------------------------------
// EraInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvEraInfo` (Gameinfo/CIV4EraInfos.xml). Soundtrack/soundscape/audio members not ported.
#[derive(Debug, Clone, Default)]
pub struct EraInfo {
    pub base: InfoBase,
    pub starting_unit_multiplier: i32,
    pub starting_defense_units: i32,
    pub starting_worker_units: i32,
    pub starting_explore_units: i32,
    pub advanced_start_points: i32,
    pub starting_gold: i32,
    pub free_population: i32,
    pub start_percent: i32,
    pub growth_percent: i32,
    pub train_percent: i32,
    pub construct_percent: i32,
    pub create_percent: i32,
    pub research_percent: i32,
    pub build_percent: i32,
    pub improvement_percent: i32,
    pub great_people_percent: i32,
    pub anarchy_percent: i32,
    pub event_chance_per_turn: i32,
    /// C++ `isNoGoodies()`
    pub no_goodies: bool,
    /// C++ `isNoAnimals()`
    pub no_animals: bool,
    /// C++ `isNoBarbUnits()`
    pub no_barb_units: bool,
    /// C++ `isNoBarbCities()`
    pub no_barb_cities: bool,
}

/// C++ `CvEraInfo::read` (minus the audio tags).
pub fn read_era(rd: &Rd) -> EraInfo {
    EraInfo {
        base: InfoBase::read(rd),
        no_goodies: rd.bool("bNoGoodies"),
        no_animals: rd.bool("bNoAnimals"),
        no_barb_units: rd.bool("bNoBarbUnits"),
        no_barb_cities: rd.bool("bNoBarbCities"),
        starting_unit_multiplier: rd.int("iStartingUnitMultiplier"),
        starting_defense_units: rd.int("iStartingDefenseUnits"),
        starting_worker_units: rd.int("iStartingWorkerUnits"),
        starting_explore_units: rd.int("iStartingExploreUnits"),
        advanced_start_points: rd.int("iAdvancedStartPoints"),
        starting_gold: rd.int("iStartingGold"),
        free_population: rd.int("iFreePopulation"),
        start_percent: rd.int("iStartPercent"),
        growth_percent: rd.int("iGrowthPercent"),
        train_percent: rd.int("iTrainPercent"),
        construct_percent: rd.int("iConstructPercent"),
        create_percent: rd.int("iCreatePercent"),
        research_percent: rd.int("iResearchPercent"),
        build_percent: rd.int("iBuildPercent"),
        improvement_percent: rd.int("iImprovementPercent"),
        great_people_percent: rd.int("iGreatPeoplePercent"),
        anarchy_percent: rd.int("iAnarchyPercent"),
        event_chance_per_turn: rd.int("iEventChancePerTurn"),
    }
}

// ---------------------------------------------------------------------------------------------
// HandicapInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvHandicapInfo` (Gameinfo/CIV4HandicapInfo.xml). Several XML tags differ from the
/// getter names; the tag is noted where it does.
#[derive(Debug, Clone, Default)]
pub struct HandicapInfo {
    pub base: InfoBase,
    pub free_wins_vs_barbs: i32,
    pub animal_attack_prob: i32,
    /// C++ `getStartingLocationPercent()` (XML `iStartingLocPercent`)
    pub starting_location_percent: i32,
    pub advanced_start_points_mod: i32,
    /// C++ `getStartingGold()` (XML `iGold`)
    pub starting_gold: i32,
    pub free_units: i32,
    pub unit_cost_percent: i32,
    pub research_percent: i32,
    pub distance_maintenance_percent: i32,
    pub num_cities_maintenance_percent: i32,
    pub max_num_cities_maintenance: i32,
    pub colony_maintenance_percent: i32,
    pub max_colony_maintenance: i32,
    pub corporation_maintenance_percent: i32,
    pub civic_upkeep_percent: i32,
    pub inflation_percent: i32,
    pub health_bonus: i32,
    pub happy_bonus: i32,
    pub attitude_change: i32,
    pub no_tech_trade_modifier: i32,
    pub tech_trade_known_modifier: i32,
    pub unowned_tiles_per_game_animal: i32,
    pub unowned_tiles_per_barbarian_unit: i32,
    pub unowned_water_tiles_per_barbarian_unit: i32,
    pub unowned_tiles_per_barbarian_city: i32,
    pub barbarian_creation_turns_elapsed: i32,
    pub barbarian_city_creation_turns_elapsed: i32,
    pub barbarian_city_creation_prob: i32,
    /// C++ `getAnimalCombatModifier()` (XML `iAnimalBonus`)
    pub animal_combat_modifier: i32,
    /// C++ `getBarbarianCombatModifier()` (XML `iBarbarianBonus`)
    pub barbarian_combat_modifier: i32,
    /// C++ `getAIAnimalCombatModifier()` (XML `iAIAnimalBonus`)
    pub ai_animal_combat_modifier: i32,
    /// C++ `getAIBarbarianCombatModifier()` (XML `iAIBarbarianBonus`)
    pub ai_barbarian_combat_modifier: i32,
    pub starting_defense_units: i32,
    pub starting_worker_units: i32,
    pub starting_explore_units: i32,
    /// C++ `getAIStartingUnitMultiplier()`
    pub ai_starting_unit_multiplier: i32,
    /// C++ `getAIStartingDefenseUnits()`
    pub ai_starting_defense_units: i32,
    /// C++ `getAIStartingWorkerUnits()`
    pub ai_starting_worker_units: i32,
    /// C++ `getAIStartingExploreUnits()`
    pub ai_starting_explore_units: i32,
    /// C++ `getBarbarianInitialDefenders()` (XML `iBarbarianDefenders`)
    pub barbarian_initial_defenders: i32,
    /// C++ `getAIDeclareWarProb()`
    pub ai_declare_war_prob: i32,
    /// C++ `getAIWorkRateModifier()`
    pub ai_work_rate_modifier: i32,
    /// C++ `getAIGrowthPercent()`
    pub ai_growth_percent: i32,
    /// C++ `getAITrainPercent()`
    pub ai_train_percent: i32,
    /// C++ `getAIWorldTrainPercent()`
    pub ai_world_train_percent: i32,
    /// C++ `getAIConstructPercent()`
    pub ai_construct_percent: i32,
    /// C++ `getAIWorldConstructPercent()`
    pub ai_world_construct_percent: i32,
    /// C++ `getAICreatePercent()`
    pub ai_create_percent: i32,
    /// C++ `getAIWorldCreatePercent()`
    pub ai_world_create_percent: i32,
    /// C++ `getAICivicUpkeepPercent()`
    pub ai_civic_upkeep_percent: i32,
    /// C++ `getAIUnitCostPercent()`
    pub ai_unit_cost_percent: i32,
    /// C++ `getAIUnitSupplyPercent()`
    pub ai_unit_supply_percent: i32,
    /// C++ `getAIUnitUpgradePercent()`
    pub ai_unit_upgrade_percent: i32,
    /// C++ `getAIInflationPercent()`
    pub ai_inflation_percent: i32,
    /// C++ `getAIWarWearinessPercent()`
    pub ai_war_weariness_percent: i32,
    /// C++ `getAIPerEraModifier()`
    pub ai_per_era_modifier: i32,
    /// C++ `getAIAdvancedStartPercent()`
    pub ai_advanced_start_percent: i32,
    /// FfH: C++ `getAIFreeXP()`
    pub ai_free_xp: i32,
    /// FfH: C++ `getLairSpawnRate()`
    pub lair_spawn_rate: i32,
    /// C++ `getGoodies(i)` for `i < getNumGoodies()`, in XML order. Duplicates are kept: they
    /// weight the random pick in `CvPlayer::doGoody`.
    pub goodies: Vec<GoodyType>,
    /// C++ `isFreeTechs(i)`
    pub free_techs: IdxVec<TechType, bool>,
    /// C++ `isAIFreeTechs(i)`
    pub ai_free_techs: IdxVec<TechType, bool>,
}

impl HandicapInfo {
    /// C++ `getNumGoodies()`.
    pub fn num_goodies(&self) -> i32 {
        self.goodies.len() as i32
    }
}

/// C++ `CvHandicapInfo::read`.
pub fn read_handicap(rd: &Rd) -> HandicapInfo {
    HandicapInfo {
        base: InfoBase::read(rd),
        free_wins_vs_barbs: rd.int("iFreeWinsVsBarbs"),
        animal_attack_prob: rd.int("iAnimalAttackProb"),
        starting_location_percent: rd.int("iStartingLocPercent"),
        advanced_start_points_mod: rd.int("iAdvancedStartPointsMod"),
        starting_gold: rd.int("iGold"),
        free_units: rd.int("iFreeUnits"),
        unit_cost_percent: rd.int("iUnitCostPercent"),
        research_percent: rd.int("iResearchPercent"),
        distance_maintenance_percent: rd.int("iDistanceMaintenancePercent"),
        num_cities_maintenance_percent: rd.int("iNumCitiesMaintenancePercent"),
        max_num_cities_maintenance: rd.int("iMaxNumCitiesMaintenance"),
        colony_maintenance_percent: rd.int("iColonyMaintenancePercent"),
        max_colony_maintenance: rd.int("iMaxColonyMaintenance"),
        corporation_maintenance_percent: rd.int("iCorporationMaintenancePercent"),
        civic_upkeep_percent: rd.int("iCivicUpkeepPercent"),
        inflation_percent: rd.int("iInflationPercent"),
        health_bonus: rd.int("iHealthBonus"),
        happy_bonus: rd.int("iHappyBonus"),
        attitude_change: rd.int("iAttitudeChange"),
        no_tech_trade_modifier: rd.int("iNoTechTradeModifier"),
        tech_trade_known_modifier: rd.int("iTechTradeKnownModifier"),
        unowned_tiles_per_game_animal: rd.int("iUnownedTilesPerGameAnimal"),
        unowned_tiles_per_barbarian_unit: rd.int("iUnownedTilesPerBarbarianUnit"),
        unowned_water_tiles_per_barbarian_unit: rd.int("iUnownedWaterTilesPerBarbarianUnit"),
        unowned_tiles_per_barbarian_city: rd.int("iUnownedTilesPerBarbarianCity"),
        barbarian_creation_turns_elapsed: rd.int("iBarbarianCreationTurnsElapsed"),
        barbarian_city_creation_turns_elapsed: rd.int("iBarbarianCityCreationTurnsElapsed"),
        barbarian_city_creation_prob: rd.int("iBarbarianCityCreationProb"),
        animal_combat_modifier: rd.int("iAnimalBonus"),
        barbarian_combat_modifier: rd.int("iBarbarianBonus"),
        ai_animal_combat_modifier: rd.int("iAIAnimalBonus"),
        ai_barbarian_combat_modifier: rd.int("iAIBarbarianBonus"),
        starting_defense_units: rd.int("iStartingDefenseUnits"),
        starting_worker_units: rd.int("iStartingWorkerUnits"),
        starting_explore_units: rd.int("iStartingExploreUnits"),
        ai_starting_unit_multiplier: rd.int("iAIStartingUnitMultiplier"),
        ai_starting_defense_units: rd.int("iAIStartingDefenseUnits"),
        ai_starting_worker_units: rd.int("iAIStartingWorkerUnits"),
        ai_starting_explore_units: rd.int("iAIStartingExploreUnits"),
        barbarian_initial_defenders: rd.int("iBarbarianDefenders"),
        ai_declare_war_prob: rd.int("iAIDeclareWarProb"),
        ai_work_rate_modifier: rd.int("iAIWorkRateModifier"),
        ai_growth_percent: rd.int("iAIGrowthPercent"),
        ai_train_percent: rd.int("iAITrainPercent"),
        ai_world_train_percent: rd.int("iAIWorldTrainPercent"),
        ai_construct_percent: rd.int("iAIConstructPercent"),
        ai_world_construct_percent: rd.int("iAIWorldConstructPercent"),
        ai_create_percent: rd.int("iAICreatePercent"),
        ai_world_create_percent: rd.int("iAIWorldCreatePercent"),
        ai_civic_upkeep_percent: rd.int("iAICivicUpkeepPercent"),
        ai_unit_cost_percent: rd.int("iAIUnitCostPercent"),
        ai_unit_supply_percent: rd.int("iAIUnitSupplyPercent"),
        ai_unit_upgrade_percent: rd.int("iAIUnitUpgradePercent"),
        ai_inflation_percent: rd.int("iAIInflationPercent"),
        ai_war_weariness_percent: rd.int("iAIWarWearinessPercent"),
        ai_per_era_modifier: rd.int("iAIPerEraModifier"),
        ai_advanced_start_percent: rd.int("iAIAdvancedStartPercent"),
        // C++: SetStringList over <Goodies>, then FindInInfoClass on every name. All names are
        // known GoodyInfo types (an unknown one is reported as a load warning and skipped).
        goodies: rd.ref_list::<GoodyType>("Goodies"),
        free_techs: rd.pairs_bool::<TechType>("FreeTechs"),
        ai_free_techs: rd.pairs_bool::<TechType>("AIFreeTechs"),
        ai_free_xp: rd.int("iAIFreeXP"),
        lair_spawn_rate: rd.int("iLairSpawnRate"),
    }
}

// ---------------------------------------------------------------------------------------------
// GameOptionInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvGameOptionInfo` (Gameinfo/CIV4GameOptionInfos.xml). The table order must match the
/// hard-coded `GameOptionType` consts (checked at load).
#[derive(Debug, Clone, Default)]
pub struct GameOptionInfo {
    pub base: InfoBase,
    /// C++ `getDefault()`
    pub default: bool,
    /// C++ `getVisible()`
    pub visible: bool,
}

/// C++ `CvGameOptionInfo::read`.
pub fn read_game_option(rd: &Rd) -> GameOptionInfo {
    GameOptionInfo {
        base: InfoBase::read(rd),
        default: rd.bool("bDefault"),
        // C++ quirk: the constructor sets m_bVisible = true, but read() calls
        // GetChildXmlValByName(&m_bVisible, "bVisible") whose default (false) overwrites it,
        // so an option without <bVisible> ends up hidden. Kept as is.
        visible: rd.bool("bVisible"),
    }
}

// ---------------------------------------------------------------------------------------------
// GoodyInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvGoodyInfo` (Gameinfo/CIV4GoodyInfo.xml). `m_szSound` not ported.
#[derive(Debug, Clone, Default)]
pub struct GoodyInfo {
    pub base: InfoBase,
    pub gold: i32,
    pub gold_rand1: i32,
    pub gold_rand2: i32,
    pub map_offset: i32,
    pub map_range: i32,
    pub map_prob: i32,
    pub experience: i32,
    pub healing: i32,
    pub damage_prereq: i32,
    pub barbarian_unit_prob: i32,
    pub min_barbarians: i32,
    /// C++ `getUnitClassType()` (XML `UnitClass`): unit class granted.
    pub unit_class_type: Option<UnitClassType>,
    /// C++ `getBarbarianUnitClass()` (XML `BarbarianClass`): hostile unit class spawned.
    pub barbarian_unit_class: Option<UnitClassType>,
    /// C++ `isTech()`
    pub tech: bool,
    /// C++ `isBad()`
    pub bad: bool,
}

/// C++ `CvGoodyInfo::read` (minus `Sound`).
pub fn read_goody(rd: &Rd) -> GoodyInfo {
    GoodyInfo {
        base: InfoBase::read(rd),
        gold: rd.int("iGold"),
        gold_rand1: rd.int("iGoldRand1"),
        gold_rand2: rd.int("iGoldRand2"),
        map_offset: rd.int("iMapOffset"),
        map_range: rd.int("iMapRange"),
        map_prob: rd.int("iMapProb"),
        experience: rd.int("iExperience"),
        healing: rd.int("iHealing"),
        damage_prereq: rd.int("iDamagePrereq"),
        tech: rd.bool("bTech"),
        bad: rd.bool("bBad"),
        unit_class_type: rd.rf::<UnitClassType>("UnitClass"),
        barbarian_unit_class: rd.rf::<UnitClassType>("BarbarianClass"),
        barbarian_unit_prob: rd.int("iBarbarianUnitProb"),
        min_barbarians: rd.int("iMinBarbarians"),
    }
}

// ---------------------------------------------------------------------------------------------
// HurryInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvHurryInfo` (Gameinfo/CIV4HurryInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct HurryInfo {
    pub base: InfoBase,
    pub gold_per_production: i32,
    pub production_per_population: i32,
    /// C++ `isAnger()`
    pub anger: bool,
}

/// C++ `CvHurryInfo::read`.
pub fn read_hurry(rd: &Rd) -> HurryInfo {
    HurryInfo {
        base: InfoBase::read(rd),
        gold_per_production: rd.int("iGoldPerProduction"),
        production_per_population: rd.int("iProductionPerPopulation"),
        anger: rd.bool("bAnger"),
    }
}

// ---------------------------------------------------------------------------------------------
// UpkeepInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvUpkeepInfo` (Gameinfo/CIV4UpkeepInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct UpkeepInfo {
    pub base: InfoBase,
    pub population_percent: i32,
    pub city_percent: i32,
}

/// C++ `CvUpkeepInfo::read`.
pub fn read_upkeep(rd: &Rd) -> UpkeepInfo {
    UpkeepInfo {
        base: InfoBase::read(rd),
        population_percent: rd.int("iPopulationPercent"),
        city_percent: rd.int("iCityPercent"),
    }
}

// ---------------------------------------------------------------------------------------------
// CultureLevelInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvCultureLevelInfo` (Gameinfo/CIV4CultureLevelInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct CultureLevelInfo {
    pub base: InfoBase,
    pub city_defense_modifier: i32,
    /// C++ `getSpeedThreshold(eGameSpeed)`: culture needed for this level (default 0).
    pub speed_threshold: IdxVec<GameSpeedType, i32>,
}

/// C++ `CvCultureLevelInfo::read`.
pub fn read_culture_level(rd: &Rd) -> CultureLevelInfo {
    CultureLevelInfo {
        base: InfoBase::read(rd),
        city_defense_modifier: rd.int("iCityDefenseModifier"),
        speed_threshold: rd.pairs_int::<GameSpeedType>("SpeedThresholds", 0),
    }
}

// ---------------------------------------------------------------------------------------------
// VictoryInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvVictoryInfo` (Gameinfo/CIV4VictoryInfo.xml). `m_szMovie` not ported.
#[derive(Debug, Clone, Default)]
pub struct VictoryInfo {
    pub base: InfoBase,
    pub population_percent_lead: i32,
    pub land_percent: i32,
    pub min_land_percent: i32,
    pub religion_percent: i32,
    /// C++ `getCityCulture()`: culture level each culture city must reach.
    pub city_culture: Option<CultureLevelType>,
    pub num_culture_cities: i32,
    pub total_culture_ratio: i32,
    pub victory_delay_turns: i32,
    /// C++ `isTargetScore()`
    pub target_score: bool,
    /// C++ `isEndScore()`
    pub end_score: bool,
    /// C++ `isConquest()`
    pub conquest: bool,
    /// C++ `isDiploVote()`
    pub diplo_vote: bool,
    /// C++ `isPermanent()`
    pub permanent: bool,
}

/// C++ `CvVictoryInfo::read` (minus `VictoryMovie`).
pub fn read_victory(rd: &Rd) -> VictoryInfo {
    VictoryInfo {
        base: InfoBase::read(rd),
        target_score: rd.bool("bTargetScore"),
        end_score: rd.bool("bEndScore"),
        conquest: rd.bool("bConquest"),
        diplo_vote: rd.bool("bDiploVote"),
        permanent: rd.bool("bPermanent"),
        population_percent_lead: rd.int("iPopulationPercentLead"),
        land_percent: rd.int("iLandPercent"),
        min_land_percent: rd.int("iMinLandPercent"),
        religion_percent: rd.int("iReligionPercent"),
        city_culture: rd.rf::<CultureLevelType>("CityCulture"),
        num_culture_cities: rd.int("iNumCultureCities"),
        total_culture_ratio: rd.int("iTotalCultureRatio"),
        victory_delay_turns: rd.int("iVictoryDelayTurns"),
    }
}

// ---------------------------------------------------------------------------------------------
// ProcessInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvProcessInfo` (Gameinfo/CIV4ProcessInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct ProcessInfo {
    pub base: InfoBase,
    pub tech_prereq: Option<TechType>,
    /// C++ `getProductionToCommerceModifier(eCommerce)`
    pub production_to_commerce_modifier: [i32; CommerceType::NUM],
}

/// C++ `CvProcessInfo::read`.
pub fn read_process(rd: &Rd) -> ProcessInfo {
    ProcessInfo {
        base: InfoBase::read(rd),
        tech_prereq: rd.rf::<TechType>("TechPrereq"),
        // SetCommerce if the tag exists, else InitList(.., 0): zeros either way when absent.
        production_to_commerce_modifier: rd.commerces("ProductionToCommerceModifiers"),
    }
}

// ---------------------------------------------------------------------------------------------
// EmphasizeInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvEmphasizeInfo` (Gameinfo/CIV4EmphasizeInfo.xml): city governor emphasis.
#[derive(Debug, Clone, Default)]
pub struct EmphasizeInfo {
    pub base: InfoBase,
    /// C++ `isAvoidGrowth()`
    pub avoid_growth: bool,
    /// C++ `isGreatPeople()`
    pub great_people: bool,
    /// GrowthControl addon: C++ `isAvoidAngryCitizens()`
    pub avoid_angry_citizens: bool,
    /// GrowthControl addon: C++ `isAvoidUnhealthyCitizens()`
    pub avoid_unhealthy_citizens: bool,
    /// C++ `getYieldChange(eYield)` (member `m_piYieldModifiers`, XML `YieldModifiers`)
    pub yield_change: [i32; YieldType::NUM],
    /// C++ `getCommerceChange(eCommerce)` (member `m_piCommerceModifiers`, XML
    /// `CommerceModifiers`; the XML lists 3 values, espionage stays 0)
    pub commerce_change: [i32; CommerceType::NUM],
}

/// C++ `CvEmphasizeInfo::read`.
pub fn read_emphasize(rd: &Rd) -> EmphasizeInfo {
    EmphasizeInfo {
        base: InfoBase::read(rd),
        avoid_growth: rd.bool("bAvoidGrowth"),
        great_people: rd.bool("bGreatPeople"),
        // SetYields / SetCommerce if the tag exists, else InitList(.., 0).
        yield_change: rd.yields("YieldModifiers"),
        commerce_change: rd.commerces("CommerceModifiers"),
        avoid_angry_citizens: rd.bool("bAvoidAngryCitizens"),
        avoid_unhealthy_citizens: rd.bool("bAvoidUnhealthyCitizens"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Ctx, Registry};
    use crate::xml::XmlDoc;

    /// Runs `read` on the root element of `xml` against a registry holding only `classes`;
    /// returns the info and the load warnings.
    fn read_with<T>(classes: &[(&'static str, &[&str])], xml: &str, read: fn(&Rd) -> T) -> (T, Vec<String>) {
        let mut reg = Registry::new();
        for (class, names) in classes {
            reg.register_class(class, names.iter().map(|s| s.to_string()).collect());
        }
        let doc = XmlDoc::parse(xml).expect("test xml");
        let cx = Ctx::new(&reg, "test.xml");
        let info = read(&Rd::new(doc.root(), &cx));
        (info, cx.warnings.into_inner())
    }

    const SPEEDS: (&str, &[&str]) = ("GameSpeedInfo", &["GAMESPEED_A", "GAMESPEED_B"]);
    const TECHS: (&str, &[&str]) = ("TechInfo", &["TECH_A", "TECH_B", "TECH_C"]);
    const GOODIES: (&str, &[&str]) = ("GoodyInfo", &["GOODY_A", "GOODY_B"]);

    #[test]
    fn game_option_visible_defaults_to_false() {
        // C++ quirk: the constructor says visible, read() resets it to false when <bVisible> is absent.
        let (o, w) = read_with(&[], "<GameOptionInfo><Type>GAMEOPTION_X</Type></GameOptionInfo>", read_game_option);
        assert!(!o.visible && !o.default);
        assert!(w.is_empty());
        let (o, _) = read_with(&[], "<GameOptionInfo><bDefault>1</bDefault><bVisible>1</bVisible></GameOptionInfo>", read_game_option);
        assert!(o.visible && o.default);
    }

    #[test]
    fn absent_tags_take_cpp_defaults() {
        let (h, w) = read_with(&[TECHS, GOODIES], "<HandicapInfo><Type>HANDICAP_X</Type></HandicapInfo>", read_handicap);
        assert!(w.is_empty());
        assert_eq!(h.num_goodies(), 0);
        assert_eq!(h.free_techs.raw, vec![false; 3]);
        assert_eq!(h.ai_free_techs.raw, vec![false; 3]);

        let (c, _) = read_with(&[SPEEDS], "<CultureLevelInfo/>", read_culture_level);
        assert_eq!(c.speed_threshold.raw, vec![0, 0]);

        let (s, _) = read_with(&[], "<GameSpeedInfo/>", read_game_speed);
        assert_eq!(s.num_turn_increments(), 0);

        let (p, _) = read_with(&[TECHS], "<ProcessInfo/>", read_process);
        assert_eq!((p.tech_prereq, p.production_to_commerce_modifier), (None, [0; 4]));

        let (e, _) = read_with(&[], "<EmphasizeInfo/>", read_emphasize);
        assert_eq!((e.yield_change, e.commerce_change), ([0; 3], [0; 4]));

        let (g, w) = read_with(&[], "<GoodyInfo><UnitClass>NONE</UnitClass></GoodyInfo>", read_goody);
        assert_eq!((g.unit_class_type, g.barbarian_unit_class), (None, None));
        assert!(w.is_empty());
    }

    #[test]
    fn lists_keep_order_duplicates_and_partial_entries() {
        let xml = "<HandicapInfo>\
            <Goodies><GoodyType>GOODY_B</GoodyType><GoodyType>GOODY_A</GoodyType><GoodyType>GOODY_B</GoodyType></Goodies>\
            <FreeTechs><FreeTech><TechType>TECH_C</TechType><bFreeTech>1</bFreeTech></FreeTech></FreeTechs>\
            </HandicapInfo>";
        let (h, w) = read_with(&[TECHS, GOODIES], xml, read_handicap);
        assert!(w.is_empty());
        assert_eq!(h.goodies, vec![GoodyType(1), GoodyType(0), GoodyType(1)]);
        assert_eq!(h.free_techs.raw, vec![false, false, true]);

        // Each GameTurnInfo child's values are looked up by name, default 0.
        let xml = "<GameSpeedInfo><GameTurnInfos>\
            <GameTurnInfo><iTurnsPerIncrement>75</iTurnsPerIncrement><iMonthIncrement>6</iMonthIncrement></GameTurnInfo>\
            <GameTurnInfo><iTurnsPerIncrement>10</iTurnsPerIncrement></GameTurnInfo>\
            </GameTurnInfos></GameSpeedInfo>";
        let (s, _) = read_with(&[], xml, read_game_speed);
        assert_eq!(
            s.game_turn_info,
            vec![
                GameTurnInfo { month_increment: 6, num_game_turns_per_increment: 75 },
                GameTurnInfo { month_increment: 0, num_game_turns_per_increment: 10 },
            ]
        );

        // Speeds missing from SpeedThresholds keep 0.
        let xml = "<CultureLevelInfo><SpeedThresholds><SpeedThreshold><GameSpeedType>GAMESPEED_B</GameSpeedType>\
            <iThreshold>40</iThreshold></SpeedThreshold></SpeedThresholds></CultureLevelInfo>";
        let (c, _) = read_with(&[SPEEDS], xml, read_culture_level);
        assert_eq!(c.speed_threshold.raw, vec![0, 40]);
    }
}
