//! Port of `CvUnit` (CvUnit.h / CvUnit.cpp): the [`Unit`] struct, the C++ `CombatDetails`
//! struct and the `CvUnit::reset` defaults ([`Unit::new`]).
//!
//! `Unit` has one field per gameplay data member of `CvUnit` (C++ declaration order) plus the
//! `CvUnitAI` bookkeeping that rules code reads and writes (`ai_*` fields). Trivial C++
//! accessors (`getDamage()`, `isMadeAttack()`, `setDuration()`, ...) are plain field access;
//! everything with logic is an `impl Game` method `unit_*(u: UnitId, ..)` in one of the topic
//! files below. `docs/porting/unit.md` maps every CvUnit.cpp function (with line ranges) to its
//! Rust file, method or field.

mod actions;
mod combat;
mod ffh;
mod lifecycle;
mod missions;
mod movement;
mod promotions;

use crate::defs::{GROUPFLAG_NONE, INVALID_PLOT_COORD};
use crate::ids::*;
use ffh_data::*;

/// C++ `CombatDetails` (CvUnit.h): the breakdown `maxCombatStr` fills in for combat logs and
/// odds displays (`Option<&mut CombatDetails>` where C++ passes `CombatDetails*`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatDetails {
    pub extra_combat_percent: i32,
    pub animal_combat_modifier_ta: i32,
    pub ai_animal_combat_modifier_ta: i32,
    pub animal_combat_modifier_aa: i32,
    pub ai_animal_combat_modifier_aa: i32,
    pub barbarian_combat_modifier_tb: i32,
    pub ai_barbarian_combat_modifier_tb: i32,
    pub barbarian_combat_modifier_ab: i32,
    pub ai_barbarian_combat_modifier_ab: i32,
    pub plot_defense_modifier: i32,
    pub fortify_modifier: i32,
    pub city_defense_modifier: i32,
    pub hills_attack_modifier: i32,
    pub hills_defense_modifier: i32,
    pub feature_attack_modifier: i32,
    pub feature_defense_modifier: i32,
    pub terrain_attack_modifier: i32,
    pub terrain_defense_modifier: i32,
    pub city_attack_modifier: i32,
    pub domain_defense_modifier: i32,
    pub city_barbarian_defense_modifier: i32,
    pub class_defense_modifier: i32,
    pub class_attack_modifier: i32,
    pub combat_modifier_t: i32,
    pub combat_modifier_a: i32,
    pub domain_modifier_a: i32,
    pub domain_modifier_t: i32,
    pub animal_combat_modifier_a: i32,
    pub animal_combat_modifier_t: i32,
    pub river_attack_modifier: i32,
    pub amphib_attack_modifier: i32,
    pub kamikaze_modifier: i32,
    pub modifier_total: i32,
    pub base_combat_str: i32,
    pub combat: i32,
    pub max_combat_str: i32,
    pub curr_hit_points: i32,
    pub max_hit_points: i32,
    pub curr_combat_str: i32,
    /// C++ `eOwner`.
    pub owner: Option<PlayerId>,
    /// C++ `eVisualOwner`.
    pub visual_owner: Option<PlayerId>,
    /// C++ `sUnitName`.
    pub unit_name: String,
}

/// The `int&` out-params of C++ `CvUnit::getDefenderCombatValues`, returned by
/// `Game::unit_get_defender_combat_values`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DefenderCombatValues {
    /// C++ `iTheirOdds`: the defender's odds per round out of `COMBAT_DIE_SIDES`.
    pub their_odds: i32,
    /// C++ `iTheirStrength`: the defender's current combat strength.
    pub their_strength: i32,
    /// C++ `iOurDamage`: damage the attacker takes per lost round.
    pub our_damage: i32,
    /// C++ `iTheirDamage`: damage the defender takes per lost round.
    pub their_damage: i32,
}

/// The `CvUnitInfo` values that C++ `CvUnit::reset` copies into a new unit (each one is 0 /
/// none in C++ when `eUnit == NO_UNIT`).
#[derive(Clone, Debug, Default)]
pub struct UnitResetInfo {
    /// `getCombat()` -> `m_iBaseCombat`.
    pub combat: i32,
    /// `getCargoSpace()` -> `m_iCargoCapacity`.
    pub cargo_space: i32,
    /// `getDefensiveStrikeChance()` -> `m_iDefensiveStrikeChance`.
    pub defensive_strike_chance: i32,
    /// `getDefensiveStrikeDamage()` -> `m_iDefensiveStrikeDamage`.
    pub defensive_strike_damage: i32,
    /// `isHiddenNationality()` -> `m_iHiddenNationality` (0/1).
    pub hidden_nationality: bool,
    /// `isIgnoreBuildingDefense()` -> `m_iIgnoreBuildingDefense` (0/1).
    pub ignore_building_defense: bool,
    /// `isImmortal()` -> `m_iImmortal` (0/1).
    pub immortal: bool,
    /// `isImmuneToDefensiveStrike()` -> `m_iImmuneToDefensiveStrike` (0/1).
    pub immune_to_defensive_strike: bool,
    /// `getCombatDefense()` -> `m_iBaseCombatDefense`.
    pub combat_defense: i32,
    /// `getCombatLimit()` -> `m_iCombatLimit`.
    pub combat_limit: i32,
    /// `getGoldFromCombat()` -> `m_iGoldFromCombat`.
    pub gold_from_combat: i32,
    /// `getInvisibleType()` -> `m_iInvisibleType`.
    pub invisible_type: Option<InvisibleType>,
    /// `getDamageTypeCombat(i)` -> `m_paiDamageTypeCombat[i]` (entries missing here read as 0).
    pub damage_type_combat: IdxVec<DamageType, i32>,
}

impl UnitResetInfo {
    /// Reads the values `CvUnit::reset` takes from `m_pUnitInfo` (CvUnit.cpp:335-536, the
    /// `(NO_UNIT != m_eUnitType) ? m_pUnitInfo->getX() : 0` members). `m_iGroupSize` (formation
    /// size) is graphics and not ported.
    pub fn from_info(info: &UnitInfo) -> UnitResetInfo {
        UnitResetInfo {
            combat: info.combat,
            cargo_space: info.cargo_space,
            defensive_strike_chance: info.defensive_strike_chance,
            defensive_strike_damage: info.defensive_strike_damage,
            hidden_nationality: info.hidden_nationality,
            ignore_building_defense: info.ignore_building_defense,
            immortal: info.immortal,
            immune_to_defensive_strike: info.immune_to_defensive_strike,
            combat_defense: info.combat_defense,
            combat_limit: info.combat_limit,
            gold_from_combat: info.gold_from_combat,
            invisible_type: info.invisible_type,
            damage_type_combat: info.damage_type_combat.clone(),
        }
    }
}

/// C++ `CvUnit` plus the `CvUnitAI` members used by rules code.
///
/// Field docs name the C++ member and the trivial C++ accessors that map to the field.
/// Counters named after a flag (`fear`, `flying`, `immortal`, ...) are FfH "number of sources"
/// counts; the flag is `!= 0` (see the `unit_is_*` methods).
#[derive(Clone, Debug)]
pub struct Unit {
    /// C++ `m_iID` (`getID`): this unit's key in `Game::units`.
    pub id: UnitId,
    /// C++ `m_iGroupID` (`getGroupID`, `getGroup`); `None` = `FFreeList::INVALID_INDEX`.
    pub group: Option<GroupId>,
    /// C++ `m_iX` (`getX`); `INVALID_PLOT_COORD` while off the map.
    pub x: i32,
    /// C++ `m_iY` (`getY`); `INVALID_PLOT_COORD` while off the map.
    pub y: i32,
    /// C++ `m_iLastMoveTurn` (`getLastMoveTurn`, `setLastMoveTurn`): game turn slice of the last move.
    pub last_move_turn: i32,
    /// C++ `m_iReconX` (see `unit_get_recon_plot`).
    pub recon_x: i32,
    /// C++ `m_iReconY`.
    pub recon_y: i32,
    /// C++ `m_iGameTurnCreated` (`getGameTurnCreated`, `setGameTurnCreated`).
    pub game_turn_created: i32,
    /// C++ `m_iDamage` (`getDamage`; set through `unit_set_damage` / `unit_change_damage`).
    pub damage: i32,
    /// C++ `m_iMoves` (`getMoves`, `setMoves`): movement points used, in `MOVE_DENOMINATOR` units.
    pub moves: i32,
    /// C++ `m_iExperience` (`getExperience`; set through `unit_set_experience`).
    pub experience: i32,
    /// C++ `m_iLevel` (`getLevel`; set through `unit_set_level`).
    pub level: i32,
    /// C++ `m_iCargo` (`getCargo`): number of units carried.
    pub cargo: i32,
    /// C++ `m_iCargoCapacity` (`cargoSpace`).
    pub cargo_capacity: i32,
    /// C++ `m_iAttackPlotX` (see `unit_get_attack_plot`).
    pub attack_plot_x: i32,
    /// C++ `m_iAttackPlotY`.
    pub attack_plot_y: i32,
    /// C++ `m_iCombatTimer` (`getCombatTimer`, `setCombatTimer`).
    pub combat_timer: i32,
    /// C++ `m_iCombatFirstStrikes` (`getCombatFirstStrikes`, `setCombatFirstStrikes`).
    pub combat_first_strikes: i32,
    /// C++ `m_iFortifyTurns` (`getFortifyTurns`; set through `unit_set_fortify_turns`).
    pub fortify_turns: i32,
    /// C++ `m_iBlitzCount` (`getBlitzCount`).
    pub blitz_count: i32,
    /// C++ `m_iAmphibCount` (`getAmphibCount`).
    pub amphib_count: i32,
    /// C++ `m_iRiverCount` (`getRiverCount`).
    pub river_count: i32,
    /// C++ `m_iEnemyRouteCount` (`getEnemyRouteCount`).
    pub enemy_route_count: i32,
    /// C++ `m_iAlwaysHealCount` (`getAlwaysHealCount`).
    pub always_heal_count: i32,
    /// C++ `m_iHillsDoubleMoveCount` (`getHillsDoubleMoveCount`).
    pub hills_double_move_count: i32,
    /// C++ `m_iImmuneToFirstStrikesCount` (`getImmuneToFirstStrikesCount`).
    pub immune_to_first_strikes_count: i32,
    /// C++ `m_iExtraVisibilityRange` (`getExtraVisibilityRange`).
    pub extra_visibility_range: i32,
    /// C++ `m_iExtraMoves` (`getExtraMoves`).
    pub extra_moves: i32,
    /// C++ `m_iExtraMoveDiscount` (`getExtraMoveDiscount`).
    pub extra_move_discount: i32,
    /// C++ `m_iExtraAirRange` (`getExtraAirRange`).
    pub extra_air_range: i32,
    /// C++ `m_iExtraIntercept` (`getExtraIntercept`).
    pub extra_intercept: i32,
    /// C++ `m_iExtraEvasion` (`getExtraEvasion`).
    pub extra_evasion: i32,
    /// C++ `m_iExtraFirstStrikes` (`getExtraFirstStrikes`).
    pub extra_first_strikes: i32,
    /// C++ `m_iExtraChanceFirstStrikes` (`getExtraChanceFirstStrikes`).
    pub extra_chance_first_strikes: i32,
    /// C++ `m_iExtraWithdrawal` (`getExtraWithdrawal`).
    pub extra_withdrawal: i32,
    /// C++ `m_iExtraCollateralDamage` (`getExtraCollateralDamage`).
    pub extra_collateral_damage: i32,
    /// C++ `m_iExtraBombardRate` (`getExtraBombardRate`).
    pub extra_bombard_rate: i32,
    /// C++ `m_iExtraEnemyHeal` (`getExtraEnemyHeal`).
    pub extra_enemy_heal: i32,
    /// C++ `m_iExtraNeutralHeal` (`getExtraNeutralHeal`).
    pub extra_neutral_heal: i32,
    /// C++ `m_iExtraFriendlyHeal` (`getExtraFriendlyHeal`).
    pub extra_friendly_heal: i32,
    /// C++ `m_iSameTileHeal` (`getSameTileHeal`).
    pub same_tile_heal: i32,
    /// C++ `m_iAdjacentTileHeal` (`getAdjacentTileHeal`).
    pub adjacent_tile_heal: i32,
    /// C++ `m_iExtraCombatPercent` (FfH `getExtraCombatPercent` adds the in-borders bonus:
    /// `unit_get_extra_combat_percent`).
    pub extra_combat_percent: i32,
    /// C++ `m_iExtraCityAttackPercent` (`getExtraCityAttackPercent`).
    pub extra_city_attack_percent: i32,
    /// C++ `m_iExtraCityDefensePercent` (`getExtraCityDefensePercent`).
    pub extra_city_defense_percent: i32,
    /// C++ `m_iExtraHillsAttackPercent` (`getExtraHillsAttackPercent`).
    pub extra_hills_attack_percent: i32,
    /// C++ `m_iExtraHillsDefensePercent` (`getExtraHillsDefensePercent`).
    pub extra_hills_defense_percent: i32,
    /// C++ `m_iRevoltProtection` (`getRevoltProtection`).
    pub revolt_protection: i32,
    /// C++ `m_iCollateralDamageProtection` (`getCollateralDamageProtection`).
    pub collateral_damage_protection: i32,
    /// C++ `m_iPillageChange` (`getPillageChange`).
    pub pillage_change: i32,
    /// C++ `m_iUpgradeDiscount` (`getUpgradeDiscount`).
    pub upgrade_discount: i32,
    /// C++ `m_iExperiencePercent` (`getExperiencePercent`).
    pub experience_percent: i32,
    /// C++ `m_iKamikazePercent` (`getKamikazePercent`).
    pub kamikaze_percent: i32,
    /// C++ `m_iBaseCombat` (`setBaseCombatStr`; FfH `baseCombatStr()` adds damage-type strength).
    pub base_combat: i32,
    /// C++ `m_iImmobileTimer` (FfH `getImmobileTimer()` is 999 while held: `unit_get_immobile_timer`).
    pub immobile_timer: i32,
    /// C++ `m_bMadeAttack` (`isMadeAttack`, `setMadeAttack`).
    pub made_attack: bool,
    /// C++ `m_bMadeInterception` (`isMadeInterception`, `setMadeInterception`).
    pub made_interception: bool,
    /// C++ `m_bPromotionReady` (`isPromotionReady`; set through `unit_set_promotion_ready`).
    pub promotion_ready: bool,
    /// C++ `m_bDeathDelay` (`isDelayedDeath`, `startDelayedDeath`).
    pub death_delay: bool,
    /// C++ `m_bBlockading` (`isBlockading`; set through `unit_set_blockading`).
    pub blockading: bool,
    /// C++ `m_bAirCombat` (`isAirCombat`; set by `unit_set_attack_plot`).
    pub air_combat: bool,

    // ---- FfH (Kael) ----
    /// C++ `m_bFleeWithdrawl` (`isFleeWithdrawl`, `setFleeWithdrawl`).
    pub flee_withdrawl: bool,
    /// C++ `m_bHasCasted` (`isHasCasted`, `setHasCasted`).
    pub has_casted: bool,
    /// C++ `m_bIgnoreHide` (`isIgnoreHide`, `setIgnoreHide`).
    pub ignore_hide: bool,
    /// C++ `m_bTerraformer` (`isTerraformer`; set by `unit_update_terraformer`).
    pub terraformer: bool,
    /// C++ `m_iAlive`: number of "not alive" sources; `isAlive()` is `alive == 0`.
    pub alive: i32,
    /// C++ `m_iAIControl` (`isAIControl`): FfH promotion counter (the unit cannot be grouped and
    /// is moved by the AI even for human owners). A `CvUnit` member, not `CvUnitAI` state.
    pub ai_control: i32,
    /// C++ `m_iBoarding` (`isBoarding`).
    pub boarding: i32,
    /// C++ `m_iDefensiveStrikeChance` (`getDefensiveStrikeChance`).
    pub defensive_strike_chance: i32,
    /// C++ `m_iDefensiveStrikeDamage` (`getDefensiveStrikeDamage`).
    pub defensive_strike_damage: i32,
    /// C++ `m_iDoubleFortifyBonus` (`isDoubleFortifyBonus`).
    pub double_fortify_bonus: i32,
    /// C++ `m_iFear` (`isFear`).
    pub fear: i32,
    /// C++ `m_iFlying` (`isFlying`).
    pub flying: i32,
    /// C++ `m_iHeld` (`isHeld`).
    pub held: i32,
    /// C++ `m_iHiddenNationality` (`isHiddenNationality` also looks at the transport).
    pub hidden_nationality: i32,
    /// C++ `m_iIgnoreBuildingDefense` (`ignoreBuildingDefense` also checks the unit info).
    pub ignore_building_defense: i32,
    /// C++ `m_iImmortal` (`isImmortal`).
    pub immortal: i32,
    /// C++ `m_iImmuneToCapture` (`isImmuneToCapture`).
    pub immune_to_capture: i32,
    /// C++ `m_iImmuneToDefensiveStrike` (`isImmuneToDefensiveStrike`).
    pub immune_to_defensive_strike: i32,
    /// C++ `m_iImmuneToFear` (`isImmuneToFear`: also true when not alive).
    pub immune_to_fear: i32,
    /// C++ `m_iImmuneToMagic` (`isImmuneToMagic`).
    pub immune_to_magic: i32,
    /// C++ `m_iInvisible` (`isInvisibleFromPromotion`).
    pub invisible: i32,
    /// C++ `m_iOnlyDefensive` (`isOnlyDefensive` also checks the unit info).
    pub only_defensive: i32,
    /// C++ `m_iSeeInvisible` (`isSeeInvisible`).
    pub see_invisible: i32,
    /// C++ `m_iSummoner` (`getSummoner`, `setSummoner`): the unit that summoned this one
    /// (C++ stores the summoner's per-player id, -1 = none).
    pub summoner: Option<UnitId>,
    /// C++ `m_iTargetWeakestUnit` (`isTargetWeakestUnit`).
    pub target_weakest_unit: i32,
    /// C++ `m_iTargetWeakestUnitCounter` (`isTargetWeakestUnitCounter`).
    pub target_weakest_unit_counter: i32,
    /// C++ `m_iTwincast` (`isTwincast`).
    pub twincast: i32,
    /// C++ `m_iWaterWalking` (`isWaterWalking`).
    pub water_walking: i32,
    /// C++ `m_iBaseCombatDefense` (`setBaseCombatStrDefense`; see `unit_base_combat_str_defense`).
    pub base_combat_defense: i32,
    /// C++ `m_iBetterDefenderThanPercent` (`getBetterDefenderThanPercent`).
    pub better_defender_than_percent: i32,
    /// C++ `m_iCombatHealPercent` (`getCombatHealPercent`).
    pub combat_heal_percent: i32,
    /// C++ `m_iCombatLimit` (`combatLimit`; recomputed by `unit_calc_combat_limit`).
    pub combat_limit: i32,
    /// C++ `m_iCombatPercentInBorders` (`getCombatPercentInBorders`).
    pub combat_percent_in_borders: i32,
    /// C++ `m_iCombatPercentGlobalCounter` (`getCombatPercentGlobalCounter`).
    pub combat_percent_global_counter: i32,
    /// C++ `m_iDelayedSpell` (`getDelayedSpell`, `setDelayedSpell`).
    pub delayed_spell: Option<SpellType>,
    /// C++ `m_iDuration` (`getDuration`, `setDuration`): turns left for summons (0 = permanent).
    pub duration: i32,
    /// C++ `m_iFreePromotionPick` (`getFreePromotionPick`).
    pub free_promotion_pick: i32,
    /// C++ `m_iGoldFromCombat` (`getGoldFromCombat`).
    pub gold_from_combat: i32,
    /// C++ `m_iInvisibleType` (`setInvisibleType`; `getInvisibleType()` has FfH logic:
    /// `unit_get_invisible_type`).
    pub invisible_type: Option<InvisibleType>,
    /// C++ `m_iRace` (`getRace`; set through `unit_set_race`): the race promotion.
    pub race: Option<PromotionType>,
    /// C++ `m_iReligion` (`getReligion`, `setReligion`).
    pub religion: Option<ReligionType>,
    /// C++ `m_iResist` (`getResist`, `setResist`).
    pub resist: i32,
    /// C++ `m_iResistModify` (`getResistModify`, `setResistModify`).
    pub resist_modify: i32,
    /// C++ `m_iScenarioCounter` (`getScenarioCounter`, `setScenarioCounter`): free counter for
    /// scenario Python, -1 by default.
    pub scenario_counter: i32,
    /// C++ `m_iSpellCasterXP` (`getSpellCasterXP()` is 0 unless the unit info has bFreeXP).
    pub spell_caster_xp: i32,
    /// C++ `m_iSpellDamageModify` (`getSpellDamageModify`).
    pub spell_damage_modify: i32,
    /// C++ `m_iTotalDamageTypeCombat` (`getTotalDamageTypeCombat`): sum of `damage_type_combat`.
    pub total_damage_type_combat: i32,
    /// C++ `m_iWorkRateModify` (`getWorkRateModify`).
    pub work_rate_modify: i32,
    /// C++ `m_paiBonusAffinity` (`getBonusAffinity`).
    pub bonus_affinity: IdxVec<BonusType, i32>,
    /// C++ `m_paiBonusAffinityAmount`: strength currently granted by each bonus affinity.
    pub bonus_affinity_amount: IdxVec<BonusType, i32>,
    /// C++ `m_paiDamageTypeCombat` (`getDamageTypeCombat`).
    pub damage_type_combat: IdxVec<DamageType, i32>,
    /// C++ `m_paiDamageTypeResist` (raw; `getDamageTypeResist` clamps: `unit_get_damage_type_resist`).
    pub damage_type_resist: IdxVec<DamageType, i32>,

    // ---- Sephi: automatic spell casting ----
    /// C++ `m_iAutoSpellCast` (`getAutoSpellCast`, `setAutoSpellCast`).
    pub auto_spell_cast: Option<SpellType>,

    /// C++ `m_eOwner` (`getOwner`, `getOwnerINLINE`).
    pub owner: PlayerId,
    /// C++ `m_eCapturingPlayer` (`getCapturingPlayer`; set through `unit_set_capturing_player`).
    pub capturing_player: Option<PlayerId>,
    /// C++ `m_eUnitType` (`getUnitType`; `getUnitInfo()` is `d.units[unit_type]`).
    pub unit_type: UnitType,
    /// C++ `m_eLeaderUnitType` (`getLeaderUnitType`, `setLeaderUnitType`): the great general
    /// that led this unit.
    pub leader_unit_type: Option<UnitType>,
    /// C++ `m_combatUnit` (`getCombatUnit`; set through `unit_set_combat_unit`).
    pub combat_unit: Option<UnitId>,
    /// C++ `m_transportUnit` (`getTransportUnit`; set through `unit_set_transport_unit`).
    pub transport_unit: Option<UnitId>,
    /// C++ `m_aiExtraDomainModifier` (`getExtraDomainModifier`), indexed by `DomainType::idx()`.
    pub extra_domain_modifier: [i32; DomainType::NUM],
    /// C++ `m_szName` (`getNameNoDesc`; set through `unit_set_name`).
    pub name: String,
    /// C++ `m_szScriptData` (`getScriptData`, `setScriptData`).
    pub script_data: String,
    /// C++ `m_pabHasPromotion` (`isHasPromotion`; set through `unit_set_has_promotion`).
    pub has_promotion: IdxVec<PromotionType, bool>,
    /// C++ `m_paiTerrainDoubleMoveCount` (`getTerrainDoubleMoveCount`).
    pub terrain_double_move_count: IdxVec<TerrainType, i32>,
    /// C++ `m_paiFeatureDoubleMoveCount` (`getFeatureDoubleMoveCount`).
    pub feature_double_move_count: IdxVec<FeatureType, i32>,
    /// C++ `m_paiExtraTerrainAttackPercent` (`getExtraTerrainAttackPercent`).
    pub extra_terrain_attack_percent: IdxVec<TerrainType, i32>,
    /// C++ `m_paiExtraTerrainDefensePercent` (`getExtraTerrainDefensePercent`).
    pub extra_terrain_defense_percent: IdxVec<TerrainType, i32>,
    /// C++ `m_paiExtraFeatureAttackPercent` (`getExtraFeatureAttackPercent`).
    pub extra_feature_attack_percent: IdxVec<FeatureType, i32>,
    /// C++ `m_paiExtraFeatureDefensePercent` (`getExtraFeatureDefensePercent`).
    pub extra_feature_defense_percent: IdxVec<FeatureType, i32>,
    /// C++ `m_paiExtraUnitCombatModifier` (`getExtraUnitCombatModifier`).
    pub extra_unit_combat_modifier: IdxVec<UnitCombatType, i32>,

    // ---- CvUnitAI bookkeeping read/written by rules code ----
    /// C++ `CvUnitAI::m_iBirthmark` (`AI_getBirthmark`): drawn from the sync rand in `AI_init`.
    pub ai_birthmark: i32,
    /// C++ `CvUnitAI::m_eUnitAIType` (`AI_getUnitAIType`; set through `unit_ai_set_unit_ai_type`).
    /// `None` (NO_UNITAI) between `reset` and `AI_init`; `setXY` relies on that during `init`.
    pub ai_type: Option<UnitAIType>,
    /// C++ `CvUnitAI::m_iGroupflag` (`AI_getGroupflag`, `AI_setGroupflag`): Better AI GROUPFLAG_*.
    pub ai_groupflag: i32,
    /// C++ `CvUnitAI::m_iOriginX` (see `unit_ai_get_origin_plot`), -1 = none.
    pub ai_origin_x: i32,
    /// C++ `CvUnitAI::m_iOriginY`, -1 = none.
    pub ai_origin_y: i32,
    /// C++ `CvUnitAI::m_bSuicideSummon` (`isSuicideSummon`, `setSuicideSummon`).
    pub ai_suicide_summon: bool,
    /// C++ `CvUnitAI::m_bAllowedPermDefense` (`changeAllowedPermDefense`, exposed to Python).
    pub ai_allowed_perm_defense: bool,
    /// C++ `CvUnitAI::m_bAllowedPatrol` (`changeAllowedPatrol`, exposed to Python).
    pub ai_allowed_patrol: bool,
    /// C++ `CvUnitAI::m_bAllowedExplore` (`changeAllowedExplore`, exposed to Python).
    pub ai_allowed_explore: bool,
}

impl Unit {
    /// C++ `CvUnit::reset(iID, eUnit, eOwner, false)` followed by `CvUnitAI::AI_reset()`: a unit
    /// with every member at its default, the per-type arrays sized from the game data and the
    /// `CvUnitInfo`-derived members taken from `info`.
    pub fn new(id: UnitId, unit_type: UnitType, owner: PlayerId, d: &GameData, info: &UnitResetInfo) -> Unit {
        let mut unit = Unit::reset_members(id, unit_type, owner, info);
        // C++ `if (!bConstructorCall)`: allocate and fill the per-type arrays.
        unit.damage_type_combat =
            IdxVec::from_fn(d.damages.len(), |i| info.damage_type_combat.get(i).copied().unwrap_or(0));
        unit.damage_type_resist = IdxVec::from_elem(0, d.damages.len());
        unit.total_damage_type_combat = unit.damage_type_combat.iter().sum();
        unit.bonus_affinity = IdxVec::from_elem(0, d.bonuses.len());
        unit.bonus_affinity_amount = IdxVec::from_elem(0, d.bonuses.len());
        unit.has_promotion = IdxVec::from_elem(false, d.promotions.len());
        unit.terrain_double_move_count = IdxVec::from_elem(0, d.terrains.len());
        unit.extra_terrain_attack_percent = IdxVec::from_elem(0, d.terrains.len());
        unit.extra_terrain_defense_percent = IdxVec::from_elem(0, d.terrains.len());
        unit.feature_double_move_count = IdxVec::from_elem(0, d.features.len());
        unit.extra_feature_attack_percent = IdxVec::from_elem(0, d.features.len());
        unit.extra_feature_defense_percent = IdxVec::from_elem(0, d.features.len());
        unit.extra_unit_combat_modifier = IdxVec::from_elem(0, d.unit_combats.len());
        unit
    }

    /// The scalar part of `CvUnit::reset` (everything outside the `!bConstructorCall` blocks) and
    /// `CvUnitAI::AI_reset(NO_UNITAI)`. Per-type arrays are left empty.
    fn reset_members(id: UnitId, unit_type: UnitType, owner: PlayerId, info: &UnitResetInfo) -> Unit {
        Unit {
            id,
            group: None,
            x: INVALID_PLOT_COORD,
            y: INVALID_PLOT_COORD,
            last_move_turn: 0,
            recon_x: INVALID_PLOT_COORD,
            recon_y: INVALID_PLOT_COORD,
            game_turn_created: 0,
            damage: 0,
            moves: 0,
            experience: 0,
            level: 1,
            cargo: 0,
            cargo_capacity: info.cargo_space,
            attack_plot_x: INVALID_PLOT_COORD,
            attack_plot_y: INVALID_PLOT_COORD,
            combat_timer: 0,
            combat_first_strikes: 0,
            fortify_turns: 0,
            blitz_count: 0,
            amphib_count: 0,
            river_count: 0,
            enemy_route_count: 0,
            always_heal_count: 0,
            hills_double_move_count: 0,
            immune_to_first_strikes_count: 0,
            extra_visibility_range: 0,
            extra_moves: 0,
            extra_move_discount: 0,
            extra_air_range: 0,
            extra_intercept: 0,
            extra_evasion: 0,
            extra_first_strikes: 0,
            extra_chance_first_strikes: 0,
            extra_withdrawal: 0,
            extra_collateral_damage: 0,
            extra_bombard_rate: 0,
            extra_enemy_heal: 0,
            extra_neutral_heal: 0,
            extra_friendly_heal: 0,
            same_tile_heal: 0,
            adjacent_tile_heal: 0,
            extra_combat_percent: 0,
            extra_city_attack_percent: 0,
            extra_city_defense_percent: 0,
            extra_hills_attack_percent: 0,
            extra_hills_defense_percent: 0,
            revolt_protection: 0,
            collateral_damage_protection: 0,
            pillage_change: 0,
            upgrade_discount: 0,
            experience_percent: 0,
            kamikaze_percent: 0,
            base_combat: info.combat,
            immobile_timer: 0,
            made_attack: false,
            made_interception: false,
            promotion_ready: false,
            death_delay: false,
            blockading: false,
            air_combat: false,
            flee_withdrawl: false,
            has_casted: false,
            ignore_hide: false,
            terraformer: false,
            alive: 0,
            ai_control: 0,
            boarding: 0,
            defensive_strike_chance: info.defensive_strike_chance,
            defensive_strike_damage: info.defensive_strike_damage,
            double_fortify_bonus: 0,
            fear: 0,
            flying: 0,
            held: 0,
            hidden_nationality: info.hidden_nationality as i32,
            ignore_building_defense: info.ignore_building_defense as i32,
            immortal: info.immortal as i32,
            immune_to_capture: 0,
            immune_to_defensive_strike: info.immune_to_defensive_strike as i32,
            immune_to_fear: 0,
            immune_to_magic: 0,
            invisible: 0,
            only_defensive: 0,
            see_invisible: 0,
            summoner: None,
            target_weakest_unit: 0,
            target_weakest_unit_counter: 0,
            twincast: 0,
            water_walking: 0,
            base_combat_defense: info.combat_defense,
            better_defender_than_percent: 100,
            combat_heal_percent: 0,
            combat_limit: info.combat_limit,
            combat_percent_in_borders: 0,
            combat_percent_global_counter: 0,
            delayed_spell: None,
            duration: 0,
            free_promotion_pick: 0,
            gold_from_combat: info.gold_from_combat,
            invisible_type: info.invisible_type,
            race: None,
            religion: None,
            resist: 0,
            resist_modify: 0,
            scenario_counter: -1,
            spell_caster_xp: 0,
            spell_damage_modify: 0,
            total_damage_type_combat: 0,
            work_rate_modify: 0,
            bonus_affinity: IdxVec::new(),
            bonus_affinity_amount: IdxVec::new(),
            damage_type_combat: IdxVec::new(),
            damage_type_resist: IdxVec::new(),
            auto_spell_cast: None,
            owner,
            capturing_player: None,
            unit_type,
            leader_unit_type: None,
            combat_unit: None,
            transport_unit: None,
            extra_domain_modifier: [0; DomainType::NUM],
            name: String::new(),
            script_data: String::new(),
            has_promotion: IdxVec::new(),
            terrain_double_move_count: IdxVec::new(),
            feature_double_move_count: IdxVec::new(),
            extra_terrain_attack_percent: IdxVec::new(),
            extra_terrain_defense_percent: IdxVec::new(),
            extra_feature_attack_percent: IdxVec::new(),
            extra_feature_defense_percent: IdxVec::new(),
            extra_unit_combat_modifier: IdxVec::new(),
            ai_birthmark: 0,
            ai_type: None,
            ai_groupflag: GROUPFLAG_NONE,
            ai_origin_x: -1,
            ai_origin_y: -1,
            ai_suicide_summon: false,
            ai_allowed_perm_defense: true,
            ai_allowed_patrol: true,
            ai_allowed_explore: true,
        }
    }
}

impl Default for Unit {
    /// C++ `CvUnit::CvUnit()`: `reset(0, NO_UNIT, NO_PLAYER, true)` (no per-type arrays) and
    /// `CvUnitAI::AI_reset()`. Type and owner index 0 stand in for NO_UNIT / NO_PLAYER; use
    /// [`Unit::new`] for real units (C++ `init` always resets first).
    fn default() -> Unit {
        let mut unit = Unit::reset_members(UnitId(0), UnitType(0), PlayerId(0), &UnitResetInfo::default());
        // C++ quirk: with NO_UNIT the constructor leaves `m_iInvisibleType = 0` (not NO_INVISIBLE).
        unit.invisible_type = Some(InvisibleType(0));
        unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_defaults() {
        let d = ffh_data::global();
        let mut info = UnitResetInfo {
            combat: 5,
            cargo_space: 2,
            combat_defense: 4,
            combat_limit: 100,
            immortal: true,
            ..Default::default()
        };
        info.damage_type_combat = IdxVec::from_elem(0, d.damages.len());
        info.damage_type_combat[DamageType::DAMAGE_FIRE] = 2;
        let unit = Unit::new(UnitId(7), UnitType(3), PlayerId(1), d, &info);
        assert_eq!(unit.id, UnitId(7));
        assert_eq!((unit.x, unit.y), (INVALID_PLOT_COORD, INVALID_PLOT_COORD));
        assert_eq!(unit.level, 1);
        assert_eq!(unit.base_combat, 5);
        assert_eq!(unit.base_combat_defense, 4);
        assert_eq!(unit.cargo_capacity, 2);
        assert_eq!(unit.immortal, 1);
        assert_eq!(unit.better_defender_than_percent, 100);
        assert_eq!(unit.scenario_counter, -1);
        assert_eq!(unit.total_damage_type_combat, 2);
        assert_eq!(unit.has_promotion.len(), d.promotions.len());
        assert_eq!(unit.terrain_double_move_count.len(), d.terrains.len());
        assert_eq!(unit.extra_feature_defense_percent.len(), d.features.len());
        assert_eq!(unit.extra_unit_combat_modifier.len(), d.unit_combats.len());
        assert_eq!(unit.bonus_affinity_amount.len(), d.bonuses.len());
        assert_eq!(unit.damage_type_resist.len(), d.damages.len());
        assert_eq!(unit.ai_type, None);
        assert_eq!(unit.ai_groupflag, GROUPFLAG_NONE);
        assert!(unit.ai_allowed_perm_defense && unit.ai_allowed_patrol && unit.ai_allowed_explore);

        let blank = Unit::default();
        assert!(blank.has_promotion.is_empty());
        assert_eq!(blank.base_combat, 0);
        assert_eq!(blank.invisible_type, Some(InvisibleType(0)));
    }

    #[test]
    fn reset_info_from_unit_info() {
        let d = ffh_data::global();
        let info = |name: &str| UnitResetInfo::from_info(&d.units[d.t::<UnitType>(name)]);

        // Units/CIV4UnitInfos.xml values.
        let archer = info("UNIT_ARCHER_SCORPION_CLAN");
        assert_eq!((archer.combat, archer.combat_defense, archer.combat_limit), (3, 5, 100));
        assert_eq!((archer.defensive_strike_chance, archer.defensive_strike_damage), (20, 10));
        assert_eq!(archer.invisible_type, None);
        assert!(!archer.immortal && !archer.hidden_nationality);
        assert_eq!(archer.damage_type_combat[DamageType::DAMAGE_POISON], 1);
        assert_eq!(archer.damage_type_combat.iter().sum::<i32>(), 1);

        let pirate = info("UNIT_PIRATE");
        assert_eq!((pirate.cargo_space, pirate.gold_from_combat, pirate.combat), (1, 3, 8));

        let shadowrider = info("UNIT_SHADOWRIDER");
        assert!(shadowrider.ignore_building_defense && shadowrider.immune_to_defensive_strike);

        let brigit = info("UNIT_BRIGIT");
        assert!(brigit.immortal);
        assert_eq!(brigit.damage_type_combat[DamageType::DAMAGE_HOLY], 3);

        // `reset` copies them into the new unit.
        let t = d.t::<UnitType>("UNIT_BRIGIT");
        let unit = Unit::new(UnitId(1), t, PlayerId(0), d, &brigit);
        assert_eq!((unit.immortal, unit.base_combat, unit.total_damage_type_combat), (1, 6, 3));
    }
}
