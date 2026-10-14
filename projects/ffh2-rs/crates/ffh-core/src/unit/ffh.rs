//! FfH2 additions to CvUnit: spells (`canCast`, `cast`, ...), summons, damage types and
//! resistances, immortality, weapons, bonus affinity, betrayal, terraformers and the FfH status
//! counters (`isFlying`, `changeFear`, ...).
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges. Every `todo!()` names the C++ function still to port.

#![allow(unused_variables, unused_imports)]

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::unit::{CombatDetails, DefenderCombatValues, Unit, UnitResetInfo};
use ffh_data::*;

impl Game {
    /// C++ `CvUnit::canSpellAutomate` (CvUnit.cpp:3827-3915).
    /// Sephi: human-only automatic spell casting (COMMAND_AUTOMATE_SPELL). C++ non-const;
    /// changes nothing.
    pub fn unit_can_spell_automate(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canSpellAutomate")
    }

    /// C++ `CvUnit::canCast` (CvUnit.cpp:13675-14101).
    /// C++ non-const; changes nothing (the FfH `req*` Python requirement functions have no side
    /// effects). The spell's `PyRequirement` goes through `crate::ffh`.
    pub fn unit_can_cast(&self, u: UnitId, spell: SpellType, test_visible: bool) -> bool {
        todo!("CvUnit::canCast")
    }

    /// C++ `CvUnit::canCreateUnit` (CvUnit.cpp:14103-14140).
    pub fn unit_can_create_unit(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canCreateUnit")
    }

    /// C++ `CvUnit::canAddPromotion` (CvUnit.cpp:14142-14234).
    /// C++ non-const; changes nothing.
    pub fn unit_can_add_promotion(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canAddPromotion")
    }

    /// C++ `CvUnit::canCreateBuilding` (CvUnit.cpp:14236-14247).
    pub fn unit_can_create_building(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canCreateBuilding")
    }

    /// C++ `CvUnit::canCreateFeature` (CvUnit.cpp:14249-14271).
    pub fn unit_can_create_feature(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canCreateFeature")
    }

    /// C++ `CvUnit::canCreateImprovement` (CvUnit.cpp:14273-14288).
    pub fn unit_can_create_improvement(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canCreateImprovement")
    }

    /// C++ `CvUnit::canDispel` (CvUnit.cpp:14290-14330).
    /// C++ non-const; changes nothing.
    pub fn unit_can_dispel(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canDispel")
    }

    /// C++ `CvUnit::canImmobile` (CvUnit.cpp:14332-14359).
    /// C++ non-const; changes nothing.
    pub fn unit_can_immobile(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canImmobile")
    }

    /// C++ `CvUnit::canPush` (CvUnit.cpp:14361-14391).
    /// C++ non-const; changes nothing.
    pub fn unit_can_push(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canPush")
    }

    /// C++ `CvUnit::canRemovePromotion` (CvUnit.cpp:14393-14482).
    /// C++ non-const; changes nothing.
    pub fn unit_can_remove_promotion(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canRemovePromotion")
    }

    /// C++ `CvUnit::canSpreadReligion` (CvUnit.cpp:14484-14501).
    pub fn unit_can_spread_religion(&self, u: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::canSpreadReligion")
    }

    /// C++ `CvUnit::cast` (CvUnit.cpp:14503-14675).
    /// Runs the spell's `PyResult` through `crate::ffh` and the built-in effects below.
    pub fn unit_cast(&mut self, u: UnitId, spell: SpellType) {
        todo!("CvUnit::cast")
    }

    /// C++ `CvUnit::castAddPromotion` (CvUnit.cpp:14677-14793).
    pub fn unit_cast_add_promotion(&mut self, u: UnitId, spell: SpellType) {
        todo!("CvUnit::castAddPromotion")
    }

    /// C++ `CvUnit::castDamage` (CvUnit.cpp:14795-14838).
    pub fn unit_cast_damage(&mut self, u: UnitId, spell: SpellType) {
        todo!("CvUnit::castDamage")
    }

    /// C++ `CvUnit::castDispel` (CvUnit.cpp:14840-14893).
    pub fn unit_cast_dispel(&mut self, u: UnitId, spell: SpellType) {
        todo!("CvUnit::castDispel")
    }

    /// C++ `CvUnit::castImmobile` (CvUnit.cpp:14895-14940).
    pub fn unit_cast_immobile(&mut self, u: UnitId, spell: SpellType) {
        todo!("CvUnit::castImmobile")
    }

    /// C++ `CvUnit::castPush` (CvUnit.cpp:14942-14998).
    pub fn unit_cast_push(&mut self, u: UnitId, spell: SpellType) {
        todo!("CvUnit::castPush")
    }

    /// C++ `CvUnit::castRemovePromotion` (CvUnit.cpp:15000-15080).
    pub fn unit_cast_remove_promotion(&mut self, u: UnitId, spell: SpellType) {
        todo!("CvUnit::castRemovePromotion")
    }

    /// C++ `CvUnit::castConvertUnit` (CvUnit.cpp:15082-15088).
    pub fn unit_cast_convert_unit(&mut self, u: UnitId, spell: SpellType) {
        todo!("CvUnit::castConvertUnit")
    }

    /// C++ `CvUnit::castCreateUnit` (CvUnit.cpp:15090-15175).
    /// In AI "summon suicide mode" the summon is flagged (`ai_suicide_summon`, groupflag) and
    /// immediately runs `AI_update` (AI: controller hook).
    pub fn unit_cast_create_unit(&mut self, u: UnitId, spell: SpellType) {
        todo!("CvUnit::castCreateUnit")
    }

    /// C++ `CvUnit::isImmuneToSpell` (CvUnit.cpp:15197-15245).
    pub fn unit_is_immune_to_spell(&self, u: UnitId, caster: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::isImmuneToSpell")
    }

    /// C++ `CvUnit::changeDuration` (CvUnit.cpp:15267-15270).
    pub fn unit_change_duration(&mut self, u: UnitId, change: i32) {
        // C++ `setDuration(getDuration() + iChange)`.
        self.units[u].duration += change;
    }

    /// C++ `CvUnit::isAlive` (CvUnit.cpp:15282-15285).
    pub fn unit_is_alive(&self, u: UnitId) -> bool {
        // `alive` counts "not alive" sources (mechanical units, undead promotions, ...).
        self.units[u].alive == 0
    }

    /// C++ `CvUnit::changeAlive` (CvUnit.cpp:15287-15293).
    pub fn unit_change_alive(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].alive += change;
        }
    }

    /// C++ `CvUnit::isAIControl` (CvUnit.cpp:15295-15298).
    pub fn unit_is_ai_control(&self, u: UnitId) -> bool {
        self.units[u].ai_control != 0
    }

    /// C++ `CvUnit::changeAIControl` (CvUnit.cpp:15300-15306).
    pub fn unit_change_ai_control(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].ai_control += change;
        }
    }

    /// C++ `CvUnit::isBoarding` (CvUnit.cpp:15308-15311).
    pub fn unit_is_boarding(&self, u: UnitId) -> bool {
        self.units[u].boarding != 0
    }

    /// C++ `CvUnit::changeBoarding` (CvUnit.cpp:15313-15319).
    pub fn unit_change_boarding(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].boarding += change;
        }
    }

    /// C++ `CvUnit::changeDefensiveStrikeChance` (CvUnit.cpp:15326-15332).
    pub fn unit_change_defensive_strike_chance(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].defensive_strike_chance += change;
        }
    }

    /// C++ `CvUnit::changeDefensiveStrikeDamage` (CvUnit.cpp:15339-15345).
    pub fn unit_change_defensive_strike_damage(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].defensive_strike_damage += change;
        }
    }

    /// C++ `CvUnit::isDoubleFortifyBonus` (CvUnit.cpp:15347-15350).
    pub fn unit_is_double_fortify_bonus(&self, u: UnitId) -> bool {
        self.units[u].double_fortify_bonus != 0
    }

    /// C++ `CvUnit::changeDoubleFortifyBonus` (CvUnit.cpp:15352-15358).
    pub fn unit_change_double_fortify_bonus(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].double_fortify_bonus += change;
        }
    }

    /// C++ `CvUnit::isFear` (CvUnit.cpp:15360-15363).
    pub fn unit_is_fear(&self, u: UnitId) -> bool {
        self.units[u].fear != 0
    }

    /// C++ `CvUnit::changeFear` (CvUnit.cpp:15365-15371).
    pub fn unit_change_fear(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].fear += change;
        }
    }

    /// C++ `CvUnit::isFlying` (CvUnit.cpp:15373-15376).
    pub fn unit_is_flying(&self, u: UnitId) -> bool {
        self.units[u].flying != 0
    }

    /// C++ `CvUnit::changeFlying` (CvUnit.cpp:15378-15384).
    pub fn unit_change_flying(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].flying += change;
        }
    }

    /// C++ `CvUnit::isHeld` (CvUnit.cpp:15386-15389).
    pub fn unit_is_held(&self, u: UnitId) -> bool {
        self.units[u].held != 0
    }

    /// C++ `CvUnit::changeHeld` (CvUnit.cpp:15391-15397).
    pub fn unit_change_held(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].held += change;
        }
    }

    /// C++ `CvUnit::isHiddenNationality` (CvUnit.cpp:15399-15409).
    pub fn unit_is_hidden_nationality(&self, u: UnitId) -> bool {
        // Cargo is only hidden if its transport is.
        if self.unit_is_cargo(u) {
            if let Some(transport) = self.units[u].transport_unit {
                if !self.unit_is_hidden_nationality(transport) {
                    return false;
                }
            }
        }
        self.units[u].hidden_nationality != 0
    }

    /// C++ `CvUnit::changeHiddenNationality` (CvUnit.cpp:15411-15427).
    /// Losing the last source re-applies blockades and leaves the group (`joinGroup(NULL, true)`).
    pub fn unit_change_hidden_nationality(&mut self, u: UnitId, change: i32) {
        todo!("CvUnit::changeHiddenNationality")
    }

    /// C++ `CvUnit::changeIgnoreBuildingDefense` (CvUnit.cpp:15429-15435).
    pub fn unit_change_ignore_building_defense(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].ignore_building_defense += change;
        }
    }

    /// C++ `CvUnit::isImmortal` (CvUnit.cpp:15437-15440).
    pub fn unit_is_immortal(&self, u: UnitId) -> bool {
        self.units[u].immortal != 0
    }

    /// C++ `CvUnit::changeImmortal` (CvUnit.cpp:15442-15452).
    pub fn unit_change_immortal(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            let unit = &mut self.units[u];
            unit.immortal += change;
            if unit.immortal < 0 {
                unit.immortal = 0;
            }
        }
    }

    /// C++ `CvUnit::isImmuneToCapture` (CvUnit.cpp:15454-15457).
    pub fn unit_is_immune_to_capture(&self, u: UnitId) -> bool {
        self.units[u].immune_to_capture != 0
    }

    /// C++ `CvUnit::changeImmuneToCapture` (CvUnit.cpp:15459-15465).
    pub fn unit_change_immune_to_capture(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].immune_to_capture += change;
        }
    }

    /// C++ `CvUnit::isImmuneToDefensiveStrike` (CvUnit.cpp:15467-15470).
    pub fn unit_is_immune_to_defensive_strike(&self, u: UnitId) -> bool {
        self.units[u].immune_to_defensive_strike != 0
    }

    /// C++ `CvUnit::changeImmuneToDefensiveStrike` (CvUnit.cpp:15472-15478).
    pub fn unit_change_immune_to_defensive_strike(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].immune_to_defensive_strike += change;
        }
    }

    /// C++ `CvUnit::isImmuneToFear` (CvUnit.cpp:15480-15487).
    pub fn unit_is_immune_to_fear(&self, u: UnitId) -> bool {
        // FfH: units that are not alive are immune to fear.
        if !self.unit_is_alive(u) {
            return true;
        }
        self.units[u].immune_to_fear != 0
    }

    /// C++ `CvUnit::changeImmuneToFear` (CvUnit.cpp:15489-15495).
    pub fn unit_change_immune_to_fear(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].immune_to_fear += change;
        }
    }

    /// C++ `CvUnit::isImmuneToMagic` (CvUnit.cpp:15497-15500).
    pub fn unit_is_immune_to_magic(&self, u: UnitId) -> bool {
        self.units[u].immune_to_magic != 0
    }

    /// C++ `CvUnit::changeImmuneToMagic` (CvUnit.cpp:15502-15508).
    pub fn unit_change_immune_to_magic(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].immune_to_magic += change;
        }
    }

    /// C++ `CvUnit::isInvisibleFromPromotion` (CvUnit.cpp:15510-15513).
    pub fn unit_is_invisible_from_promotion(&self, u: UnitId) -> bool {
        self.units[u].invisible != 0
    }

    /// C++ `CvUnit::changeInvisibleFromPromotion` (CvUnit.cpp:15515-15528).
    /// Becoming visible next to enemies withdraws the unit (`withdrawlToNearestValidPlot(true)`).
    pub fn unit_change_invisible_from_promotion(&mut self, u: UnitId, change: i32) {
        todo!("CvUnit::changeInvisibleFromPromotion")
    }

    /// C++ `CvUnit::isSeeInvisible` (CvUnit.cpp:15530-15533).
    pub fn unit_is_see_invisible(&self, u: UnitId) -> bool {
        self.units[u].see_invisible != 0
    }

    /// C++ `CvUnit::changeSeeInvisible` (CvUnit.cpp:15535-15541).
    pub fn unit_change_see_invisible(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].see_invisible += change;
        }
    }

    /// C++ `CvUnit::changeOnlyDefensive` (CvUnit.cpp:15543-15549).
    pub fn unit_change_only_defensive(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].only_defensive += change;
        }
    }

    /// C++ `CvUnit::isTargetWeakestUnit` (CvUnit.cpp:15551-15554).
    pub fn unit_is_target_weakest_unit(&self, u: UnitId) -> bool {
        self.units[u].target_weakest_unit != 0
    }

    /// C++ `CvUnit::changeTargetWeakestUnit` (CvUnit.cpp:15556-15562).
    pub fn unit_change_target_weakest_unit(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].target_weakest_unit += change;
        }
    }

    /// C++ `CvUnit::isTargetWeakestUnitCounter` (CvUnit.cpp:15564-15567).
    pub fn unit_is_target_weakest_unit_counter(&self, u: UnitId) -> bool {
        self.units[u].target_weakest_unit_counter != 0
    }

    /// C++ `CvUnit::changeTargetWeakestUnitCounter` (CvUnit.cpp:15569-15575).
    pub fn unit_change_target_weakest_unit_counter(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].target_weakest_unit_counter += change;
        }
    }

    /// C++ `CvUnit::isTwincast` (CvUnit.cpp:15577-15580).
    pub fn unit_is_twincast(&self, u: UnitId) -> bool {
        self.units[u].twincast != 0
    }

    /// C++ `CvUnit::changeTwincast` (CvUnit.cpp:15582-15588).
    pub fn unit_change_twincast(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].twincast += change;
        }
    }

    /// C++ `CvUnit::isWaterWalking` (CvUnit.cpp:15590-15593).
    pub fn unit_is_water_walking(&self, u: UnitId) -> bool {
        self.units[u].water_walking != 0
    }

    /// C++ `CvUnit::changeWaterWalking` (CvUnit.cpp:15595-15601).
    pub fn unit_change_water_walking(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].water_walking += change;
        }
    }

    /// C++ `CvUnit::changeBetterDefenderThanPercent` (CvUnit.cpp:15608-15614).
    pub fn unit_change_better_defender_than_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].better_defender_than_percent += change;
        }
    }

    /// C++ `CvUnit::changeCombatHealPercent` (CvUnit.cpp:15621-15627).
    pub fn unit_change_combat_heal_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].combat_heal_percent += change;
        }
    }

    /// C++ `CvUnit::calcCombatLimit` (CvUnit.cpp:15629-15648).
    /// `combat_limit` = min(unit info `iCombatLimit`, every non-zero promotion `iCombatLimit` the
    /// unit has).
    pub fn unit_calc_combat_limit(&mut self, u: UnitId) {
        todo!("CvUnit::calcCombatLimit")
    }

    /// C++ `CvUnit::changeCombatPercentInBorders` (CvUnit.cpp:15655-15662).
    pub fn unit_change_combat_percent_in_borders(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].combat_percent_in_borders += change;
        }
    }

    /// C++ `CvUnit::changeCombatPercentGlobalCounter` (CvUnit.cpp:15669-15676).
    pub fn unit_change_combat_percent_global_counter(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].combat_percent_global_counter += change;
        }
    }

    /// C++ `CvUnit::changeFreePromotionPick` (CvUnit.cpp:15683-15689).
    pub fn unit_change_free_promotion_pick(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].free_promotion_pick += change;
        }
    }

    /// C++ `CvUnit::changeGoldFromCombat` (CvUnit.cpp:15696-15702).
    pub fn unit_change_gold_from_combat(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].gold_from_combat += change;
        }
    }

    /// C++ `CvUnit::mutate` (CvUnit.cpp:15709-15722).
    pub fn unit_mutate(&mut self, u: UnitId) {
        todo!("CvUnit::mutate")
    }

    /// C++ `CvUnit::getWithdrawlProbDefensive` (CvUnit.cpp:15724-15732).
    pub fn unit_get_withdrawl_prob_defensive(&self, u: UnitId) -> i32 {
        todo!("CvUnit::getWithdrawlProbDefensive")
    }

    /// C++ `CvUnit::setRace` (CvUnit.cpp:15744-15751).
    /// Removes the old race promotion first (`setHasPromotion(old, false)`).
    pub fn unit_set_race(&mut self, u: UnitId, race: Option<PromotionType>) {
        todo!("CvUnit::setRace")
    }

    /// C++ `CvUnit::changeResist` (CvUnit.cpp:15773-15776).
    pub fn unit_change_resist(&mut self, u: UnitId, change: i32) {
        // C++ `setResist(getResist() + iChange)`.
        self.units[u].resist += change;
    }

    /// C++ `CvUnit::changeResistModify` (CvUnit.cpp:15788-15791).
    pub fn unit_change_resist_modify(&mut self, u: UnitId, change: i32) {
        // C++ `setResistModify(getResistModify() + iChange)`.
        self.units[u].resist_modify += change;
    }

    /// C++ `CvUnit::getSpellCasterXP` (CvUnit.cpp:15803-15810).
    /// 0 unless the unit info has `bFreeXP`; else the `spell_caster_xp` field.
    pub fn unit_get_spell_caster_xp(&self, u: UnitId) -> i32 {
        todo!("CvUnit::getSpellCasterXP")
    }

    /// C++ `CvUnit::changeSpellCasterXP` (CvUnit.cpp:15812-15818).
    pub fn unit_change_spell_caster_xp(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].spell_caster_xp += change;
        }
    }

    /// C++ `CvUnit::changeSpellDamageModify` (CvUnit.cpp:15825-15831).
    pub fn unit_change_spell_damage_modify(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].spell_damage_modify += change;
        }
    }

    /// C++ `CvUnit::changeWorkRateModify` (CvUnit.cpp:15848-15854).
    pub fn unit_change_work_rate_modify(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].work_rate_modify += change;
        }
    }

    /// C++ `CvUnit::isResisted` (CvUnit.cpp:15856-15866).
    /// C++ is `const` but draws the "is Resisted" sync rand, hence `&mut self`.
    pub fn unit_is_resisted(&mut self, u: UnitId, caster: UnitId, spell: SpellType) -> bool {
        todo!("CvUnit::isResisted")
    }

    /// C++ `CvUnit::getResistChance` (CvUnit.cpp:15868-15895).
    pub fn unit_get_resist_chance(&self, u: UnitId, caster: UnitId, spell: SpellType) -> i32 {
        todo!("CvUnit::getResistChance")
    }

    /// C++ `CvUnit::changeBaseCombatStr` (CvUnit.cpp:15897-15900).
    pub fn unit_change_base_combat_str(&mut self, u: UnitId, change: i32) {
        // C++ `setBaseCombatStr(m_iBaseCombat + iChange)`.
        self.units[u].base_combat += change;
    }

    /// C++ `CvUnit::changeBaseCombatStrDefense` (CvUnit.cpp:15902-15905).
    pub fn unit_change_base_combat_str_defense(&mut self, u: UnitId, change: i32) {
        // C++ `setBaseCombatStrDefense(m_iBaseCombatDefense + iChange)`.
        self.units[u].base_combat_defense += change;
    }

    /// C++ `CvUnit::getExtraSpellMove` (CvUnit.cpp:16090-16104).
    /// Only used by AI code (`chooseSpell`, CvUnitAI); a pure query.
    pub fn unit_get_extra_spell_move(&self, u: UnitId) -> i32 {
        todo!("CvUnit::getExtraSpellMove")
    }

    /// C++ `CvUnit::doDamage` (CvUnit.cpp:16106-16204).
    /// C++ `iDmgType` -1 = no damage type.
    pub fn unit_do_damage(
        &mut self,
        u: UnitId,
        damage: i32,
        damage_limit: i32,
        attacker: Option<UnitId>,
        damage_type: Option<DamageType>,
        start_war: bool,
    ) {
        todo!("CvUnit::doDamage")
    }

    /// C++ `CvUnit::doEscape` (CvUnit.cpp:16261-16267).
    pub fn unit_do_escape(&mut self, u: UnitId) {
        todo!("CvUnit::doEscape")
    }

    /// C++ `CvUnit::doImmortalRebirth` (CvUnit.cpp:16269-16304).
    /// Ends with `pushMission(MISSION_HEAL)` for AI units (Better AI bookkeeping in rules code).
    pub fn unit_do_immortal_rebirth(&mut self, u: UnitId) {
        todo!("CvUnit::doImmortalRebirth")
    }

    /// C++ `CvUnit::setWeapons` (CvUnit.cpp:16562-16617).
    pub fn unit_set_weapons(&mut self, u: UnitId) {
        todo!("CvUnit::setWeapons")
    }

    /// C++ `CvUnit::changeBonusAffinity` (CvUnit.cpp:16619-16626).
    pub fn unit_change_bonus_affinity(&mut self, u: UnitId, bonus: BonusType, change: i32) {
        todo!("CvUnit::changeBonusAffinity")
    }

    /// C++ `CvUnit::updateBonusAffinity` (CvUnit.cpp:16633-16648).
    pub fn unit_update_bonus_affinity(&mut self, u: UnitId, bonus: BonusType) {
        todo!("CvUnit::updateBonusAffinity")
    }

    /// C++ `CvUnit::changeDamageTypeCombat` (CvUnit.cpp:16650-16657).
    pub fn unit_change_damage_type_combat(&mut self, u: UnitId, damage_type: DamageType, change: i32) {
        if change != 0 {
            let unit = &mut self.units[u];
            unit.damage_type_combat[damage_type] += change;
            unit.total_damage_type_combat += change;
        }
    }

    /// C++ `CvUnit::getDamageTypeResist` (CvUnit.cpp:16669-16681).
    pub fn unit_get_damage_type_resist(&self, u: UnitId, damage_type: DamageType) -> i32 {
        // Clamped to [-100, 100].
        let i = self.units[u].damage_type_resist[damage_type];
        if i <= -100 {
            return -100;
        }
        if i >= 100 {
            return 100;
        }
        i
    }

    /// C++ `CvUnit::changeDamageTypeResist` (CvUnit.cpp:16683-16689).
    pub fn unit_change_damage_type_resist(&mut self, u: UnitId, damage_type: DamageType, change: i32) {
        if change != 0 {
            self.units[u].damage_type_resist[damage_type] += change;
        }
    }

    /// C++ `CvUnit::countUnitsWithinRange` (CvUnit.cpp:16691-16726).
    /// C++ non-const; changes nothing.
    pub fn unit_count_units_within_range(&self, u: UnitId, range: i32, enemy: bool, neutral: bool, team: bool) -> i32 {
        todo!("CvUnit::countUnitsWithinRange")
    }

    /// C++ `CvUnit::betray` (CvUnit.cpp:16765-16781).
    pub fn unit_betray(&mut self, u: UnitId, player: PlayerId) {
        todo!("CvUnit::betray")
    }

    /// C++ `CvUnit::updateTerraformer` (CvUnit.cpp:16783-16798).
    /// `terraformer` = can cast any spell with `bAllowAutomateTerrain`.
    pub fn unit_update_terraformer(&mut self, u: UnitId) {
        todo!("CvUnit::updateTerraformer")
    }
}
