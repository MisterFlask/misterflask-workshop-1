//! CvRandomEventInterface.py callbacks, named in EventTriggerInfo / EventInfo XML
//! (`PythonCanDo`, `PythonCanDoCity`, `PythonCanDoUnit`, `PythonCallback`,
//! `PythonExpireCheck`). Dispatched by name through `dispatch.rs`; the trigger data
//! may be modified by `CanDo` checks exactly as the Python may modify `pTriggerData`.
//! Empty XML strings mean "no callback" (C++ checks `!empty()` first).

use crate::ffh::dispatch::tables;
use crate::game::Game;
use crate::ids::*;
use crate::structs::EventTriggeredData;
use ffh_data::*;

impl Game {
    /// Trigger `PythonCanDo(triggerData)`: false cancels the trigger.
    pub fn py_trigger_can_do(&mut self, data: &mut EventTriggeredData) -> bool {
        match &tables(self.d).trigger_can_do[data.trigger] {
            Some(b) => (b.f)(self, data),
            None => true,
        }
    }
    /// Trigger `PythonCanDoCity(eTrigger, ePlayer, iCityId)`.
    pub fn py_trigger_can_do_city(&self, trigger: EventTriggerType, p: PlayerId, c: CityId) -> bool {
        match &tables(self.d).trigger_can_do_city[trigger] {
            Some(b) => (b.f)(self, trigger, p, c),
            None => true,
        }
    }
    /// Trigger `PythonCanDoUnit(eTrigger, ePlayer, iUnitId)`.
    pub fn py_trigger_can_do_unit(&self, trigger: EventTriggerType, p: PlayerId, u: UnitId) -> bool {
        match &tables(self.d).trigger_can_do_unit[trigger] {
            Some(b) => (b.f)(self, trigger, p, u),
            None => true,
        }
    }
    /// Trigger `PythonCallback(triggerData, ePlayer)` (FFH2 calls it when the trigger fires).
    pub fn py_trigger_callback(&mut self, data: &EventTriggeredData, p: PlayerId) {
        if let Some(b) = &tables(self.d).trigger_callback[data.trigger] {
            (b.f)(self, data, p);
        }
    }
    /// Event `PythonCanDo(eEvent, triggerData)`.
    pub fn py_event_can_do(&self, event: EventType, data: &EventTriggeredData) -> bool {
        match &tables(self.d).event_can_do[event] {
            Some(b) => (b.f)(self, event, data),
            None => true,
        }
    }
    /// Event `PythonCallback(eEvent, triggerData)`.
    pub fn py_event_callback(&mut self, event: EventType, data: &EventTriggeredData) {
        if let Some(b) = &tables(self.d).event_callback[event] {
            (b.f)(self, event, data);
        }
    }
    /// Event `PythonExpireCheck(eEvent, triggerData)`: true = expire.
    pub fn py_event_expire_check(&self, event: EventType, data: &EventTriggeredData) -> bool {
        match &tables(self.d).event_expire_check[event] {
            Some(b) => (b.f)(self, event, data),
            None => false,
        }
    }
}
