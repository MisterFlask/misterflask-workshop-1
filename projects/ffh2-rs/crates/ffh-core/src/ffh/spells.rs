//! Spell-interface hooks: port of CvSpellInterface.py entry points, called where
//! the DLL calls `callFunction(PYSpellModule, ..)`. The per-callback Python
//! functions (spellX / reqX / postCombatX / ...) are dispatched by the name stored
//! in XML (see `pycall.rs`).

#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    /// `cast(pCaster, eSpell)`: run the spell's `PyResult`.
    pub fn py_cast(&mut self, caster: UnitId, spell: SpellType) {}
    /// `canCast(pCaster, eSpell)`: evaluate the spell's `PyRequirement` (true if empty).
    pub fn py_can_cast(&self, caster: UnitId, spell: SpellType) -> bool {
        true
    }
    /// `miscast(pCaster, eSpell)`: run the spell's `PyMiscast`.
    pub fn py_miscast(&mut self, caster: UnitId, spell: SpellType) {}
    /// `effect(pCaster, eProm)`: run the promotion's `PyPerTurn`.
    pub fn py_promotion_effect(&mut self, unit: UnitId, promotion: PromotionType) {}
    /// `atRange(pCaster, pPlot, eImp)`: run the improvement's `PythonAtRange`.
    pub fn py_improvement_at_range(&mut self, unit: UnitId, plot: PlotIdx, improvement: ImprovementType) {}
    /// `onMove(pCaster, pPlot, eImp)`: run the improvement's `PythonOnMove`.
    pub fn py_improvement_on_move(&mut self, unit: UnitId, plot: PlotIdx, improvement: ImprovementType) {}
    /// `onMoveFeature(pCaster, pPlot, eFeature)`: run the feature's `PythonOnMove`.
    pub fn py_feature_on_move(&mut self, unit: UnitId, plot: PlotIdx, feature: FeatureType) {}
    /// `vote(eVote, int)`: run the vote's `PyResult`.
    pub fn py_vote(&mut self, vote: VoteType, data: i32) {}
    /// `postCombatLost(pCaster, pOpponent)`: run the unit type's `PythonPostCombatLost`.
    pub fn py_post_combat_lost(&mut self, unit: UnitId, opponent: UnitId) {}
    /// `postCombatWon(pCaster, pOpponent)`: run the unit type's `PythonPostCombatWon`.
    pub fn py_post_combat_won(&mut self, unit: UnitId, opponent: UnitId) {}
}
