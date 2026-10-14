//! CvRandomEventInterface.py callbacks, named in EventTriggerInfo / EventInfo XML
//! (`PythonCanDo`, `PythonCanDoCity`, `PythonCanDoUnit`, `PythonCallback`,
//! `PythonExpireCheck`). Dispatched by name; the trigger data may be modified by
//! `CanDo` checks exactly as the Python may modify `pTriggerData`.

#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use crate::structs::EventTriggeredData;
use ffh_data::*;

impl Game {
    /// Trigger `PythonCanDo(argsList=[triggerData])`: false cancels the trigger.
    pub fn py_trigger_can_do(&mut self, data: &mut EventTriggeredData) -> bool {
        true
    }
    /// Trigger `PythonCanDoCity(eTrigger, ePlayer, iCityId)`.
    pub fn py_trigger_can_do_city(&mut self, trigger: EventTriggerType, p: PlayerId, c: CityId) -> bool {
        true
    }
    /// Trigger `PythonCanDoUnit(eTrigger, ePlayer, iUnitId)`.
    pub fn py_trigger_can_do_unit(&mut self, trigger: EventTriggerType, p: PlayerId, u: UnitId) -> bool {
        true
    }
    /// Trigger `PythonCallback(triggerData, ePlayer)` (FFH2 calls it when the trigger fires).
    pub fn py_trigger_callback(&mut self, data: &EventTriggeredData, p: PlayerId) {}
    /// Event `PythonCanDo(eEvent, triggerData)`.
    pub fn py_event_can_do(&mut self, event: EventType, data: &EventTriggeredData) -> bool {
        true
    }
    /// Event `PythonCallback(eEvent, triggerData)`.
    pub fn py_event_callback(&mut self, event: EventType, data: &EventTriggeredData) {}
    /// Event `PythonExpireCheck(eEvent, triggerData)`: true = expire.
    pub fn py_event_expire_check(&mut self, event: EventType, data: &EventTriggeredData) -> bool {
        false
    }
}
