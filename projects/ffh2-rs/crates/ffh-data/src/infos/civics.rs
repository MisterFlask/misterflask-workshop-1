//! Info classes: SpecialistInfo, ReligionInfo, CorporationInfo, CivicOptionInfo, CivicInfo, VoteInfo, VoteSourceInfo, EspionageMissionInfo, ProjectInfo, TechInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp.

#![allow(unused_imports)]
use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

/// C++ `CvSpecialistInfo` (Gameinfo/CIV4SpecialistInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct SpecialistInfo {
    pub base: InfoBase,
}

pub fn read_specialist(rd: &Rd) -> SpecialistInfo {
    SpecialistInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvReligionInfo` (Gameinfo/CIV4ReligionInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct ReligionInfo {
    pub base: InfoBase,
}

pub fn read_religion(rd: &Rd) -> ReligionInfo {
    ReligionInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvCorporationInfo` (Gameinfo/CIV4CorporationInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct CorporationInfo {
    pub base: InfoBase,
}

pub fn read_corporation(rd: &Rd) -> CorporationInfo {
    CorporationInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvCivicOptionInfo` (Gameinfo/CIV4CivicOptionInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct CivicOptionInfo {
    pub base: InfoBase,
}

pub fn read_civic_option(rd: &Rd) -> CivicOptionInfo {
    CivicOptionInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvCivicInfo` (Gameinfo/CIV4CivicInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct CivicInfo {
    pub base: InfoBase,
}

pub fn read_civic(rd: &Rd) -> CivicInfo {
    CivicInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvVoteInfo` (Gameinfo/CIV4VoteInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct VoteInfo {
    pub base: InfoBase,
}

pub fn read_vote(rd: &Rd) -> VoteInfo {
    VoteInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvVoteSourceInfo` (Gameinfo/CIV4VoteSourceInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct VoteSourceInfo {
    pub base: InfoBase,
}

pub fn read_vote_source(rd: &Rd) -> VoteSourceInfo {
    VoteSourceInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvEspionageMissionInfo` (Gameinfo/CIV4EspionageMissionInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct EspionageMissionInfo {
    pub base: InfoBase,
}

pub fn read_espionage_mission(rd: &Rd) -> EspionageMissionInfo {
    EspionageMissionInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvProjectInfo` (Gameinfo/CIV4ProjectInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct ProjectInfo {
    pub base: InfoBase,
}

pub fn read_project(rd: &Rd) -> ProjectInfo {
    ProjectInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvTechInfo` (Technologies/CIV4TechInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct TechInfo {
    pub base: InfoBase,
}

pub fn read_tech(rd: &Rd) -> TechInfo {
    TechInfo { base: InfoBase::read(rd), ..Default::default() }
}
