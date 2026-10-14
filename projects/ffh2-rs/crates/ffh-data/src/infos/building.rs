//! Info classes: SpecialBuildingInfo, BuildingClassInfo, BuildingInfo.
//! Port of `CvSpecialBuildingInfo::read`, `CvBuildingClassInfo::read` (+ `readPass3`) and
//! `CvBuildingInfo::read` (+ `readPass3`) in CvInfos.cpp.
//!
//! Not ported (graphics/audio/UI only): the `CvHotkeyInfo` hotkey fields, `ArtDefineTag`,
//! `MovieDefineTag`, `ConstructSound`, `fVisibilityPriority` (city art layering),
//! `Advisor` (only feeds the human "recommended" production popup; AdvisorInfos are not
//! loaded) and `m_iMissionType` (set for the action bar after loading, not by `read`).

use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::{InfoId, Rd};

/// C++ `GC.getNUM_BUILDING_AND_TECH_PREREQS()` (GlobalDefines.xml). `Rd` cannot see the
/// defines, so the value is mirrored here; the data tests check it against
/// `defines.num_building_and_tech_prereqs`.
const NUM_BUILDING_AND_TECH_PREREQS: usize = 3;
/// C++ `GC.getNUM_BUILDING_PREREQ_OR_BONUSES()`: 5 from GlobalDefinesAlt.xml (overrides the
/// BTS 4 before `cacheGlobals`). Checked against the defines by the data tests.
const NUM_BUILDING_PREREQ_OR_BONUSES: usize = 5;

/// C++ `CvSpecialBuildingInfo` (Buildings/CIV4SpecialBuildingInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct SpecialBuildingInfo {
    pub base: InfoBase,
    pub obsolete_tech: Option<TechType>,
    pub tech_prereq: Option<TechType>,
    pub tech_prereq_anyone: Option<TechType>,
    /// C++ `isValid()`.
    pub valid: bool,
    /// C++ `getProductionTraits(i)`: production modifier (%) for players with the trait.
    pub production_traits: IdxVec<TraitType, i32>,
}

/// C++ `CvSpecialBuildingInfo::read`.
pub fn read_special_building(rd: &Rd) -> SpecialBuildingInfo {
    SpecialBuildingInfo {
        base: InfoBase::read(rd),
        obsolete_tech: rd.rf("ObsoleteTech"),
        tech_prereq: rd.rf("TechPrereq"),
        tech_prereq_anyone: rd.rf("TechPrereqAnyone"),
        valid: rd.bool("bValid"),
        production_traits: rd.pairs_int("ProductionTraits", 0),
    }
}

/// C++ `CvBuildingClassInfo` (Buildings/CIV4BuildingClassInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct BuildingClassInfo {
    pub base: InfoBase,
    /// `!= -1` makes the class a world wonder (C++ `isWorldWonderClass`).
    pub max_global_instances: i32,
    /// `!= -1` makes the class a team wonder (C++ `isTeamWonderClass`).
    pub max_team_instances: i32,
    /// `!= -1` makes the class a national wonder (C++ `isNationalWonderClass`).
    pub max_player_instances: i32,
    pub extra_player_instances: i32,
    /// C++ `getDefaultBuildingIndex()`: `<DefaultBuilding>`, resolved in `readPass3()`.
    /// Default building of the class for civs that do not override it
    /// (see `CivilizationInfo::civilization_buildings`).
    pub default_building_index: Option<BuildingType>,
    pub no_limit: bool,
    pub monument: bool,
    /// C++ `getVictoryThreshold(i)`.
    pub victory_threshold: IdxVec<VictoryType, i32>,
}

/// C++ `CvBuildingClassInfo::read` + `readPass3`.
pub fn read_building_class(rd: &Rd) -> BuildingClassInfo {
    BuildingClassInfo {
        base: InfoBase::read(rd),
        // C++ defaults are 0 (not -1) when the tags are absent; every FFH2 entry sets them.
        max_global_instances: rd.int("iMaxGlobalInstances"),
        max_team_instances: rd.int("iMaxTeamInstances"),
        max_player_instances: rd.int("iMaxPlayerInstances"),
        extra_player_instances: rd.int("iExtraPlayerInstances"),
        no_limit: rd.bool("bNoLimit"),
        monument: rd.bool("bMonument"),
        victory_threshold: rd.pairs_int("VictoryThresholds", 0),
        // readPass3: m_iDefaultBuildingIndex = GC.getInfoTypeForString(<DefaultBuilding>)
        default_building_index: rd.rf("DefaultBuilding"),
    }
}

/// C++ `CvBuildingInfo` (Buildings/CIV4BuildingInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct BuildingInfo {
    pub base: InfoBase,

    // ---- type references ----
    /// C++ `getBuildingClassType()`, XML `<BuildingClass>`.
    pub building_class_type: Option<BuildingClassType>,
    pub victory_prereq: Option<VictoryType>,
    pub free_start_era: Option<EraType>,
    pub max_start_era: Option<EraType>,
    pub obsolete_tech: Option<TechType>,
    /// C++ `getPrereqAndTech()`, XML `<PrereqTech>`.
    pub prereq_and_tech: Option<TechType>,
    pub no_bonus: Option<BonusType>,
    pub power_bonus: Option<BonusType>,
    pub free_bonus: Option<BonusType>,
    pub num_free_bonuses: i32,
    /// C++ `getFreeBuildingClass()`, XML `<FreeBuilding>`.
    pub free_building_class: Option<BuildingClassType>,
    pub free_promotion: Option<PromotionType>,
    pub civic_option: Option<CivicOptionType>,
    pub special_building_type: Option<SpecialBuildingType>,
    pub holy_city: Option<ReligionType>,
    pub religion_type: Option<ReligionType>,
    /// C++ `getStateReligion()` (the religion). The bool `isStateReligion()` is
    /// [`Self::is_state_religion`].
    pub state_religion: Option<ReligionType>,
    /// C++ `getPrereqReligion()` (the religion). The bool `isPrereqReligion()` is
    /// [`Self::is_prereq_religion`].
    pub prereq_religion: Option<ReligionType>,
    pub prereq_corporation: Option<CorporationType>,
    pub founds_corporation: Option<CorporationType>,
    /// C++ `getGlobalReligionCommerce()`: religion whose global spread yields commerce here.
    pub global_religion_commerce: Option<ReligionType>,
    /// C++ `getGlobalCorporationCommerce()`.
    pub global_corporation_commerce: Option<CorporationType>,
    /// C++ `getPrereqAndBonus()`, XML `<Bonus>`.
    pub prereq_and_bonus: Option<BonusType>,
    pub great_people_unit_class: Option<UnitClassType>,
    /// C++ `getVoteSourceType()`, XML `<DiploVoteType>`.
    pub vote_source_type: Option<VoteSourceType>,

    // ---- ints ----
    /// C++ `getAIWeight()`.
    pub ai_weight: i32,
    /// C++ `getProductionCost()`, XML `<iCost>` (-1 = cannot be built).
    pub production_cost: i32,
    pub hurry_cost_modifier: i32,
    pub hurry_anger_modifier: i32,
    pub advanced_start_cost: i32,
    pub advanced_start_cost_increase: i32,
    pub min_area_size: i32,
    /// C++ `getNumCitiesPrereq()`, XML `<iCitiesPrereq>`.
    pub num_cities_prereq: i32,
    /// C++ `getNumTeamsPrereq()`, XML `<iTeamsPrereq>`.
    pub num_teams_prereq: i32,
    /// C++ `getUnitLevelPrereq()`, XML `<iLevelPrereq>`.
    pub unit_level_prereq: i32,
    pub min_latitude: i32,
    /// Defaults to 90 when absent.
    pub max_latitude: i32,
    pub great_people_rate_modifier: i32,
    pub great_general_rate_modifier: i32,
    pub domestic_great_general_rate_modifier: i32,
    pub global_great_people_rate_modifier: i32,
    pub anarchy_modifier: i32,
    pub golden_age_modifier: i32,
    pub global_hurry_modifier: i32,
    /// C++ `getFreeExperience()`, XML `<iExperience>`.
    pub free_experience: i32,
    /// C++ `getGlobalFreeExperience()`, XML `<iGlobalExperience>`.
    pub global_free_experience: i32,
    pub food_kept: i32,
    pub airlift: i32,
    pub air_modifier: i32,
    pub air_unit_capacity: i32,
    pub nuke_modifier: i32,
    pub nuke_explosion_rand: i32,
    pub free_specialist: i32,
    pub area_free_specialist: i32,
    pub global_free_specialist: i32,
    pub happiness: i32,
    pub area_happiness: i32,
    pub global_happiness: i32,
    pub state_religion_happiness: i32,
    pub worker_speed_modifier: i32,
    pub military_production_modifier: i32,
    pub space_production_modifier: i32,
    pub global_space_production_modifier: i32,
    pub trade_routes: i32,
    pub coastal_trade_routes: i32,
    pub global_trade_routes: i32,
    pub trade_route_modifier: i32,
    pub foreign_trade_route_modifier: i32,
    /// C++ `getAssetValue()`, XML `<iAsset>`.
    pub asset_value: i32,
    /// C++ `getPowerValue()`, XML `<iPower>`.
    pub power_value: i32,
    pub great_people_rate_change: i32,
    /// C++ `getConquestProbability()`, XML `<iConquestProb>`.
    pub conquest_probability: i32,
    pub maintenance_modifier: i32,
    pub war_weariness_modifier: i32,
    pub global_war_weariness_modifier: i32,
    pub enemy_war_weariness_modifier: i32,
    pub heal_rate_change: i32,
    pub health: i32,
    pub area_health: i32,
    pub global_health: i32,
    pub global_population_change: i32,
    pub free_techs: i32,
    /// C++ `getDefenseModifier()`, XML `<iDefense>`.
    pub defense_modifier: i32,
    /// C++ `getBombardDefenseModifier()`, XML `<iBombardDefense>`.
    pub bombard_defense_modifier: i32,
    /// C++ `getAllCityDefenseModifier()`, XML `<iAllCityDefense>`.
    pub all_city_defense_modifier: i32,
    /// C++ `getEspionageDefenseModifier()`, XML `<iEspionageDefense>`.
    pub espionage_defense_modifier: i32,

    // ---- bools ----
    pub team_share: bool,
    pub water: bool,
    pub river: bool,
    pub power: bool,
    pub dirty_power: bool,
    pub area_clean_power: bool,
    /// C++ `isAreaBorderObstacle()`, XML `<bBorderObstacle>`.
    pub area_border_obstacle: bool,
    pub force_team_vote_eligible: bool,
    pub capital: bool,
    pub government_center: bool,
    pub golden_age: bool,
    pub map_centering: bool,
    pub no_unhappiness: bool,
    pub no_unhealthy_population: bool,
    pub building_only_healthy: bool,
    pub never_capture: bool,
    pub nuke_immune: bool,
    /// C++ `isPrereqReligion()` (named apart from the religion [`Self::prereq_religion`]).
    pub is_prereq_religion: bool,
    pub center_in_city: bool,
    /// C++ `isStateReligion()` (named apart from the religion [`Self::state_religion`]).
    pub is_state_religion: bool,
    pub allows_nukes: bool,

    // ---- FfH additions ----
    pub apply_free_promotion_on_move: bool,
    pub equipment: bool,
    pub hide_units: bool,
    pub no_civic_anger: bool,
    pub requires_caster: bool,
    pub see_invisible: bool,
    pub unhappy_production: bool,
    pub crime: i32,
    pub free_promotion_pick: i32,
    pub free_bonus2: Option<BonusType>,
    pub free_bonus3: Option<BonusType>,
    pub global_resist_enemy_modify: i32,
    pub global_resist_modify: i32,
    pub modify_global_counter: i32,
    pub plot_radius: i32,
    /// C++ `getPrereqCiv()`: always `None`.
    ///
    /// C++ quirk: `read()` only stashes `<PrereqCiv>` for `readPass3()`, and the loop that
    /// would call `CvBuildingInfo::readPass3()` in `LoadPreMenuGlobals` is commented out
    /// (`/** ... **/`), so `m_iPrereqCiv` keeps its constructor value `NO_CIVILIZATION` and
    /// the `CvCity::canConstruct` check never fires. No gameplay effect in FFH2 0.41: every
    /// building with a `<PrereqCiv>` sits in a `BUILDINGCLASS_CIV_BUILDINGn` class whose
    /// default building is `NONE`, so only civs mapping the class to it can build it.
    /// The XML value is kept in [`Self::prereq_civ_xml`].
    pub prereq_civ: Option<CivilizationType>,
    /// `<PrereqCiv>` as written in the XML (the string C++ keeps in
    /// `m_aszExtraXMLforPass3`), resolved. Never applied by the C++ -- rules code must use
    /// [`Self::prereq_civ`]; this is for inspection and documented deviations only.
    pub prereq_civ_xml: Option<CivilizationType>,
    pub prereq_trait: Option<TraitType>,
    pub remove_promotion: Option<PromotionType>,
    pub resist_magic: i32,
    /// C++ `getSpecialistCommerceChange(i, j)`: per-specialist commerce change.
    pub specialist_commerce_change: IdxVec<SpecialistType, [i32; CommerceType::NUM]>,
    /// C++ `isVictoryBuilding()` (Better AI), XML `<bVictoryBuilding>`.
    pub victory_building: bool,

    // ---- arrays ----
    /// C++ `getPrereqAndTechs(i)`, XML `<TechTypes>`: `NUM_BUILDING_AND_TECH_PREREQS` (3)
    /// slots, `None`-filled.
    pub prereq_and_techs: Vec<Option<TechType>>,
    /// C++ `getPrereqOrBonuses(i)`, XML `<PrereqBonuses>`: `NUM_BUILDING_PREREQ_OR_BONUSES`
    /// (5) slots, `None`-filled.
    pub prereq_or_bonuses: Vec<Option<BonusType>>,
    /// C++ `getProductionTraits(i)`: production modifier (%) for players with the trait.
    pub production_traits: IdxVec<TraitType, i32>,
    /// C++ `getHappinessTraits(i)`: extra happiness for players with the trait.
    pub happiness_traits: IdxVec<TraitType, i32>,
    pub sea_plot_yield_change: [i32; YieldType::NUM],
    pub river_plot_yield_change: [i32; YieldType::NUM],
    pub global_sea_plot_yield_change: [i32; YieldType::NUM],
    pub yield_change: [i32; YieldType::NUM],
    pub yield_modifier: [i32; YieldType::NUM],
    pub power_yield_modifier: [i32; YieldType::NUM],
    pub area_yield_modifier: [i32; YieldType::NUM],
    pub global_yield_modifier: [i32; YieldType::NUM],
    pub commerce_change: [i32; CommerceType::NUM],
    pub obsolete_safe_commerce_change: [i32; CommerceType::NUM],
    pub commerce_change_double_time: [i32; CommerceType::NUM],
    pub commerce_modifier: [i32; CommerceType::NUM],
    pub global_commerce_modifier: [i32; CommerceType::NUM],
    pub specialist_extra_commerce: [i32; CommerceType::NUM],
    pub state_religion_commerce: [i32; CommerceType::NUM],
    pub commerce_happiness: [i32; CommerceType::NUM],
    pub religion_change: IdxVec<ReligionType, i32>,
    pub specialist_count: IdxVec<SpecialistType, i32>,
    pub free_specialist_count: IdxVec<SpecialistType, i32>,
    pub bonus_health_changes: IdxVec<BonusType, i32>,
    pub bonus_happiness_changes: IdxVec<BonusType, i32>,
    pub bonus_production_modifier: IdxVec<BonusType, i32>,
    pub unit_combat_free_experience: IdxVec<UnitCombatType, i32>,
    pub domain_free_experience: IdxVec<DomainType, i32>,
    pub domain_production_modifier: IdxVec<DomainType, i32>,
    pub building_happiness_changes: IdxVec<BuildingClassType, i32>,
    /// C++ `getPrereqNumOfBuildingClass(i)`, XML `<PrereqBuildingClasses>`.
    pub prereq_num_of_building_class: IdxVec<BuildingClassType, i32>,
    pub flavor_value: IdxVec<FlavorType, i32>,
    pub improvement_free_specialist: IdxVec<ImprovementType, i32>,
    /// C++ `isCommerceFlexible(i)`, XML `<CommerceFlexibles>`.
    pub commerce_flexible: [bool; CommerceType::NUM],
    /// C++ `isCommerceChangeOriginalOwner(i)`, XML `<CommerceChangeOriginalOwners>`.
    pub commerce_change_original_owner: [bool; CommerceType::NUM],
    /// C++ `isBuildingClassNeededInCity(i)`, XML `<BuildingClassNeededs>`.
    pub building_class_needed_in_city: IdxVec<BuildingClassType, bool>,
    /// C++ `getSpecialistYieldChange(i, j)`.
    pub specialist_yield_change: IdxVec<SpecialistType, [i32; YieldType::NUM]>,
    /// C++ `getBonusYieldModifier(i, j)`.
    pub bonus_yield_modifier: IdxVec<BonusType, [i32; YieldType::NUM]>,
}

/// C++ `CvBuildingInfo::read` (+ the never-called `readPass3`, see `prereq_civ`).
pub fn read_building(rd: &Rd) -> BuildingInfo {
    BuildingInfo {
        base: InfoBase::read(rd),

        building_class_type: rd.rf("BuildingClass"),
        special_building_type: rd.rf("SpecialBuildingType"),
        holy_city: rd.rf("HolyCity"),
        religion_type: rd.rf("ReligionType"),
        state_religion: rd.rf("StateReligion"),
        prereq_religion: rd.rf("PrereqReligion"),
        prereq_corporation: rd.rf("PrereqCorporation"),
        founds_corporation: rd.rf("FoundsCorporation"),
        global_religion_commerce: rd.rf("GlobalReligionCommerce"),
        global_corporation_commerce: rd.rf("GlobalCorporationCommerce"),
        victory_prereq: rd.rf("VictoryPrereq"),
        free_start_era: rd.rf("FreeStartEra"),
        max_start_era: rd.rf("MaxStartEra"),
        obsolete_tech: rd.rf("ObsoleteTech"),
        prereq_and_tech: rd.rf("PrereqTech"),
        prereq_and_techs: rd.ref_list_fixed("TechTypes", NUM_BUILDING_AND_TECH_PREREQS),
        prereq_and_bonus: rd.rf("Bonus"),
        prereq_or_bonuses: rd.ref_list_fixed("PrereqBonuses", NUM_BUILDING_PREREQ_OR_BONUSES),
        production_traits: rd.pairs_int("ProductionTraits", 0),
        happiness_traits: rd.pairs_int("HappinessTraits", 0),
        no_bonus: rd.rf("NoBonus"),
        power_bonus: rd.rf("PowerBonus"),
        free_bonus: rd.rf("FreeBonus"),
        num_free_bonuses: rd.int("iNumFreeBonuses"),
        free_building_class: rd.rf("FreeBuilding"),
        free_promotion: rd.rf("FreePromotion"),
        civic_option: rd.rf("CivicOption"),
        great_people_unit_class: rd.rf("GreatPeopleUnitClass"),
        vote_source_type: rd.rf("DiploVoteType"),

        great_people_rate_change: rd.int("iGreatPeopleRateChange"),
        team_share: rd.bool("bTeamShare"),
        water: rd.bool("bWater"),
        river: rd.bool("bRiver"),
        power: rd.bool("bPower"),
        dirty_power: rd.bool("bDirtyPower"),
        area_clean_power: rd.bool("bAreaCleanPower"),
        area_border_obstacle: rd.bool("bBorderObstacle"),
        force_team_vote_eligible: rd.bool("bForceTeamVoteEligible"),
        capital: rd.bool("bCapital"),
        government_center: rd.bool("bGovernmentCenter"),
        golden_age: rd.bool("bGoldenAge"),
        allows_nukes: rd.bool("bAllowsNukes"),
        map_centering: rd.bool("bMapCentering"),
        no_unhappiness: rd.bool("bNoUnhappiness"),
        no_unhealthy_population: rd.bool("bNoUnhealthyPopulation"),
        building_only_healthy: rd.bool("bBuildingOnlyHealthy"),
        never_capture: rd.bool("bNeverCapture"),
        nuke_immune: rd.bool("bNukeImmune"),
        is_prereq_religion: rd.bool("bPrereqReligion"),
        center_in_city: rd.bool("bCenterInCity"),
        is_state_religion: rd.bool("bStateReligion"),
        ai_weight: rd.int("iAIWeight"),
        production_cost: rd.int("iCost"),
        hurry_cost_modifier: rd.int("iHurryCostModifier"),
        hurry_anger_modifier: rd.int("iHurryAngerModifier"),
        advanced_start_cost: rd.int("iAdvancedStartCost"),
        advanced_start_cost_increase: rd.int("iAdvancedStartCostIncrease"),
        min_area_size: rd.int("iMinAreaSize"),
        conquest_probability: rd.int("iConquestProb"),
        num_cities_prereq: rd.int("iCitiesPrereq"),
        num_teams_prereq: rd.int("iTeamsPrereq"),
        unit_level_prereq: rd.int("iLevelPrereq"),
        min_latitude: rd.int("iMinLatitude"),
        max_latitude: rd.int_or("iMaxLatitude", 90),
        great_people_rate_modifier: rd.int("iGreatPeopleRateModifier"),
        great_general_rate_modifier: rd.int("iGreatGeneralRateModifier"),
        domestic_great_general_rate_modifier: rd.int("iDomesticGreatGeneralRateModifier"),
        global_great_people_rate_modifier: rd.int("iGlobalGreatPeopleRateModifier"),
        anarchy_modifier: rd.int("iAnarchyModifier"),
        golden_age_modifier: rd.int("iGoldenAgeModifier"),
        global_hurry_modifier: rd.int("iGlobalHurryModifier"),
        free_experience: rd.int("iExperience"),
        global_free_experience: rd.int("iGlobalExperience"),
        food_kept: rd.int("iFoodKept"),
        airlift: rd.int("iAirlift"),
        air_modifier: rd.int("iAirModifier"),
        air_unit_capacity: rd.int("iAirUnitCapacity"),
        nuke_modifier: rd.int("iNukeModifier"),
        nuke_explosion_rand: rd.int("iNukeExplosionRand"),
        free_specialist: rd.int("iFreeSpecialist"),
        area_free_specialist: rd.int("iAreaFreeSpecialist"),
        global_free_specialist: rd.int("iGlobalFreeSpecialist"),
        maintenance_modifier: rd.int("iMaintenanceModifier"),
        war_weariness_modifier: rd.int("iWarWearinessModifier"),
        global_war_weariness_modifier: rd.int("iGlobalWarWearinessModifier"),
        enemy_war_weariness_modifier: rd.int("iEnemyWarWearinessModifier"),
        heal_rate_change: rd.int("iHealRateChange"),
        health: rd.int("iHealth"),
        area_health: rd.int("iAreaHealth"),
        global_health: rd.int("iGlobalHealth"),
        happiness: rd.int("iHappiness"),
        area_happiness: rd.int("iAreaHappiness"),
        global_happiness: rd.int("iGlobalHappiness"),
        state_religion_happiness: rd.int("iStateReligionHappiness"),
        worker_speed_modifier: rd.int("iWorkerSpeedModifier"),
        military_production_modifier: rd.int("iMilitaryProductionModifier"),
        space_production_modifier: rd.int("iSpaceProductionModifier"),
        global_space_production_modifier: rd.int("iGlobalSpaceProductionModifier"),
        trade_routes: rd.int("iTradeRoutes"),
        coastal_trade_routes: rd.int("iCoastalTradeRoutes"),
        global_trade_routes: rd.int("iGlobalTradeRoutes"),
        trade_route_modifier: rd.int("iTradeRouteModifier"),
        foreign_trade_route_modifier: rd.int("iForeignTradeRouteModifier"),
        global_population_change: rd.int("iGlobalPopulationChange"),
        free_techs: rd.int("iFreeTechs"),
        defense_modifier: rd.int("iDefense"),
        bombard_defense_modifier: rd.int("iBombardDefense"),
        all_city_defense_modifier: rd.int("iAllCityDefense"),
        espionage_defense_modifier: rd.int("iEspionageDefense"),
        asset_value: rd.int("iAsset"),
        power_value: rd.int("iPower"),

        apply_free_promotion_on_move: rd.bool("bApplyFreePromotionOnMove"),
        equipment: rd.bool("bEquipment"),
        hide_units: rd.bool("bHideUnits"),
        no_civic_anger: rd.bool("bNoCivicAnger"),
        requires_caster: rd.bool("bRequiresCaster"),
        see_invisible: rd.bool("bSeeInvisible"),
        unhappy_production: rd.bool("bUnhappyProduction"),
        crime: rd.int("iCrime"),
        free_promotion_pick: rd.int("iFreePromotionPick"),
        free_bonus2: rd.rf("FreeBonus2"),
        free_bonus3: rd.rf("FreeBonus3"),
        global_resist_enemy_modify: rd.int("iGlobalResistEnemyModify"),
        global_resist_modify: rd.int("iGlobalResistModify"),
        modify_global_counter: rd.int("iModifyGlobalCounter"),
        plot_radius: rd.int("iPlotRadius"),
        // C++ quirk: readPass3 never runs for buildings (see the field docs).
        prereq_civ: None,
        prereq_civ_xml: rd.rf("PrereqCiv"),
        prereq_trait: rd.rf("PrereqTrait"),
        remove_promotion: rd.rf("RemovePromotion"),
        resist_magic: rd.int("iResistMagic"),
        victory_building: rd.bool("bVictoryBuilding"),

        sea_plot_yield_change: rd.yields("SeaPlotYieldChanges"),
        river_plot_yield_change: rd.yields("RiverPlotYieldChanges"),
        global_sea_plot_yield_change: rd.yields("GlobalSeaPlotYieldChanges"),
        yield_change: rd.yields("YieldChanges"),
        yield_modifier: rd.yields("YieldModifiers"),
        power_yield_modifier: rd.yields("PowerYieldModifiers"),
        area_yield_modifier: rd.yields("AreaYieldModifiers"),
        global_yield_modifier: rd.yields("GlobalYieldModifiers"),
        commerce_change: rd.commerces("CommerceChanges"),
        obsolete_safe_commerce_change: rd.commerces("ObsoleteSafeCommerceChanges"),
        commerce_change_double_time: rd.commerces("CommerceChangeDoubleTimes"),
        commerce_modifier: rd.commerces("CommerceModifiers"),
        global_commerce_modifier: rd.commerces("GlobalCommerceModifiers"),
        specialist_extra_commerce: rd.commerces("SpecialistExtraCommerces"),
        state_religion_commerce: rd.commerces("StateReligionCommerces"),
        commerce_happiness: rd.commerces("CommerceHappinesses"),
        religion_change: rd.pairs_int("ReligionChanges", 0),
        specialist_count: rd.pairs_int("SpecialistCounts", 0),
        free_specialist_count: rd.pairs_int("FreeSpecialistCounts", 0),
        commerce_flexible: commerce_bools(rd, "CommerceFlexibles"),
        commerce_change_original_owner: commerce_bools(rd, "CommerceChangeOriginalOwners"),
        bonus_health_changes: rd.pairs_int("BonusHealthChanges", 0),
        bonus_happiness_changes: rd.pairs_int("BonusHappinessChanges", 0),
        bonus_production_modifier: rd.pairs_int("BonusProductionModifiers", 0),
        unit_combat_free_experience: rd.pairs_int("UnitCombatFreeExperiences", 0),
        domain_free_experience: rd.pairs_int("DomainFreeExperiences", 0),
        domain_production_modifier: rd.pairs_int("DomainProductionModifiers", 0),
        prereq_num_of_building_class: rd.pairs_int("PrereqBuildingClasses", 0),
        building_class_needed_in_city: rd.pairs_bool("BuildingClassNeededs"),
        specialist_yield_change: keyed_arrays(rd, "SpecialistYieldChanges", "SpecialistYieldChange", "SpecialistType", "YieldChanges"),
        bonus_yield_modifier: keyed_arrays(rd, "BonusYieldModifiers", "BonusYieldModifier", "BonusType", "YieldModifiers"),
        flavor_value: rd.pairs_int("Flavors", 0),
        improvement_free_specialist: rd.pairs_int("ImprovementFreeSpecialists", 0),
        building_happiness_changes: rd.pairs_int("BuildingHappinessChanges", 0),
        specialist_commerce_change: keyed_arrays(rd, "SpecialistCommerceChanges", "SpecialistCommerceChange", "SpecialistType", "CommerceChanges"),
    }
}

/// C++ `SetCommerce(&m_pbX)` on a bool list (or `InitList(.., NUM_COMMERCE_TYPES)` when absent).
fn commerce_bools(rd: &Rd, tag: &str) -> [bool; CommerceType::NUM] {
    let v = rd.positional_bools(tag, CommerceType::NUM);
    [v[0], v[1], v[2], v[3]]
}

/// The entries the C++ loop `SetToChildByTagName(list); SetToChildByTagName(entry);
/// for (j < GetNumChildren(list)) { ...; NextSibling }` visits: every child of `<list>`
/// from the first `<entry>` on (later sibling names are not checked).
fn list_entries<'a>(rd: &Rd<'a>, list: &str, entry: &str) -> Vec<Rd<'a>> {
    match rd.child(list) {
        Some(l) => l.children().skip_while(|c| c.e.name() != entry).collect(),
        None => Vec::new(),
    }
}

/// 2D array keyed by an info type with `N` positional values per key: C++
/// `Init2DIntList(&m_ppai, GC.getNumXInfos(), N)` (all zeros) followed by the
/// `SpecialistYieldChanges` / `BonusYieldModifiers` / `SpecialistCommerceChanges` loops of
/// `CvBuildingInfo::read`: per entry, `<key_tag>` is looked up and, if valid, its row is
/// replaced by the `SetYields`/`SetCommerce` list `<vals_tag>` (zeros when that is absent).
fn keyed_arrays<K: InfoId, const N: usize>(
    rd: &Rd,
    list: &str,
    entry: &str,
    key_tag: &str,
    vals_tag: &str,
) -> IdxVec<K, [i32; N]> {
    let mut out = IdxVec::from_elem([0; N], rd.reg().count::<K>());
    for e in list_entries(rd, list, entry) {
        // C++ skips invalid keys for yields; for SpecialistCommerceChanges an invalid key
        // would index the array with -1 (undefined behaviour) -- skipped here as well.
        if let Some(k) = e.rf::<K>(key_tag) {
            let v = e.positional_ints(vals_tag, N);
            out[k].copy_from_slice(&v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Ctx, Registry};
    use crate::xml::XmlDoc;

    fn registry() -> Registry {
        let mut reg = Registry::new();
        let classes: [(&'static str, &[&str]); 5] = [
            ("BuildingClassInfo", &["BUILDINGCLASS_A"]),
            ("BuildingInfo", &["BUILDING_A"]),
            ("SpecialistInfo", &["SPECIALIST_A", "SPECIALIST_B"]),
            ("CivilizationInfo", &["CIVILIZATION_T"]),
            ("TechInfo", &["TECH_A"]),
        ];
        for (class, names) in classes {
            reg.register_class(class, names.iter().map(|s| s.to_string()).collect());
        }
        reg
    }

    /// Runs a reader on `xml` against [`registry`]; returns the result and the warnings.
    fn read<T>(xml: &str, f: fn(&Rd) -> T) -> (T, Vec<String>) {
        let reg = registry();
        let doc = XmlDoc::parse(xml).expect("test xml");
        let cx = Ctx::new(&reg, "test.xml");
        let v = f(&Rd::new(doc.root(), &cx));
        (v, cx.warnings.into_inner())
    }

    #[test]
    fn building_defaults_and_prereq_civ_quirk() {
        let xml = "<BuildingInfo><Type>BUILDING_A</Type><PrereqCiv>CIVILIZATION_T</PrereqCiv>\
                   <TechTypes><PrereqTech>TECH_A</PrereqTech></TechTypes></BuildingInfo>";
        let (b, w) = read(xml, read_building);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(b.prereq_civ, None);
        assert_eq!(b.prereq_civ_xml, Some(CivilizationType(0)));
        assert_eq!(b.max_latitude, 90);
        assert_eq!(b.prereq_and_techs, vec![Some(TechType(0)), None, None]);
        assert_eq!(b.prereq_or_bonuses, vec![None; 5]);
        assert_eq!(b.specialist_yield_change.raw, vec![[0; 3]; 2]);
        assert_eq!(b.yield_change, [0; 3]);
        assert_eq!(b.commerce_flexible, [false; 4]);
        assert!(!b.victory_building);
    }

    #[test]
    fn keyed_2d_lists_follow_the_cxx_loop() {
        let xml = r#"<BuildingInfo>
            <SpecialistYieldChanges>
                <Junk><SpecialistType>SPECIALIST_A</SpecialistType><YieldChanges><iYield>9</iYield></YieldChanges></Junk>
                <SpecialistYieldChange><SpecialistType>SPECIALIST_B</SpecialistType>
                    <YieldChanges><iYield>1</iYield><iYield>2</iYield></YieldChanges></SpecialistYieldChange>
                <SpecialistYieldChange><SpecialistType>SPECIALIST_NOPE</SpecialistType>
                    <YieldChanges><iYield>5</iYield></YieldChanges></SpecialistYieldChange>
                <SpecialistYieldChange><SpecialistType>SPECIALIST_A</SpecialistType></SpecialistYieldChange>
            </SpecialistYieldChanges>
            <SpecialistCommerceChanges>
                <SpecialistCommerceChange><SpecialistType>SPECIALIST_A</SpecialistType>
                    <CommerceChanges><iCommerce>0</iCommerce><iCommerce>3</iCommerce></CommerceChanges></SpecialistCommerceChange>
            </SpecialistCommerceChanges>
            <CommerceFlexibles><bFlexible>0</bFlexible><bFlexible>1</bFlexible></CommerceFlexibles>
        </BuildingInfo>"#;
        let (b, w) = read(xml, read_building);
        // Entries before the first <SpecialistYieldChange> are not visited; unknown keys
        // are skipped (with a warning); an entry without <YieldChanges> leaves zeros.
        assert_eq!(b.specialist_yield_change.raw, vec![[0, 0, 0], [1, 2, 0]]);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(b.specialist_commerce_change.raw, vec![[0, 3, 0, 0], [0; 4]]);
        assert_eq!(b.commerce_flexible, [false, true, false, false]);
    }

    #[test]
    fn building_class_and_special_building() {
        let xml = "<BuildingClassInfo><DefaultBuilding>BUILDING_A</DefaultBuilding>\
                   <iMaxPlayerInstances>-1</iMaxPlayerInstances></BuildingClassInfo>";
        let (c, w) = read(xml, read_building_class);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(c.default_building_index, Some(BuildingType(0)));
        // Absent instance limits read as 0 (C++ GetChildXmlValByName default), not -1.
        assert_eq!((c.max_global_instances, c.max_team_instances, c.max_player_instances), (0, 0, -1));
        let (c, _) = read("<BuildingClassInfo><DefaultBuilding>NONE</DefaultBuilding></BuildingClassInfo>", read_building_class);
        assert_eq!(c.default_building_index, None);
        let (s, w) = read("<SpecialBuildingInfo><TechPrereq>TECH_A</TechPrereq></SpecialBuildingInfo>", read_special_building);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(s.tech_prereq, Some(TechType(0)));
        assert!(!s.valid);
    }
}
