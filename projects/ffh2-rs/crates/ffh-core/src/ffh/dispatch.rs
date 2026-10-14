//! Resolution of the Python callback names stored in XML into Rust functions.
//!
//! The original `eval`s strings like `spellSummon(pCaster,'UNIT_IMP')` on every
//! call. Here every callback string is parsed once ([`PyCall`]) and bound to a
//! function pointer looked up by name in the implementation modules
//! (`spells_*.rs`, `random_events_*.rs`). Unresolved names are collected in
//! [`Tables::unresolved`]; a test requires it to be empty once porting is complete.

use std::sync::OnceLock;

use crate::ffh::pycall::PyCall;
use crate::game::Game;
use crate::ids::*;
use crate::structs::EventTriggeredData;
use ffh_data::*;

/// `spellX(pCaster, ...)` -- PyResult.
pub type SpellFn = fn(&mut Game, UnitId, &PyCall);
/// `reqX(pCaster, ...)` -- PyRequirement.
pub type ReqFn = fn(&Game, UnitId, &PyCall) -> bool;
/// Unit-only effect: promotion `PyPerTurn`, spell `PyMiscast`.
pub type UnitFn = fn(&mut Game, UnitId, &PyCall);
/// `(pCaster, pPlot)`: improvement/feature `PythonOnMove`, improvement `PythonAtRange`.
pub type PlotFn = fn(&mut Game, UnitId, PlotIdx, &PyCall);
/// `(pCaster, pOpponent)`: unit `PythonPostCombatWon` / `PythonPostCombatLost`.
pub type CombatFn = fn(&mut Game, UnitId, UnitId, &PyCall);
/// `(eVote, iData)`: vote `PyResult`.
pub type VoteFn = fn(&mut Game, VoteType, i32, &PyCall);
/// Trigger `PythonCanDo(triggerData)`.
pub type TriggerCanDoFn = fn(&mut Game, &mut EventTriggeredData) -> bool;
/// Trigger `PythonCanDoCity(eTrigger, ePlayer, iCityId)`.
pub type TriggerCityFn = fn(&Game, EventTriggerType, PlayerId, CityId) -> bool;
/// Trigger `PythonCanDoUnit(eTrigger, ePlayer, iUnitId)`.
pub type TriggerUnitFn = fn(&Game, EventTriggerType, PlayerId, UnitId) -> bool;
/// Trigger `PythonCallback(triggerData, ePlayer)`.
pub type TriggerCallbackFn = fn(&mut Game, &EventTriggeredData, PlayerId);
/// Event `PythonCanDo(eEvent, triggerData)`.
pub type EventCanDoFn = fn(&Game, EventType, &EventTriggeredData) -> bool;
/// Event `PythonCallback(eEvent, triggerData)`.
pub type EventCallbackFn = fn(&mut Game, EventType, &EventTriggeredData);
/// Event `PythonExpireCheck(eEvent, triggerData)`.
pub type EventExpireFn = fn(&Game, EventType, &EventTriggeredData) -> bool;

/// A resolved callback: function + its parsed call (for literal arguments).
pub struct Bound<F> {
    pub f: F,
    pub call: PyCall,
}

pub struct Tables {
    pub spell_result: IdxVec<SpellType, Option<Bound<SpellFn>>>,
    pub spell_requirement: IdxVec<SpellType, Option<Bound<ReqFn>>>,
    pub spell_miscast: IdxVec<SpellType, Option<Bound<UnitFn>>>,
    pub promotion_per_turn: IdxVec<PromotionType, Option<Bound<UnitFn>>>,
    pub improvement_on_move: IdxVec<ImprovementType, Option<Bound<PlotFn>>>,
    pub improvement_at_range: IdxVec<ImprovementType, Option<Bound<PlotFn>>>,
    pub feature_on_move: IdxVec<FeatureType, Option<Bound<PlotFn>>>,
    pub unit_post_combat_won: IdxVec<UnitType, Option<Bound<CombatFn>>>,
    pub unit_post_combat_lost: IdxVec<UnitType, Option<Bound<CombatFn>>>,
    pub vote_result: IdxVec<VoteType, Option<Bound<VoteFn>>>,
    pub trigger_can_do: IdxVec<EventTriggerType, Option<Bound<TriggerCanDoFn>>>,
    pub trigger_can_do_city: IdxVec<EventTriggerType, Option<Bound<TriggerCityFn>>>,
    pub trigger_can_do_unit: IdxVec<EventTriggerType, Option<Bound<TriggerUnitFn>>>,
    pub trigger_callback: IdxVec<EventTriggerType, Option<Bound<TriggerCallbackFn>>>,
    pub event_can_do: IdxVec<EventType, Option<Bound<EventCanDoFn>>>,
    pub event_callback: IdxVec<EventType, Option<Bound<EventCallbackFn>>>,
    pub event_expire_check: IdxVec<EventType, Option<Bound<EventExpireFn>>>,
    /// "<table>: <callback string>" for every non-empty string with no Rust implementation.
    pub unresolved: Vec<String>,
}

fn bind<K: Idx, F: Copy>(
    n: usize,
    unresolved: &mut Vec<String>,
    what: &str,
    src: impl Fn(K) -> String,
    find: impl Fn(&str) -> Option<F>,
) -> IdxVec<K, Option<Bound<F>>> {
    IdxVec::from_fn(n, |k| {
        let s = src(k);
        let call = PyCall::parse(&s)?;
        match find(&call.name) {
            Some(f) => Some(Bound { f, call }),
            None => {
                unresolved.push(format!("{what}: {s}"));
                None
            }
        }
    })
}

impl Tables {
    pub fn build(d: &GameData) -> Tables {
        use crate::ffh::{random_events_a as ra, random_events_b as rb, spells_a as sa, spells_b as sb};
        let mut un = Vec::new();
        let spell_result = bind(d.spells.len(), &mut un, "SpellInfo.PyResult", |k: SpellType| d.spells[k].py_result.clone(), |n| sa::find_spell(n).or_else(|| sb::find_spell(n)));
        let spell_requirement = bind(d.spells.len(), &mut un, "SpellInfo.PyRequirement", |k: SpellType| d.spells[k].py_requirement.clone(), |n| sa::find_req(n).or_else(|| sb::find_req(n)));
        let spell_miscast = bind(d.spells.len(), &mut un, "SpellInfo.PyMiscast", |k: SpellType| d.spells[k].py_miscast.clone(), |n| sa::find_unit_fn(n).or_else(|| sb::find_unit_fn(n)));
        let promotion_per_turn = bind(d.promotions.len(), &mut un, "PromotionInfo.PyPerTurn", |k: PromotionType| d.promotions[k].py_per_turn.clone(), |n| sa::find_unit_fn(n).or_else(|| sb::find_unit_fn(n)));
        let improvement_on_move = bind(d.improvements.len(), &mut un, "ImprovementInfo.PythonOnMove", |k: ImprovementType| d.improvements[k].python_on_move.clone(), |n| sa::find_plot_fn(n).or_else(|| sb::find_plot_fn(n)));
        let improvement_at_range = bind(d.improvements.len(), &mut un, "ImprovementInfo.PythonAtRange", |k: ImprovementType| d.improvements[k].python_at_range.clone(), |n| sa::find_plot_fn(n).or_else(|| sb::find_plot_fn(n)));
        let feature_on_move = bind(d.features.len(), &mut un, "FeatureInfo.PythonOnMove", |k: FeatureType| d.features[k].python_on_move.clone(), |n| sa::find_plot_fn(n).or_else(|| sb::find_plot_fn(n)));
        let unit_post_combat_won = bind(d.units.len(), &mut un, "UnitInfo.PythonPostCombatWon", |k: UnitType| d.units[k].py_post_combat_won.clone(), |n| sa::find_combat_fn(n).or_else(|| sb::find_combat_fn(n)));
        let unit_post_combat_lost = bind(d.units.len(), &mut un, "UnitInfo.PythonPostCombatLost", |k: UnitType| d.units[k].py_post_combat_lost.clone(), |n| sa::find_combat_fn(n).or_else(|| sb::find_combat_fn(n)));
        let vote_result = bind(d.votes.len(), &mut un, "VoteInfo.PyResult", |k: VoteType| d.votes[k].py_result.clone(), |n| sa::find_vote_fn(n).or_else(|| sb::find_vote_fn(n)));
        let trigger_can_do = bind(d.event_triggers.len(), &mut un, "EventTriggerInfo.PythonCanDo", |k: EventTriggerType| d.event_triggers[k].python_can_do.clone(), |n| ra::find_trigger_can_do(n).or_else(|| rb::find_trigger_can_do(n)));
        let trigger_can_do_city = bind(d.event_triggers.len(), &mut un, "EventTriggerInfo.PythonCanDoCity", |k: EventTriggerType| d.event_triggers[k].python_can_do_city.clone(), |n| ra::find_trigger_city(n).or_else(|| rb::find_trigger_city(n)));
        let trigger_can_do_unit = bind(d.event_triggers.len(), &mut un, "EventTriggerInfo.PythonCanDoUnit", |k: EventTriggerType| d.event_triggers[k].python_can_do_unit.clone(), |n| ra::find_trigger_unit(n).or_else(|| rb::find_trigger_unit(n)));
        let trigger_callback = bind(d.event_triggers.len(), &mut un, "EventTriggerInfo.PythonCallback", |k: EventTriggerType| d.event_triggers[k].python_callback.clone(), |n| ra::find_trigger_callback(n).or_else(|| rb::find_trigger_callback(n)));
        let event_can_do = bind(d.events.len(), &mut un, "EventInfo.PythonCanDo", |k: EventType| d.events[k].python_can_do.clone(), |n| ra::find_event_can_do(n).or_else(|| rb::find_event_can_do(n)));
        let event_callback = bind(d.events.len(), &mut un, "EventInfo.PythonCallback", |k: EventType| d.events[k].python_callback.clone(), |n| ra::find_event_callback(n).or_else(|| rb::find_event_callback(n)));
        let event_expire_check = bind(d.events.len(), &mut un, "EventInfo.PythonExpireCheck", |k: EventType| d.events[k].python_expire_check.clone(), |n| ra::find_event_expire(n).or_else(|| rb::find_event_expire(n)));
        Tables {
            spell_result,
            spell_requirement,
            spell_miscast,
            promotion_per_turn,
            improvement_on_move,
            improvement_at_range,
            feature_on_move,
            unit_post_combat_won,
            unit_post_combat_lost,
            vote_result,
            trigger_can_do,
            trigger_can_do_city,
            trigger_can_do_unit,
            trigger_callback,
            event_can_do,
            event_callback,
            event_expire_check,
            unresolved: un,
        }
    }
}

static TABLES: OnceLock<(usize, Tables)> = OnceLock::new();

/// Dispatch tables for this game's data (built on first use).
pub fn tables(d: &'static GameData) -> &'static Tables {
    let (ptr, t) = TABLES.get_or_init(|| (d as *const GameData as usize, Tables::build(d)));
    assert_eq!(*ptr, d as *const GameData as usize, "dispatch tables were built for different GameData");
    t
}
