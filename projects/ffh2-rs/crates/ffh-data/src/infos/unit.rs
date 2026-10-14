//! Info classes: SpecialUnitInfo, UnitClassInfo, PromotionInfo, BuildInfo, UnitInfo, SpellInfo, AutomateInfo, CommandInfo, MissionInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp.

#![allow(unused_imports)]
use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

/// C++ `CvSpecialUnitInfo` (Units/CIV4SpecialUnitInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct SpecialUnitInfo {
    pub base: InfoBase,
}

pub fn read_special_unit(rd: &Rd) -> SpecialUnitInfo {
    SpecialUnitInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvUnitClassInfo` (Units/CIV4UnitClassInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct UnitClassInfo {
    pub base: InfoBase,
}

pub fn read_unit_class(rd: &Rd) -> UnitClassInfo {
    UnitClassInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvPromotionInfo` (Units/CIV4PromotionInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct PromotionInfo {
    pub base: InfoBase,
}

pub fn read_promotion(rd: &Rd) -> PromotionInfo {
    PromotionInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvBuildInfo` (Units/CIV4BuildInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct BuildInfo {
    pub base: InfoBase,
}

pub fn read_build(rd: &Rd) -> BuildInfo {
    BuildInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvUnitInfo` (Units/CIV4UnitInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct UnitInfo {
    pub base: InfoBase,
}

pub fn read_unit(rd: &Rd) -> UnitInfo {
    UnitInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvSpellInfo` (Units/CIV4SpellInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct SpellInfo {
    pub base: InfoBase,
}

pub fn read_spell(rd: &Rd) -> SpellInfo {
    SpellInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvAutomateInfo` (Units/CIV4AutomateInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct AutomateInfo {
    pub base: InfoBase,
}

pub fn read_automate(rd: &Rd) -> AutomateInfo {
    AutomateInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvCommandInfo` (Units/CIV4CommandInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct CommandInfo {
    pub base: InfoBase,
}

pub fn read_command(rd: &Rd) -> CommandInfo {
    CommandInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvMissionInfo` (Units/CIV4MissionInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct MissionInfo {
    pub base: InfoBase,
}

pub fn read_mission(rd: &Rd) -> MissionInfo {
    MissionInfo { base: InfoBase::read(rd), ..Default::default() }
}
