//! Info classes: SpecialUnitInfo, UnitClassInfo, PromotionInfo, BuildInfo, UnitInfo, SpellInfo, AutomateInfo, CommandInfo, MissionInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp.
//!
//! - `readPass2`/`readPass3` references (promotion prereqs, `DefaultUnit`, `PrereqCiv`,
//!   `UpgradeCiv`, `UnitConvertFromCombat`, ...) are resolved directly in `read_*`: the registry
//!   knows every type before any table is read.
//! - `CvHotkeyInfo::read` (base of the Promotion/Spell/Build/Unit/Mission/Command/Automate infos)
//!   only reads hotkeys, modifier keys and `iOrderPriority` (action-button sort order). All of it
//!   is UI, so it is not ported; these infos read [`InfoBase`] only.
//! - Not ported (graphics/audio/UI only): sounds, art define tags, unit mesh groups (group size,
//!   wave sizes, max speed, pad time), formation types, animation paths, entity events, effects,
//!   waypoints, unit art styles, advisor types, popup images.
//! - Members C++ sets outside `read()` while building the UI action list
//!   (`SetGlobalActionInfo`: `m_iCommandType`, `m_iMissionType`, `m_iActionInfoIndex`) are
//!   not ported either; they are constants (`COMMAND_PROMOTION`, `COMMAND_CAST`,
//!   `COMMAND_UPGRADE`, `MISSION_BUILD`) only used by the interface and entity animations.

use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

// ---------------------------------------------------------------------------------------------
// SpecialUnitInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvSpecialUnitInfo` (Units/CIV4SpecialUnitInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct SpecialUnitInfo {
    pub base: InfoBase,
    pub valid: bool,
    pub city_load: bool,
    /// C++ `isCarrierUnitAIType(i)`. Always all `false`: see the quirk note in [`read_special_unit`].
    pub carrier_unit_ai_type: IdxVec<UnitAIType, bool>,
    pub production_traits: IdxVec<TraitType, i32>,
}

/// C++ `CvSpecialUnitInfo::read`.
pub fn read_special_unit(rd: &Rd) -> SpecialUnitInfo {
    SpecialUnitInfo {
        base: InfoBase::read(rd),
        valid: rd.bool("bValid"),
        city_load: rd.bool("bCityLoad"),
        // C++ quirk: C++ reads <CarrierUnitAITypes>, but the XML and its schema use
        // <CarrierUnitAIs>, so the list is never loaded and `isCarrierUnitAIType` is always
        // false (stock BTS bug, inherited by FFH2; consulted by CvPlayerAI::AI_unitValue).
        carrier_unit_ai_type: rd.pairs_bool::<UnitAIType>("CarrierUnitAITypes"),
        production_traits: rd.pairs_int::<TraitType>("ProductionTraits", 0),
    }
}

// ---------------------------------------------------------------------------------------------
// UnitClassInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvUnitClassInfo` (Units/CIV4UnitClassInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct UnitClassInfo {
    pub base: InfoBase,
    pub max_global_instances: i32,
    pub max_team_instances: i32,
    pub max_player_instances: i32,
    pub instance_cost_modifier: i32,
    /// C++ `getDefaultUnitIndex()` (`<DefaultUnit>`, resolved by C++ `readPass3`).
    pub default_unit_index: Option<UnitType>,
}

/// C++ `CvUnitClassInfo::read` + `readPass3`.
pub fn read_unit_class(rd: &Rd) -> UnitClassInfo {
    UnitClassInfo {
        base: InfoBase::read(rd),
        // The C++ default for the three instance limits is 0 (not -1) when the tag is absent;
        // every FFH2 entry sets them explicitly.
        max_global_instances: rd.int("iMaxGlobalInstances"),
        max_team_instances: rd.int("iMaxTeamInstances"),
        max_player_instances: rd.int("iMaxPlayerInstances"),
        instance_cost_modifier: rd.int("iInstanceCostModifier"),
        default_unit_index: rd.rf::<UnitType>("DefaultUnit"),
    }
}

// ---------------------------------------------------------------------------------------------
// PromotionInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvPromotionInfo` (Units/CIV4PromotionInfos.xml)
///
/// Not ported: `Sound`, `LayerAnimationPath`, `UnitArtStyleType` (unit art style, C++ `readPass3`)
/// and FfH `iGroupSize` (number of figures drawn; only feeds `CvUnit::getSubUnitsAlive`, which is
/// battle-animation code).
#[derive(Debug, Clone, Default)]
pub struct PromotionInfo {
    pub base: InfoBase,
    /// C++ `getPrereqPromotion()` (`<PromotionPrereq>`, C++ `readPass2`).
    pub prereq_promotion: Option<PromotionType>,
    /// C++ `getPrereqOrPromotion1()` (`<PromotionPrereqOr1>`, C++ `readPass2`).
    pub prereq_or_promotion1: Option<PromotionType>,
    /// C++ `getPrereqOrPromotion2()` (`<PromotionPrereqOr2>`, C++ `readPass2`).
    pub prereq_or_promotion2: Option<PromotionType>,
    pub tech_prereq: Option<TechType>,
    pub state_religion_prereq: Option<ReligionType>,
    pub visibility_change: i32,
    pub moves_change: i32,
    pub move_discount_change: i32,
    pub air_range_change: i32,
    pub intercept_change: i32,
    pub evasion_change: i32,
    pub withdrawal_change: i32,
    pub cargo_change: i32,
    pub collateral_damage_change: i32,
    pub bombard_rate_change: i32,
    pub first_strikes_change: i32,
    pub chance_first_strikes_change: i32,
    pub enemy_heal_change: i32,
    pub neutral_heal_change: i32,
    pub friendly_heal_change: i32,
    pub same_tile_heal_change: i32,
    pub adjacent_tile_heal_change: i32,
    pub combat_percent: i32,
    /// C++ `getCityAttackPercent()` (`<iCityAttack>`).
    pub city_attack_percent: i32,
    /// C++ `getCityDefensePercent()` (`<iCityDefense>`).
    pub city_defense_percent: i32,
    /// C++ `getHillsAttackPercent()` (`<iHillsAttack>`).
    pub hills_attack_percent: i32,
    /// C++ `getHillsDefensePercent()` (`<iHillsDefense>`).
    pub hills_defense_percent: i32,
    pub revolt_protection: i32,
    pub collateral_damage_protection: i32,
    pub pillage_change: i32,
    pub upgrade_discount: i32,
    pub experience_percent: i32,
    pub kamikaze_percent: i32,
    /// C++ `isLeader()`. A leader promotion also forces `base.graphical_only` (as in C++).
    pub leader: bool,
    pub blitz: bool,
    pub amphib: bool,
    pub river: bool,
    pub enemy_route: bool,
    pub always_heal: bool,
    pub hills_double_move: bool,
    pub immune_to_first_strikes: bool,
    /// C++ `getTerrainAttackPercent(i)` (`<TerrainAttacks>`).
    pub terrain_attack_percent: IdxVec<TerrainType, i32>,
    /// C++ `getTerrainDefensePercent(i)` (`<TerrainDefenses>`).
    pub terrain_defense_percent: IdxVec<TerrainType, i32>,
    /// C++ `getFeatureAttackPercent(i)` (`<FeatureAttacks>`).
    pub feature_attack_percent: IdxVec<FeatureType, i32>,
    /// C++ `getFeatureDefensePercent(i)` (`<FeatureDefenses>`).
    pub feature_defense_percent: IdxVec<FeatureType, i32>,
    /// C++ `getUnitCombatModifierPercent(i)` (`<UnitCombatMods>`).
    pub unit_combat_modifier_percent: IdxVec<UnitCombatType, i32>,
    /// C++ `getDomainModifierPercent(i)` (`<DomainMods>`).
    pub domain_modifier_percent: IdxVec<DomainType, i32>,
    /// C++ `getTerrainDoubleMove(i)` (`<TerrainDoubleMoves>`).
    pub terrain_double_move: IdxVec<TerrainType, bool>,
    /// C++ `getFeatureDoubleMove(i)` (`<FeatureDoubleMoves>`).
    pub feature_double_move: IdxVec<FeatureType, bool>,
    /// C++ `getUnitCombat(i)` (`<UnitCombats>`): unit combat types that may take the promotion.
    pub unit_combat: IdxVec<UnitCombatType, bool>,
    // ---- FfH ----
    /// C++ `isAIControl()`.
    pub ai_control: bool,
    pub boarding: bool,
    pub only_defensive: bool,
    pub dispellable: bool,
    pub double_fortify_bonus: bool,
    pub equipment: bool,
    pub fear: bool,
    pub flying: bool,
    pub held: bool,
    pub hidden_nationality: bool,
    pub ignore_building_defense: bool,
    pub immortal: bool,
    pub immune_to_capture: bool,
    pub immune_to_defensive_strike: bool,
    pub immune_to_fear: bool,
    pub immune_to_magic: bool,
    pub invisible: bool,
    pub mutation: bool,
    pub not_alive: bool,
    pub prereq_alive: bool,
    pub race: bool,
    pub removed_by_casting: bool,
    pub removed_by_combat: bool,
    pub removed_when_healed: bool,
    pub see_invisible: bool,
    pub target_weakest_unit: bool,
    pub target_weakest_unit_counter: bool,
    pub twincast: bool,
    pub validate: bool,
    pub water_walking: bool,
    pub ai_weight: i32,
    pub betrayal_chance: i32,
    pub better_defender_than_percent: i32,
    pub caster_resist_modify: i32,
    pub combat_capture_percent: i32,
    pub combat_heal_percent: i32,
    pub combat_limit: i32,
    pub combat_percent_defense: i32,
    pub combat_percent_global_counter: i32,
    pub combat_percent_in_borders: i32,
    pub defensive_strike_chance: i32,
    pub defensive_strike_damage: i32,
    pub expire_chance: i32,
    pub extra_combat_str: i32,
    pub extra_combat_defense: i32,
    /// C++ `getFreeXPPerTurn()`.
    pub free_xp_per_turn: i32,
    /// C++ `getFreeXPFromCombat()`.
    pub free_xp_from_combat: i32,
    pub gold_from_combat: i32,
    pub min_level: i32,
    pub modify_global_counter: i32,
    pub modify_global_counter_on_combat: i32,
    pub resist_magic: i32,
    /// C++ `getSpellCasterXP()`.
    pub spell_caster_xp: i32,
    pub spell_damage_modify: i32,
    pub work_rate_modify: i32,
    pub capture_unit_combat: Option<UnitCombatType>,
    /// C++ `getPromotionCombatApply()` (C++ `readPass2`).
    pub promotion_combat_apply: Option<PromotionType>,
    pub promotion_immune1: Option<PromotionType>,
    pub promotion_immune2: Option<PromotionType>,
    pub promotion_immune3: Option<PromotionType>,
    pub promotion_random_apply: Option<PromotionType>,
    pub promotion_summon_perk: Option<PromotionType>,
    pub bonus_prereq: Option<BonusType>,
    /// C++ `getPromotionPrereqOr3()` (`<PromotionPrereqOr3>`, C++ `readPass2`).
    pub promotion_prereq_or3: Option<PromotionType>,
    /// C++ `getPromotionPrereqOr4()` (`<PromotionPrereqOr4>`, C++ `readPass2`).
    pub promotion_prereq_or4: Option<PromotionType>,
    /// C++ `getPromotionPrereqAnd()` (`<PromotionPrereqAnd>`, C++ `readPass2`).
    pub promotion_prereq_and: Option<PromotionType>,
    pub promotion_next_level: Option<PromotionType>,
    /// Python callback run every turn for units with this promotion (`<PyPerTurn>`).
    pub py_per_turn: String,
    pub promotion_combat_type: Option<PromotionType>,
    pub promotion_combat_mod: i32,
    /// C++ `getBonusAffinity(i)` (`<BonusAffinities>`).
    pub bonus_affinity: IdxVec<BonusType, i32>,
    /// C++ `getDamageTypeCombat(i)` (`<DamageTypeCombats>`).
    pub damage_type_combat: IdxVec<DamageType, i32>,
    /// C++ `getDamageTypeResist(i)` (`<DamageTypeResists>`).
    pub damage_type_resist: IdxVec<DamageType, i32>,
}

/// C++ `CvPromotionInfo::read` + `readPass2` + `readPass3`.
pub fn read_promotion(rd: &Rd) -> PromotionInfo {
    let mut base = InfoBase::read(rd);
    let leader = rd.bool("bLeader");
    if leader {
        // C++: "don't show in Civilopedia list of promotions".
        base.graphical_only = true;
    }
    PromotionInfo {
        base,
        // readPass2 (C++ uses GC.getInfoTypeForString for these three).
        prereq_promotion: rd.rf::<PromotionType>("PromotionPrereq"),
        prereq_or_promotion1: rd.rf::<PromotionType>("PromotionPrereqOr1"),
        prereq_or_promotion2: rd.rf::<PromotionType>("PromotionPrereqOr2"),
        tech_prereq: rd.rf::<TechType>("TechPrereq"),
        state_religion_prereq: rd.rf::<ReligionType>("StateReligionPrereq"),
        leader,
        blitz: rd.bool("bBlitz"),
        amphib: rd.bool("bAmphib"),
        river: rd.bool("bRiver"),
        enemy_route: rd.bool("bEnemyRoute"),
        always_heal: rd.bool("bAlwaysHeal"),
        hills_double_move: rd.bool("bHillsDoubleMove"),
        immune_to_first_strikes: rd.bool("bImmuneToFirstStrikes"),
        visibility_change: rd.int("iVisibilityChange"),
        moves_change: rd.int("iMovesChange"),
        move_discount_change: rd.int("iMoveDiscountChange"),
        air_range_change: rd.int("iAirRangeChange"),
        intercept_change: rd.int("iInterceptChange"),
        evasion_change: rd.int("iEvasionChange"),
        withdrawal_change: rd.int("iWithdrawalChange"),
        cargo_change: rd.int("iCargoChange"),
        collateral_damage_change: rd.int("iCollateralDamageChange"),
        bombard_rate_change: rd.int("iBombardRateChange"),
        first_strikes_change: rd.int("iFirstStrikesChange"),
        chance_first_strikes_change: rd.int("iChanceFirstStrikesChange"),
        enemy_heal_change: rd.int("iEnemyHealChange"),
        neutral_heal_change: rd.int("iNeutralHealChange"),
        friendly_heal_change: rd.int("iFriendlyHealChange"),
        same_tile_heal_change: rd.int("iSameTileHealChange"),
        adjacent_tile_heal_change: rd.int("iAdjacentTileHealChange"),
        combat_percent: rd.int("iCombatPercent"),
        city_attack_percent: rd.int("iCityAttack"),
        city_defense_percent: rd.int("iCityDefense"),
        hills_attack_percent: rd.int("iHillsAttack"),
        hills_defense_percent: rd.int("iHillsDefense"),
        revolt_protection: rd.int("iRevoltProtection"),
        collateral_damage_protection: rd.int("iCollateralDamageProtection"),
        pillage_change: rd.int("iPillageChange"),
        upgrade_discount: rd.int("iUpgradeDiscount"),
        experience_percent: rd.int("iExperiencePercent"),
        kamikaze_percent: rd.int("iKamikazePercent"),
        terrain_attack_percent: rd.pairs_int::<TerrainType>("TerrainAttacks", 0),
        terrain_defense_percent: rd.pairs_int::<TerrainType>("TerrainDefenses", 0),
        feature_attack_percent: rd.pairs_int::<FeatureType>("FeatureAttacks", 0),
        feature_defense_percent: rd.pairs_int::<FeatureType>("FeatureDefenses", 0),
        unit_combat_modifier_percent: rd.pairs_int::<UnitCombatType>("UnitCombatMods", 0),
        domain_modifier_percent: rd.pairs_int::<DomainType>("DomainMods", 0),
        terrain_double_move: rd.pairs_bool::<TerrainType>("TerrainDoubleMoves"),
        feature_double_move: rd.pairs_bool::<FeatureType>("FeatureDoubleMoves"),
        unit_combat: rd.pairs_bool::<UnitCombatType>("UnitCombats"),
        // ---- FfH ----
        ai_control: rd.bool("bAIControl"),
        boarding: rd.bool("bBoarding"),
        only_defensive: rd.bool("bOnlyDefensive"),
        dispellable: rd.bool("bDispellable"),
        double_fortify_bonus: rd.bool("bDoubleFortifyBonus"),
        equipment: rd.bool("bEquipment"),
        fear: rd.bool("bFear"),
        flying: rd.bool("bFlying"),
        held: rd.bool("bHeld"),
        hidden_nationality: rd.bool("bHiddenNationality"),
        ignore_building_defense: rd.bool("bIgnoreBuildingDefense"),
        immortal: rd.bool("bImmortal"),
        immune_to_capture: rd.bool("bImmuneToCapture"),
        immune_to_defensive_strike: rd.bool("bImmuneToDefensiveStrike"),
        immune_to_fear: rd.bool("bImmuneToFear"),
        immune_to_magic: rd.bool("bImmuneToMagic"),
        invisible: rd.bool("bInvisible"),
        mutation: rd.bool("bMutation"),
        not_alive: rd.bool("bNotAlive"),
        prereq_alive: rd.bool("bPrereqAlive"),
        race: rd.bool("bRace"),
        removed_by_casting: rd.bool("bRemovedByCasting"),
        removed_by_combat: rd.bool("bRemovedByCombat"),
        removed_when_healed: rd.bool("bRemovedWhenHealed"),
        see_invisible: rd.bool("bSeeInvisible"),
        target_weakest_unit: rd.bool("bTargetWeakestUnit"),
        target_weakest_unit_counter: rd.bool("bTargetWeakestUnitCounter"),
        twincast: rd.bool("bTwincast"),
        validate: rd.bool("bValidate"),
        water_walking: rd.bool("bWaterWalking"),
        ai_weight: rd.int("iAIWeight"),
        betrayal_chance: rd.int("iBetrayalChance"),
        better_defender_than_percent: rd.int("iBetterDefenderThanPercent"),
        caster_resist_modify: rd.int("iCasterResistModify"),
        combat_capture_percent: rd.int("iCombatCapturePercent"),
        combat_heal_percent: rd.int("iCombatHealPercent"),
        combat_limit: rd.int("iCombatLimit"),
        combat_percent_defense: rd.int("iCombatPercentDefense"),
        combat_percent_global_counter: rd.int("iCombatPercentGlobalCounter"),
        combat_percent_in_borders: rd.int("iCombatPercentInBorders"),
        defensive_strike_chance: rd.int("iDefensiveStrikeChance"),
        defensive_strike_damage: rd.int("iDefensiveStrikeDamage"),
        expire_chance: rd.int("iExpireChance"),
        extra_combat_str: rd.int("iExtraCombatStr"),
        extra_combat_defense: rd.int("iExtraCombatDefense"),
        free_xp_per_turn: rd.int("iFreeXPPerTurn"),
        free_xp_from_combat: rd.int("iFreeXPFromCombat"),
        gold_from_combat: rd.int("iGoldFromCombat"),
        min_level: rd.int("iMinLevel"),
        modify_global_counter: rd.int("iModifyGlobalCounter"),
        modify_global_counter_on_combat: rd.int("iModifyGlobalCounterOnCombat"),
        resist_magic: rd.int("iResistMagic"),
        spell_caster_xp: rd.int("iSpellCasterXP"),
        spell_damage_modify: rd.int("iSpellDamageModify"),
        work_rate_modify: rd.int("iWorkRateModify"),
        capture_unit_combat: rd.rf::<UnitCombatType>("CaptureUnitCombat"),
        bonus_prereq: rd.rf::<BonusType>("BonusPrereq"),
        py_per_turn: rd.string("PyPerTurn"),
        promotion_combat_mod: rd.int("iPromotionCombatMod"),
        bonus_affinity: rd.pairs_int::<BonusType>("BonusAffinities", 0),
        damage_type_combat: rd.pairs_int::<DamageType>("DamageTypeCombats", 0),
        damage_type_resist: rd.pairs_int::<DamageType>("DamageTypeResists", 0),
        // readPass2 (FfH)
        promotion_combat_apply: rd.rf::<PromotionType>("PromotionCombatApply"),
        promotion_immune1: rd.rf::<PromotionType>("PromotionImmune1"),
        promotion_immune2: rd.rf::<PromotionType>("PromotionImmune2"),
        promotion_immune3: rd.rf::<PromotionType>("PromotionImmune3"),
        promotion_random_apply: rd.rf::<PromotionType>("PromotionRandomApply"),
        promotion_summon_perk: rd.rf::<PromotionType>("PromotionSummonPerk"),
        promotion_prereq_or3: rd.rf::<PromotionType>("PromotionPrereqOr3"),
        promotion_prereq_or4: rd.rf::<PromotionType>("PromotionPrereqOr4"),
        promotion_prereq_and: rd.rf::<PromotionType>("PromotionPrereqAnd"),
        promotion_next_level: rd.rf::<PromotionType>("PromotionNextLevel"),
        promotion_combat_type: rd.rf::<PromotionType>("PromotionCombatType"),
    }
}

// ---------------------------------------------------------------------------------------------
// BuildInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvBuildInfo` (Units/CIV4BuildInfos.xml)
///
/// Not ported: `EntityEvent` (worker animation) and `m_iMissionType` (always `MISSION_BUILD`, set
/// by the UI action-list setup).
#[derive(Debug, Clone, Default)]
pub struct BuildInfo {
    pub base: InfoBase,
    pub time: i32,
    pub cost: i32,
    /// C++ `getTechPrereq()` (`<PrereqTech>`).
    pub tech_prereq: Option<TechType>,
    /// C++ `getImprovement()` (`<ImprovementType>`).
    pub improvement: Option<ImprovementType>,
    /// C++ `getRoute()` (`<RouteType>`).
    pub route: Option<RouteType>,
    pub kill: bool,
    /// C++ `getFeatureTech(i)`: tech needed to build on the feature (`<FeatureStructs>`).
    pub feature_tech: IdxVec<FeatureType, Option<TechType>>,
    /// C++ `getFeatureTime(i)`: extra build time on the feature.
    pub feature_time: IdxVec<FeatureType, i32>,
    /// C++ `getFeatureProduction(i)`: production gained when the feature is removed.
    pub feature_production: IdxVec<FeatureType, i32>,
    /// C++ `isFeatureRemove(i)`.
    pub feature_remove: IdxVec<FeatureType, bool>,
}

/// C++ `CvBuildInfo::read`.
pub fn read_build(rd: &Rd) -> BuildInfo {
    let mut info = BuildInfo {
        base: InfoBase::read(rd),
        tech_prereq: rd.rf::<TechType>("PrereqTech"),
        time: rd.int("iTime"),
        cost: rd.int("iCost"),
        kill: rd.bool("bKill"),
        improvement: rd.rf::<ImprovementType>("ImprovementType"),
        route: rd.rf::<RouteType>("RouteType"),
        ..Default::default()
    };
    set_feature_struct(rd, &mut info);
    info
}

/// C++ `CvXMLLoadUtility::SetFeatureStruct`: every feature gets tech -1, time 0, production 0,
/// remove false; then each `<FeatureStruct>` overwrites its feature's entries. Absent child tags
/// take the `GetChildXmlValByName` defaults (`""` -> no tech, 0, false).
fn set_feature_struct(rd: &Rd, info: &mut BuildInfo) {
    let n = rd.reg().count::<FeatureType>();
    info.feature_tech = IdxVec::from_elem(None, n);
    info.feature_time = IdxVec::from_elem(0, n);
    info.feature_production = IdxVec::from_elem(0, n);
    info.feature_remove = IdxVec::from_elem(false, n);
    let Some(list) = rd.child("FeatureStructs") else { return };
    // C++ positions on the first <FeatureStruct> child and then walks all of its siblings.
    for s in list.children().skip_while(|c| c.e.name() != "FeatureStruct") {
        let Some(f) = s.rf::<FeatureType>("FeatureType") else {
            // C++ shows "iFeatureIndex is -1" and then writes out of bounds.
            rd.cx.warn(format!("<FeatureStructs> entry without a valid FeatureType in {}", info.base.type_));
            continue;
        };
        info.feature_tech[f] = s.rf::<TechType>("PrereqTech");
        info.feature_time[f] = s.int("iTime");
        info.feature_production[f] = s.int("iProduction");
        info.feature_remove[f] = s.bool("bRemove");
    }
}

// ---------------------------------------------------------------------------------------------
// UnitInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvUnitInfo` (Units/CIV4UnitInfos.xml)
///
/// Not ported (graphics/UI): `Advisor`, `bRenderBelowWater`, `bRenderAlways`, `UnitMeshGroups`
/// (`getGroupSize`, `getGroupDefinitions`, `getUnitGroupRequired`, wave sizes, `fMaxSpeed`,
/// `fPadTime`, art define tags), `FormationType`, `Image` (popup picture), the art-define button.
#[derive(Debug, Clone, Default)]
pub struct UnitInfo {
    pub base: InfoBase,
    /// C++ `getUnitClassType()` (`<Class>`).
    pub unit_class_type: Option<UnitClassType>,
    /// C++ `getSpecialUnitType()` (`<Special>`).
    pub special_unit_type: Option<SpecialUnitType>,
    /// C++ `getUnitCaptureClassType()` (`<Capture>`): unit class created when this unit is captured.
    pub unit_capture_class_type: Option<UnitClassType>,
    /// C++ `getUnitCombatType()` (`<Combat>`).
    pub unit_combat_type: Option<UnitCombatType>,
    /// C++ `getDomainType()` (`<Domain>`).
    pub domain_type: Option<DomainType>,
    /// C++ `getDefaultUnitAIType()` (`<DefaultUnitAI>`).
    pub default_unit_ai_type: Option<UnitAIType>,
    /// C++ `getInvisibleType()` (`<Invisible>`).
    pub invisible_type: Option<InvisibleType>,
    /// C++ `getSeeInvisibleType(i)` / `getNumSeeInvisibleTypes()` (comma-separated `<SeeInvisible>`,
    /// NONE/unknown tokens dropped).
    pub see_invisible_types: Vec<InvisibleType>,
    pub animal: bool,
    /// C++ `isFoodProduction()` (`<bFood>`).
    pub food_production: bool,
    pub no_bad_goodies: bool,
    pub only_defensive: bool,
    pub no_capture: bool,
    /// C++ `isQuickCombat()`; only decides whether combat is animated (`CvUnit::isCombatVisible`).
    pub quick_combat: bool,
    pub rival_territory: bool,
    pub military_happiness: bool,
    pub military_support: bool,
    pub military_production: bool,
    pub pillage: bool,
    pub spy: bool,
    pub sabotage: bool,
    pub destroy: bool,
    pub steal_plans: bool,
    pub investigate: bool,
    pub counter_spy: bool,
    pub found: bool,
    pub golden_age: bool,
    pub invisible: bool,
    pub first_strike_immune: bool,
    pub no_defensive_bonus: bool,
    pub ignore_building_defense: bool,
    pub can_move_impassable: bool,
    pub can_move_all_terrain: bool,
    pub flat_movement_cost: bool,
    pub ignore_terrain_cost: bool,
    pub nuke_immune: bool,
    pub prereq_bonuses: bool,
    /// C++ `isPrereqReligion()` (`<bPrereqReligion>`); named `is_` because `getPrereqReligion()`
    /// (the religion type) takes `prereq_religion`.
    pub is_prereq_religion: bool,
    /// C++ `isMechUnit()` (`<bMechanized>`). FfH: mechanized units are not alive.
    pub mech_unit: bool,
    pub suicide: bool,
    pub line_of_sight: bool,
    pub hidden_nationality: bool,
    pub always_hostile: bool,
    pub no_reveal_map: bool,
    /// C++ `getUpgradeUnitClass(i)` (`<UnitClassUpgrades>`).
    pub upgrade_unit_class: IdxVec<UnitClassType, bool>,
    /// C++ `getTargetUnitClass(i)` (`<UnitClassTargets>`).
    pub target_unit_class: IdxVec<UnitClassType, bool>,
    /// C++ `getTargetUnitCombat(i)` (`<UnitCombatTargets>`).
    pub target_unit_combat: IdxVec<UnitCombatType, bool>,
    /// C++ `getDefenderUnitClass(i)` (`<UnitClassDefenders>`).
    pub defender_unit_class: IdxVec<UnitClassType, bool>,
    /// C++ `getDefenderUnitCombat(i)` (`<UnitCombatDefenders>`).
    pub defender_unit_combat: IdxVec<UnitCombatType, bool>,
    /// C++ `getFlankingStrikeUnitClass(i)` (`<FlankingStrikes>`).
    pub flanking_strike_unit_class: IdxVec<UnitClassType, i32>,
    /// C++ `getUnitAIType(i)` (`<UnitAIs>`): UnitAIs the AI may assign to this unit.
    pub unit_ai_type: IdxVec<UnitAIType, bool>,
    /// C++ `getNotUnitAIType(i)` (`<NotUnitAIs>`).
    pub not_unit_ai_type: IdxVec<UnitAIType, bool>,
    pub builds: IdxVec<BuildType, bool>,
    pub religion_spreads: IdxVec<ReligionType, i32>,
    pub corporation_spreads: IdxVec<CorporationType, i32>,
    /// C++ `getTerrainPassableTech(i)` (`<TerrainPassableTechs>`).
    pub terrain_passable_tech: IdxVec<TerrainType, Option<TechType>>,
    /// C++ `getFeaturePassableTech(i)` (`<FeaturePassableTechs>`).
    pub feature_passable_tech: IdxVec<FeatureType, Option<TechType>>,
    pub great_peoples: IdxVec<SpecialistType, bool>,
    pub buildings: IdxVec<BuildingType, bool>,
    pub force_buildings: IdxVec<BuildingType, bool>,
    pub holy_city: Option<ReligionType>,
    pub religion_type: Option<ReligionType>,
    pub state_religion: Option<ReligionType>,
    /// C++ `getPrereqReligion()` (`<PrereqReligion>`).
    pub prereq_religion: Option<ReligionType>,
    pub prereq_corporation: Option<CorporationType>,
    pub prereq_building: Option<BuildingType>,
    /// C++ `getPrereqAndTech()` (`<PrereqTech>`).
    pub prereq_and_tech: Option<TechType>,
    /// C++ `getPrereqAndTechs(i)` (`<TechTypes>`). C++ stores a fixed array of
    /// `NUM_UNIT_AND_TECH_PREREQS` (GlobalDefines, 3) entries filled with -1; defines are not
    /// available while reading, so this keeps only the listed techs, in order (every C++ caller
    /// scans all slots and skips NO_TECH, so the two forms are equivalent).
    pub prereq_and_techs: Vec<TechType>,
    /// C++ `getPrereqAndBonus()` (`<BonusType>`).
    pub prereq_and_bonus: Option<BonusType>,
    /// C++ `getPrereqOrBonuses(i)` (`<PrereqBonuses>`). Same compaction as [`Self::prereq_and_techs`]
    /// (C++ array size: `NUM_UNIT_PREREQ_OR_BONUSES`, 4).
    pub prereq_or_bonuses: Vec<BonusType>,
    pub production_traits: IdxVec<TraitType, i32>,
    /// C++ `getFlavorValue(i)` (`<Flavors>`).
    pub flavor_value: IdxVec<FlavorType, i32>,
    pub ai_weight: i32,
    /// C++ `getProductionCost()` (`<iCost>`).
    pub production_cost: i32,
    pub hurry_cost_modifier: i32,
    pub advanced_start_cost: i32,
    pub advanced_start_cost_increase: i32,
    pub min_area_size: i32,
    pub moves: i32,
    pub air_range: i32,
    pub air_unit_cap: i32,
    pub drop_range: i32,
    pub nuke_range: i32,
    pub work_rate: i32,
    pub base_discover: i32,
    pub discover_multiplier: i32,
    pub base_hurry: i32,
    pub hurry_multiplier: i32,
    pub base_trade: i32,
    pub trade_multiplier: i32,
    pub great_work_culture: i32,
    pub espionage_points: i32,
    /// C++ `getTerrainImpassable(i)` (`<TerrainImpassables>`).
    pub terrain_impassable: IdxVec<TerrainType, bool>,
    /// C++ `getFeatureImpassable(i)` (`<FeatureImpassables>`).
    pub feature_impassable: IdxVec<FeatureType, bool>,
    pub combat: i32,
    pub combat_limit: i32,
    pub air_combat: i32,
    pub air_combat_limit: i32,
    /// C++ `getXPValueAttack()`.
    pub xp_value_attack: i32,
    /// C++ `getXPValueDefense()`.
    pub xp_value_defense: i32,
    pub first_strikes: i32,
    pub chance_first_strikes: i32,
    pub interception_probability: i32,
    pub evasion_probability: i32,
    /// C++ `getWithdrawalProbability()` (`<iWithdrawalProb>`).
    pub withdrawal_probability: i32,
    pub collateral_damage: i32,
    pub collateral_damage_limit: i32,
    pub collateral_damage_max_units: i32,
    /// C++ `getCityAttackModifier()` (`<iCityAttack>`).
    pub city_attack_modifier: i32,
    /// C++ `getCityDefenseModifier()` (`<iCityDefense>`).
    pub city_defense_modifier: i32,
    /// C++ `getAnimalCombatModifier()` (`<iAnimalCombat>`).
    pub animal_combat_modifier: i32,
    /// C++ `getHillsAttackModifier()` (`<iHillsAttack>`).
    pub hills_attack_modifier: i32,
    /// C++ `getHillsDefenseModifier()` (`<iHillsDefense>`).
    pub hills_defense_modifier: i32,
    /// C++ `getTerrainNative(i)` (`<TerrainNatives>`).
    pub terrain_native: IdxVec<TerrainType, bool>,
    /// C++ `getFeatureNative(i)` (`<FeatureNatives>`).
    pub feature_native: IdxVec<FeatureType, bool>,
    /// C++ `getTerrainAttackModifier(i)` (`<TerrainAttacks>`).
    pub terrain_attack_modifier: IdxVec<TerrainType, i32>,
    /// C++ `getTerrainDefenseModifier(i)` (`<TerrainDefenses>`).
    pub terrain_defense_modifier: IdxVec<TerrainType, i32>,
    /// C++ `getFeatureAttackModifier(i)` (`<FeatureAttacks>`).
    pub feature_attack_modifier: IdxVec<FeatureType, i32>,
    /// C++ `getFeatureDefenseModifier(i)` (`<FeatureDefenses>`).
    pub feature_defense_modifier: IdxVec<FeatureType, i32>,
    /// C++ `getUnitClassAttackModifier(i)` (`<UnitClassAttackMods>`).
    pub unit_class_attack_modifier: IdxVec<UnitClassType, i32>,
    /// C++ `getUnitClassDefenseModifier(i)` (`<UnitClassDefenseMods>`).
    pub unit_class_defense_modifier: IdxVec<UnitClassType, i32>,
    /// C++ `getUnitCombatModifier(i)` (`<UnitCombatMods>`).
    pub unit_combat_modifier: IdxVec<UnitCombatType, i32>,
    /// C++ `getUnitCombatCollateralImmune(i)` (`<UnitCombatCollateralImmunes>`); an int in C++.
    pub unit_combat_collateral_immune: IdxVec<UnitCombatType, i32>,
    /// C++ `getDomainModifier(i)` (`<DomainMods>`).
    pub domain_modifier: IdxVec<DomainType, i32>,
    /// C++ `getBonusProductionModifier(i)` (`<BonusProductionModifiers>`).
    pub bonus_production_modifier: IdxVec<BonusType, i32>,
    pub bomb_rate: i32,
    pub bombard_rate: i32,
    /// C++ `getSpecialCargo()` (`<SpecialCargo>`).
    pub special_cargo: Option<SpecialUnitType>,
    /// C++ `getDomainCargo()` (`<DomainCargo>`).
    pub domain_cargo: Option<DomainType>,
    /// C++ `getCargoSpace()` (`<iCargo>`).
    pub cargo_space: i32,
    /// C++ `getConscriptionValue()` (`<iConscription>`).
    pub conscription_value: i32,
    /// C++ `getCultureGarrisonValue()` (`<iCultureGarrison>`).
    pub culture_garrison_value: i32,
    pub extra_cost: i32,
    /// C++ `getAssetValue()` (`<iAsset>`).
    pub asset_value: i32,
    /// C++ `getPowerValue()` (`<iPower>`).
    pub power_value: i32,
    /// C++ `getUnitNames(i)` / `getNumUnitNames()` (`<UniqueNames>`, text keys).
    pub unit_names: Vec<String>,
    pub free_promotions: IdxVec<PromotionType, bool>,
    pub leader_promotion: Option<PromotionType>,
    pub leader_experience: i32,
    // ---- FfH ----
    pub abandon: bool,
    pub auto_raze: bool,
    pub disable_upgrade_to: bool,
    pub explode_in_combat: bool,
    /// C++ `isFreeXP()`.
    pub free_xp: bool,
    pub immortal: bool,
    pub immune_to_defensive_strike: bool,
    pub never_obsolete: bool,
    pub no_war_weariness: bool,
    pub combat_defense: i32,
    pub defensive_strike_chance: i32,
    pub defensive_strike_damage: i32,
    pub duration_from_combat: i32,
    pub enslavement_chance: i32,
    pub free_promotion_pick: i32,
    pub gold_from_combat: i32,
    pub min_level: i32,
    pub miscast_chance: i32,
    pub modify_global_counter: i32,
    /// C++ `getDiploVoteType()` (`<DiploVoteType>`): the vote source this unit carries.
    pub diplo_vote_type: Option<VoteSourceType>,
    pub equipment_promotion: Option<PromotionType>,
    pub prereq_alignment: Option<AlignmentType>,
    pub prereq_building_class: Option<BuildingClassType>,
    /// C++ `getPrereqCiv()` (`<PrereqCiv>`, resolved by C++ `readPass3`).
    pub prereq_civ: Option<CivilizationType>,
    pub prereq_civic: Option<CivicType>,
    pub prereq_global_counter: i32,
    pub promotion_from_combat: Option<PromotionType>,
    pub tier: i32,
    /// C++ `getUnitConvertFromCombat()` (C++ `readPass2`).
    pub unit_convert_from_combat: Option<UnitType>,
    pub unit_convert_from_combat_chance: i32,
    /// C++ `getUnitCreateFromCombat()` (C++ `readPass2`).
    pub unit_create_from_combat: Option<UnitType>,
    pub unit_create_from_combat_chance: i32,
    /// C++ `getUpgradeCiv()` (`<UpgradeCiv>`, resolved by C++ `readPass3`).
    pub upgrade_civ: Option<CivilizationType>,
    pub weapon_tier: i32,
    /// C++ `getWithdrawlProbDefensive()` (sic).
    pub withdrawl_prob_defensive: i32,
    /// Python callback after this unit loses a combat (`<PythonPostCombatLost>`).
    pub py_post_combat_lost: String,
    /// Python callback after this unit wins a combat (`<PythonPostCombatWon>`).
    pub py_post_combat_won: String,
    /// C++ `getDamageTypeCombat(i)` (`<DamageTypeCombats>`).
    pub damage_type_combat: IdxVec<DamageType, i32>,
    /// C++ `getBonusAffinity(i)` (`<BonusAffinities>`).
    pub bonus_affinity: IdxVec<BonusType, i32>,
    // ---- Sephi AI addon ----
    /// C++ `isAIblockExplore()`.
    pub ai_block_explore: bool,
    /// C++ `isAIblockPermDefense()`.
    pub ai_block_perm_defense: bool,
    /// C++ `isAIblockPatrol()`.
    pub ai_block_patrol: bool,
}

/// C++ `CvUnitInfo::read` + `readPass2` + `readPass3`.
pub fn read_unit(rd: &Rd) -> UnitInfo {
    UnitInfo {
        base: InfoBase::read(rd),
        unit_class_type: rd.rf::<UnitClassType>("Class"),
        special_unit_type: rd.rf::<SpecialUnitType>("Special"),
        unit_capture_class_type: rd.rf::<UnitClassType>("Capture"),
        unit_combat_type: rd.rf::<UnitCombatType>("Combat"),
        domain_type: rd.rf::<DomainType>("Domain"),
        default_unit_ai_type: rd.rf::<UnitAIType>("DefaultUnitAI"),
        invisible_type: rd.rf::<InvisibleType>("Invisible"),
        see_invisible_types: rd.ref_tokens::<InvisibleType>("SeeInvisible"),
        animal: rd.bool("bAnimal"),
        food_production: rd.bool("bFood"),
        no_bad_goodies: rd.bool("bNoBadGoodies"),
        only_defensive: rd.bool("bOnlyDefensive"),
        no_capture: rd.bool("bNoCapture"),
        quick_combat: rd.bool("bQuickCombat"),
        rival_territory: rd.bool("bRivalTerritory"),
        military_happiness: rd.bool("bMilitaryHappiness"),
        military_support: rd.bool("bMilitarySupport"),
        military_production: rd.bool("bMilitaryProduction"),
        pillage: rd.bool("bPillage"),
        spy: rd.bool("bSpy"),
        sabotage: rd.bool("bSabotage"),
        destroy: rd.bool("bDestroy"),
        steal_plans: rd.bool("bStealPlans"),
        investigate: rd.bool("bInvestigate"),
        counter_spy: rd.bool("bCounterSpy"),
        found: rd.bool("bFound"),
        golden_age: rd.bool("bGoldenAge"),
        invisible: rd.bool("bInvisible"),
        first_strike_immune: rd.bool("bFirstStrikeImmune"),
        no_defensive_bonus: rd.bool("bNoDefensiveBonus"),
        ignore_building_defense: rd.bool("bIgnoreBuildingDefense"),
        can_move_impassable: rd.bool("bCanMoveImpassable"),
        can_move_all_terrain: rd.bool("bCanMoveAllTerrain"),
        flat_movement_cost: rd.bool("bFlatMovementCost"),
        ignore_terrain_cost: rd.bool("bIgnoreTerrainCost"),
        nuke_immune: rd.bool("bNukeImmune"),
        prereq_bonuses: rd.bool("bPrereqBonuses"),
        is_prereq_religion: rd.bool("bPrereqReligion"),
        mech_unit: rd.bool("bMechanized"),
        suicide: rd.bool("bSuicide"),
        line_of_sight: rd.bool("bLineOfSight"),
        hidden_nationality: rd.bool("bHiddenNationality"),
        always_hostile: rd.bool("bAlwaysHostile"),
        no_reveal_map: rd.bool("bNoRevealMap"),
        upgrade_unit_class: rd.pairs_bool::<UnitClassType>("UnitClassUpgrades"),
        target_unit_class: rd.pairs_bool::<UnitClassType>("UnitClassTargets"),
        target_unit_combat: rd.pairs_bool::<UnitCombatType>("UnitCombatTargets"),
        defender_unit_class: rd.pairs_bool::<UnitClassType>("UnitClassDefenders"),
        defender_unit_combat: rd.pairs_bool::<UnitCombatType>("UnitCombatDefenders"),
        flanking_strike_unit_class: rd.pairs_int::<UnitClassType>("FlankingStrikes", 0),
        unit_ai_type: rd.pairs_bool::<UnitAIType>("UnitAIs"),
        not_unit_ai_type: rd.pairs_bool::<UnitAIType>("NotUnitAIs"),
        builds: rd.pairs_bool::<BuildType>("Builds"),
        religion_spreads: rd.pairs_int::<ReligionType>("ReligionSpreads", 0),
        corporation_spreads: rd.pairs_int::<CorporationType>("CorporationSpreads", 0),
        terrain_passable_tech: rd.pairs_ref::<TerrainType, TechType>("TerrainPassableTechs"),
        feature_passable_tech: rd.pairs_ref::<FeatureType, TechType>("FeaturePassableTechs"),
        great_peoples: rd.pairs_bool::<SpecialistType>("GreatPeoples"),
        buildings: rd.pairs_bool::<BuildingType>("Buildings"),
        force_buildings: rd.pairs_bool::<BuildingType>("ForceBuildings"),
        holy_city: rd.rf::<ReligionType>("HolyCity"),
        religion_type: rd.rf::<ReligionType>("ReligionType"),
        state_religion: rd.rf::<ReligionType>("StateReligion"),
        prereq_religion: rd.rf::<ReligionType>("PrereqReligion"),
        prereq_corporation: rd.rf::<CorporationType>("PrereqCorporation"),
        prereq_building: rd.rf::<BuildingType>("PrereqBuilding"),
        prereq_and_tech: rd.rf::<TechType>("PrereqTech"),
        prereq_and_techs: rd.ref_list::<TechType>("TechTypes"),
        prereq_and_bonus: rd.rf::<BonusType>("BonusType"),
        prereq_or_bonuses: rd.ref_list::<BonusType>("PrereqBonuses"),
        production_traits: rd.pairs_int::<TraitType>("ProductionTraits", 0),
        flavor_value: rd.pairs_int::<FlavorType>("Flavors", 0),
        ai_weight: rd.int("iAIWeight"),
        production_cost: rd.int("iCost"),
        hurry_cost_modifier: rd.int("iHurryCostModifier"),
        advanced_start_cost: rd.int("iAdvancedStartCost"),
        advanced_start_cost_increase: rd.int("iAdvancedStartCostIncrease"),
        min_area_size: rd.int("iMinAreaSize"),
        moves: rd.int("iMoves"),
        air_range: rd.int("iAirRange"),
        air_unit_cap: rd.int("iAirUnitCap"),
        drop_range: rd.int("iDropRange"),
        nuke_range: rd.int("iNukeRange"),
        work_rate: rd.int("iWorkRate"),
        base_discover: rd.int("iBaseDiscover"),
        discover_multiplier: rd.int("iDiscoverMultiplier"),
        base_hurry: rd.int("iBaseHurry"),
        hurry_multiplier: rd.int("iHurryMultiplier"),
        base_trade: rd.int("iBaseTrade"),
        trade_multiplier: rd.int("iTradeMultiplier"),
        great_work_culture: rd.int("iGreatWorkCulture"),
        espionage_points: rd.int("iEspionagePoints"),
        terrain_impassable: rd.pairs_bool::<TerrainType>("TerrainImpassables"),
        feature_impassable: rd.pairs_bool::<FeatureType>("FeatureImpassables"),
        combat: rd.int("iCombat"),
        combat_limit: rd.int("iCombatLimit"),
        air_combat: rd.int("iAirCombat"),
        air_combat_limit: rd.int("iAirCombatLimit"),
        xp_value_attack: rd.int("iXPValueAttack"),
        xp_value_defense: rd.int("iXPValueDefense"),
        first_strikes: rd.int("iFirstStrikes"),
        chance_first_strikes: rd.int("iChanceFirstStrikes"),
        interception_probability: rd.int("iInterceptionProbability"),
        evasion_probability: rd.int("iEvasionProbability"),
        withdrawal_probability: rd.int("iWithdrawalProb"),
        collateral_damage: rd.int("iCollateralDamage"),
        collateral_damage_limit: rd.int("iCollateralDamageLimit"),
        collateral_damage_max_units: rd.int("iCollateralDamageMaxUnits"),
        city_attack_modifier: rd.int("iCityAttack"),
        city_defense_modifier: rd.int("iCityDefense"),
        animal_combat_modifier: rd.int("iAnimalCombat"),
        hills_attack_modifier: rd.int("iHillsAttack"),
        hills_defense_modifier: rd.int("iHillsDefense"),
        terrain_native: rd.pairs_bool::<TerrainType>("TerrainNatives"),
        feature_native: rd.pairs_bool::<FeatureType>("FeatureNatives"),
        terrain_attack_modifier: rd.pairs_int::<TerrainType>("TerrainAttacks", 0),
        terrain_defense_modifier: rd.pairs_int::<TerrainType>("TerrainDefenses", 0),
        feature_attack_modifier: rd.pairs_int::<FeatureType>("FeatureAttacks", 0),
        feature_defense_modifier: rd.pairs_int::<FeatureType>("FeatureDefenses", 0),
        unit_class_attack_modifier: rd.pairs_int::<UnitClassType>("UnitClassAttackMods", 0),
        unit_class_defense_modifier: rd.pairs_int::<UnitClassType>("UnitClassDefenseMods", 0),
        unit_combat_modifier: rd.pairs_int::<UnitCombatType>("UnitCombatMods", 0),
        unit_combat_collateral_immune: rd.pairs_int::<UnitCombatType>("UnitCombatCollateralImmunes", 0),
        domain_modifier: rd.pairs_int::<DomainType>("DomainMods", 0),
        bonus_production_modifier: rd.pairs_int::<BonusType>("BonusProductionModifiers", 0),
        bomb_rate: rd.int("iBombRate"),
        bombard_rate: rd.int("iBombardRate"),
        // The C++ constructor sets these two to 0, but read() always overwrites them with
        // FindInInfoClass(..), i.e. -1 when the tag is absent or NONE.
        special_cargo: rd.rf::<SpecialUnitType>("SpecialCargo"),
        domain_cargo: rd.rf::<DomainType>("DomainCargo"),
        cargo_space: rd.int("iCargo"),
        conscription_value: rd.int("iConscription"),
        culture_garrison_value: rd.int("iCultureGarrison"),
        extra_cost: rd.int("iExtraCost"),
        asset_value: rd.int("iAsset"),
        power_value: rd.int("iPower"),
        unit_names: rd.string_list("UniqueNames"),
        free_promotions: rd.pairs_bool::<PromotionType>("FreePromotions"),
        leader_promotion: rd.rf::<PromotionType>("LeaderPromotion"),
        leader_experience: rd.int("iLeaderExperience"),
        // ---- FfH ----
        abandon: rd.bool("bAbandon"),
        auto_raze: rd.bool("bAutoRaze"),
        disable_upgrade_to: rd.bool("bDisableUpgradeTo"),
        explode_in_combat: rd.bool("bExplodeInCombat"),
        free_xp: rd.bool("bFreeXP"),
        immortal: rd.bool("bImmortal"),
        immune_to_defensive_strike: rd.bool("bImmuneToDefensiveStrike"),
        never_obsolete: rd.bool("bNeverObsolete"),
        no_war_weariness: rd.bool("bNoWarWeariness"),
        combat_defense: rd.int("iCombatDefense"),
        defensive_strike_chance: rd.int("iDefensiveStrikeChance"),
        defensive_strike_damage: rd.int("iDefensiveStrikeDamage"),
        duration_from_combat: rd.int("iDurationFromCombat"),
        enslavement_chance: rd.int("iEnslavementChance"),
        free_promotion_pick: rd.int("iFreePromotionPick"),
        gold_from_combat: rd.int("iGoldFromCombat"),
        min_level: rd.int("iMinLevel"),
        miscast_chance: rd.int("iMiscastChance"),
        modify_global_counter: rd.int("iModifyGlobalCounter"),
        diplo_vote_type: rd.rf::<VoteSourceType>("DiploVoteType"),
        equipment_promotion: rd.rf::<PromotionType>("EquipmentPromotion"),
        prereq_alignment: rd.rf::<AlignmentType>("PrereqAlignment"),
        prereq_building_class: rd.rf::<BuildingClassType>("PrereqBuildingClass"),
        prereq_civ: rd.rf::<CivilizationType>("PrereqCiv"),
        prereq_civic: rd.rf::<CivicType>("PrereqCivic"),
        promotion_from_combat: rd.rf::<PromotionType>("PromotionFromCombat"),
        prereq_global_counter: rd.int("iPrereqGlobalCounter"),
        tier: rd.int("iTier"),
        unit_convert_from_combat_chance: rd.int("iUnitConvertFromCombatChance"),
        unit_create_from_combat_chance: rd.int("iUnitCreateFromCombatChance"),
        upgrade_civ: rd.rf::<CivilizationType>("UpgradeCiv"),
        weapon_tier: rd.int("iWeaponTier"),
        withdrawl_prob_defensive: rd.int("iWithdrawlProbDefensive"),
        py_post_combat_lost: rd.string("PythonPostCombatLost"),
        py_post_combat_won: rd.string("PythonPostCombatWon"),
        damage_type_combat: rd.pairs_int::<DamageType>("DamageTypeCombats", 0),
        bonus_affinity: rd.pairs_int::<BonusType>("BonusAffinities", 0),
        // ---- Sephi AI addon ----
        ai_block_explore: rd.bool("bAIblockExplore"),
        ai_block_perm_defense: rd.bool("bAIblockPermDefense"),
        ai_block_patrol: rd.bool("bAIblockPatrol"),
        // ---- readPass2 ----
        unit_convert_from_combat: rd.rf::<UnitType>("UnitConvertFromCombat"),
        unit_create_from_combat: rd.rf::<UnitType>("UnitCreateFromCombat"),
    }
}

// ---------------------------------------------------------------------------------------------
// SpellInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvSpellInfo` (Units/CIV4SpellInfos.xml)
///
/// Not ported: `Effect` (spell graphics) and `Sound`.
#[derive(Debug, Clone, Default)]
pub struct SpellInfo {
    pub base: InfoBase,
    pub promotion_prereq1: Option<PromotionType>,
    pub promotion_prereq2: Option<PromotionType>,
    pub unit_prereq: Option<UnitType>,
    pub unit_combat_prereq: Option<UnitCombatType>,
    pub unit_class_prereq: Option<UnitClassType>,
    pub unit_in_stack_prereq: Option<UnitType>,
    pub building_prereq: Option<BuildingType>,
    pub building_class_owned_prereq: Option<BuildingClassType>,
    pub civilization_prereq: Option<CivilizationType>,
    pub corporation_prereq: Option<CorporationType>,
    pub feature_or_prereq1: Option<FeatureType>,
    pub feature_or_prereq2: Option<FeatureType>,
    pub improvement_prereq: Option<ImprovementType>,
    pub promotion_in_stack_prereq: Option<PromotionType>,
    pub religion_prereq: Option<ReligionType>,
    pub state_religion_prereq: Option<ReligionType>,
    pub tech_prereq: Option<TechType>,
    /// C++ `isAllowAI()`.
    pub allow_ai: bool,
    /// C++ `isAllowAuto()` (Sephi addon).
    pub allow_auto: bool,
    pub allow_automate_terrain: bool,
    pub adjacent_to_water_only: bool,
    pub caster_must_be_alive: bool,
    pub caster_no_duration: bool,
    pub causes_war: bool,
    pub global: bool,
    pub in_borders_only: bool,
    pub in_city_only: bool,
    pub prereq_slave_trade: bool,
    pub resistable: bool,
    /// C++ `getAIWeight()`.
    pub ai_weight: i32,
    /// C++ `getAIWeightCity()`.
    pub ai_weight_city: i32,
    /// C++ `getAIWeightWar()`.
    pub ai_weight_war: i32,
    pub caster_min_level: i32,
    pub display_when_disabled: bool,
    pub has_casted: bool,
    pub ignore_has_casted: bool,
    pub range: i32,
    pub resist_modify: i32,
    pub damage: i32,
    pub damage_limit: i32,
    pub damage_type: Option<DamageType>,
    pub add_promotion_type1: Option<PromotionType>,
    pub add_promotion_type2: Option<PromotionType>,
    pub add_promotion_type3: Option<PromotionType>,
    pub remove_promotion_type1: Option<PromotionType>,
    pub remove_promotion_type2: Option<PromotionType>,
    pub remove_promotion_type3: Option<PromotionType>,
    pub buff_caster_only: bool,
    pub convert_unit_type: Option<UnitType>,
    pub create_building_type: Option<BuildingType>,
    pub create_feature_type: Option<FeatureType>,
    pub create_improvement_type: Option<ImprovementType>,
    pub spread_religion: Option<ReligionType>,
    pub create_unit_type: Option<UnitType>,
    pub create_unit_num: i32,
    pub copy_casters_promotions: bool,
    pub permanent_unit_create: bool,
    pub create_unit_promotion: Option<PromotionType>,
    pub immune_team: bool,
    pub immune_neutral: bool,
    pub immune_enemy: bool,
    pub immune_flying: bool,
    pub immune_not_alive: bool,
    pub ability: bool,
    pub dispel: bool,
    pub push: bool,
    pub remove_has_casted: bool,
    pub sacrifice_caster: bool,
    pub change_population: i32,
    pub cost: i32,
    pub delay: i32,
    pub immobile_turns: i32,
    pub miscast_chance: i32,
    /// Python callback run on a miscast (`<PyMiscast>`).
    pub py_miscast: String,
    /// Python callback that performs the spell (`<PyResult>`).
    pub py_result: String,
    /// Python callback that decides whether the spell can be cast (`<PyRequirement>`).
    pub py_requirement: String,
}

/// C++ `CvSpellInfo::read`.
///
/// C++ only assigns a type reference when the tag text is non-empty
/// (`if (szTextVal != "") m_iX = FindInInfoClass(szTextVal)`); otherwise the constructor's
/// NO_X stays. `rf` yields the same `None` for absent, empty and NONE tags.
pub fn read_spell(rd: &Rd) -> SpellInfo {
    SpellInfo {
        base: InfoBase::read(rd),
        promotion_prereq1: rd.rf::<PromotionType>("PromotionPrereq1"),
        promotion_prereq2: rd.rf::<PromotionType>("PromotionPrereq2"),
        unit_prereq: rd.rf::<UnitType>("UnitPrereq"),
        unit_combat_prereq: rd.rf::<UnitCombatType>("UnitCombatPrereq"),
        unit_class_prereq: rd.rf::<UnitClassType>("UnitClassPrereq"),
        unit_in_stack_prereq: rd.rf::<UnitType>("UnitInStackPrereq"),
        building_prereq: rd.rf::<BuildingType>("BuildingPrereq"),
        building_class_owned_prereq: rd.rf::<BuildingClassType>("BuildingClassOwnedPrereq"),
        civilization_prereq: rd.rf::<CivilizationType>("CivilizationPrereq"),
        corporation_prereq: rd.rf::<CorporationType>("CorporationPrereq"),
        feature_or_prereq1: rd.rf::<FeatureType>("FeatureOrPrereq1"),
        feature_or_prereq2: rd.rf::<FeatureType>("FeatureOrPrereq2"),
        improvement_prereq: rd.rf::<ImprovementType>("ImprovementPrereq"),
        promotion_in_stack_prereq: rd.rf::<PromotionType>("PromotionInStackPrereq"),
        religion_prereq: rd.rf::<ReligionType>("ReligionPrereq"),
        state_religion_prereq: rd.rf::<ReligionType>("StateReligionPrereq"),
        tech_prereq: rd.rf::<TechType>("TechPrereq"),
        allow_ai: rd.bool("bAllowAI"),
        allow_auto: rd.bool("bAllowAuto"),
        allow_automate_terrain: rd.bool("bAllowAutomateTerrain"),
        adjacent_to_water_only: rd.bool("bAdjacentToWaterOnly"),
        caster_must_be_alive: rd.bool("bCasterMustBeAlive"),
        caster_no_duration: rd.bool("bCasterNoDuration"),
        causes_war: rd.bool("bCausesWar"),
        global: rd.bool("bGlobal"),
        in_borders_only: rd.bool("bInBordersOnly"),
        in_city_only: rd.bool("bInCityOnly"),
        prereq_slave_trade: rd.bool("bPrereqSlaveTrade"),
        resistable: rd.bool("bResistable"),
        ai_weight: rd.int("iAIWeight"),
        ai_weight_city: rd.int("iAIWeightCity"),
        ai_weight_war: rd.int("iAIWeightWar"),
        caster_min_level: rd.int("iCasterMinLevel"),
        display_when_disabled: rd.bool("bDisplayWhenDisabled"),
        has_casted: rd.bool("bHasCasted"),
        ignore_has_casted: rd.bool("bIgnoreHasCasted"),
        range: rd.int("iRange"),
        resist_modify: rd.int("iResistModify"),
        damage: rd.int("iDamage"),
        damage_limit: rd.int("iDamageLimit"),
        damage_type: rd.rf::<DamageType>("DamageType"),
        add_promotion_type1: rd.rf::<PromotionType>("AddPromotionType1"),
        add_promotion_type2: rd.rf::<PromotionType>("AddPromotionType2"),
        add_promotion_type3: rd.rf::<PromotionType>("AddPromotionType3"),
        remove_promotion_type1: rd.rf::<PromotionType>("RemovePromotionType1"),
        remove_promotion_type2: rd.rf::<PromotionType>("RemovePromotionType2"),
        remove_promotion_type3: rd.rf::<PromotionType>("RemovePromotionType3"),
        buff_caster_only: rd.bool("bBuffCasterOnly"),
        convert_unit_type: rd.rf::<UnitType>("ConvertUnitType"),
        create_building_type: rd.rf::<BuildingType>("CreateBuildingType"),
        create_feature_type: rd.rf::<FeatureType>("CreateFeatureType"),
        create_improvement_type: rd.rf::<ImprovementType>("CreateImprovementType"),
        spread_religion: rd.rf::<ReligionType>("SpreadReligion"),
        create_unit_type: rd.rf::<UnitType>("CreateUnitType"),
        create_unit_num: rd.int("iCreateUnitNum"),
        copy_casters_promotions: rd.bool("bCopyCastersPromotions"),
        permanent_unit_create: rd.bool("bPermanentUnitCreate"),
        create_unit_promotion: rd.rf::<PromotionType>("CreateUnitPromotion"),
        immune_team: rd.bool("bImmuneTeam"),
        immune_neutral: rd.bool("bImmuneNeutral"),
        immune_enemy: rd.bool("bImmuneEnemy"),
        immune_flying: rd.bool("bImmuneFlying"),
        immune_not_alive: rd.bool("bImmuneNotAlive"),
        ability: rd.bool("bAbility"),
        dispel: rd.bool("bDispel"),
        push: rd.bool("bPush"),
        remove_has_casted: rd.bool("bRemoveHasCasted"),
        sacrifice_caster: rd.bool("bSacrificeCaster"),
        change_population: rd.int("iChangePopulation"),
        cost: rd.int("iCost"),
        delay: rd.int("iDelay"),
        immobile_turns: rd.int("iImmobileTurns"),
        miscast_chance: rd.int("iMiscastChance"),
        py_miscast: rd.string("PyMiscast"),
        py_result: rd.string("PyResult"),
        py_requirement: rd.string("PyRequirement"),
    }
}

// ---------------------------------------------------------------------------------------------
// AutomateInfo / CommandInfo / MissionInfo
// ---------------------------------------------------------------------------------------------

/// C++ `CvAutomateInfo` (Units/CIV4AutomateInfos.xml)
///
/// The XML entries have no `<Type>` (so `base.type_` is empty, as in C++); their names come from
/// GlobalTypes.xml `AutomateTypes`.
#[derive(Debug, Clone, Default)]
pub struct AutomateInfo {
    pub base: InfoBase,
    /// C++ `getCommand()`: the command that starts this automation (COMMAND_AUTOMATE).
    pub command: Option<CommandType>,
    /// C++ `getAutomate()`.
    pub automate: Option<AutomateType>,
    /// C++ `getConfirmCommand()` (interface only).
    pub confirm_command: bool,
    /// C++ `getVisible()` (interface only).
    pub visible: bool,
}

/// C++ `CvAutomateInfo::read`.
pub fn read_automate(rd: &Rd) -> AutomateInfo {
    AutomateInfo {
        base: InfoBase::read(rd),
        command: rd.rf::<CommandType>("Command"),
        // C++ GC.getTypesEnum: AutomateTypes from GlobalTypes.xml.
        automate: rd.rf::<AutomateType>("Automate"),
        confirm_command: rd.bool("bConfirmCommand"),
        visible: rd.bool("bVisible"),
    }
}

/// C++ `CvCommandInfo` (Units/CIV4CommandInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct CommandInfo {
    pub base: InfoBase,
    /// C++ `getAutomate()` (only assigned when `<Automate>` is present; NO_AUTOMATE otherwise).
    pub automate: Option<AutomateType>,
    /// C++ `getConfirmCommand()` (interface only).
    pub confirm_command: bool,
    /// C++ `getVisible()` (interface only).
    pub visible: bool,
    /// C++ `getAll()`: with ALT held the command applies to every unit of the type
    /// (`CvMessageData` DO_COMMAND).
    pub all: bool,
}

/// C++ `CvCommandInfo::read`.
pub fn read_command(rd: &Rd) -> CommandInfo {
    CommandInfo {
        base: InfoBase::read(rd),
        automate: rd.rf::<AutomateType>("Automate"),
        confirm_command: rd.bool("bConfirmCommand"),
        visible: rd.bool("bVisible"),
        all: rd.bool("bAll"),
    }
}

/// C++ `CvMissionInfo` (Units/CIV4MissionInfos.xml)
///
/// Not ported: `Waypoint` (effect name), `bSound` (only plays the action sound) and
/// `EntityEventType` (animation).
#[derive(Debug, Clone, Default)]
pub struct MissionInfo {
    pub base: InfoBase,
    /// C++ `getTime()`: mission/combat timer length (`CvSelectionGroup`/`CvUnit` timers).
    pub time: i32,
    /// C++ `isTarget()`.
    pub target: bool,
    /// C++ `isBuild()`.
    pub build: bool,
    /// C++ `getVisible()` (interface only).
    pub visible: bool,
}

/// C++ `CvMissionInfo::read`.
pub fn read_mission(rd: &Rd) -> MissionInfo {
    MissionInfo {
        base: InfoBase::read(rd),
        time: rd.int("iTime"),
        target: rd.bool("bTarget"),
        build: rd.bool("bBuild"),
        visible: rd.bool("bVisible"),
    }
}
