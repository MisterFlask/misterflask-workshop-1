//! Info classes: AlignmentInfo, AttitudeInfo, CalendarInfo, DamageInfo, DenialInfo, DomainInfo, InvisibleInfo, MemoryInfo, MonthInfo, SeasonInfo, UnitAIInfo, UnitCombatInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp.

#![allow(unused_imports)]
use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

/// C++ `CvAlignmentInfo` (Basicinfos/CIV4AlignmentInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct AlignmentInfo {
    pub base: InfoBase,
}

pub fn read_alignment(rd: &Rd) -> AlignmentInfo {
    AlignmentInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvAttitudeInfo` (Basicinfos/CIV4AttitudeInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct AttitudeInfo {
    pub base: InfoBase,
}

pub fn read_attitude(rd: &Rd) -> AttitudeInfo {
    AttitudeInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvCalendarInfo` (Basicinfos/CIV4CalendarInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct CalendarInfo {
    pub base: InfoBase,
}

pub fn read_calendar(rd: &Rd) -> CalendarInfo {
    CalendarInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvDamageInfo` (Basicinfos/CIV4DamageInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct DamageInfo {
    pub base: InfoBase,
}

pub fn read_damage(rd: &Rd) -> DamageInfo {
    DamageInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvDenialInfo` (Basicinfos/CIV4DenialInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct DenialInfo {
    pub base: InfoBase,
}

pub fn read_denial(rd: &Rd) -> DenialInfo {
    DenialInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvDomainInfo` (Basicinfos/CIV4DomainInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct DomainInfo {
    pub base: InfoBase,
}

pub fn read_domain(rd: &Rd) -> DomainInfo {
    DomainInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvInvisibleInfo` (Basicinfos/CIV4InvisibleInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct InvisibleInfo {
    pub base: InfoBase,
}

pub fn read_invisible(rd: &Rd) -> InvisibleInfo {
    InvisibleInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvMemoryInfo` (Basicinfos/CIV4MemoryInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct MemoryInfo {
    pub base: InfoBase,
}

pub fn read_memory(rd: &Rd) -> MemoryInfo {
    MemoryInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvMonthInfo` (Basicinfos/CIV4MonthInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct MonthInfo {
    pub base: InfoBase,
}

pub fn read_month(rd: &Rd) -> MonthInfo {
    MonthInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvSeasonInfo` (Basicinfos/CIV4SeasonInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct SeasonInfo {
    pub base: InfoBase,
}

pub fn read_season(rd: &Rd) -> SeasonInfo {
    SeasonInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvUnitAIInfo` (Basicinfos/CIV4UnitAIInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct UnitAIInfo {
    pub base: InfoBase,
}

pub fn read_unit_ai(rd: &Rd) -> UnitAIInfo {
    UnitAIInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvUnitCombatInfo` (Basicinfos/CIV4UnitCombatInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct UnitCombatInfo {
    pub base: InfoBase,
}

pub fn read_unit_combat(rd: &Rd) -> UnitCombatInfo {
    UnitCombatInfo { base: InfoBase::read(rd), ..Default::default() }
}
