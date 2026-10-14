//! Info classes: YieldInfo, TerrainInfo, FeatureInfo, BonusClassInfo, BonusInfo,
//! ImprovementInfo (with its per-bonus ImprovementBonusInfo), RouteInfo.
//!
//! Port of `Cv*Info::read` (plus `readPass2`/`readPass3`) in CvInfos.cpp and of
//! `CvXMLLoadUtility::SetImprovementBonuses`. C++ defers some type lookups to
//! `readPass2`/`readPass3` because the referenced file is not loaded yet; our registry
//! knows every type up front, so they resolve directly here. Every lookup in this file
//! was checked against the C++ load order (`CvXMLLoadUtility::LoadBasicInfos` /
//! `LoadPreMenuGlobals`): the referenced class is always registered by the time C++
//! resolves it, so no value differs from C++.
//!
//! Not ported (graphics/audio/UI only): `ArtDefineTag`, `WorldSoundscapeAudioScript`,
//! `FootstepSounds`, the feature `EffectType`/`iEffectProbability` (a bird-scatter effect
//! rolled on the async RNG in `CvUnit::move`), the yield `ColorType`/`SymbolPaths`, and the
//! font glyph `m_iChar` of yields and bonuses (assigned at runtime, not read from XML).
//! `CvBonusInfo::m_piImprovementChange` is never read from XML (only by the savegame
//! stream code) and has no rules use, so C++ `getImprovementChange()` is always -1.

use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::{InfoId, Rd};
use crate::xml::Elem;

/// C++ `GC.getNUM_ROUTE_PREREQ_OR_BONUSES()` (GlobalDefines.xml). Readers cannot see the
/// defines, so the value is mirrored here; the data tests check it against `defines`.
const NUM_ROUTE_PREREQ_OR_BONUSES: usize = 2;

// ---------------------------------------------------------------------------------------
// YieldInfo
// ---------------------------------------------------------------------------------------

/// C++ `CvYieldInfo` (Terrain/CIV4YieldInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct YieldInfo {
    pub base: InfoBase,
    pub hills_change: i32,
    pub peak_change: i32,
    pub lake_change: i32,
    pub city_change: i32,
    pub population_change_offset: i32,
    pub population_change_divisor: i32,
    pub min_city: i32,
    pub trade_modifier: i32,
    pub golden_age_yield: i32,
    pub golden_age_yield_threshold: i32,
    pub ai_weight_percent: i32,
}

/// C++ `CvYieldInfo::read`.
pub fn read_yield(rd: &Rd) -> YieldInfo {
    YieldInfo {
        base: InfoBase::read(rd),
        hills_change: rd.int("iHillsChange"),
        peak_change: rd.int("iPeakChange"),
        lake_change: rd.int("iLakeChange"),
        city_change: rd.int("iCityChange"),
        population_change_offset: rd.int("iPopulationChangeOffset"),
        population_change_divisor: rd.int("iPopulationChangeDivisor"),
        min_city: rd.int("iMinCity"),
        trade_modifier: rd.int("iTradeModifier"),
        golden_age_yield: rd.int("iGoldenAgeYield"),
        golden_age_yield_threshold: rd.int("iGoldenAgeYieldThreshold"),
        ai_weight_percent: rd.int("iAIWeightPercent"),
    }
}

// ---------------------------------------------------------------------------------------
// TerrainInfo
// ---------------------------------------------------------------------------------------

/// C++ `CvTerrainInfo` (Terrain/CIV4TerrainInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct TerrainInfo {
    pub base: InfoBase,
    /// `getMovementCost()` (`iMovement`)
    pub movement_cost: i32,
    /// `getSeeFromLevel()` (`iSeeFrom`)
    pub see_from_level: i32,
    /// `getSeeThroughLevel()` (`iSeeThrough`)
    pub see_through_level: i32,
    pub build_modifier: i32,
    /// `getDefenseModifier()` (`iDefense`)
    pub defense_modifier: i32,
    pub water: bool,
    pub impassable: bool,
    pub found: bool,
    pub found_coast: bool,
    pub found_fresh_water: bool,
    // ---- FfH ----
    pub normalize: bool,
    /// `getCivilizationYieldType()`: the civilization that gets `civilization_yield_change`
    /// on this terrain (C++ `readPass3`).
    pub civilization_yield_type: Option<CivilizationType>,
    /// `getCivilizationYieldChange(i)`
    pub civilization_yield_change: [i32; YieldType::NUM],
    /// `getPlotCounterDown()`: `CvPlot::changePlotCounter` turns the terrain into
    /// `terrain_down` when the plot counter (0..=100) drops below this. C++ quirk: the
    /// constructor default is -1, but `read()` resets it to 0 when the tag is absent.
    pub plot_counter_down: i32,
    /// `getTerrainDown()` (C++ `readPass2`)
    pub terrain_down: Option<TerrainType>,
    /// `getPlotCounterUp()`: the terrain becomes `terrain_up` when the plot counter rises
    /// above this. Same -1 (constructor) vs 0 (absent tag) quirk as `plot_counter_down`.
    pub plot_counter_up: i32,
    /// `getTerrainUp()` (C++ `readPass2`)
    pub terrain_up: Option<TerrainType>,
    // ---- arrays ----
    /// `getYield(i)` (`Yields`; `yield` is a Rust keyword)
    pub yield_: [i32; YieldType::NUM],
    pub river_yield_change: [i32; YieldType::NUM],
    pub hills_yield_change: [i32; YieldType::NUM],
}

/// C++ `CvTerrainInfo::read` + `readPass2` (`TerrainDown`/`TerrainUp`) + `readPass3`
/// (`CivilizationYieldType`).
pub fn read_terrain(rd: &Rd) -> TerrainInfo {
    TerrainInfo {
        base: InfoBase::read(rd),
        movement_cost: rd.int("iMovement"),
        see_from_level: rd.int("iSeeFrom"),
        see_through_level: rd.int("iSeeThrough"),
        build_modifier: rd.int("iBuildModifier"),
        defense_modifier: rd.int("iDefense"),
        water: rd.bool("bWater"),
        impassable: rd.bool("bImpassable"),
        found: rd.bool("bFound"),
        found_coast: rd.bool("bFoundCoast"),
        found_fresh_water: rd.bool("bFoundFreshWater"),
        normalize: rd.bool("bNormalize"),
        civilization_yield_type: rd.rf("CivilizationYieldType"),
        civilization_yield_change: rd.yields("CivilizationYieldChange"),
        plot_counter_down: rd.int("iPlotCounterDown"),
        terrain_down: rd.rf("TerrainDown"),
        plot_counter_up: rd.int("iPlotCounterUp"),
        terrain_up: rd.rf("TerrainUp"),
        yield_: rd.yields("Yields"),
        river_yield_change: rd.yields("RiverYieldChange"),
        hills_yield_change: rd.yields("HillsYieldChange"),
    }
}

// ---------------------------------------------------------------------------------------
// FeatureInfo
// ---------------------------------------------------------------------------------------

/// C++ `CvFeatureInfo` (Terrain/CIV4FeatureInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct FeatureInfo {
    pub base: InfoBase,
    /// `getMovementCost()` (`iMovement`)
    pub movement_cost: i32,
    /// `getSeeThroughChange()` (`iSeeThrough`)
    pub see_through_change: i32,
    pub health_percent: i32,
    /// `getAppearanceProbability()` (`iAppearance`, per 10000 at map generation)
    pub appearance_probability: i32,
    /// `getDisappearanceProbability()` (`iDisappearance`)
    pub disappearance_probability: i32,
    /// `getGrowthProbability()` (`iGrowth`)
    pub growth_probability: i32,
    /// `getDefenseModifier()` (`iDefense`)
    pub defense_modifier: i32,
    pub advanced_start_remove_cost: i32,
    pub turn_damage: i32,
    pub no_coast: bool,
    pub no_river: bool,
    pub no_adjacent: bool,
    pub requires_flatlands: bool,
    pub requires_river: bool,
    pub adds_fresh_water: bool,
    pub impassable: bool,
    pub no_city: bool,
    pub no_improvement: bool,
    pub visible_always: bool,
    pub nuke_immune: bool,
    /// `getOnUnitChangeTo()`: the feature this one becomes when a unit moves in
    /// (`CvUnit::move`; FfH doors). C++ keeps the type string and resolves it on use, where
    /// a non-empty unknown name would clear the feature; here it resolves at load (and warns).
    pub on_unit_change_to: Option<FeatureType>,
    // ---- FfH ----
    pub flammable: bool,
    /// `getFeatureUpgrade()` (C++ `readPass2`)
    pub feature_upgrade: Option<FeatureType>,
    pub prereq_state_religion: Option<ReligionType>,
    /// `getPythonOnMove()`: Python called (spell module `onMoveFeature`) when a unit moves
    /// onto the plot (`CvUnit::setXY`).
    pub python_on_move: String,
    /// `getRequireResist()`: units resisting this damage type by less than
    /// `FEATURE_REQUIRE_RESIST_AMOUNT` may not enter (`CvUnit::canMoveInto`).
    pub require_resist: Option<DamageType>,
    // ---- arrays ----
    /// `getYieldChange(i)` (`YieldChanges`)
    pub yield_change: [i32; YieldType::NUM],
    pub river_yield_change: [i32; YieldType::NUM],
    pub hills_yield_change: [i32; YieldType::NUM],
    /// `isTerrain(i)` (`TerrainBooleans`): terrains the feature may be placed on.
    pub terrain: IdxVec<TerrainType, bool>,
}

/// C++ `CvFeatureInfo::read` + `readPass2` (`FeatureUpgrade`).
pub fn read_feature(rd: &Rd) -> FeatureInfo {
    FeatureInfo {
        base: InfoBase::read(rd),
        movement_cost: rd.int("iMovement"),
        see_through_change: rd.int("iSeeThrough"),
        health_percent: rd.int("iHealthPercent"),
        appearance_probability: rd.int("iAppearance"),
        disappearance_probability: rd.int("iDisappearance"),
        growth_probability: rd.int("iGrowth"),
        defense_modifier: rd.int("iDefense"),
        advanced_start_remove_cost: rd.int("iAdvancedStartRemoveCost"),
        turn_damage: rd.int("iTurnDamage"),
        no_coast: rd.bool("bNoCoast"),
        no_river: rd.bool("bNoRiver"),
        no_adjacent: rd.bool("bNoAdjacent"),
        requires_flatlands: rd.bool("bRequiresFlatlands"),
        requires_river: rd.bool("bRequiresRiver"),
        adds_fresh_water: rd.bool("bAddsFreshWater"),
        impassable: rd.bool("bImpassable"),
        no_city: rd.bool("bNoCity"),
        no_improvement: rd.bool("bNoImprovement"),
        visible_always: rd.bool("bVisibleAlways"),
        nuke_immune: rd.bool("bNukeImmune"),
        on_unit_change_to: rd.rf("OnUnitChangeTo"),
        flammable: rd.bool("bFlammable"),
        feature_upgrade: rd.rf("FeatureUpgrade"),
        prereq_state_religion: rd.rf("PrereqStateReligion"),
        python_on_move: rd.string("PythonOnMove"),
        require_resist: rd.rf("RequireResist"),
        yield_change: rd.yields("YieldChanges"),
        river_yield_change: rd.yields("RiverYieldChange"),
        hills_yield_change: rd.yields("HillsYieldChange"),
        terrain: rd.pairs_bool("TerrainBooleans"),
    }
}

// ---------------------------------------------------------------------------------------
// BonusClassInfo
// ---------------------------------------------------------------------------------------

/// C++ `CvBonusClassInfo` (Terrain/CIV4BonusClassInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct BonusClassInfo {
    pub base: InfoBase,
    /// `getUniqueRange()` (`iUnique`): map generation keeps other bonus types of this class
    /// out of this range (`CvMapGenerator::canPlaceBonusAt`).
    pub unique_range: i32,
}

/// C++ `CvBonusClassInfo::read`.
pub fn read_bonus_class(rd: &Rd) -> BonusClassInfo {
    BonusClassInfo { base: InfoBase::read(rd), unique_range: rd.int("iUnique") }
}

// ---------------------------------------------------------------------------------------
// BonusInfo
// ---------------------------------------------------------------------------------------

/// C++ `CvBonusInfo` (Terrain/CIV4BonusInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct BonusInfo {
    pub base: InfoBase,
    pub bonus_class_type: Option<BonusClassType>,
    pub tech_reveal: Option<TechType>,
    pub tech_city_trade: Option<TechType>,
    pub tech_obsolete: Option<TechType>,
    pub ai_trade_modifier: i32,
    pub ai_objective: i32,
    pub health: i32,
    pub happiness: i32,
    pub min_area_size: i32,
    pub min_latitude: i32,
    /// `getMaxLatitude()`: 90 when the tag is absent.
    pub max_latitude: i32,
    pub placement_order: i32,
    pub const_appearance: i32,
    /// `getRandAppearance1()` (`Rands/iRandApp1`); likewise 2..4.
    pub rand_appearance1: i32,
    pub rand_appearance2: i32,
    pub rand_appearance3: i32,
    pub rand_appearance4: i32,
    /// `getPercentPerPlayer()` (`iPlayer`)
    pub percent_per_player: i32,
    pub tiles_per: i32,
    pub min_land_percent: i32,
    /// `getUniqueRange()` (`iUnique`): map generation keeps this same bonus out of this range.
    pub unique_range: i32,
    pub group_range: i32,
    pub group_rand: i32,
    /// `isOneArea()` (`bArea`)
    pub one_area: bool,
    pub hills: bool,
    pub flatlands: bool,
    pub no_river_side: bool,
    pub normalize: bool,
    // ---- FfH ----
    pub modifier_per_bonus: bool,
    pub bad_attitude: i32,
    pub damage_type: Option<DamageType>,
    pub discover_rand_modifier: i32,
    /// `getFreePromotion()` (C++ `readPass3`)
    pub free_promotion: Option<PromotionType>,
    pub great_people_rate_modifier: i32,
    pub heal_change: i32,
    pub heal_change_enemy: i32,
    pub maintenance_modifier: i32,
    pub mutate_chance: i32,
    pub research_modifier: i32,
    // ---- arrays ----
    /// `getYieldChange(i)` (`YieldChanges`)
    pub yield_change: [i32; YieldType::NUM],
    /// `isTerrain(i)` (`TerrainBooleans`)
    pub terrain: IdxVec<TerrainType, bool>,
    /// `isFeature(i)` (`FeatureBooleans`)
    pub feature: IdxVec<FeatureType, bool>,
    /// `isFeatureTerrain(i)` (`FeatureTerrainBooleans`)
    pub feature_terrain: IdxVec<TerrainType, bool>,
}

/// C++ `CvBonusInfo::read` + `readPass3` (`FreePromotion`).
pub fn read_bonus(rd: &Rd) -> BonusInfo {
    // Without a `<Rands>` block the members keep their constructor default (0).
    let rands = rd.child("Rands");
    let rand = |tag: &str| rands.map_or(0, |r| r.int(tag));
    BonusInfo {
        base: InfoBase::read(rd),
        bonus_class_type: rd.rf("BonusClassType"),
        tech_reveal: rd.rf("TechReveal"),
        tech_city_trade: rd.rf("TechCityTrade"),
        tech_obsolete: rd.rf("TechObsolete"),
        ai_trade_modifier: rd.int("iAITradeModifier"),
        ai_objective: rd.int("iAIObjective"),
        health: rd.int("iHealth"),
        happiness: rd.int("iHappiness"),
        min_area_size: rd.int("iMinAreaSize"),
        min_latitude: rd.int("iMinLatitude"),
        max_latitude: rd.int_or("iMaxLatitude", 90),
        placement_order: rd.int("iPlacementOrder"),
        const_appearance: rd.int("iConstAppearance"),
        rand_appearance1: rand("iRandApp1"),
        rand_appearance2: rand("iRandApp2"),
        rand_appearance3: rand("iRandApp3"),
        rand_appearance4: rand("iRandApp4"),
        percent_per_player: rd.int("iPlayer"),
        tiles_per: rd.int("iTilesPer"),
        min_land_percent: rd.int("iMinLandPercent"),
        unique_range: rd.int("iUnique"),
        group_range: rd.int("iGroupRange"),
        group_rand: rd.int("iGroupRand"),
        one_area: rd.bool("bArea"),
        hills: rd.bool("bHills"),
        flatlands: rd.bool("bFlatlands"),
        no_river_side: rd.bool("bNoRiverSide"),
        normalize: rd.bool("bNormalize"),
        modifier_per_bonus: rd.bool("bModifierPerBonus"),
        bad_attitude: rd.int("iBadAttitude"),
        damage_type: rd.rf("DamageType"),
        discover_rand_modifier: rd.int("iDiscoverRandModifier"),
        free_promotion: rd.rf("FreePromotion"),
        great_people_rate_modifier: rd.int("iGreatPeopleRateModifier"),
        heal_change: rd.int("iHealChange"),
        heal_change_enemy: rd.int("iHealChangeEnemy"),
        maintenance_modifier: rd.int("iMaintenanceModifier"),
        mutate_chance: rd.int("iMutateChance"),
        research_modifier: rd.int("iResearchModifier"),
        yield_change: rd.yields("YieldChanges"),
        terrain: rd.pairs_bool("TerrainBooleans"),
        feature: rd.pairs_bool("FeatureBooleans"),
        feature_terrain: rd.pairs_bool("FeatureTerrainBooleans"),
    }
}

// ---------------------------------------------------------------------------------------
// ImprovementInfo
// ---------------------------------------------------------------------------------------

/// C++ `CvImprovementBonusInfo`: what an improvement does with one bonus (a
/// `<BonusTypeStruct>`), filled by `CvXMLLoadUtility::SetImprovementBonuses`. Its
/// `CvInfoBase` part is never read, so there is no `base` field.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImprovementBonusInfo {
    /// `getDiscoverRand()`: 1-in-N chance per turn that a worked plot with this improvement
    /// discovers the bonus (`CvPlot::doImprovement`, before the player's discover modifier
    /// and game-speed scaling); 0 = never.
    pub discover_rand: i32,
    pub bonus_makes_valid: bool,
    pub bonus_trade: bool,
    /// `getYieldChange(i)` (`YieldChanges`)
    pub yield_change: [i32; YieldType::NUM],
}

/// C++ `CvImprovementInfo` (Terrain/CIV4ImprovementInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct ImprovementInfo {
    pub base: InfoBase,
    pub advanced_start_cost: i32,
    pub advanced_start_cost_increase: i32,
    pub tiles_per_goody: i32,
    /// `getGoodyUniqueRange()` (`iGoodyRange`)
    pub goody_unique_range: i32,
    /// `getFeatureGrowthProbability()` (`iFeatureGrowth`)
    pub feature_growth_probability: i32,
    pub upgrade_time: i32,
    pub air_bomb_defense: i32,
    pub defense_modifier: i32,
    pub happiness: i32,
    pub pillage_gold: i32,
    /// `getImprovementPillage()`: what is left when this improvement is pillaged
    /// (C++ `readPass2`).
    pub improvement_pillage: Option<ImprovementType>,
    /// `getImprovementUpgrade()` (C++ `readPass2`)
    pub improvement_upgrade: Option<ImprovementType>,
    /// `isActsAsCity()`. C++ quirk: the constructor default is `true`, but `read()` resets
    /// it to `false` when the tag is absent (every FFH2 entry sets it explicitly).
    pub acts_as_city: bool,
    pub hills_makes_valid: bool,
    pub fresh_water_makes_valid: bool,
    pub river_side_makes_valid: bool,
    pub no_fresh_water: bool,
    pub requires_flatlands: bool,
    pub requires_river_side: bool,
    pub requires_irrigation: bool,
    pub carries_irrigation: bool,
    pub requires_feature: bool,
    pub water: bool,
    pub goody: bool,
    pub permanent: bool,
    pub outside_borders: bool,
    // ---- FfH ----
    pub requires_peak: bool,
    pub unique: bool,
    /// `getAppearanceProbability()`: per-10000 chance per eligible plot at map generation.
    pub appearance_probability: i32,
    pub heal_rate_change: i32,
    pub range: i32,
    pub range_defense_modifier: i32,
    pub bonus_convert: Option<BonusType>,
    pub feature_upgrade: Option<FeatureType>,
    /// `getPrereqCivilization()` (C++ `readPass3`)
    pub prereq_civilization: Option<CivilizationType>,
    /// `getPythonAtRange()`: Python called (spell module `atRange`, if the plot's Python is
    /// active) when a unit moves onto an ADJACENT plot (`CvUnit::setXY`).
    pub python_at_range: String,
    /// `getPythonOnMove()`: Python called (spell module `onMove`, if the plot's Python is
    /// active) when a unit moves onto the plot (`CvUnit::setXY`).
    pub python_on_move: String,
    /// `getSpawnUnitType()` (C++ `readPass3`)
    pub spawn_unit_type: Option<UnitType>,
    pub visibility_change: i32,
    // ---- arrays ----
    /// `getPrereqNatureYield(i)` (`PrereqNatureYields`)
    pub prereq_nature_yield: [i32; YieldType::NUM],
    /// `getYieldChange(i)` (`YieldChanges`)
    pub yield_change: [i32; YieldType::NUM],
    pub river_side_yield_change: [i32; YieldType::NUM],
    pub hills_yield_change: [i32; YieldType::NUM],
    /// `getIrrigatedYieldChange(i)` (C++ member `m_piIrrigatedChange`)
    pub irrigated_yield_change: [i32; YieldType::NUM],
    /// `getTerrainMakesValid(i)` (`TerrainMakesValids`)
    pub terrain_makes_valid: IdxVec<TerrainType, bool>,
    /// `getFeatureMakesValid(i)` (`FeatureMakesValids`)
    pub feature_makes_valid: IdxVec<FeatureType, bool>,
    /// `getTechYieldChanges(tech, yield)` (`TechYieldChanges`)
    pub tech_yield_changes: IdxVec<TechType, [i32; YieldType::NUM]>,
    /// `getRouteYieldChanges(route, yield)` (`RouteYieldChanges`)
    pub route_yield_changes: IdxVec<RouteType, [i32; YieldType::NUM]>,
    /// C++ `m_paImprovementBonus` (`BonusTypeStructs`), one entry per bonus:
    /// `getImprovementBonusYield(b, y)` is `improvement_bonus[b].yield_change[y]`;
    /// `isImprovementBonusMakesValid(b)`, `isImprovementBonusTrade(b)` and
    /// `getImprovementBonusDiscoverRand(b)` are its other fields.
    pub improvement_bonus: IdxVec<BonusType, ImprovementBonusInfo>,
}

/// C++ `CvImprovementInfo::read`, plus `readPass2` (`ImprovementPillage`,
/// `ImprovementUpgrade`) and `readPass3` (`PrereqCivilization`, `SpawnUnitType`).
pub fn read_improvement(rd: &Rd) -> ImprovementInfo {
    ImprovementInfo {
        base: InfoBase::read(rd),
        advanced_start_cost: rd.int("iAdvancedStartCost"),
        advanced_start_cost_increase: rd.int("iAdvancedStartCostIncrease"),
        tiles_per_goody: rd.int("iTilesPerGoody"),
        goody_unique_range: rd.int("iGoodyRange"),
        feature_growth_probability: rd.int("iFeatureGrowth"),
        upgrade_time: rd.int("iUpgradeTime"),
        air_bomb_defense: rd.int("iAirBombDefense"),
        defense_modifier: rd.int("iDefenseModifier"),
        happiness: rd.int("iHappiness"),
        pillage_gold: rd.int("iPillageGold"),
        improvement_pillage: rd.rf("ImprovementPillage"),
        improvement_upgrade: rd.rf("ImprovementUpgrade"),
        acts_as_city: rd.bool("bActsAsCity"),
        hills_makes_valid: rd.bool("bHillsMakesValid"),
        fresh_water_makes_valid: rd.bool("bFreshWaterMakesValid"),
        river_side_makes_valid: rd.bool("bRiverSideMakesValid"),
        no_fresh_water: rd.bool("bNoFreshWater"),
        requires_flatlands: rd.bool("bRequiresFlatlands"),
        requires_river_side: rd.bool("bRequiresRiverSide"),
        requires_irrigation: rd.bool("bRequiresIrrigation"),
        carries_irrigation: rd.bool("bCarriesIrrigation"),
        requires_feature: rd.bool("bRequiresFeature"),
        water: rd.bool("bWater"),
        goody: rd.bool("bGoody"),
        permanent: rd.bool("bPermanent"),
        outside_borders: rd.bool("bOutsideBorders"),
        requires_peak: rd.bool("bRequiresPeak"),
        unique: rd.bool("bUnique"),
        appearance_probability: rd.int("iAppearanceProbability"),
        heal_rate_change: rd.int("iHealRateChange"),
        range: rd.int("iRange"),
        range_defense_modifier: rd.int("iRangeDefenseModifier"),
        bonus_convert: rd.rf("BonusConvert"),
        feature_upgrade: rd.rf("FeatureUpgrade"),
        prereq_civilization: rd.rf("PrereqCivilization"),
        python_at_range: rd.string("PythonAtRange"),
        python_on_move: rd.string("PythonOnMove"),
        spawn_unit_type: rd.rf("SpawnUnitType"),
        visibility_change: rd.int("iVisibilityChange"),
        prereq_nature_yield: rd.yields("PrereqNatureYields"),
        yield_change: rd.yields("YieldChanges"),
        river_side_yield_change: rd.yields("RiverSideYieldChange"),
        hills_yield_change: rd.yields("HillsYieldChange"),
        irrigated_yield_change: rd.yields("IrrigatedYieldChange"),
        terrain_makes_valid: rd.pairs_bool("TerrainMakesValids"),
        feature_makes_valid: rd.pairs_bool("FeatureMakesValids"),
        tech_yield_changes: yields_by_type(rd, "TechYieldChanges", "PrereqTech", "TechYields"),
        route_yield_changes: yields_by_type(rd, "RouteYieldChanges", "RouteType", "RouteYields"),
        improvement_bonus: improvement_bonuses(rd),
    }
}

/// The `TechYieldChanges`/`RouteYieldChanges` loops of `CvImprovementInfo::read`:
/// `Init2DIntList` (all zero), then every child of `<tag>` whose `<key_tag>` names a type
/// gets that type's yields from its `<yields_tag>` (`SetYields`; zeros if absent). Children
/// with a missing or unknown key are skipped; a repeated key overwrites.
fn yields_by_type<K: InfoId>(
    rd: &Rd,
    tag: &str,
    key_tag: &str,
    yields_tag: &str,
) -> IdxVec<K, [i32; YieldType::NUM]> {
    let mut v = IdxVec::from_elem([0; YieldType::NUM], rd.reg().count::<K>());
    if let Some(list) = rd.child(tag) {
        for entry in list.children() {
            if let Some(k) = entry.rf::<K>(key_tag) {
                v[k] = entry.yields(yields_tag);
            }
        }
    }
    v
}

/// C++ `CvXMLLoadUtility::SetImprovementBonuses` (`BonusTypeStructs`). Every bonus gets a
/// zeroed entry (`InitImprovementBonusList`). For each `<BonusTypeStruct>` whose FIRST child
/// names a bonus, the next three children are read by POSITION as `bBonusMakesValid`,
/// `bBonusTrade` and `iDiscoverRand` (C++ `GetNextXmlVal` ignores tag names; a missing
/// sibling leaves the default), while `YieldChanges` is looked up by name. A repeated bonus
/// overwrites the earlier entry.
fn improvement_bonuses(rd: &Rd) -> IdxVec<BonusType, ImprovementBonusInfo> {
    let mut v = IdxVec::from_elem(ImprovementBonusInfo::default(), rd.reg().count::<BonusType>());
    let Some(list) = rd.child("BonusTypeStructs") else {
        return v;
    };
    for s in list.children() {
        let mut kids = s.e.children();
        let Some(key) = kids.next() else { continue };
        let Some(bonus) = s.resolve::<BonusType>(key.text()) else { continue };
        let (makes_valid, trade, discover) = (kids.next(), kids.next(), kids.next());
        v[bonus] = ImprovementBonusInfo {
            bonus_makes_valid: next_bool(&s, makes_valid, "bBonusMakesValid"),
            bonus_trade: next_bool(&s, trade, "bBonusTrade"),
            discover_rand: next_int(&s, discover, "iDiscoverRand"),
            yield_change: s.yields("YieldChanges"),
        };
    }
    v
}

/// C++ `GetNextXmlVal(bool*)` on an already-located sibling: its value, or `false` when
/// there is no such sibling.
fn next_bool(rd: &Rd, e: Option<Elem>, expected: &str) -> bool {
    e.is_some_and(|e| {
        check_position(rd, e, expected);
        parse_bool(e.text()).unwrap_or(false)
    })
}

/// C++ `GetNextXmlVal(int*)` on an already-located sibling: its value, or 0 when there is
/// no such sibling. Warns on a malformed number, like `Rd::int_or`.
fn next_int(rd: &Rd, e: Option<Elem>, expected: &str) -> i32 {
    e.map_or(0, |e| {
        check_position(rd, e, expected);
        parse_int(e.text()).unwrap_or_else(|| {
            if !e.text().is_empty() {
                rd.cx.warn(format!("bad int in <{}>: {:?}", e.name(), e.text()));
            }
            0
        })
    })
}

/// Diagnostic only: values are taken by position exactly as in C++, but a tag other than
/// the one the schema puts there means the data does not say what it seems to say.
fn check_position(rd: &Rd, e: Elem, expected: &str) {
    if e.name() != expected {
        rd.cx.warn(format!("<{}>: <{}> where <{expected}> belongs (read by position, as in C++)", rd.e.name(), e.name()));
    }
}

/// Same number syntax as `Rd::int_or` (registry.rs): integers, or floats truncated.
fn parse_int(s: &str) -> Option<i32> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<i32>().ok().or_else(|| t.parse::<f64>().ok().map(|f| f as i32))
}

/// Same boolean syntax as `Rd::bool_or` (registry.rs).
fn parse_bool(s: &str) -> Option<bool> {
    match s.trim() {
        "1" | "true" | "True" | "TRUE" => Some(true),
        "0" | "false" | "False" | "FALSE" => Some(false),
        "" => None,
        other => parse_int(other).map(|i| i != 0),
    }
}

// ---------------------------------------------------------------------------------------
// RouteInfo
// ---------------------------------------------------------------------------------------

/// C++ `CvRouteInfo` (Misc/CIV4RouteInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct RouteInfo {
    pub base: InfoBase,
    pub advanced_start_cost: i32,
    pub advanced_start_cost_increase: i32,
    pub value: i32,
    /// `getMovementCost()` (`iMovement`)
    pub movement_cost: i32,
    /// `getFlatMovementCost()` (`iFlatMovement`)
    pub flat_movement_cost: i32,
    /// `getPrereqBonus()` (`BonusType`)
    pub prereq_bonus: Option<BonusType>,
    /// `getYieldChange(i)` (`Yields`)
    pub yield_change: [i32; YieldType::NUM],
    /// `getTechMovementChange(i)` (`TechMovementChanges`)
    pub tech_movement_change: IdxVec<TechType, i32>,
    /// `getPrereqOrBonus(i)` (`PrereqOrBonuses`): `GC.getNUM_ROUTE_PREREQ_OR_BONUSES()`
    /// slots filled in order, `None` = empty (C++ -1).
    pub prereq_or_bonus: Vec<Option<BonusType>>,
}

/// C++ `CvRouteInfo::read`.
pub fn read_route(rd: &Rd) -> RouteInfo {
    RouteInfo {
        base: InfoBase::read(rd),
        advanced_start_cost: rd.int("iAdvancedStartCost"),
        advanced_start_cost_increase: rd.int("iAdvancedStartCostIncrease"),
        value: rd.int("iValue"),
        movement_cost: rd.int("iMovement"),
        flat_movement_cost: rd.int("iFlatMovement"),
        prereq_bonus: rd.rf("BonusType"),
        yield_change: rd.yields("Yields"),
        tech_movement_change: rd.pairs_int("TechMovementChanges", 0),
        prereq_or_bonus: rd.ref_list_fixed("PrereqOrBonuses", NUM_ROUTE_PREREQ_OR_BONUSES),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Ctx, Registry};
    use crate::xml::XmlDoc;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn registry() -> Registry {
        let mut reg = Registry::new();
        reg.register_class("TechInfo", names(&["TECH_X", "TECH_Y", "TECH_Z"]));
        reg.register_class("RouteInfo", names(&["ROUTE_ROAD", "ROUTE_RAILROAD"]));
        reg.register_class("BonusInfo", names(&["BONUS_A", "BONUS_B", "BONUS_C"]));
        reg
    }

    /// Runs `f` on the root element of `xml` and returns its result and the warnings.
    fn read<T>(xml: &str, f: impl Fn(&Rd) -> T) -> (T, Vec<String>) {
        let reg = registry();
        let doc = XmlDoc::parse(xml).expect("test xml parses");
        let cx = Ctx::new(&reg, "test.xml");
        let out = f(&Rd::new(doc.root(), &cx));
        (out, cx.warnings.into_inner())
    }

    #[test]
    fn tech_and_route_yield_changes() {
        let xml = r#"<ImprovementInfo>
            <TechYieldChanges>
                <TechYieldChange>
                    <PrereqTech>TECH_Y</PrereqTech>
                    <TechYields><iYield>1</iYield><iYield>2</iYield></TechYields>
                </TechYieldChange>
                <TechYieldChange><PrereqTech>TECH_Z</PrereqTech></TechYieldChange>
                <TechYieldChange><TechYields><iYield>9</iYield></TechYields></TechYieldChange>
            </TechYieldChanges>
            <RouteYieldChanges>
                <RouteYieldChange>
                    <RouteType>ROUTE_RAILROAD</RouteType>
                    <RouteYields><iYield>0</iYield><iYield>1</iYield><iYield>0</iYield></RouteYields>
                </RouteYieldChange>
            </RouteYieldChanges>
        </ImprovementInfo>"#;
        let ((tech, route), warnings) = read(xml, |rd| {
            let tech: IdxVec<TechType, _> = yields_by_type(rd, "TechYieldChanges", "PrereqTech", "TechYields");
            let route: IdxVec<RouteType, _> = yields_by_type(rd, "RouteYieldChanges", "RouteType", "RouteYields");
            (tech, route)
        });
        // Short yield lists are zero-padded; a key without yields is all zero; an entry
        // without a key is ignored.
        assert_eq!(tech.raw, vec![[0, 0, 0], [1, 2, 0], [0, 0, 0]]);
        assert_eq!(route.raw, vec![[0, 0, 0], [0, 1, 0]]);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn improvement_bonuses_are_read_positionally() {
        let xml = r#"<ImprovementInfo>
            <BonusTypeStructs>
                <BonusTypeStruct>
                    <BonusType>BONUS_B</BonusType>
                    <bBonusMakesValid>1</bBonusMakesValid>
                    <bBonusTrade>0</bBonusTrade>
                    <iDiscoverRand>100</iDiscoverRand>
                    <YieldChanges><iYieldChange>0</iYieldChange><iYieldChange>1</iYieldChange></YieldChanges>
                </BonusTypeStruct>
                <BonusTypeStruct>
                    <BonusType>BONUS_C</BonusType>
                    <bBonusTrade>1</bBonusTrade>
                </BonusTypeStruct>
            </BonusTypeStructs>
        </ImprovementInfo>"#;
        let (v, warnings) = read(xml, improvement_bonuses);
        assert_eq!(v.len(), 3);
        assert_eq!(v.raw[0], ImprovementBonusInfo::default());
        assert_eq!(
            v.raw[1],
            ImprovementBonusInfo { discover_rand: 100, bonus_makes_valid: true, bonus_trade: false, yield_change: [0, 1, 0] }
        );
        // C++ takes the second child as bBonusMakesValid whatever its tag; the missing
        // third and fourth children leave the defaults.
        assert_eq!(v.raw[2], ImprovementBonusInfo { bonus_makes_valid: true, ..Default::default() });
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn faithful_defaults_for_absent_tags() {
        let (b, _) = read("<BonusInfo><Type>BONUS_A</Type></BonusInfo>", read_bonus);
        assert_eq!(b.max_latitude, 90);
        assert_eq!((b.rand_appearance1, b.tech_reveal, b.bonus_class_type), (0, None, None));

        let (t, _) = read("<TerrainInfo><Type>T</Type></TerrainInfo>", read_terrain);
        assert_eq!((t.plot_counter_down, t.plot_counter_up), (0, 0));
        assert_eq!((t.terrain_down, t.civilization_yield_type), (None, None));

        let (i, _) = read("<ImprovementInfo><Type>I</Type></ImprovementInfo>", read_improvement);
        assert!(!i.acts_as_city);
        assert_eq!(i.improvement_bonus.len(), 3);
        assert_eq!(i.tech_yield_changes.len(), 3);
        assert_eq!(i.route_yield_changes.len(), 2);

        let (r, _) = read("<RouteInfo><Type>R</Type><PrereqOrBonuses><BonusType>BONUS_C</BonusType></PrereqOrBonuses></RouteInfo>", read_route);
        assert_eq!(r.prereq_or_bonus, vec![Some(BonusType(2)), None]);
        assert_eq!(r.tech_movement_change.raw, vec![0, 0, 0]);
    }
}
