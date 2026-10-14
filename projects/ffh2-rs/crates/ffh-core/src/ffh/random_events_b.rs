//! Port of CvRandomEventInterface.py (part b). Each Python function becomes a Rust
//! `fn` registered by its Python name in the `find_*` lookups below (see dispatch.rs).

#![allow(unused_imports)]
use crate::ffh::dispatch::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::EventTriggeredData;
use ffh_data::*;

/// Trigger PythonCanDo (`canTriggerX(argsList=[triggerData])`).
pub fn find_trigger_can_do(name: &str) -> Option<TriggerCanDoFn> {
    match name {
        _ => None,
    }
}

/// Trigger PythonCanDoCity (`canTriggerXCity(eTrigger, ePlayer, iCity)`).
pub fn find_trigger_city(name: &str) -> Option<TriggerCityFn> {
    match name {
        _ => None,
    }
}

/// Trigger PythonCanDoUnit (`canTriggerXUnit(eTrigger, ePlayer, iUnit)`).
pub fn find_trigger_unit(name: &str) -> Option<TriggerUnitFn> {
    match name {
        _ => None,
    }
}

/// Trigger PythonCallback (`doX(triggerData, ePlayer)`).
pub fn find_trigger_callback(name: &str) -> Option<TriggerCallbackFn> {
    match name {
        _ => None,
    }
}

/// Event PythonCanDo (`canApplyX(eEvent, triggerData)`).
pub fn find_event_can_do(name: &str) -> Option<EventCanDoFn> {
    match name {
        _ => None,
    }
}

/// Event PythonCallback (`applyX(eEvent, triggerData)`).
pub fn find_event_callback(name: &str) -> Option<EventCallbackFn> {
    match name {
        _ => None,
    }
}

/// Event PythonExpireCheck (`expireX(eEvent, triggerData)`).
pub fn find_event_expire(name: &str) -> Option<EventExpireFn> {
    match name {
        _ => None,
    }
}
