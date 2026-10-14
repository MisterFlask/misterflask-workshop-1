//! Info classes: TraitInfo, CivilizationInfo, LeaderHeadInfo.
//! Port of `CvTraitInfo::read`, `CvCivilizationInfo::read` (+ `readPass2`) and
//! `CvLeaderHeadInfo::read` in CvInfos.cpp.
//!
//! Not ported (graphics/audio/UI only): civilization `DefaultPlayerColor`, `ArtDefineTag`,
//! `ArtStyleType`, `UnitArtStyleType` and selection/action sounds; leader `ArtDefineTag`
//! and the four diplomacy music lists.

use crate::enums::*;
use crate::idx::{opt_from_i32, Idx, IdxVec};
use crate::infos::{BuildingClassInfo, InfoBase};
use crate::registry::{InfoId, Rd};

/// C++ `CvTraitInfo` (Civilizations/CIV4TraitInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct TraitInfo {
    pub base: InfoBase,
    /// C++ `getShortDescription()` (text key).
    pub short_description: String,
    pub health: i32,
    pub happiness: i32,
    pub max_anarchy: i32,
    pub upkeep_modifier: i32,
    pub level_experience_modifier: i32,
    pub great_people_rate_modifier: i32,
    pub great_general_rate_modifier: i32,
    pub domestic_great_general_rate_modifier: i32,
    pub max_global_building_production_modifier: i32,
    pub max_team_building_production_modifier: i32,
    pub max_player_building_production_modifier: i32,
    // ---- FfH ----
    pub adaptive: bool,
    pub agnostic: bool,
    pub assimilation: bool,
    pub barbarian_ally: bool,
    pub ignore_food: bool,
    pub insane: bool,
    pub selectable: bool,
    pub sprawling: bool,
    /// C++ `getFreeXPFromCombat()`.
    pub free_xp_from_combat: i32,
    /// -1 = no limit. 0 if the tag is absent: the constructor's -1 is overwritten by
    /// `GetChildXmlValByName`'s default (every FFH2 trait sets the tag).
    pub max_cities: i32,
    pub pillaging_gold: i32,
    pub starting_gold: i32,
    pub summon_duration: i32,
    pub upgrade_cost_modifier: i32,
    // ---- arrays ----
    pub extra_yield_threshold: [i32; YieldType::NUM],
    pub trade_yield_modifier: [i32; YieldType::NUM],
    pub commerce_change: [i32; CommerceType::NUM],
    pub commerce_modifier: [i32; CommerceType::NUM],
    /// C++ `isFreePromotion(i)`.
    pub free_promotion: IdxVec<PromotionType, bool>,
    /// C++ `isFreePromotionUnitCombat(i)`: unit combats that receive [`Self::free_promotion`].
    pub free_promotion_unit_combat: IdxVec<UnitCombatType, bool>,
}

/// C++ `CvTraitInfo::read`.
pub fn read_trait(rd: &Rd) -> TraitInfo {
    TraitInfo {
        base: InfoBase::read(rd),
        short_description: rd.string("ShortDescription"),
        health: rd.int("iHealth"),
        happiness: rd.int("iHappiness"),
        max_anarchy: rd.int("iMaxAnarchy"),
        upkeep_modifier: rd.int("iUpkeepModifier"),
        level_experience_modifier: rd.int("iLevelExperienceModifier"),
        great_people_rate_modifier: rd.int("iGreatPeopleRateModifier"),
        great_general_rate_modifier: rd.int("iGreatGeneralRateModifier"),
        domestic_great_general_rate_modifier: rd.int("iDomesticGreatGeneralRateModifier"),
        max_global_building_production_modifier: rd.int("iMaxGlobalBuildingProductionModifier"),
        max_team_building_production_modifier: rd.int("iMaxTeamBuildingProductionModifier"),
        max_player_building_production_modifier: rd.int("iMaxPlayerBuildingProductionModifier"),
        adaptive: rd.bool("bAdaptive"),
        agnostic: rd.bool("bAgnostic"),
        assimilation: rd.bool("bAssimilation"),
        barbarian_ally: rd.bool("bBarbarianAlly"),
        ignore_food: rd.bool("bIgnoreFood"),
        insane: rd.bool("bInsane"),
        selectable: rd.bool("bSelectable"),
        sprawling: rd.bool("bSprawling"),
        free_xp_from_combat: rd.int("iFreeXPFromCombat"),
        max_cities: rd.int("iMaxCities"),
        pillaging_gold: rd.int("iPillagingGold"),
        starting_gold: rd.int("iStartingGold"),
        summon_duration: rd.int("iSummonDuration"),
        upgrade_cost_modifier: rd.int("iUpgradeCostModifier"),
        extra_yield_threshold: rd.yields("ExtraYieldThresholds"),
        trade_yield_modifier: rd.yields("TradeYieldModifiers"),
        commerce_change: rd.commerces("CommerceChanges"),
        commerce_modifier: rd.commerces("CommerceModifiers"),
        free_promotion: rd.pairs_bool("FreePromotions"),
        free_promotion_unit_combat: rd.pairs_bool("FreePromotionUnitCombats"),
    }
}

/// C++ `CvCivilizationInfo` (Civilizations/CIV4CivilizationInfos.xml)
///
/// Civ-specific buildings/units: C++ fills `m_piCivilizationBuildings`/`m_piCivilizationUnits`
/// with every class's default (`InitBuildingDefaults`/`InitUnitDefaults`, i.e.
/// `getDefaultBuildingIndex()`/`getDefaultUnitIndex()`) and then applies the `<Buildings>`/
/// `<Units>` entries. Readers here cannot see other tables, so only the overrides are
/// stored ([`Self::civilization_building_overrides`], [`Self::civilization_unit_overrides`])
/// and the default is resolved at query time by [`Self::civilization_buildings`] /
/// [`Self::civilization_units`], which return exactly what the C++ getters return.
#[derive(Debug, Clone, Default)]
pub struct CivilizationInfo {
    pub base: InfoBase,
    /// C++ `getShortDescriptionKey()` (text key).
    pub short_description_key: String,
    /// C++ `getAdjectiveKey()` (text key).
    pub adjective_key: String,
    /// C++ `isPlayable()` as read. FfH: the getter also returns false for a
    /// `bGraphicalOnly` civ once the game is fully initialized.
    pub playable: bool,
    /// C++ `isAIPlayable()` as read (same FfH graphical-only rule as [`Self::playable`]).
    pub ai_playable: bool,
    /// C++ `getCityNames(i)` (`getNumCityNames()` is the length).
    pub city_names: Vec<String>,
    /// `<Buildings>` as read, per building class: `None` = the class default applies,
    /// `Some(b)` = this civ's override (`Some(None)` = `NONE`, the civ cannot build the
    /// class). If `<Buildings>` is absent C++ leaves the array NULL and the getter returns
    /// -1 for every class, stored as `Some(None)` everywhere. Use
    /// [`Self::civilization_buildings`].
    pub civilization_building_overrides: IdxVec<BuildingClassType, Option<Option<BuildingType>>>,
    /// `<Units>` as read, per unit class; same encoding as
    /// [`Self::civilization_building_overrides`]. Use [`Self::civilization_units`].
    pub civilization_unit_overrides: IdxVec<UnitClassType, Option<Option<UnitType>>>,
    /// C++ `getCivilizationFreeUnitsClass(i)`: starting units per class (`<FreeUnitClasses>`).
    pub civilization_free_units_class: IdxVec<UnitClassType, i32>,
    /// C++ `isCivilizationFreeBuildingClass(i)` (`<FreeBuildingClasses>`).
    pub civilization_free_building_class: IdxVec<BuildingClassType, bool>,
    /// C++ `isCivilizationFreeTechs(i)` (`<FreeTechs>`).
    pub civilization_free_techs: IdxVec<TechType, bool>,
    /// C++ `isCivilizationDisableTechs(i)` (`<DisableTechs>`).
    pub civilization_disable_techs: IdxVec<TechType, bool>,
    /// C++ `getCivilizationInitialCivics(i)`, per civic option (positional `<InitialCivics>`).
    /// C++ quirk: when the tag is present, slots it does not list hold 0 (the first civic),
    /// not `NO_CIVIC`; when it is absent, every slot is -1 (`None`).
    pub civilization_initial_civics: IdxVec<CivicOptionType, Option<CivicType>>,
    /// C++ `isLeaders(i)`.
    pub leaders: IdxVec<LeaderHeadType, bool>,
    // ---- FfH ----
    pub civ_trait: Option<TraitType>,
    /// C++ `getDefaultRace()`: race promotion given to the civ's units.
    pub default_race: Option<PromotionType>,
    pub hero: Option<UnitType>,
    /// C++ `isMaintainFeatures(i)`.
    pub maintain_features: IdxVec<FeatureType, bool>,
    // ---- Better AI ----
    /// C++ `getBestDefender1()`; `UNITCLASS_ARCHER` when the tag is absent.
    pub best_defender1: Option<UnitClassType>,
    /// C++ `getBestDefender2()`; `UNITCLASS_LONGBOWMAN` when the tag is absent.
    pub best_defender2: Option<UnitClassType>,
    /// C++ `getBestDefender3()`; `UNITCLASS_ARQUEBUS` when the tag is absent.
    pub best_defender3: Option<UnitClassType>,
    /// C++ `getBestPatrol1()`; `UNITCLASS_HUNTER` when the tag is absent.
    pub best_patrol1: Option<UnitClassType>,
    /// C++ `getBestPatrol2()`; `UNITCLASS_RANGER` when the tag is absent.
    pub best_patrol2: Option<UnitClassType>,
    /// C++ `getBestPatrol3()`; no default.
    pub best_patrol3: Option<UnitClassType>,
    /// C++ `getDerivativeCiv()` (read in `readPass2`).
    pub derivative_civ: Option<CivilizationType>,
}

impl CivilizationInfo {
    /// C++ `getCivilizationBuildings(eClass)`: this civ's building for a class (`None` =
    /// cannot build it). `classes` is the building class table (`data.building_classes`).
    pub fn civilization_buildings(
        &self,
        class: BuildingClassType,
        classes: &IdxVec<BuildingClassType, BuildingClassInfo>,
    ) -> Option<BuildingType> {
        match self.civilization_building_overrides[class] {
            Some(b) => b,
            None => classes[class].default_building_index,
        }
    }

    /// C++ `getCivilizationUnits(eClass)`: this civ's unit for a class (`None` = cannot
    /// train it). `default_unit` must be the class's `UnitClassInfo` default unit
    /// (C++ `getDefaultUnitIndex()`).
    pub fn civilization_units(&self, class: UnitClassType, default_unit: Option<UnitType>) -> Option<UnitType> {
        self.civilization_unit_overrides[class].unwrap_or(default_unit)
    }
}

/// C++ `CvCivilizationInfo::read` + `readPass2`.
pub fn read_civilization(rd: &Rd) -> CivilizationInfo {
    CivilizationInfo {
        base: InfoBase::read(rd),
        short_description_key: rd.string("ShortDescription"),
        adjective_key: rd.string("Adjective"),
        playable: rd.bool("bPlayable"),
        ai_playable: rd.bool("bAIPlayable"),
        city_names: rd.string_list("Cities"),
        civilization_building_overrides: class_overrides(rd, "Buildings"),
        civilization_unit_overrides: class_overrides(rd, "Units"),
        civilization_free_units_class: rd.pairs_int("FreeUnitClasses", 0),
        civilization_free_building_class: rd.pairs_bool("FreeBuildingClasses"),
        civilization_free_techs: rd.pairs_bool("FreeTechs"),
        civilization_disable_techs: rd.pairs_bool("DisableTechs"),
        civilization_initial_civics: initial_civics(rd),
        leaders: rd.pairs_bool("Leaders"),
        civ_trait: rd.rf("CivTrait"),
        default_race: rd.rf("DefaultRace"),
        hero: rd.rf("Hero"),
        maintain_features: rd.pairs_bool("MaintainFeatures"),
        best_defender1: rf_or(rd, "eBestDefender1", "UNITCLASS_ARCHER"),
        best_defender2: rf_or(rd, "eBestDefender2", "UNITCLASS_LONGBOWMAN"),
        best_defender3: rf_or(rd, "eBestDefender3", "UNITCLASS_ARQUEBUS"),
        best_patrol1: rf_or(rd, "eBestPatrol1", "UNITCLASS_HUNTER"),
        best_patrol2: rf_or(rd, "eBestPatrol2", "UNITCLASS_RANGER"),
        best_patrol3: rd.rf("eBestPatrol3"),
        // readPass2
        derivative_civ: rd.rf("DerivativeCiv"),
    }
}

/// `GetChildXmlValByName(szTextVal, tag, def); GC.getInfoTypeForString(szTextVal)`:
/// a type reference whose name defaults to `def` when the tag is absent.
fn rf_or<T: InfoId>(rd: &Rd, tag: &str, def: &str) -> Option<T> {
    rd.resolve(&rd.string_or(tag, def))
}

/// The `<Buildings>`/`<Units>` loops of `CvCivilizationInfo::read`: each entry's first
/// child names a class and its second child the civ's type for it (`NONE` = none). See
/// [`CivilizationInfo::civilization_building_overrides`] for the encoding.
fn class_overrides<K: InfoId, V: InfoId>(rd: &Rd, tag: &str) -> IdxVec<K, Option<Option<V>>> {
    let n = rd.reg().count::<K>();
    let Some(list) = rd.child(tag) else {
        // No list: the C++ array stays NULL and the getter returns -1 for every class.
        return IdxVec::from_elem(Some(None), n);
    };
    // InitBuildingDefaults / InitUnitDefaults: every class starts at its default.
    let mut out = IdxVec::from_elem(None, n);
    for entry in list.children() {
        let mut it = entry.children();
        let Some(class) = it.next() else { continue };
        if let Some(k) = list.resolve::<K>(class.text()) {
            // GetNextXmlVal: a missing value element reads as "" -> -1.
            out[k] = Some(it.next().and_then(|v| list.resolve::<V>(v.text())));
        }
    }
    out
}

/// The `<InitialCivics>` read of `CvCivilizationInfo::read`: positional, one civic per
/// civic option.
fn initial_civics(rd: &Rd) -> IdxVec<CivicOptionType, Option<CivicType>> {
    let n = rd.reg().count::<CivicOptionType>();
    let Some(list) = rd.child("InitialCivics") else {
        return IdxVec::from_elem(None, n);
    };
    // C++ quirk: InitList(.., GC.getNumCivicOptionInfos()) fills 0 (the first civic).
    let mut out = IdxVec::from_elem(Some(CivicType(0)), n);
    for (i, c) in list.children().enumerate() {
        if i >= n {
            rd.cx.warn(format!("<InitialCivics> has more than {n} entries"));
            break;
        }
        out.raw[i] = list.resolve::<CivicType>(c.text());
    }
    out
}

/// C++ `CvLeaderHeadInfo` (Civilizations/CIV4LeaderHeadInfos.xml): leader personality,
/// mostly AI weights. Defaults noted below are the C++ values used when the tag is absent.
#[derive(Debug, Clone, Default)]
pub struct LeaderHeadInfo {
    pub base: InfoBase,
    pub wonder_construct_rand: i32,
    pub base_attitude: i32,
    pub base_peace_weight: i32,
    pub peace_weight_rand: i32,
    pub warmonger_respect: i32,
    pub espionage_weight: i32,
    pub refuse_to_talk_war_threshold: i32,
    pub no_tech_trade_threshold: i32,
    pub tech_trade_known_percent: i32,
    pub max_gold_trade_percent: i32,
    pub max_gold_per_turn_trade_percent: i32,
    /// Default 50.
    pub max_war_rand: i32,
    pub max_war_nearby_power_ratio: i32,
    pub max_war_distant_power_ratio: i32,
    pub max_war_min_adjacent_land_percent: i32,
    pub limited_war_rand: i32,
    pub limited_war_power_ratio: i32,
    pub dogpile_war_rand: i32,
    pub make_peace_rand: i32,
    pub declare_war_trade_rand: i32,
    pub demand_rebuked_sneak_prob: i32,
    pub demand_rebuked_war_prob: i32,
    pub raze_city_prob: i32,
    /// Default 25.
    pub build_unit_prob: i32,
    pub base_attack_odds_change: i32,
    pub attack_odds_change_rand: i32,
    pub worse_rank_difference_attitude_change: i32,
    pub better_rank_difference_attitude_change: i32,
    pub close_borders_attitude_change: i32,
    pub lost_war_attitude_change: i32,
    /// Default 10.
    pub at_war_attitude_divisor: i32,
    pub at_war_attitude_change_limit: i32,
    /// Default 10.
    pub at_peace_attitude_divisor: i32,
    /// Default 5.
    pub at_peace_attitude_change_limit: i32,
    pub same_religion_attitude_change: i32,
    /// Default 30.
    pub same_religion_attitude_divisor: i32,
    /// Default 5.
    pub same_religion_attitude_change_limit: i32,
    pub different_religion_attitude_change: i32,
    /// Default -30.
    pub different_religion_attitude_divisor: i32,
    /// Default -5.
    pub different_religion_attitude_change_limit: i32,
    /// Default 30.
    pub bonus_trade_attitude_divisor: i32,
    /// Default 1.
    pub bonus_trade_attitude_change_limit: i32,
    /// Default 30.
    pub open_borders_attitude_divisor: i32,
    /// Default 1.
    pub open_borders_attitude_change_limit: i32,
    /// Default 50.
    pub defensive_pact_attitude_divisor: i32,
    /// Default 2.
    pub defensive_pact_attitude_change_limit: i32,
    /// Default 2.
    pub share_war_attitude_change: i32,
    /// Default 10.
    pub share_war_attitude_divisor: i32,
    /// Default 2.
    pub share_war_attitude_change_limit: i32,
    /// Default 2.
    pub favorite_civic_attitude_change: i32,
    /// Default 30.
    pub favorite_civic_attitude_divisor: i32,
    /// Default 2.
    pub favorite_civic_attitude_change_limit: i32,
    pub demand_tribute_attitude_threshold: Option<AttitudeType>,
    pub no_give_help_attitude_threshold: Option<AttitudeType>,
    pub tech_refuse_attitude_threshold: Option<AttitudeType>,
    pub strategic_bonus_refuse_attitude_threshold: Option<AttitudeType>,
    pub happiness_bonus_refuse_attitude_threshold: Option<AttitudeType>,
    pub health_bonus_refuse_attitude_threshold: Option<AttitudeType>,
    pub map_refuse_attitude_threshold: Option<AttitudeType>,
    pub declare_war_refuse_attitude_threshold: Option<AttitudeType>,
    pub declare_war_them_refuse_attitude_threshold: Option<AttitudeType>,
    pub stop_trading_refuse_attitude_threshold: Option<AttitudeType>,
    pub stop_trading_them_refuse_attitude_threshold: Option<AttitudeType>,
    pub adopt_civic_refuse_attitude_threshold: Option<AttitudeType>,
    pub convert_religion_refuse_attitude_threshold: Option<AttitudeType>,
    pub open_borders_refuse_attitude_threshold: Option<AttitudeType>,
    pub defensive_pact_refuse_attitude_threshold: Option<AttitudeType>,
    pub permanent_alliance_refuse_attitude_threshold: Option<AttitudeType>,
    pub vassal_refuse_attitude_threshold: Option<AttitudeType>,
    pub vassal_power_modifier: i32,
    pub freedom_appreciation: i32,
    pub favorite_civic: Option<CivicType>,
    pub favorite_religion: Option<ReligionType>,
    // ---- arrays ----
    /// C++ `hasTrait(i)`.
    pub has_trait: IdxVec<TraitType, bool>,
    pub flavor_value: IdxVec<FlavorType, i32>,
    pub contact_rand: IdxVec<ContactType, i32>,
    pub contact_delay: IdxVec<ContactType, i32>,
    /// C++ `getMemoryDecayRand(i)`, XML `<MemoryDecays>`.
    pub memory_decay_rand: IdxVec<MemoryType, i32>,
    pub memory_attitude_percent: IdxVec<MemoryType, i32>,
    pub no_war_attitude_prob: IdxVec<AttitudeType, i32>,
    pub unit_ai_weight_modifier: IdxVec<UnitAIType, i32>,
    pub improvement_weight_modifier: IdxVec<ImprovementType, i32>,
    // ---- FfH ----
    pub female: bool,
    pub alignment: Option<AlignmentType>,
    pub attitude_bad_bonus: i32,
    pub attitude_from_females: i32,
    pub attitude_from_males: i32,
    pub attitude_to_females: i32,
    pub attitude_to_males: i32,
    pub somnium_aggressiveness: i32,
    pub favorite_tech: Option<TechType>,
    pub favorite_unit_combat: Option<UnitCombatType>,
    pub favorite_wonder: Option<BuildingType>,
    pub permanent_trait: Option<TraitType>,
    pub religion_weight_modifier: IdxVec<ReligionType, i32>,
    // ---- Better AI ----
    pub altar_victory: bool,
    pub arcane_tower_victory: bool,
    pub culture_victory: bool,
    pub religion_victory: bool,
    pub religion_opportunist: bool,
    /// C++ `getAIPatrolGroupSize()`; default 100.
    pub ai_patrol_group_size: i32,
    /// C++ `getAIPatrolGroupMage()`; default 100.
    pub ai_patrol_group_mage: i32,
    /// C++ `getAIValueDefense()`; default 100.
    pub ai_value_defense: i32,
    /// C++ `getAIValueMage()`; default 100.
    pub ai_value_mage: i32,
    /// Default 100.
    pub economy_tech_value: i32,
    /// XML `<iFavoriteEarlyReligion>` (a type name despite the `i` prefix).
    pub favorite_early_religion: Option<ReligionType>,
    pub favorite_early_mil_tech: Option<TechType>,
    pub favorite_early_tech1: Option<TechType>,
    pub favorite_early_tech2: Option<TechType>,
    pub favorite_early_tech3: Option<TechType>,
    /// Used as a building class (Python `CvGameUtils`). C++ quirk: the class-agnostic
    /// `FindInInfoClass` stores the index of whatever type the XML names, so an entry
    /// naming a `BUILDING_*` (LEADER_THESSA: `BUILDING_CATACOMB_LIBRALUS`) yields that
    /// building's index read as an unrelated building class. Kept as in C++.
    pub favorite_early_wonder: Option<BuildingClassType>,
    /// Same C++ quirk as [`Self::favorite_early_wonder`] (LEADER_CAPRIA:
    /// `BUILDING_AQUAE_SUCELLUS`, LEADER_THESSA: `BUILDING_GREAT_LIBRARY`).
    pub favorite_late_wonder: Option<BuildingClassType>,
}

/// C++ `CvLeaderHeadInfo::read`.
pub fn read_leader_head(rd: &Rd) -> LeaderHeadInfo {
    LeaderHeadInfo {
        base: InfoBase::read(rd),
        wonder_construct_rand: rd.int("iWonderConstructRand"),
        base_attitude: rd.int("iBaseAttitude"),
        base_peace_weight: rd.int("iBasePeaceWeight"),
        peace_weight_rand: rd.int("iPeaceWeightRand"),
        warmonger_respect: rd.int("iWarmongerRespect"),
        espionage_weight: rd.int("iEspionageWeight"),
        refuse_to_talk_war_threshold: rd.int("iRefuseToTalkWarThreshold"),
        no_tech_trade_threshold: rd.int("iNoTechTradeThreshold"),
        tech_trade_known_percent: rd.int("iTechTradeKnownPercent"),
        max_gold_trade_percent: rd.int("iMaxGoldTradePercent"),
        max_gold_per_turn_trade_percent: rd.int("iMaxGoldPerTurnTradePercent"),
        max_war_rand: rd.int_or("iMaxWarRand", 50),
        max_war_nearby_power_ratio: rd.int("iMaxWarNearbyPowerRatio"),
        max_war_distant_power_ratio: rd.int("iMaxWarDistantPowerRatio"),
        max_war_min_adjacent_land_percent: rd.int("iMaxWarMinAdjacentLandPercent"),
        limited_war_rand: rd.int("iLimitedWarRand"),
        limited_war_power_ratio: rd.int("iLimitedWarPowerRatio"),
        dogpile_war_rand: rd.int("iDogpileWarRand"),
        make_peace_rand: rd.int("iMakePeaceRand"),
        declare_war_trade_rand: rd.int("iDeclareWarTradeRand"),
        demand_rebuked_sneak_prob: rd.int("iDemandRebukedSneakProb"),
        demand_rebuked_war_prob: rd.int("iDemandRebukedWarProb"),
        raze_city_prob: rd.int("iRazeCityProb"),
        build_unit_prob: rd.int_or("iBuildUnitProb", 25),
        base_attack_odds_change: rd.int("iBaseAttackOddsChange"),
        attack_odds_change_rand: rd.int("iAttackOddsChangeRand"),
        worse_rank_difference_attitude_change: rd.int("iWorseRankDifferenceAttitudeChange"),
        better_rank_difference_attitude_change: rd.int("iBetterRankDifferenceAttitudeChange"),
        close_borders_attitude_change: rd.int("iCloseBordersAttitudeChange"),
        lost_war_attitude_change: rd.int("iLostWarAttitudeChange"),
        at_war_attitude_divisor: rd.int_or("iAtWarAttitudeDivisor", 10),
        at_war_attitude_change_limit: rd.int("iAtWarAttitudeChangeLimit"),
        at_peace_attitude_divisor: rd.int_or("iAtPeaceAttitudeDivisor", 10),
        at_peace_attitude_change_limit: rd.int_or("iAtPeaceAttitudeChangeLimit", 5),
        same_religion_attitude_change: rd.int("iSameReligionAttitudeChange"),
        same_religion_attitude_divisor: rd.int_or("iSameReligionAttitudeDivisor", 30),
        same_religion_attitude_change_limit: rd.int_or("iSameReligionAttitudeChangeLimit", 5),
        different_religion_attitude_change: rd.int("iDifferentReligionAttitudeChange"),
        different_religion_attitude_divisor: rd.int_or("iDifferentReligionAttitudeDivisor", -30),
        different_religion_attitude_change_limit: rd.int_or("iDifferentReligionAttitudeChangeLimit", -5),
        bonus_trade_attitude_divisor: rd.int_or("iBonusTradeAttitudeDivisor", 30),
        bonus_trade_attitude_change_limit: rd.int_or("iBonusTradeAttitudeChangeLimit", 1),
        open_borders_attitude_divisor: rd.int_or("iOpenBordersAttitudeDivisor", 30),
        open_borders_attitude_change_limit: rd.int_or("iOpenBordersAttitudeChangeLimit", 1),
        defensive_pact_attitude_divisor: rd.int_or("iDefensivePactAttitudeDivisor", 50),
        defensive_pact_attitude_change_limit: rd.int_or("iDefensivePactAttitudeChangeLimit", 2),
        share_war_attitude_change: rd.int_or("iShareWarAttitudeChange", 2),
        share_war_attitude_divisor: rd.int_or("iShareWarAttitudeDivisor", 10),
        share_war_attitude_change_limit: rd.int_or("iShareWarAttitudeChangeLimit", 2),
        favorite_civic_attitude_change: rd.int_or("iFavoriteCivicAttitudeChange", 2),
        favorite_civic_attitude_divisor: rd.int_or("iFavoriteCivicAttitudeDivisor", 30),
        favorite_civic_attitude_change_limit: rd.int_or("iFavoriteCivicAttitudeChangeLimit", 2),
        vassal_power_modifier: rd.int("iVassalPowerModifier"),
        freedom_appreciation: rd.int("iFreedomAppreciation"),
        demand_tribute_attitude_threshold: rd.rf("DemandTributeAttitudeThreshold"),
        no_give_help_attitude_threshold: rd.rf("NoGiveHelpAttitudeThreshold"),
        tech_refuse_attitude_threshold: rd.rf("TechRefuseAttitudeThreshold"),
        strategic_bonus_refuse_attitude_threshold: rd.rf("StrategicBonusRefuseAttitudeThreshold"),
        happiness_bonus_refuse_attitude_threshold: rd.rf("HappinessBonusRefuseAttitudeThreshold"),
        health_bonus_refuse_attitude_threshold: rd.rf("HealthBonusRefuseAttitudeThreshold"),
        map_refuse_attitude_threshold: rd.rf("MapRefuseAttitudeThreshold"),
        declare_war_refuse_attitude_threshold: rd.rf("DeclareWarRefuseAttitudeThreshold"),
        declare_war_them_refuse_attitude_threshold: rd.rf("DeclareWarThemRefuseAttitudeThreshold"),
        stop_trading_refuse_attitude_threshold: rd.rf("StopTradingRefuseAttitudeThreshold"),
        stop_trading_them_refuse_attitude_threshold: rd.rf("StopTradingThemRefuseAttitudeThreshold"),
        adopt_civic_refuse_attitude_threshold: rd.rf("AdoptCivicRefuseAttitudeThreshold"),
        convert_religion_refuse_attitude_threshold: rd.rf("ConvertReligionRefuseAttitudeThreshold"),
        open_borders_refuse_attitude_threshold: rd.rf("OpenBordersRefuseAttitudeThreshold"),
        defensive_pact_refuse_attitude_threshold: rd.rf("DefensivePactRefuseAttitudeThreshold"),
        permanent_alliance_refuse_attitude_threshold: rd.rf("PermanentAllianceRefuseAttitudeThreshold"),
        vassal_refuse_attitude_threshold: rd.rf("VassalRefuseAttitudeThreshold"),
        favorite_civic: rd.rf("FavoriteCivic"),
        favorite_religion: rd.rf("FavoriteReligion"),
        has_trait: rd.pairs_bool("Traits"),
        flavor_value: rd.pairs_int("Flavors", 0),
        contact_rand: rd.pairs_int("ContactRands", 0),
        contact_delay: rd.pairs_int("ContactDelays", 0),
        memory_decay_rand: rd.pairs_int("MemoryDecays", 0),
        memory_attitude_percent: rd.pairs_int("MemoryAttitudePercents", 0),
        no_war_attitude_prob: rd.pairs_int("NoWarAttitudeProbs", 0),
        unit_ai_weight_modifier: rd.pairs_int("UnitAIWeightModifiers", 0),
        improvement_weight_modifier: rd.pairs_int("ImprovementWeightModifiers", 0),
        female: rd.bool("bFemale"),
        alignment: rd.rf("Alignment"),
        attitude_bad_bonus: rd.int("iAttitudeBadBonus"),
        attitude_from_females: rd.int("iAttitudeFromFemales"),
        attitude_from_males: rd.int("iAttitudeFromMales"),
        attitude_to_females: rd.int("iAttitudeToFemales"),
        attitude_to_males: rd.int("iAttitudeToMales"),
        somnium_aggressiveness: rd.int("iSomniumAggressiveness"),
        favorite_tech: rd.rf("FavoriteTech"),
        favorite_unit_combat: rd.rf("FavoriteUnitCombat"),
        favorite_wonder: rd.rf("FavoriteWonder"),
        permanent_trait: rd.rf("PermanentTrait"),
        altar_victory: rd.bool("bAltarVictory"),
        arcane_tower_victory: rd.bool("bArcaneTowerVictory"),
        culture_victory: rd.bool("bCultureVictory"),
        religion_victory: rd.bool("bReligionVictory"),
        religion_opportunist: rd.bool("bReligionOpportunist"),
        ai_patrol_group_mage: rd.int_or("iAIPatrolGroupMage", 100),
        ai_patrol_group_size: rd.int_or("iAIPatrolGroupSize", 100),
        ai_value_defense: rd.int_or("iAIValueDefense", 100),
        ai_value_mage: rd.int_or("iAIValueMage", 100),
        economy_tech_value: rd.int_or("iEconomyTechValue", 100),
        favorite_early_religion: rd.rf("iFavoriteEarlyReligion"),
        favorite_early_mil_tech: rd.rf("iFavoriteEarlyMilTech"),
        favorite_early_tech1: rd.rf("iFavoriteEarlyTech1"),
        favorite_early_tech2: rd.rf("iFavoriteEarlyTech2"),
        favorite_early_tech3: rd.rf("iFavoriteEarlyTech3"),
        favorite_early_wonder: find_in_info_class(rd, "iFavoriteEarlyWonder"),
        favorite_late_wonder: find_in_info_class(rd, "iFavoriteLateWonder"),
        religion_weight_modifier: rd.pairs_int("ReligionWeightModifiers", 0),
    }
}

/// C++ `FindInInfoClass(<tag>)` kept class-agnostic: the index of the name within
/// whatever info class registers it, reinterpreted as `T` (see `favorite_early_wonder`).
/// Warns only for unknown names, like the C++ assert.
fn find_in_info_class<T: Idx>(rd: &Rd, tag: &str) -> Option<T> {
    rd.child(tag).and_then(|c| opt_from_i32(rd.find_in_info_class(c.text())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Ctx, Registry};
    use crate::xml::XmlDoc;

    fn registry() -> Registry {
        let mut reg = Registry::new();
        let classes: [(&'static str, &[&str]); 8] = [
            ("BuildingClassInfo", &["BUILDINGCLASS_A", "BUILDINGCLASS_B"]),
            ("BuildingInfo", &["BUILDING_A", "BUILDING_B2"]),
            ("UnitClassInfo", &["UNITCLASS_ARCHER", "UNITCLASS_LONGBOWMAN", "UNITCLASS_ARQUEBUS", "UNITCLASS_HUNTER", "UNITCLASS_RANGER"]),
            ("UnitInfo", &["UNIT_X"]),
            ("CivicOptionInfo", &["CIVICOPTION_1", "CIVICOPTION_2", "CIVICOPTION_3"]),
            ("CivicInfo", &["CIVIC_A", "CIVIC_B"]),
            ("AttitudeInfo", &["ATTITUDE_FURIOUS", "ATTITUDE_ANNOYED"]),
            ("CivilizationInfo", &["CIVILIZATION_T"]),
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

    fn classes() -> IdxVec<BuildingClassType, BuildingClassInfo> {
        IdxVec::from_vec(vec![
            BuildingClassInfo { default_building_index: Some(BuildingType(0)), ..Default::default() },
            BuildingClassInfo { default_building_index: Some(BuildingType(1)), ..Default::default() },
        ])
    }

    #[test]
    fn civ_without_lists_gets_cxx_null_array_values() {
        let (c, w) = read("<CivilizationInfo><Type>CIVILIZATION_T</Type></CivilizationInfo>", read_civilization);
        assert!(w.is_empty(), "{w:?}");
        // No <Buildings>/<Units>: C++ arrays stay NULL -> -1 for every class, no defaults.
        assert!(c.civilization_building_overrides.iter().all(|o| *o == Some(None)));
        assert_eq!(c.civilization_buildings(BuildingClassType(0), &classes()), None);
        assert_eq!(c.civilization_unit_overrides.len(), 5);
        assert_eq!(c.civilization_units(UnitClassType(3), Some(UnitType(0))), None);
        // No <InitialCivics>: -1 for every option.
        assert_eq!(c.civilization_initial_civics.raw, vec![None, None, None]);
        // Better AI string defaults.
        assert_eq!(c.best_defender1, Some(UnitClassType(0)));
        assert_eq!(c.best_defender2, Some(UnitClassType(1)));
        assert_eq!(c.best_defender3, Some(UnitClassType(2)));
        assert_eq!(c.best_patrol1, Some(UnitClassType(3)));
        assert_eq!(c.best_patrol2, Some(UnitClassType(4)));
        assert_eq!(c.best_patrol3, None);
        assert!(c.city_names.is_empty() && !c.playable);
    }

    #[test]
    fn civ_overrides_defaults_and_initial_civics_quirk() {
        let xml = r#"<CivilizationInfo>
            <Buildings>
                <Building><BuildingClassType>BUILDINGCLASS_B</BuildingClassType><BuildingType>NONE</BuildingType></Building>
            </Buildings>
            <Units>
                <Unit><UnitClassType>UNITCLASS_HUNTER</UnitClassType><UnitType>UNIT_X</UnitType></Unit>
                <Unit><UnitClassType>UNITCLASS_RANGER</UnitClassType></Unit>
            </Units>
            <InitialCivics><CivicType>CIVIC_B</CivicType><CivicType>NONE</CivicType></InitialCivics>
            <Cities><City>TXT_A</City><City>TXT_B</City></Cities>
            <eBestDefender1>UNITCLASS_RANGER</eBestDefender1>
            <eBestPatrol3>UNITCLASS_HUNTER</eBestPatrol3>
        </CivilizationInfo>"#;
        let (c, w) = read(xml, read_civilization);
        assert!(w.is_empty(), "{w:?}");
        let classes = classes();
        // Class A untouched -> class default; class B explicitly NONE.
        assert_eq!(c.civilization_building_overrides.raw, vec![None, Some(None)]);
        assert_eq!(c.civilization_buildings(BuildingClassType(0), &classes), Some(BuildingType(0)));
        assert_eq!(c.civilization_buildings(BuildingClassType(1), &classes), None);
        // HUNTER -> UNIT_X; RANGER without a value element -> "" -> -1; ARCHER -> default.
        assert_eq!(c.civilization_unit_overrides[UnitClassType(3)], Some(Some(UnitType(0))));
        assert_eq!(c.civilization_unit_overrides[UnitClassType(4)], Some(None));
        assert_eq!(c.civilization_units(UnitClassType(0), Some(UnitType(0))), Some(UnitType(0)));
        assert_eq!(c.civilization_units(UnitClassType(4), Some(UnitType(0))), None);
        // C++ quirk: the unlisted third slot holds civic 0, not NO_CIVIC.
        assert_eq!(c.civilization_initial_civics.raw, vec![Some(CivicType(1)), None, Some(CivicType(0))]);
        assert_eq!(c.city_names, vec!["TXT_A", "TXT_B"]);
        assert_eq!(c.best_defender1, Some(UnitClassType(4)));
        assert_eq!(c.best_patrol3, Some(UnitClassType(3)));
    }

    #[test]
    fn trait_max_cities_defaults_to_zero_not_minus_one() {
        let (t, w) = read("<TraitInfo><Type>TRAIT_T</Type></TraitInfo>", read_trait);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(t.max_cities, 0);
        assert_eq!(t.commerce_change, [0; 4]);
        let (t, _) = read("<TraitInfo><iMaxCities>-1</iMaxCities></TraitInfo>", read_trait);
        assert_eq!(t.max_cities, -1);
    }

    #[test]
    fn leader_head_defaults() {
        let (l, w) = read("<LeaderHeadInfo><Type>LEADER_T</Type></LeaderHeadInfo>", read_leader_head);
        assert!(w.is_empty(), "{w:?}");
        let got = [
            l.max_war_rand, l.build_unit_prob, l.at_war_attitude_divisor, l.at_peace_attitude_divisor,
            l.at_peace_attitude_change_limit, l.same_religion_attitude_divisor, l.same_religion_attitude_change_limit,
            l.different_religion_attitude_divisor, l.different_religion_attitude_change_limit,
            l.bonus_trade_attitude_divisor, l.bonus_trade_attitude_change_limit, l.open_borders_attitude_divisor,
            l.open_borders_attitude_change_limit, l.defensive_pact_attitude_divisor,
            l.defensive_pact_attitude_change_limit, l.share_war_attitude_change, l.share_war_attitude_divisor,
            l.share_war_attitude_change_limit, l.favorite_civic_attitude_change, l.favorite_civic_attitude_divisor,
            l.favorite_civic_attitude_change_limit, l.ai_patrol_group_size, l.ai_patrol_group_mage,
            l.ai_value_defense, l.ai_value_mage, l.economy_tech_value,
        ];
        let want = [50, 25, 10, 10, 5, 30, 5, -30, -5, 30, 1, 30, 1, 50, 2, 2, 10, 2, 2, 30, 2, 100, 100, 100, 100, 100];
        assert_eq!(got, want);
        assert_eq!((l.wonder_construct_rand, l.raze_city_prob, l.at_war_attitude_change_limit), (0, 0, 0));
        assert_eq!(l.demand_tribute_attitude_threshold, None);
        assert_eq!(l.alignment, None);
        assert_eq!(l.favorite_late_wonder, None);
    }

    #[test]
    fn leader_favorite_wonders_are_class_agnostic() {
        let xml = r#"<LeaderHeadInfo>
            <iFavoriteEarlyWonder>BUILDINGCLASS_B</iFavoriteEarlyWonder>
            <iFavoriteLateWonder>BUILDING_B2</iFavoriteLateWonder>
            <TechRefuseAttitudeThreshold>ATTITUDE_ANNOYED</TechRefuseAttitudeThreshold>
        </LeaderHeadInfo>"#;
        let (l, w) = read(xml, read_leader_head);
        // BUILDING_B2 is BuildingInfo index 1, kept as building class 1 like C++ (no warning).
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(l.favorite_early_wonder, Some(BuildingClassType(1)));
        assert_eq!(l.favorite_late_wonder, Some(BuildingClassType(1)));
        assert_eq!(l.tech_refuse_attitude_threshold, Some(AttitudeType(1)));
        let (l, w) = read("<LeaderHeadInfo><iFavoriteLateWonder>BUILDING_NOPE</iFavoriteLateWonder></LeaderHeadInfo>", read_leader_head);
        assert_eq!(l.favorite_late_wonder, None);
        assert_eq!(w.len(), 1, "unknown names still warn: {w:?}");
    }
}
