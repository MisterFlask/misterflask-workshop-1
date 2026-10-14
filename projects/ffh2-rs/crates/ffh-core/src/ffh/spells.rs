//! Spell-interface hooks: port of CvSpellInterface.py entry points, called where
//! the DLL calls `callFunction(PYSpellModule, ..)`. Each entry dispatches to the
//! function named in XML through the tables in `dispatch.rs`; an empty XML string
//! means "no callback" (C++ checks `!isEmpty()` before calling Python).

use crate::ffh::dispatch::tables;
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    /// `cast(pCaster, eSpell)`: run the spell's `PyResult`.
    pub fn py_cast(&mut self, caster: UnitId, spell: SpellType) {
        if let Some(b) = &tables(self.d).spell_result[spell] {
            (b.f)(self, caster, &b.call);
        }
    }
    /// `canCast(pCaster, eSpell)`: evaluate the spell's `PyRequirement` (true if empty).
    pub fn py_can_cast(&self, caster: UnitId, spell: SpellType) -> bool {
        match &tables(self.d).spell_requirement[spell] {
            Some(b) => (b.f)(self, caster, &b.call),
            None => true,
        }
    }
    /// `miscast(pCaster, eSpell)`: run the spell's `PyMiscast`.
    pub fn py_miscast(&mut self, caster: UnitId, spell: SpellType) {
        if let Some(b) = &tables(self.d).spell_miscast[spell] {
            (b.f)(self, caster, &b.call);
        }
    }
    /// `effect(pCaster, eProm)`: run the promotion's `PyPerTurn`.
    pub fn py_promotion_effect(&mut self, unit: UnitId, promotion: PromotionType) {
        if let Some(b) = &tables(self.d).promotion_per_turn[promotion] {
            (b.f)(self, unit, &b.call);
        }
    }
    /// `atRange(pCaster, pPlot, eImp)`: run the improvement's `PythonAtRange`.
    pub fn py_improvement_at_range(&mut self, unit: UnitId, plot: PlotIdx, improvement: ImprovementType) {
        if let Some(b) = &tables(self.d).improvement_at_range[improvement] {
            (b.f)(self, unit, plot, &b.call);
        }
    }
    /// `onMove(pCaster, pPlot, eImp)`: run the improvement's `PythonOnMove`.
    pub fn py_improvement_on_move(&mut self, unit: UnitId, plot: PlotIdx, improvement: ImprovementType) {
        if let Some(b) = &tables(self.d).improvement_on_move[improvement] {
            (b.f)(self, unit, plot, &b.call);
        }
    }
    /// `onMoveFeature(pCaster, pPlot, eFeature)`: run the feature's `PythonOnMove`.
    pub fn py_feature_on_move(&mut self, unit: UnitId, plot: PlotIdx, feature: FeatureType) {
        if let Some(b) = &tables(self.d).feature_on_move[feature] {
            (b.f)(self, unit, plot, &b.call);
        }
    }
    /// `vote(eVote, int)`: run the vote's `PyResult`.
    pub fn py_vote(&mut self, vote: VoteType, data: i32) {
        if let Some(b) = &tables(self.d).vote_result[vote] {
            (b.f)(self, vote, data, &b.call);
        }
    }
    /// `postCombatLost(pCaster, pOpponent)`: run the unit type's `PythonPostCombatLost`.
    pub fn py_post_combat_lost(&mut self, unit: UnitId, opponent: UnitId) {
        let ut = self.unit_type_of(unit);
        if let Some(b) = &tables(self.d).unit_post_combat_lost[ut] {
            (b.f)(self, unit, opponent, &b.call);
        }
    }
    /// `postCombatWon(pCaster, pOpponent)`: run the unit type's `PythonPostCombatWon`.
    pub fn py_post_combat_won(&mut self, unit: UnitId, opponent: UnitId) {
        let ut = self.unit_type_of(unit);
        if let Some(b) = &tables(self.d).unit_post_combat_won[ut] {
            (b.f)(self, unit, opponent, &b.call);
        }
    }
}

impl Game {
    /// Unit type of a unit (C++ `getUnitType()`).
    fn unit_type_of(&self, u: UnitId) -> UnitType {
        self.units[u].unit_type
    }
}
