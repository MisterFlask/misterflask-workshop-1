//! Info classes: EventInfo, EventTriggerInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp.

#![allow(unused_imports)]
use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

/// C++ `CvEventInfo` (Events/CIV4EventInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct EventInfo {
    pub base: InfoBase,
}

pub fn read_event(rd: &Rd) -> EventInfo {
    EventInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvEventTriggerInfo` (Events/CIV4EventTriggerInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct EventTriggerInfo {
    pub base: InfoBase,
}

pub fn read_event_trigger(rd: &Rd) -> EventTriggerInfo {
    EventTriggerInfo { base: InfoBase::read(rd), ..Default::default() }
}
