//! CvUnit missions: spy actions (sabotage, destroy, steal plans, infiltrate, espionage), found
//! a city, spread religion/corporation, great people (join, construct, discover, hurry, trade,
//! great work, golden age) and worker builds.
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges. Every `todo!()` names the C++ function still to port.

#![allow(unused_variables, unused_imports)]

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::unit::{CombatDetails, DefenderCombatValues, Unit, UnitResetInfo};
use ffh_data::*;

impl Game {
    /// C++ `CvUnit::sabotageCost` (CvUnit.cpp:5857-5860).
    pub fn unit_sabotage_cost(&self, u: UnitId, plot: PlotIdx) -> i32 {
        self.d.defines.base_spy_sabotage_cost
    }

    /// C++ `CvUnit::sabotageProb` (CvUnit.cpp:5864-5908).
    /// C++ default `eProbStyle = PROBABILITY_REAL`.
    pub fn unit_sabotage_prob(&self, u: UnitId, plot: PlotIdx, prob_style: ProbabilityType) -> i32 {
        todo!("CvUnit::sabotageProb")
    }

    /// C++ `CvUnit::canSabotage` (CvUnit.cpp:5911-5942).
    pub fn unit_can_sabotage(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canSabotage")
    }

    /// C++ `CvUnit::sabotage` (CvUnit.cpp:5945-6016).
    pub fn unit_sabotage(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::sabotage")
    }

    /// C++ `CvUnit::destroyCost` (CvUnit.cpp:6019-6047).
    pub fn unit_destroy_cost(&self, u: UnitId, plot: PlotIdx) -> i32 {
        todo!("CvUnit::destroyCost")
    }

    /// C++ `CvUnit::destroyProb` (CvUnit.cpp:6050-6097).
    pub fn unit_destroy_prob(&self, u: UnitId, plot: PlotIdx, prob_style: ProbabilityType) -> i32 {
        todo!("CvUnit::destroyProb")
    }

    /// C++ `CvUnit::canDestroy` (CvUnit.cpp:6100-6135).
    pub fn unit_can_destroy(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canDestroy")
    }

    /// C++ `CvUnit::destroy` (CvUnit.cpp:6138-6195).
    pub fn unit_destroy(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::destroy")
    }

    /// C++ `CvUnit::stealPlansCost` (CvUnit.cpp:6198-6210).
    pub fn unit_steal_plans_cost(&self, u: UnitId, plot: PlotIdx) -> i32 {
        todo!("CvUnit::stealPlansCost")
    }

    /// C++ `CvUnit::stealPlansProb` (CvUnit.cpp:6214-6259).
    pub fn unit_steal_plans_prob(&self, u: UnitId, plot: PlotIdx, prob_style: ProbabilityType) -> i32 {
        todo!("CvUnit::stealPlansProb")
    }

    /// C++ `CvUnit::canStealPlans` (CvUnit.cpp:6262-6292).
    pub fn unit_can_steal_plans(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canStealPlans")
    }

    /// C++ `CvUnit::stealPlans` (CvUnit.cpp:6295-6352).
    pub fn unit_steal_plans(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::stealPlans")
    }

    /// C++ `CvUnit::canFound` (CvUnit.cpp:6355-6379).
    pub fn unit_can_found(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canFound")
    }

    /// C++ `CvUnit::found` (CvUnit.cpp:6382-6441).
    /// For AI players, regroups the stack's UNITAI_COUNTER units first (Better AI bookkeeping:
    /// `isUnitAllowedPermDefense` is CvUnitAI logic -> leave an `// AI:` note or a controller hook).
    pub fn unit_found(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::found")
    }

    /// C++ `CvUnit::canSpread` (CvUnit.cpp:6444-6542).
    /// Ends with the `cannotSpreadReligion` Python callback (USE_USE_CANNOT_SPREAD_RELIGION_CALLBACK).
    pub fn unit_can_spread(
        &self,
        u: UnitId,
        plot: PlotIdx,
        religion: Option<ReligionType>,
        test_visible: bool,
    ) -> bool {
        todo!("CvUnit::canSpread")
    }

    /// C++ `CvUnit::spread` (CvUnit.cpp:6545-6609).
    pub fn unit_spread(&mut self, u: UnitId, religion: Option<ReligionType>) -> bool {
        todo!("CvUnit::spread")
    }

    /// C++ `CvUnit::canSpreadCorporation` (CvUnit.cpp:6612-6698).
    pub fn unit_can_spread_corporation(
        &self,
        u: UnitId,
        plot: PlotIdx,
        corporation: Option<CorporationType>,
        test_visible: bool,
    ) -> bool {
        todo!("CvUnit::canSpreadCorporation")
    }

    /// C++ `CvUnit::spreadCorporationCost` (CvUnit.cpp:6700-6730).
    pub fn unit_spread_corporation_cost(&self, u: UnitId, corporation: CorporationType, city: Option<CityId>) -> i32 {
        todo!("CvUnit::spreadCorporationCost")
    }

    /// C++ `CvUnit::spreadCorporation` (CvUnit.cpp:6732-6775).
    pub fn unit_spread_corporation(&mut self, u: UnitId, corporation: Option<CorporationType>) -> bool {
        todo!("CvUnit::spreadCorporation")
    }

    /// C++ `CvUnit::canJoin` (CvUnit.cpp:6778-6822).
    pub fn unit_can_join(&self, u: UnitId, plot: PlotIdx, specialist: Option<SpecialistType>) -> bool {
        todo!("CvUnit::canJoin")
    }

    /// C++ `CvUnit::join` (CvUnit.cpp:6825-6849).
    pub fn unit_join(&mut self, u: UnitId, specialist: Option<SpecialistType>) -> bool {
        todo!("CvUnit::join")
    }

    /// C++ `CvUnit::canConstruct` (CvUnit.cpp:6852-6904).
    pub fn unit_can_construct(
        &self,
        u: UnitId,
        plot: PlotIdx,
        building: Option<BuildingType>,
        test_visible: bool,
    ) -> bool {
        todo!("CvUnit::canConstruct")
    }

    /// C++ `CvUnit::construct` (CvUnit.cpp:6907-6933).
    pub fn unit_construct(&mut self, u: UnitId, building: Option<BuildingType>) -> bool {
        todo!("CvUnit::construct")
    }

    /// C++ `CvUnit::getDiscoveryTech` (CvUnit.cpp:6936-6939).
    /// C++ `::getDiscoveryTech(getUnitType(), getOwnerINLINE())` (CvGameCoreUtils).
    pub fn unit_get_discovery_tech(&self, u: UnitId) -> Option<TechType> {
        todo!("CvUnit::getDiscoveryTech")
    }

    /// C++ `CvUnit::getDiscoverResearch` (CvUnit.cpp:6942-6964).
    pub fn unit_get_discover_research(&self, u: UnitId, tech: Option<TechType>) -> i32 {
        todo!("CvUnit::getDiscoverResearch")
    }

    /// C++ `CvUnit::canDiscover` (CvUnit.cpp:6967-6989).
    pub fn unit_can_discover(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canDiscover")
    }

    /// C++ `CvUnit::discover` (CvUnit.cpp:6992-7014).
    pub fn unit_discover(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::discover")
    }

    /// C++ `CvUnit::getMaxHurryProduction` (CvUnit.cpp:7017-7034).
    pub fn unit_get_max_hurry_production(&self, u: UnitId, city: CityId) -> i32 {
        todo!("CvUnit::getMaxHurryProduction")
    }

    /// C++ `CvUnit::getHurryProduction` (CvUnit.cpp:7037-7054).
    pub fn unit_get_hurry_production(&self, u: UnitId, plot: PlotIdx) -> i32 {
        todo!("CvUnit::getHurryProduction")
    }

    /// C++ `CvUnit::canHurry` (CvUnit.cpp:7057-7092).
    pub fn unit_can_hurry(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canHurry")
    }

    /// C++ `CvUnit::hurry` (CvUnit.cpp:7095-7119).
    pub fn unit_hurry(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::hurry")
    }

    /// C++ `CvUnit::getTradeGold` (CvUnit.cpp:7122-7149).
    pub fn unit_get_trade_gold(&self, u: UnitId, plot: PlotIdx) -> i32 {
        todo!("CvUnit::getTradeGold")
    }

    /// C++ `CvUnit::canTrade` (CvUnit.cpp:7152-7185).
    pub fn unit_can_trade(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canTrade")
    }

    /// C++ `CvUnit::trade` (CvUnit.cpp:7188-7205).
    pub fn unit_trade(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::trade")
    }

    /// C++ `CvUnit::getGreatWorkCulture` (CvUnit.cpp:7208-7225).
    pub fn unit_get_great_work_culture(&self, u: UnitId, plot: PlotIdx) -> i32 {
        todo!("CvUnit::getGreatWorkCulture")
    }

    /// C++ `CvUnit::canGreatWork` (CvUnit.cpp:7228-7253).
    pub fn unit_can_great_work(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canGreatWork")
    }

    /// C++ `CvUnit::greatWork` (CvUnit.cpp:7256-7292).
    pub fn unit_great_work(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::greatWork")
    }

    /// C++ `CvUnit::getEspionagePoints` (CvUnit.cpp:7295-7305).
    /// `plot` is unused by the C++ (callers pass NULL).
    pub fn unit_get_espionage_points(&self, u: UnitId, plot: Option<PlotIdx>) -> i32 {
        todo!("CvUnit::getEspionagePoints")
    }

    /// C++ `CvUnit::canInfiltrate` (CvUnit.cpp:7307-7339).
    pub fn unit_can_infiltrate(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canInfiltrate")
    }

    /// C++ `CvUnit::infiltrate` (CvUnit.cpp:7342-7361).
    pub fn unit_infiltrate(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::infiltrate")
    }

    /// C++ `CvUnit::canEspionage` (CvUnit.cpp:7364-7423).
    pub fn unit_can_espionage(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canEspionage")
    }

    /// C++ `CvUnit::espionage` (CvUnit.cpp:7425-7488).
    /// `mission` `None` (or a two-phase mission with `data == -1`) opens a popup for humans in C++.
    pub fn unit_espionage(&mut self, u: UnitId, mission: Option<EspionageMissionType>, data: i32) -> bool {
        todo!("CvUnit::espionage")
    }

    /// C++ `CvUnit::testSpyIntercepted` (CvUnit.cpp:7490-7556).
    /// Returns true if the spy was caught (and killed).
    pub fn unit_test_spy_intercepted(&mut self, u: UnitId, target_player: PlayerId, modifier: i32) -> bool {
        todo!("CvUnit::testSpyIntercepted")
    }

    /// C++ `CvUnit::getSpyInterceptPercent` (CvUnit.cpp:7558-7585).
    pub fn unit_get_spy_intercept_percent(&self, u: UnitId, target_team: TeamId) -> i32 {
        todo!("CvUnit::getSpyInterceptPercent")
    }

    /// C++ `CvUnit::isIntruding` (CvUnit.cpp:7587-7602).
    pub fn unit_is_intruding(&self, u: UnitId) -> bool {
        todo!("CvUnit::isIntruding")
    }

    /// C++ `CvUnit::canGoldenAge` (CvUnit.cpp:7604-7620).
    pub fn unit_can_golden_age(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canGoldenAge")
    }

    /// C++ `CvUnit::goldenAge` (CvUnit.cpp:7623-7643).
    pub fn unit_golden_age(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::goldenAge")
    }

    /// C++ `CvUnit::canBuild` (CvUnit.cpp:7646-7679).
    pub fn unit_can_build(&self, u: UnitId, plot: PlotIdx, build: BuildType, test_visible: bool) -> bool {
        todo!("CvUnit::canBuild")
    }

    /// C++ `CvUnit::build` (CvUnit.cpp:7682-7714).
    /// Returns true if the build finished.
    pub fn unit_build(&mut self, u: UnitId, build: BuildType) -> bool {
        todo!("CvUnit::build")
    }

    /// C++ `CvUnit::canBuildRoute` (CvUnit.cpp:8679-8698).
    pub fn unit_can_build_route(&self, u: UnitId) -> bool {
        todo!("CvUnit::canBuildRoute")
    }

    /// C++ `CvUnit::getBuildType` (CvUnit.cpp:8700-8765).
    /// The build of the group's head mission (MISSION_BUILD, or MISSION_ROUTE_TO's best route build).
    pub fn unit_get_build_type(&self, u: UnitId) -> Option<BuildType> {
        todo!("CvUnit::getBuildType")
    }

    /// C++ `CvUnit::workRate` (CvUnit.cpp:8768-8796).
    pub fn unit_work_rate(&self, u: UnitId, max: bool) -> i32 {
        todo!("CvUnit::workRate")
    }
}
