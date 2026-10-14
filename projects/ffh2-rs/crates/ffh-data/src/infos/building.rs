//! Info classes: SpecialBuildingInfo, BuildingClassInfo, BuildingInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp.

#![allow(unused_imports)]
use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

/// C++ `CvSpecialBuildingInfo` (Buildings/CIV4SpecialBuildingInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct SpecialBuildingInfo {
    pub base: InfoBase,
}

pub fn read_special_building(rd: &Rd) -> SpecialBuildingInfo {
    SpecialBuildingInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvBuildingClassInfo` (Buildings/CIV4BuildingClassInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct BuildingClassInfo {
    pub base: InfoBase,
}

pub fn read_building_class(rd: &Rd) -> BuildingClassInfo {
    BuildingClassInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvBuildingInfo` (Buildings/CIV4BuildingInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct BuildingInfo {
    pub base: InfoBase,
}

pub fn read_building(rd: &Rd) -> BuildingInfo {
    BuildingInfo { base: InfoBase::read(rd), ..Default::default() }
}
