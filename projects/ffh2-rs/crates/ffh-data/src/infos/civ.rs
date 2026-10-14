//! Info classes: TraitInfo, CivilizationInfo, LeaderHeadInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp.

#![allow(unused_imports)]
use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

/// C++ `CvTraitInfo` (Civilizations/CIV4TraitInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct TraitInfo {
    pub base: InfoBase,
}

pub fn read_trait(rd: &Rd) -> TraitInfo {
    TraitInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvCivilizationInfo` (Civilizations/CIV4CivilizationInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct CivilizationInfo {
    pub base: InfoBase,
}

pub fn read_civilization(rd: &Rd) -> CivilizationInfo {
    CivilizationInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvLeaderHeadInfo` (Civilizations/CIV4LeaderHeadInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct LeaderHeadInfo {
    pub base: InfoBase,
}

pub fn read_leader_head(rd: &Rd) -> LeaderHeadInfo {
    LeaderHeadInfo { base: InfoBase::read(rd), ..Default::default() }
}
