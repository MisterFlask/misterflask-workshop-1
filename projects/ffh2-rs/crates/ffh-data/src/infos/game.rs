//! Info classes: CommerceInfo, GameSpeedInfo, WorldInfo, ClimateInfo, SeaLevelInfo, EraInfo, HandicapInfo, GameOptionInfo, GoodyInfo, HurryInfo, UpkeepInfo, CultureLevelInfo, VictoryInfo, ProcessInfo, EmphasizeInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp.

#![allow(unused_imports)]
use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

/// C++ `CvCommerceInfo` (Gameinfo/CIV4CommerceInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct CommerceInfo {
    pub base: InfoBase,
}

pub fn read_commerce(rd: &Rd) -> CommerceInfo {
    CommerceInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvGameSpeedInfo` (Gameinfo/CIV4GameSpeedInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct GameSpeedInfo {
    pub base: InfoBase,
}

pub fn read_game_speed(rd: &Rd) -> GameSpeedInfo {
    GameSpeedInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvWorldInfo` (Gameinfo/CIV4WorldInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct WorldInfo {
    pub base: InfoBase,
}

pub fn read_world(rd: &Rd) -> WorldInfo {
    WorldInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvClimateInfo` (Gameinfo/CIV4ClimateInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct ClimateInfo {
    pub base: InfoBase,
}

pub fn read_climate(rd: &Rd) -> ClimateInfo {
    ClimateInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvSeaLevelInfo` (Gameinfo/CIV4SeaLevelInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct SeaLevelInfo {
    pub base: InfoBase,
}

pub fn read_sea_level(rd: &Rd) -> SeaLevelInfo {
    SeaLevelInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvEraInfo` (Gameinfo/CIV4EraInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct EraInfo {
    pub base: InfoBase,
}

pub fn read_era(rd: &Rd) -> EraInfo {
    EraInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvHandicapInfo` (Gameinfo/CIV4HandicapInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct HandicapInfo {
    pub base: InfoBase,
}

pub fn read_handicap(rd: &Rd) -> HandicapInfo {
    HandicapInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvGameOptionInfo` (Gameinfo/CIV4GameOptionInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct GameOptionInfo {
    pub base: InfoBase,
}

pub fn read_game_option(rd: &Rd) -> GameOptionInfo {
    GameOptionInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvGoodyInfo` (Gameinfo/CIV4GoodyInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct GoodyInfo {
    pub base: InfoBase,
}

pub fn read_goody(rd: &Rd) -> GoodyInfo {
    GoodyInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvHurryInfo` (Gameinfo/CIV4HurryInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct HurryInfo {
    pub base: InfoBase,
}

pub fn read_hurry(rd: &Rd) -> HurryInfo {
    HurryInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvUpkeepInfo` (Gameinfo/CIV4UpkeepInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct UpkeepInfo {
    pub base: InfoBase,
}

pub fn read_upkeep(rd: &Rd) -> UpkeepInfo {
    UpkeepInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvCultureLevelInfo` (Gameinfo/CIV4CultureLevelInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct CultureLevelInfo {
    pub base: InfoBase,
}

pub fn read_culture_level(rd: &Rd) -> CultureLevelInfo {
    CultureLevelInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvVictoryInfo` (Gameinfo/CIV4VictoryInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct VictoryInfo {
    pub base: InfoBase,
}

pub fn read_victory(rd: &Rd) -> VictoryInfo {
    VictoryInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvProcessInfo` (Gameinfo/CIV4ProcessInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct ProcessInfo {
    pub base: InfoBase,
}

pub fn read_process(rd: &Rd) -> ProcessInfo {
    ProcessInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvEmphasizeInfo` (Gameinfo/CIV4EmphasizeInfo.xml)
#[derive(Debug, Clone, Default)]
pub struct EmphasizeInfo {
    pub base: InfoBase,
}

pub fn read_emphasize(rd: &Rd) -> EmphasizeInfo {
    EmphasizeInfo { base: InfoBase::read(rd), ..Default::default() }
}
