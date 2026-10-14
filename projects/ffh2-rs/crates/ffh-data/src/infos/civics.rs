//! Info classes: SpecialistInfo, ReligionInfo, CorporationInfo, CivicOptionInfo, CivicInfo, VoteInfo, VoteSourceInfo, EspionageMissionInfo, ProjectInfo, TechInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp (plus
//! `readPass2`/`readPass3`: the registry knows every type up front, so forward
//! references are resolved directly in the single read).
//!
//! Defaults follow C++: `GetChildXmlValByName` sets 0/false/"" when a tag is
//! absent (none of these classes pass an explicit default), a type tag that is
//! absent or `NONE` resolves to -1 (`None`), and every array is allocated by
//! `read()` (`InitList`/`SetYields`/`SetCommerce`/`SetVariableListTagPair`), so
//! the getters' NULL-array fallbacks never apply.
//!
//! Not ported (UI/art/audio only): `CvHotkeyInfo` fields of Specialist/Religion/
//! Corporation (hotkeys, order priority), specialist `Texture` and `bVisible`
//! (read only by the city screen), tech `Quote`/`Sound`/`SoundMP` and
//! `iGridX`/`iGridY` (tech-tree layout), religion `TechButton`/
//! `GenericTechButton`/`MovieFile`/`MovieSound`/`Sound`, corporation
//! `MovieFile`/`MovieSound`/`Sound`, project `MovieDefineTag`/`CreateSound`,
//! vote source `PopupText`/`SecretaryGeneralText` (popup and screen text only).
//! Members that `read()` does not set (`m_iMissionType`, `m_iChar`,
//! `m_iHolyCityChar`, `m_iHeadquarterChar`: action/font bookkeeping) are omitted too.

use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

// ---------------------------------------------------------------------------
// private XML helpers
// ---------------------------------------------------------------------------

/// C++ `SetCommerce` on a `bool*` (e.g. `CommerceFlexible`): positional flags,
/// all false when the tag is absent.
fn commerce_flags(rd: &Rd, tag: &str) -> [bool; CommerceType::NUM] {
    let v = rd.positional_bools(tag, CommerceType::NUM);
    std::array::from_fn(|i| v[i])
}

/// C++ `CvCivicInfo::read`, `ImprovementYieldChanges` block: `Init2DIntList` (all
/// zero), then each child's `ImprovementType` selects a row that is (re)read from
/// its `ImprovementYields` with `SetYields` (zeros if that tag is missing).
/// Children whose type is absent/`NONE` are skipped, as `FindInInfoClass` gives -1.
fn improvement_yield_changes(rd: &Rd, tag: &str) -> IdxVec<ImprovementType, [i32; YieldType::NUM]> {
    let n = rd.reg().count::<ImprovementType>();
    let mut out = IdxVec::from_elem([0; YieldType::NUM], n);
    if let Some(list) = rd.child(tag) {
        for entry in list.children() {
            if let Some(imp) = entry.rf::<ImprovementType>("ImprovementType") {
                out[imp] = entry.yields("ImprovementYields");
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// SpecialistInfo
// ---------------------------------------------------------------------------

/// C++ `CvSpecialistInfo` (Gameinfo/CIV4SpecialistInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct SpecialistInfo {
    pub base: InfoBase,
    /// `getGreatPeopleUnitClass()` (XML `GreatPeopleUnitClass`)
    pub great_people_unit_class: Option<UnitClassType>,
    pub great_people_rate_change: i32,
    pub experience: i32,
    /// `getYieldChange(i)` (XML `Yields`)
    pub yield_change: [i32; YieldType::NUM],
    /// `getCommerceChange(i)` (XML `Commerces`)
    pub commerce_change: [i32; CommerceType::NUM],
    /// `getFlavorValue(i)` (XML `Flavors`)
    pub flavor_value: IdxVec<FlavorType, i32>,
}

/// C++ `CvSpecialistInfo::read`.
pub fn read_specialist(rd: &Rd) -> SpecialistInfo {
    SpecialistInfo {
        base: InfoBase::read(rd),
        great_people_unit_class: rd.rf("GreatPeopleUnitClass"),
        great_people_rate_change: rd.int("iGreatPeopleRateChange"),
        yield_change: rd.yields("Yields"),
        commerce_change: rd.commerces("Commerces"),
        experience: rd.int("iExperience"),
        flavor_value: rd.pairs_int("Flavors", 0),
    }
}

// ---------------------------------------------------------------------------
// ReligionInfo
// ---------------------------------------------------------------------------

/// C++ `CvReligionInfo` (Gameinfo/CIV4ReligionInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct ReligionInfo {
    pub base: InfoBase,
    pub tech_prereq: Option<TechType>,
    pub free_unit_class: Option<UnitClassType>,
    /// `getNumFreeUnits()` (XML `iFreeUnits`)
    pub num_free_units: i32,
    pub spread_factor: i32,
    /// `getGlobalReligionCommerce(i)` (XML `GlobalReligionCommerces`)
    pub global_religion_commerce: [i32; CommerceType::NUM],
    /// `getHolyCityCommerce(i)` (XML `HolyCityCommerces`)
    pub holy_city_commerce: [i32; CommerceType::NUM],
    /// `getStateReligionCommerce(i)` (XML `StateReligionCommerces`)
    pub state_religion_commerce: [i32; CommerceType::NUM],
    /// `getAdjectiveKey()`: text key (XML `Adjective`), used in game messages.
    pub adjective_key: String,
    // FfH
    pub hidden: bool,
    pub sneak_attack: bool,
    pub update_sight: bool,
    pub global_counter_found: i32,
    pub global_counter_spread: i32,
    pub alignment: Option<AlignmentType>,
    pub alignment_best: Option<AlignmentType>,
    pub alignment_worst: Option<AlignmentType>,
    // Better AI (Sephi), part of the FFH2 0.41 DLL
    /// `getReligionHero1()`
    pub religion_hero1: Option<UnitClassType>,
    /// `getReligionHero2()`
    pub religion_hero2: Option<UnitClassType>,
    /// `getReligionTech1()`
    pub religion_tech1: Option<TechType>,
    /// `getReligionTech2()`
    pub religion_tech2: Option<TechType>,
}

/// C++ `CvReligionInfo::read`.
pub fn read_religion(rd: &Rd) -> ReligionInfo {
    ReligionInfo {
        base: InfoBase::read(rd),
        tech_prereq: rd.rf("TechPrereq"),
        free_unit_class: rd.rf("FreeUnitClass"),
        num_free_units: rd.int("iFreeUnits"),
        spread_factor: rd.int("iSpreadFactor"),
        global_religion_commerce: rd.commerces("GlobalReligionCommerces"),
        holy_city_commerce: rd.commerces("HolyCityCommerces"),
        state_religion_commerce: rd.commerces("StateReligionCommerces"),
        adjective_key: rd.string("Adjective"),
        hidden: rd.bool("bHidden"),
        sneak_attack: rd.bool("bSneakAttack"),
        update_sight: rd.bool("bUpdateSight"),
        global_counter_found: rd.int("iGlobalCounterFound"),
        global_counter_spread: rd.int("iGlobalCounterSpread"),
        alignment: rd.rf("Alignment"),
        alignment_best: rd.rf("AlignmentBest"),
        alignment_worst: rd.rf("AlignmentWorst"),
        religion_hero1: rd.rf("ReligionHero1"),
        religion_hero2: rd.rf("ReligionHero2"),
        religion_tech1: rd.rf("ReligionTech1"),
        religion_tech2: rd.rf("ReligionTech2"),
    }
}

// ---------------------------------------------------------------------------
// CorporationInfo (FFH2 ships an empty CIV4CorporationInfo.xml)
// ---------------------------------------------------------------------------

/// C++ `CvCorporationInfo` (Gameinfo/CIV4CorporationInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct CorporationInfo {
    pub base: InfoBase,
    pub tech_prereq: Option<TechType>,
    pub free_unit_class: Option<UnitClassType>,
    pub spread_factor: i32,
    pub spread_cost: i32,
    pub maintenance: i32,
    pub bonus_produced: Option<BonusType>,
    /// `getPrereqBonus(i)` (XML `PrereqBonuses`). C++ keeps a fixed array of
    /// `NUM_CORPORATION_PREREQ_BONUSES` entries filled with -1; this holds only the
    /// listed bonuses, in XML order, without the -1 (`NONE`) slots.
    // C++ quirk: without a `PrereqBonuses` tag the C++ array stays NULL and
    // getPrereqBonus() would crash; here the list is simply empty.
    pub prereq_bonus: Vec<BonusType>,
    /// `getHeadquarterCommerce(i)` (XML `HeadquarterCommerces`)
    pub headquarter_commerce: [i32; CommerceType::NUM],
    /// `getCommerceProduced(i)` (XML `CommercesProduced`)
    pub commerce_produced: [i32; CommerceType::NUM],
    /// `getYieldProduced(i)` (XML `YieldsProduced`)
    pub yield_produced: [i32; YieldType::NUM],
}

/// C++ `CvCorporationInfo::read`.
pub fn read_corporation(rd: &Rd) -> CorporationInfo {
    CorporationInfo {
        base: InfoBase::read(rd),
        tech_prereq: rd.rf("TechPrereq"),
        free_unit_class: rd.rf("FreeUnitClass"),
        spread_factor: rd.int("iSpreadFactor"),
        spread_cost: rd.int("iSpreadCost"),
        maintenance: rd.int("iMaintenance"),
        headquarter_commerce: rd.commerces("HeadquarterCommerces"),
        commerce_produced: rd.commerces("CommercesProduced"),
        yield_produced: rd.yields("YieldsProduced"),
        prereq_bonus: rd.ref_list("PrereqBonuses"),
        bonus_produced: rd.rf("BonusProduced"),
    }
}

// ---------------------------------------------------------------------------
// CivicOptionInfo
// ---------------------------------------------------------------------------

/// C++ `CvCivicOptionInfo` (Gameinfo/CIV4CivicOptionInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct CivicOptionInfo {
    pub base: InfoBase,
    /// `getTraitNoUpkeep(i)` (XML `TraitNoUpkeeps`)
    pub trait_no_upkeep: IdxVec<TraitType, bool>,
}

/// C++ `CvCivicOptionInfo::read`.
pub fn read_civic_option(rd: &Rd) -> CivicOptionInfo {
    CivicOptionInfo { base: InfoBase::read(rd), trait_no_upkeep: rd.pairs_bool("TraitNoUpkeeps") }
}

// ---------------------------------------------------------------------------
// CivicInfo
// ---------------------------------------------------------------------------

/// C++ `CvCivicInfo` (Gameinfo/CIV4CivicInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct CivicInfo {
    pub base: InfoBase,
    pub civic_option_type: Option<CivicOptionType>,
    pub anarchy_length: i32,
    /// `getUpkeep()`: an `UpkeepTypes` value (XML `Upkeep`).
    pub upkeep: Option<UpkeepType>,
    pub ai_weight: i32,
    pub great_people_rate_modifier: i32,
    pub great_general_rate_modifier: i32,
    pub domestic_great_general_rate_modifier: i32,
    pub state_religion_great_people_rate_modifier: i32,
    pub distance_maintenance_modifier: i32,
    pub num_cities_maintenance_modifier: i32,
    pub corporation_maintenance_modifier: i32,
    pub extra_health: i32,
    pub free_experience: i32,
    pub worker_speed_modifier: i32,
    pub improvement_upgrade_rate_modifier: i32,
    pub military_production_modifier: i32,
    pub base_free_units: i32,
    pub base_free_military_units: i32,
    pub free_units_population_percent: i32,
    pub free_military_units_population_percent: i32,
    pub gold_per_unit: i32,
    pub gold_per_military_unit: i32,
    pub happy_per_military_unit: i32,
    pub largest_city_happiness: i32,
    pub war_weariness_modifier: i32,
    pub free_specialist: i32,
    pub trade_routes: i32,
    pub tech_prereq: Option<TechType>,
    pub civic_percent_anger: i32,
    pub max_conscript: i32,
    pub state_religion_happiness: i32,
    pub non_state_religion_happiness: i32,
    pub state_religion_unit_production_modifier: i32,
    pub state_religion_building_production_modifier: i32,
    pub state_religion_free_experience: i32,
    pub exp_in_border_modifier: i32,
    pub military_food_production: bool,
    pub no_unhealthy_population: bool,
    pub building_only_healthy: bool,
    pub no_foreign_trade: bool,
    pub no_corporations: bool,
    pub no_foreign_corporations: bool,
    pub state_religion: bool,
    pub no_non_state_religion_spread: bool,
    /// `getWeLoveTheKing()`: text key of the celebration message ("" if none).
    pub we_love_the_king: String,
    // FfH
    pub hidden: bool,
    pub no_diplomacy_with_enemies: bool,
    pub prereq_war: bool,
    pub attitude_share_mod: i32,
    pub coastal_trade_routes: i32,
    pub enslavement_chance: i32,
    pub food_consumption_per_population: i32,
    pub block_alignment: Option<AlignmentType>,
    pub prereq_alignment: Option<AlignmentType>,
    pub prereq_civilization: Option<CivilizationType>,
    pub prereq_religion: Option<ReligionType>,
    // arrays
    /// `getYieldModifier(i)` (XML `YieldModifiers`)
    pub yield_modifier: [i32; YieldType::NUM],
    /// `getCapitalYieldModifier(i)` (XML `CapitalYieldModifiers`)
    pub capital_yield_modifier: [i32; YieldType::NUM],
    /// `getTradeYieldModifier(i)` (XML `TradeYieldModifiers`)
    pub trade_yield_modifier: [i32; YieldType::NUM],
    /// `getCommerceModifier(i)` (XML `CommerceModifiers`)
    pub commerce_modifier: [i32; CommerceType::NUM],
    /// `getCapitalCommerceModifier(i)` (XML `CapitalCommerceModifiers`)
    pub capital_commerce_modifier: [i32; CommerceType::NUM],
    /// `getSpecialistExtraCommerce(i)` (XML `SpecialistExtraCommerces`)
    pub specialist_extra_commerce: [i32; CommerceType::NUM],
    /// `getBuildingHappinessChanges(i)`
    pub building_happiness_changes: IdxVec<BuildingClassType, i32>,
    /// `getBuildingHealthChanges(i)`
    pub building_health_changes: IdxVec<BuildingClassType, i32>,
    /// `getFeatureHappinessChanges(i)`
    pub feature_happiness_changes: IdxVec<FeatureType, i32>,
    /// `isHurry(i)` (XML `Hurrys`)
    pub hurry: IdxVec<HurryType, bool>,
    /// `isSpecialBuildingNotRequired(i)` (XML `SpecialBuildingNotRequireds`)
    pub special_building_not_required: IdxVec<SpecialBuildingType, bool>,
    /// `isSpecialistValid(i)` (XML `SpecialistValids`)
    pub specialist_valid: IdxVec<SpecialistType, bool>,
    /// `getImprovementYieldChanges(i, j)`: per improvement, per yield.
    pub improvement_yield_changes: IdxVec<ImprovementType, [i32; YieldType::NUM]>,
}

/// C++ `CvCivicInfo::read` + `readPass3` (`PrereqCivilization`).
pub fn read_civic(rd: &Rd) -> CivicInfo {
    CivicInfo {
        base: InfoBase::read(rd),
        civic_option_type: rd.rf("CivicOptionType"),
        tech_prereq: rd.rf("TechPrereq"),
        anarchy_length: rd.int("iAnarchyLength"),
        upkeep: rd.rf("Upkeep"),
        ai_weight: rd.int("iAIWeight"),
        great_people_rate_modifier: rd.int("iGreatPeopleRateModifier"),
        great_general_rate_modifier: rd.int("iGreatGeneralRateModifier"),
        domestic_great_general_rate_modifier: rd.int("iDomesticGreatGeneralRateModifier"),
        state_religion_great_people_rate_modifier: rd.int("iStateReligionGreatPeopleRateModifier"),
        distance_maintenance_modifier: rd.int("iDistanceMaintenanceModifier"),
        num_cities_maintenance_modifier: rd.int("iNumCitiesMaintenanceModifier"),
        corporation_maintenance_modifier: rd.int("iCorporationMaintenanceModifier"),
        extra_health: rd.int("iExtraHealth"),
        free_experience: rd.int("iFreeExperience"),
        worker_speed_modifier: rd.int("iWorkerSpeedModifier"),
        improvement_upgrade_rate_modifier: rd.int("iImprovementUpgradeRateModifier"),
        military_production_modifier: rd.int("iMilitaryProductionModifier"),
        base_free_units: rd.int("iBaseFreeUnits"),
        base_free_military_units: rd.int("iBaseFreeMilitaryUnits"),
        free_units_population_percent: rd.int("iFreeUnitsPopulationPercent"),
        free_military_units_population_percent: rd.int("iFreeMilitaryUnitsPopulationPercent"),
        gold_per_unit: rd.int("iGoldPerUnit"),
        gold_per_military_unit: rd.int("iGoldPerMilitaryUnit"),
        happy_per_military_unit: rd.int("iHappyPerMilitaryUnit"),
        military_food_production: rd.bool("bMilitaryFoodProduction"),
        max_conscript: rd.int("iMaxConscript"),
        no_unhealthy_population: rd.bool("bNoUnhealthyPopulation"),
        building_only_healthy: rd.bool("bBuildingOnlyHealthy"),
        largest_city_happiness: rd.int("iLargestCityHappiness"),
        war_weariness_modifier: rd.int("iWarWearinessModifier"),
        free_specialist: rd.int("iFreeSpecialist"),
        trade_routes: rd.int("iTradeRoutes"),
        no_foreign_trade: rd.bool("bNoForeignTrade"),
        no_corporations: rd.bool("bNoCorporations"),
        no_foreign_corporations: rd.bool("bNoForeignCorporations"),
        civic_percent_anger: rd.int("iCivicPercentAnger"),
        state_religion: rd.bool("bStateReligion"),
        no_non_state_religion_spread: rd.bool("bNoNonStateReligionSpread"),
        state_religion_happiness: rd.int("iStateReligionHappiness"),
        non_state_religion_happiness: rd.int("iNonStateReligionHappiness"),
        state_religion_unit_production_modifier: rd.int("iStateReligionUnitProductionModifier"),
        state_religion_building_production_modifier: rd.int("iStateReligionBuildingProductionModifier"),
        state_religion_free_experience: rd.int("iStateReligionFreeExperience"),
        exp_in_border_modifier: rd.int("iExpInBorderModifier"),
        yield_modifier: rd.yields("YieldModifiers"),
        capital_yield_modifier: rd.yields("CapitalYieldModifiers"),
        trade_yield_modifier: rd.yields("TradeYieldModifiers"),
        commerce_modifier: rd.commerces("CommerceModifiers"),
        capital_commerce_modifier: rd.commerces("CapitalCommerceModifiers"),
        specialist_extra_commerce: rd.commerces("SpecialistExtraCommerces"),
        hurry: rd.pairs_bool("Hurrys"),
        special_building_not_required: rd.pairs_bool("SpecialBuildingNotRequireds"),
        specialist_valid: rd.pairs_bool("SpecialistValids"),
        building_happiness_changes: rd.pairs_int("BuildingHappinessChanges", 0),
        building_health_changes: rd.pairs_int("BuildingHealthChanges", 0),
        feature_happiness_changes: rd.pairs_int("FeatureHappinessChanges", 0),
        improvement_yield_changes: improvement_yield_changes(rd, "ImprovementYieldChanges"),
        we_love_the_king: rd.string("WeLoveTheKing"),
        hidden: rd.bool("bHidden"),
        no_diplomacy_with_enemies: rd.bool("bNoDiplomacyWithEnemies"),
        prereq_war: rd.bool("bPrereqWar"),
        attitude_share_mod: rd.int("iAttitudeShareMod"),
        coastal_trade_routes: rd.int("iCoastalTradeRoutes"),
        enslavement_chance: rd.int("iEnslavementChance"),
        food_consumption_per_population: rd.int("iFoodConsumptionPerPopulation"),
        block_alignment: rd.rf("BlockAlignment"),
        prereq_alignment: rd.rf("PrereqAlignment"),
        // readPass3 in C++ (civilizations load after civics)
        prereq_civilization: rd.rf("PrereqCivilization"),
        prereq_religion: rd.rf("PrereqReligion"),
    }
}

// ---------------------------------------------------------------------------
// VoteInfo
// ---------------------------------------------------------------------------

/// C++ `CvVoteInfo` (Gameinfo/CIV4VoteInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct VoteInfo {
    pub base: InfoBase,
    pub population_threshold: i32,
    pub state_religion_vote_percent: i32,
    pub trade_routes: i32,
    pub min_voters: i32,
    pub secretary_general: bool,
    pub victory: bool,
    pub free_trade: bool,
    pub no_nukes: bool,
    pub city_voting: bool,
    pub civ_voting: bool,
    pub defensive_pact: bool,
    pub open_borders: bool,
    pub force_peace: bool,
    pub force_no_trade: bool,
    pub force_war: bool,
    pub assign_city: bool,
    /// `isForceCivic(i)` (XML `ForceCivics`)
    pub force_civic: IdxVec<CivicType, bool>,
    /// `isVoteSourceType(i)`: vote sources that can hold this vote (XML `DiploVotes`).
    pub vote_source_type: IdxVec<VoteSourceType, bool>,
    // FfH
    pub gambling_ring: bool,
    pub no_outside_tech_trades: bool,
    pub slave_trade: bool,
    pub smuggling_ring: bool,
    pub cost: i32,
    pub crime: i32,
    pub free_units: i32,
    pub free_unit_class: Option<UnitClassType>,
    /// `getNoBonus()`: bonus banned by this resolution.
    pub no_bonus: Option<BonusType>,
    /// `getPyResult()`: Python callback run when the vote passes ("" if none).
    pub py_result: String,
}

/// C++ `CvVoteInfo::read`.
pub fn read_vote(rd: &Rd) -> VoteInfo {
    VoteInfo {
        base: InfoBase::read(rd),
        population_threshold: rd.int("iPopulationThreshold"),
        state_religion_vote_percent: rd.int("iStateReligionVotePercent"),
        trade_routes: rd.int("iTradeRoutes"),
        min_voters: rd.int("iMinVoters"),
        secretary_general: rd.bool("bSecretaryGeneral"),
        victory: rd.bool("bVictory"),
        free_trade: rd.bool("bFreeTrade"),
        no_nukes: rd.bool("bNoNukes"),
        city_voting: rd.bool("bCityVoting"),
        civ_voting: rd.bool("bCivVoting"),
        defensive_pact: rd.bool("bDefensivePact"),
        open_borders: rd.bool("bOpenBorders"),
        force_peace: rd.bool("bForcePeace"),
        force_no_trade: rd.bool("bForceNoTrade"),
        force_war: rd.bool("bForceWar"),
        assign_city: rd.bool("bAssignCity"),
        force_civic: rd.pairs_bool("ForceCivics"),
        vote_source_type: rd.pairs_bool("DiploVotes"),
        gambling_ring: rd.bool("bGamblingRing"),
        no_outside_tech_trades: rd.bool("bNoOutsideTechTrades"),
        slave_trade: rd.bool("bSlaveTrade"),
        smuggling_ring: rd.bool("bSmugglingRing"),
        cost: rd.int("iCost"),
        crime: rd.int("iCrime"),
        free_units: rd.int("iFreeUnits"),
        free_unit_class: rd.rf("FreeUnitClass"),
        no_bonus: rd.rf("NoBonus"),
        py_result: rd.string("PyResult"),
    }
}

// ---------------------------------------------------------------------------
// VoteSourceInfo
// ---------------------------------------------------------------------------

/// C++ `CvVoteSourceInfo` (Gameinfo/CIV4VoteSourceInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct VoteSourceInfo {
    pub base: InfoBase,
    pub vote_interval: i32,
    pub free_specialist: Option<SpecialistType>,
    pub civic: Option<CivicType>,
    /// `getReligionYield(i)` (XML `ReligionYields`)
    pub religion_yield: [i32; YieldType::NUM],
    /// `getReligionCommerce(i)` (XML `ReligionCommerces`)
    pub religion_commerce: [i32; CommerceType::NUM],
}

/// C++ `CvVoteSourceInfo::read` + `readPass3` (`Civic`).
pub fn read_vote_source(rd: &Rd) -> VoteSourceInfo {
    // C++ quirk: ReligionYields is read with SetCommerce, which allocates
    // NUM_COMMERCE_TYPES (4) slots; getReligionYield() only ever reads the first
    // NUM_YIELD_TYPES (3), so a 4th XML value is accepted silently and ignored.
    let ry = rd.positional_ints("ReligionYields", CommerceType::NUM);
    VoteSourceInfo {
        base: InfoBase::read(rd),
        vote_interval: rd.int("iVoteInterval"),
        free_specialist: rd.rf("FreeSpecialist"),
        // readPass3 in C++ (civics load after vote sources)
        civic: rd.rf("Civic"),
        religion_yield: std::array::from_fn(|i| ry[i]),
        religion_commerce: rd.commerces("ReligionCommerces"),
    }
}

// ---------------------------------------------------------------------------
// EspionageMissionInfo
// ---------------------------------------------------------------------------

/// C++ `CvEspionageMissionInfo` (Gameinfo/CIV4EspionageMissionInfo.xml)
///
/// The C++ constructor leaves every member uninitialized; `read()` assigns all of
/// them (0/false when absent, -1 for `TechPrereq`).
#[derive(Debug, Clone, Default)]
pub struct EspionageMissionInfo {
    pub base: InfoBase,
    pub cost: i32,
    /// `isPassive()` (XML `bIsPassive`)
    pub passive: bool,
    /// `isTwoPhases()` (XML `bIsTwoPhases`)
    pub two_phases: bool,
    pub targets_city: bool,
    pub select_plot: bool,
    pub tech_prereq: Option<TechType>,
    pub visibility_level: i32,
    pub investigate_city: bool,
    pub see_demographics: bool,
    pub no_active_missions: bool,
    pub see_research: bool,
    pub destroy_improvement: bool,
    pub destroy_building_cost_factor: i32,
    pub destroy_unit_cost_factor: i32,
    pub destroy_project_cost_factor: i32,
    pub destroy_production_cost_factor: i32,
    pub buy_unit_cost_factor: i32,
    pub buy_city_cost_factor: i32,
    pub steal_treasury_types: i32,
    pub city_insert_culture_amount_factor: i32,
    pub city_insert_culture_cost_factor: i32,
    pub city_poison_water_counter: i32,
    pub city_unhappiness_counter: i32,
    pub city_revolt_counter: i32,
    pub buy_tech_cost_factor: i32,
    pub switch_civic_cost_factor: i32,
    pub switch_religion_cost_factor: i32,
    pub player_anarchy_counter: i32,
    pub counterespionage_num_turns: i32,
    pub counterespionage_mod: i32,
    pub difficulty_mod: i32,
}

/// C++ `CvEspionageMissionInfo::read`.
pub fn read_espionage_mission(rd: &Rd) -> EspionageMissionInfo {
    EspionageMissionInfo {
        base: InfoBase::read(rd),
        cost: rd.int("iCost"),
        passive: rd.bool("bIsPassive"),
        two_phases: rd.bool("bIsTwoPhases"),
        targets_city: rd.bool("bTargetsCity"),
        select_plot: rd.bool("bSelectPlot"),
        tech_prereq: rd.rf("TechPrereq"),
        visibility_level: rd.int("iVisibilityLevel"),
        investigate_city: rd.bool("bInvestigateCity"),
        see_demographics: rd.bool("bSeeDemographics"),
        no_active_missions: rd.bool("bNoActiveMissions"),
        see_research: rd.bool("bSeeResearch"),
        destroy_improvement: rd.bool("bDestroyImprovement"),
        destroy_building_cost_factor: rd.int("iDestroyBuildingCostFactor"),
        destroy_unit_cost_factor: rd.int("iDestroyUnitCostFactor"),
        destroy_project_cost_factor: rd.int("iDestroyProjectCostFactor"),
        destroy_production_cost_factor: rd.int("iDestroyProductionCostFactor"),
        buy_unit_cost_factor: rd.int("iBuyUnitCostFactor"),
        buy_city_cost_factor: rd.int("iBuyCityCostFactor"),
        steal_treasury_types: rd.int("iStealTreasuryTypes"),
        city_insert_culture_amount_factor: rd.int("iCityInsertCultureAmountFactor"),
        city_insert_culture_cost_factor: rd.int("iCityInsertCultureCostFactor"),
        city_poison_water_counter: rd.int("iCityPoisonWaterCounter"),
        city_unhappiness_counter: rd.int("iCityUnhappinessCounter"),
        city_revolt_counter: rd.int("iCityRevoltCounter"),
        buy_tech_cost_factor: rd.int("iBuyTechCostFactor"),
        switch_civic_cost_factor: rd.int("iSwitchCivicCostFactor"),
        switch_religion_cost_factor: rd.int("iSwitchReligionCostFactor"),
        player_anarchy_counter: rd.int("iPlayerAnarchyCounter"),
        counterespionage_num_turns: rd.int("iCounterespionageNumTurns"),
        counterespionage_mod: rd.int("iCounterespionageMod"),
        difficulty_mod: rd.int("iDifficultyMod"),
    }
}

// ---------------------------------------------------------------------------
// ProjectInfo
// ---------------------------------------------------------------------------

/// C++ `CvProjectInfo` (Gameinfo/CIV4ProjectInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct ProjectInfo {
    pub base: InfoBase,
    pub victory_prereq: Option<VictoryType>,
    pub tech_prereq: Option<TechType>,
    /// `getAnyoneProjectPrereq()` (XML `AnyonePrereqProject`)
    pub anyone_project_prereq: Option<ProjectType>,
    /// -1 = unlimited. C++ passes no default, so an absent tag gives 0; the XML
    /// always writes -1 explicitly.
    pub max_global_instances: i32,
    /// Same convention as `max_global_instances`.
    pub max_team_instances: i32,
    /// `getProductionCost()` (XML `iCost`)
    pub production_cost: i32,
    pub nuke_interception: i32,
    pub tech_share: i32,
    pub everyone_special_unit: Option<SpecialUnitType>,
    pub everyone_special_building: Option<SpecialBuildingType>,
    pub victory_delay_percent: i32,
    pub success_rate: i32,
    pub spaceship: bool,
    pub allows_nukes: bool,
    // FfH
    pub ai_weight: i32,
    pub modify_global_counter: i32,
    pub prereq_civilization: Option<CivilizationType>,
    pub prereq_global_counter: i32,
    // arrays
    /// `getBonusProductionModifier(i)` (XML `BonusProductionModifiers`)
    pub bonus_production_modifier: IdxVec<BonusType, i32>,
    /// `getVictoryThreshold(i)` (XML `VictoryThresholds`)
    pub victory_threshold: IdxVec<VictoryType, i32>,
    /// `getVictoryMinThreshold(i)`: the value that getter returns, i.e. the XML
    /// `VictoryMinThresholds` entry, or `victory_threshold[i]` where that entry is 0.
    pub victory_min_threshold: IdxVec<VictoryType, i32>,
    /// `getProjectsNeeded(i)`: count of each project required (XML `PrereqProjects`).
    pub projects_needed: IdxVec<ProjectType, i32>,
}

/// C++ `CvProjectInfo::read` + `readPass2` (`PrereqProjects`, `AnyonePrereqProject`)
/// + `readPass3` (`PrereqCivilization`).
pub fn read_project(rd: &Rd) -> ProjectInfo {
    let victory_threshold = rd.pairs_int::<VictoryType>("VictoryThresholds", 0);
    let min_raw = rd.pairs_int::<VictoryType>("VictoryMinThresholds", 0);
    // getVictoryMinThreshold() falls back to getVictoryThreshold() for 0 entries.
    let victory_min_threshold =
        IdxVec::from_fn(min_raw.len(), |v| if min_raw[v] != 0 { min_raw[v] } else { victory_threshold[v] });
    ProjectInfo {
        base: InfoBase::read(rd),
        victory_prereq: rd.rf("VictoryPrereq"),
        tech_prereq: rd.rf("TechPrereq"),
        max_global_instances: rd.int("iMaxGlobalInstances"),
        max_team_instances: rd.int("iMaxTeamInstances"),
        production_cost: rd.int("iCost"),
        nuke_interception: rd.int("iNukeInterception"),
        tech_share: rd.int("iTechShare"),
        everyone_special_unit: rd.rf("EveryoneSpecialUnit"),
        everyone_special_building: rd.rf("EveryoneSpecialBuilding"),
        spaceship: rd.bool("bSpaceship"),
        allows_nukes: rd.bool("bAllowsNukes"),
        bonus_production_modifier: rd.pairs_int("BonusProductionModifiers", 0),
        victory_threshold,
        victory_min_threshold,
        victory_delay_percent: rd.int("iVictoryDelayPercent"),
        success_rate: rd.int("iSuccessRate"),
        ai_weight: rd.int("iAIWeight"),
        modify_global_counter: rd.int("iModifyGlobalCounter"),
        prereq_civilization: rd.rf("PrereqCivilization"),
        prereq_global_counter: rd.int("iPrereqGlobalCounter"),
        // readPass2 in C++ (references other projects)
        projects_needed: rd.pairs_int("PrereqProjects", 0),
        anyone_project_prereq: rd.rf("AnyonePrereqProject"),
    }
}

// ---------------------------------------------------------------------------
// TechInfo
// ---------------------------------------------------------------------------

/// C++ `CvTechInfo` (Technologies/CIV4TechInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct TechInfo {
    pub base: InfoBase,
    /// `getAdvisorType()` as the raw `<Advisor>` type name (e.g. `ADVISOR_SCIENCE`).
    /// AdvisorInfos (BTS Interface/CIV4AdvisorInfos.xml) are not vendored or
    /// registered, so it is not resolved to an index; C++ reads it only in
    /// `CvPlayerAI::AI_bestTech`'s `eIgnoreAdvisor` filter, which only the
    /// tech-choice popup sets.
    pub advisor_type: String,
    pub ai_weight: i32,
    pub ai_trade_modifier: i32,
    /// `getResearchCost()` (XML `iCost`)
    pub research_cost: i32,
    pub advanced_start_cost: i32,
    pub advanced_start_cost_increase: i32,
    pub era: Option<EraType>,
    pub trade_routes: i32,
    pub feature_production_modifier: i32,
    pub worker_speed_modifier: i32,
    pub first_free_unit_class: Option<UnitClassType>,
    pub health: i32,
    pub happiness: i32,
    pub first_free_techs: i32,
    /// `getAssetValue()` (XML `iAsset`)
    pub asset_value: i32,
    /// `getPowerValue()` (XML `iPower`)
    pub power_value: i32,
    pub repeat: bool,
    pub trade: bool,
    pub disable: bool,
    pub goody_tech: bool,
    pub extra_water_see_from: bool,
    pub map_centering: bool,
    pub map_visible: bool,
    pub map_trading: bool,
    pub tech_trading: bool,
    pub gold_trading: bool,
    pub open_borders_trading: bool,
    pub defensive_pact_trading: bool,
    pub permanent_alliance_trading: bool,
    /// `isVassalStateTrading()` (XML `bVassalTrading`)
    pub vassal_state_trading: bool,
    pub bridge_building: bool,
    pub irrigation: bool,
    pub ignore_irrigation: bool,
    pub water_work: bool,
    pub river_trade: bool,
    // FfH
    pub water: bool,
    pub preferred_alignment: Option<AlignmentType>,
    pub prereq_religion: Option<ReligionType>,
    // arrays
    /// `getDomainExtraMoves(i)`
    pub domain_extra_moves: IdxVec<DomainType, i32>,
    /// `getFlavorValue(i)` (XML `Flavors`)
    pub flavor_value: IdxVec<FlavorType, i32>,
    /// `getPrereqOrTechs(i)` (XML `OrPreReqs`): any one of these is required.
    /// C++ keeps a fixed array of `NUM_OR_TECH_PREREQS` entries filled with -1
    /// (NULL when the tag is absent); this holds only the listed techs, in XML
    /// order, without the -1 (`NONE`) slots.
    pub prereq_or_techs: Vec<TechType>,
    /// `getPrereqAndTechs(i)` (XML `AndPreReqs`): all of these are required.
    /// Same representation as `prereq_or_techs` (C++ array size `NUM_AND_TECH_PREREQS`).
    pub prereq_and_techs: Vec<TechType>,
    /// `isCommerceFlexible(i)` (XML `CommerceFlexible`)
    pub commerce_flexible: [bool; CommerceType::NUM],
    /// `isTerrainTrade(i)` (XML `TerrainTrades`)
    pub terrain_trade: IdxVec<TerrainType, bool>,
}

/// C++ `CvTechInfo::read` + `readPass2` (`OrPreReqs`, `AndPreReqs`) + `readPass3`
/// (`PrereqReligion`).
pub fn read_tech(rd: &Rd) -> TechInfo {
    TechInfo {
        base: InfoBase::read(rd),
        advisor_type: rd.string("Advisor"),
        ai_weight: rd.int("iAIWeight"),
        ai_trade_modifier: rd.int("iAITradeModifier"),
        research_cost: rd.int("iCost"),
        advanced_start_cost: rd.int("iAdvancedStartCost"),
        advanced_start_cost_increase: rd.int("iAdvancedStartCostIncrease"),
        era: rd.rf("Era"),
        first_free_unit_class: rd.rf("FirstFreeUnitClass"),
        feature_production_modifier: rd.int("iFeatureProductionModifier"),
        worker_speed_modifier: rd.int("iWorkerSpeedModifier"),
        trade_routes: rd.int("iTradeRoutes"),
        health: rd.int("iHealth"),
        happiness: rd.int("iHappiness"),
        first_free_techs: rd.int("iFirstFreeTechs"),
        asset_value: rd.int("iAsset"),
        power_value: rd.int("iPower"),
        repeat: rd.bool("bRepeat"),
        trade: rd.bool("bTrade"),
        disable: rd.bool("bDisable"),
        goody_tech: rd.bool("bGoodyTech"),
        extra_water_see_from: rd.bool("bExtraWaterSeeFrom"),
        map_centering: rd.bool("bMapCentering"),
        map_visible: rd.bool("bMapVisible"),
        map_trading: rd.bool("bMapTrading"),
        tech_trading: rd.bool("bTechTrading"),
        gold_trading: rd.bool("bGoldTrading"),
        open_borders_trading: rd.bool("bOpenBordersTrading"),
        defensive_pact_trading: rd.bool("bDefensivePactTrading"),
        permanent_alliance_trading: rd.bool("bPermanentAllianceTrading"),
        vassal_state_trading: rd.bool("bVassalTrading"),
        bridge_building: rd.bool("bBridgeBuilding"),
        irrigation: rd.bool("bIrrigation"),
        ignore_irrigation: rd.bool("bIgnoreIrrigation"),
        water_work: rd.bool("bWaterWork"),
        river_trade: rd.bool("bRiverTrade"),
        commerce_flexible: commerce_flags(rd, "CommerceFlexible"),
        domain_extra_moves: rd.pairs_int("DomainExtraMoves", 0),
        terrain_trade: rd.pairs_bool("TerrainTrades"),
        flavor_value: rd.pairs_int("Flavors", 0),
        water: rd.bool("bWater"),
        preferred_alignment: rd.rf("PreferredAlignment"),
        // readPass3 in C++ (religions load after techs)
        prereq_religion: rd.rf("PrereqReligion"),
        // readPass2 in C++ (references other techs)
        prereq_or_techs: rd.ref_list("OrPreReqs"),
        prereq_and_techs: rd.ref_list("AndPreReqs"),
    }
}

#[cfg(test)]
mod tests {
    //! Synthetic-XML checks of read paths the shipped data does not exercise.

    use super::*;
    use crate::idx::Idx;
    use crate::registry::{Ctx, Registry};
    use crate::xml::XmlDoc;

    fn registry() -> Registry {
        let mut reg = Registry::new();
        let mut add = |class: &'static str, names: &[&str]| {
            reg.register_class(class, names.iter().map(|s| s.to_string()).collect());
        };
        add("TechInfo", &["TECH_A", "TECH_B", "TECH_C"]);
        add("VictoryInfo", &["VICTORY_X", "VICTORY_Y", "VICTORY_Z"]);
        add("ImprovementInfo", &["IMPROVEMENT_FARM", "IMPROVEMENT_MINE", "IMPROVEMENT_QUARRY"]);
        add("FlavorTypes", &["FLAVOR_MILITARY", "FLAVOR_SCIENCE"]);
        reg
    }

    /// Runs `read` on one entry element and returns the info plus load warnings.
    fn read_with<T>(xml: &str, read: fn(&Rd) -> T) -> (T, Vec<String>) {
        let reg = registry();
        let doc = XmlDoc::parse(xml).expect("test xml parses");
        let cx = Ctx::new(&reg, "test.xml");
        let info = read(&Rd::new(doc.root(), &cx));
        (info, cx.warnings.into_inner())
    }

    #[test]
    fn absent_tags_take_cpp_defaults() {
        let (t, w) = read_with("<TechInfo><Type>TECH_C</Type></TechInfo>", read_tech);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(t.base.type_, "TECH_C");
        assert_eq!((t.research_cost, t.ai_weight, t.era, t.prereq_religion), (0, 0, None, None));
        assert!(t.prereq_or_techs.is_empty() && t.prereq_and_techs.is_empty());
        assert_eq!(t.commerce_flexible, [false; CommerceType::NUM]);
        assert_eq!(t.flavor_value.raw, vec![0, 0]);
        assert_eq!(t.advisor_type, "");

        let (p, w) = read_with("<ProjectInfo><Type>PROJECT_P</Type></ProjectInfo>", read_project);
        assert!(w.is_empty(), "{w:?}");
        // No explicit GetChildXmlValByName default: absent means 0, not -1 (unlimited).
        assert_eq!((p.max_global_instances, p.max_team_instances), (0, 0));
        assert_eq!(p.anyone_project_prereq, None);
    }

    #[test]
    fn tech_prereq_lists_keep_order_and_skip_none() {
        let xml = "<TechInfo><Type>TECH_C</Type>
            <OrPreReqs><PrereqTech>TECH_B</PrereqTech><PrereqTech>NONE</PrereqTech><PrereqTech>TECH_A</PrereqTech></OrPreReqs>
            <AndPreReqs><PrereqTech>TECH_A</PrereqTech></AndPreReqs>
            <CommerceFlexible><bFlexible>0</bFlexible><bFlexible>1</bFlexible></CommerceFlexible>
            <Flavors><Flavor><FlavorType>FLAVOR_SCIENCE</FlavorType><iFlavor>7</iFlavor></Flavor></Flavors>
        </TechInfo>";
        let (t, w) = read_with(xml, read_tech);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(t.prereq_or_techs, vec![TechType(1), TechType(0)]);
        assert_eq!(t.prereq_and_techs, vec![TechType(0)]);
        assert_eq!(t.commerce_flexible, [false, true, false, false]);
        assert_eq!(t.flavor_value.raw, vec![0, 7]);
    }

    #[test]
    fn victory_min_threshold_falls_back_to_threshold() {
        let xml = "<ProjectInfo><Type>PROJECT_P</Type>
            <VictoryThresholds>
                <VictoryThreshold><VictoryType>VICTORY_X</VictoryType><iThreshold>10</iThreshold></VictoryThreshold>
                <VictoryThreshold><VictoryType>VICTORY_Y</VictoryType><iThreshold>20</iThreshold></VictoryThreshold>
            </VictoryThresholds>
            <VictoryMinThresholds>
                <VictoryMinThreshold><VictoryType>VICTORY_Y</VictoryType><iThreshold>5</iThreshold></VictoryMinThreshold>
            </VictoryMinThresholds>
        </ProjectInfo>";
        let (p, w) = read_with(xml, read_project);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(p.victory_threshold.raw, vec![10, 20, 0]);
        // getVictoryMinThreshold(i): the min entry if non-zero, else getVictoryThreshold(i)
        assert_eq!(p.victory_min_threshold.raw, vec![10, 5, 0]);
    }

    #[test]
    fn vote_source_religion_yields_use_commerce_reader() {
        // Four values: SetCommerce accepts them silently; only three yields are kept.
        let xml = "<VoteSourceInfo><Type>DIPLOVOTE_T</Type>
            <ReligionYields><iYield>1</iYield><iYield>2</iYield><iYield>3</iYield><iYield>4</iYield></ReligionYields>
            <ReligionCommerces><iCommerce>5</iCommerce></ReligionCommerces>
        </VoteSourceInfo>";
        let (v, w) = read_with(xml, read_vote_source);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(v.religion_yield, [1, 2, 3]);
        assert_eq!(v.religion_commerce, [5, 0, 0, 0]);
        assert_eq!((v.civic, v.free_specialist), (None, None));
    }

    #[test]
    fn civic_improvement_yield_changes() {
        let xml = "<CivicInfo><Type>CIVIC_T</Type>
            <ImprovementYieldChanges>
                <ImprovementYieldChange>
                    <ImprovementType>IMPROVEMENT_MINE</ImprovementType>
                    <ImprovementYields><iYield>0</iYield><iYield>2</iYield></ImprovementYields>
                </ImprovementYieldChange>
                <ImprovementYieldChange>
                    <ImprovementType>NONE</ImprovementType>
                    <ImprovementYields><iYield>9</iYield></ImprovementYields>
                </ImprovementYieldChange>
                <ImprovementYieldChange>
                    <ImprovementType>IMPROVEMENT_QUARRY</ImprovementType>
                    <ImprovementYields><iYield>4</iYield></ImprovementYields>
                </ImprovementYieldChange>
                <ImprovementYieldChange>
                    <ImprovementType>IMPROVEMENT_QUARRY</ImprovementType>
                </ImprovementYieldChange>
            </ImprovementYieldChanges>
        </CivicInfo>";
        let (c, w) = read_with(xml, read_civic);
        assert!(w.is_empty(), "{w:?}");
        let row = |i: usize| c.improvement_yield_changes[ImprovementType::from_usize(i)];
        assert_eq!(c.improvement_yield_changes.len(), 3);
        assert_eq!(row(0), [0, 0, 0]);
        assert_eq!(row(1), [0, 2, 0]);
        // A repeated row without ImprovementYields is re-initialized to zeros (C++ InitList).
        assert_eq!(row(2), [0, 0, 0]);
    }
}
