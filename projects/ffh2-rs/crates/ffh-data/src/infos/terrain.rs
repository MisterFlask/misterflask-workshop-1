//! Info classes: YieldInfo, TerrainInfo, FeatureInfo, BonusClassInfo, BonusInfo, ImprovementInfo, RouteInfo.
//! Port of the corresponding `Cv*Info::read` functions in CvInfos.cpp.

#![allow(unused_imports)]
use crate::enums::*;
use crate::idx::IdxVec;
use crate::infos::InfoBase;
use crate::registry::Rd;

/// C++ `CvYieldInfo` (Terrain/CIV4YieldInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct YieldInfo {
    pub base: InfoBase,
}

pub fn read_yield(rd: &Rd) -> YieldInfo {
    YieldInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvTerrainInfo` (Terrain/CIV4TerrainInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct TerrainInfo {
    pub base: InfoBase,
}

pub fn read_terrain(rd: &Rd) -> TerrainInfo {
    TerrainInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvFeatureInfo` (Terrain/CIV4FeatureInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct FeatureInfo {
    pub base: InfoBase,
}

pub fn read_feature(rd: &Rd) -> FeatureInfo {
    FeatureInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvBonusClassInfo` (Terrain/CIV4BonusClassInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct BonusClassInfo {
    pub base: InfoBase,
}

pub fn read_bonus_class(rd: &Rd) -> BonusClassInfo {
    BonusClassInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvBonusInfo` (Terrain/CIV4BonusInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct BonusInfo {
    pub base: InfoBase,
}

pub fn read_bonus(rd: &Rd) -> BonusInfo {
    BonusInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvImprovementInfo` (Terrain/CIV4ImprovementInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct ImprovementInfo {
    pub base: InfoBase,
}

pub fn read_improvement(rd: &Rd) -> ImprovementInfo {
    ImprovementInfo { base: InfoBase::read(rd), ..Default::default() }
}

/// C++ `CvRouteInfo` (Misc/CIV4RouteInfos.xml)
#[derive(Debug, Clone, Default)]
pub struct RouteInfo {
    pub base: InfoBase,
}

pub fn read_route(rd: &Rd) -> RouteInfo {
    RouteInfo { base: InfoBase::read(rd), ..Default::default() }
}
