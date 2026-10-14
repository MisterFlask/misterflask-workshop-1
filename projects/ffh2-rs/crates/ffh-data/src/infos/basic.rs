//! Info classes: AlignmentInfo, AttitudeInfo, CalendarInfo, DamageInfo, DenialInfo,
//! DomainInfo, InvisibleInfo, MemoryInfo, MonthInfo, SeasonInfo, UnitAIInfo, UnitCombatInfo.
//!
//! C++ has no dedicated class for any of these: `CvGlobals` keeps each table as a plain
//! `std::vector<CvInfoBase*>` (`m_paAlignmentInfo`, `m_paDamageTypeInfo`, ...) filled by
//! `LoadGlobalClassInfo` in `CvXMLLoadUtility::LoadBasicInfos`, so their `read()` is
//! `CvInfoBase::read` and every struct here holds only the base fields (the XML has
//! nothing else: `Type`, `Description`, sometimes `Button`).
//!
//! The order of the alignment, attitude, calendar, damage, denial, domain, memory and
//! UnitAI tables must match the C++ enums in `CvEnums.h` (`enums.rs` consts).

use crate::infos::InfoBase;
use crate::registry::Rd;

/// C++ `CvInfoBase` from `GC.getAlignmentInfo(e)` (Basicinfos/CIV4AlignmentInfos.xml, FfH).
#[derive(Debug, Clone, Default)]
pub struct AlignmentInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_alignment(rd: &Rd) -> AlignmentInfo {
    AlignmentInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getAttitudeInfo(e)` (Basicinfos/CIV4AttitudeInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct AttitudeInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_attitude(rd: &Rd) -> AttitudeInfo {
    AttitudeInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getCalendarInfo(e)` (Basicinfos/CIV4CalendarInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct CalendarInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_calendar(rd: &Rd) -> CalendarInfo {
    CalendarInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getDamageTypeInfo(e)` (Basicinfos/CIV4DamageInfos.xml, FfH).
#[derive(Debug, Clone, Default)]
pub struct DamageInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_damage(rd: &Rd) -> DamageInfo {
    DamageInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getDenialInfo(e)` (Basicinfos/CIV4DenialInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct DenialInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_denial(rd: &Rd) -> DenialInfo {
    DenialInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getDomainInfo(e)` (Basicinfos/CIV4DomainInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct DomainInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_domain(rd: &Rd) -> DomainInfo {
    DomainInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getInvisibleInfo(e)` (Basicinfos/CIV4InvisibleInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct InvisibleInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_invisible(rd: &Rd) -> InvisibleInfo {
    InvisibleInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getMemoryInfo(e)` (Basicinfos/CIV4MemoryInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct MemoryInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_memory(rd: &Rd) -> MemoryInfo {
    MemoryInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getMonthInfo(e)` (Basicinfos/CIV4MonthInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct MonthInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_month(rd: &Rd) -> MonthInfo {
    MonthInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getSeasonInfo(e)` (Basicinfos/CIV4SeasonInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct SeasonInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_season(rd: &Rd) -> SeasonInfo {
    SeasonInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getUnitAIInfo(e)` (Basicinfos/CIV4UnitAIInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct UnitAIInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_unit_ai(rd: &Rd) -> UnitAIInfo {
    UnitAIInfo { base: InfoBase::read(rd) }
}

/// C++ `CvInfoBase` from `GC.getUnitCombatInfo(e)` (Basicinfos/CIV4UnitCombatInfos.xml).
#[derive(Debug, Clone, Default)]
pub struct UnitCombatInfo {
    pub base: InfoBase,
}

/// C++ `CvInfoBase::read`.
pub fn read_unit_combat(rd: &Rd) -> UnitCombatInfo {
    UnitCombatInfo { base: InfoBase::read(rd) }
}
