//! Port of `CvCity` (CvCity.h / CvCity.cpp) plus the city AI bookkeeping that rules code
//! reads and writes through `AI_*` calls (CvCityAI.h).
//!
//! `City` holds the data members; the methods are `impl Game` blocks named `city_*`, split by
//! topic (each file holds about 1900-2450 lines of C++; see `docs/porting/city.md` for the
//! function -> file map with C++ line ranges):
//!
//! - [`production`]: build checks (`canTrain`/`canConstruct`/...), production queries and
//!   modifiers, hurry and conscription.
//! - [`turn`]: `init`/`kill`/`doTurn`/`doTask`, the build queue (`pushOrder`/`popOrder`,
//!   `doProduction`, ...), events, liberation.
//! - [`citizens`]: population and growth, happiness, health, specialists, worked plots,
//!   great people, emphasize / growth control (CvCityAI).
//! - [`buildings`]: buildings, bonuses, corporations, power, defense, maintenance.
//! - [`yields_culture`]: yields, commerce, trade routes, culture, religion, espionage
//!   visibility, rank caches, owner/location helpers (`city_team`, `city_plot`, ...).
//!
//! Method naming (C++ -> Rust): `getFoo` -> `city_foo`, `isFoo` -> `city_is_foo`,
//! `setFoo`/`changeFoo`/`canFoo`/`doFoo` keep their verb; overloads get a suffix naming the
//! distinguishing argument (`getProductionNeeded(UnitTypes)` -> `city_production_needed_unit`),
//! all-types variants of per-type updates are plural (`updateCommerce()` ->
//! `city_update_commerces`). Trivial accessors are field accesses (see "Trivial accessors" in
//! `docs/porting/city.md`).

#![allow(unused_imports)]
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

pub mod buildings;
pub mod citizens;
pub mod production;
pub mod turn;
pub mod yields_culture;

/// C++ `CvCity` (+ the `CvCityAI` members that rules code reaches through `AI_*` calls).
///
/// Field names follow the C++ members (`m_iFooBar` -> `foo_bar`). Pure graphics/UI members
/// (`m_bWallOverride`, `m_bInfoDirty`, `m_bLayoutDirty`, `m_kWallOverridePoints`,
/// `m_iCitySizeBoost`) are omitted.
///
/// C++ `mutable` rank caches (written from `const` methods such as `findPopulationRank`) are
/// [`CacheI32`] / [`CacheBool`] cells so those methods keep `&self`; they are real state (a
/// stale cache is observable).
#[derive(Clone, Debug)]
pub struct City {
    /// C++ `m_iID`; the key of this city in `Game::cities`.
    pub id: CityId,
    pub x: i32,
    pub y: i32,
    /// C++ `m_iRallyX`/`m_iRallyY` (`INVALID_PLOT_COORD` = no rally plot).
    pub rally_x: i32,
    pub rally_y: i32,
    pub game_turn_founded: i32,
    pub game_turn_acquired: i32,
    pub population: i32,
    pub highest_population: i32,
    pub working_population: i32,
    pub specialist_population: i32,
    pub num_great_people: i32,
    pub base_great_people_rate: i32,
    pub great_people_rate_modifier: i32,
    pub great_people_progress: i32,
    pub num_world_wonders: i32,
    pub num_team_wonders: i32,
    pub num_national_wonders: i32,
    pub num_buildings: i32,
    pub government_center_count: i32,
    /// C++ `m_iMaintenance`, stored times 100 (`getMaintenanceTimes100`).
    pub maintenance: i32,
    pub maintenance_modifier: i32,
    pub war_weariness_modifier: i32,
    pub hurry_anger_modifier: i32,
    pub heal_rate: i32,
    pub espionage_health_counter: i32,
    pub espionage_happiness_counter: i32,
    pub fresh_water_good_health: i32,
    pub fresh_water_bad_health: i32,
    pub feature_good_health: i32,
    pub feature_bad_health: i32,
    pub building_good_health: i32,
    pub building_bad_health: i32,
    pub power_good_health: i32,
    pub power_bad_health: i32,
    pub bonus_good_health: i32,
    pub bonus_bad_health: i32,
    pub hurry_anger_timer: i32,
    pub conscript_anger_timer: i32,
    pub defy_resolution_anger_timer: i32,
    pub happiness_timer: i32,
    pub military_happiness_units: i32,
    pub building_good_happiness: i32,
    pub building_bad_happiness: i32,
    pub extra_building_good_happiness: i32,
    pub extra_building_bad_happiness: i32,
    pub extra_building_good_health: i32,
    pub extra_building_bad_health: i32,
    pub feature_good_happiness: i32,
    pub feature_bad_happiness: i32,
    pub bonus_good_happiness: i32,
    pub bonus_bad_happiness: i32,
    pub religion_good_happiness: i32,
    pub religion_bad_happiness: i32,
    pub extra_happiness: i32,
    pub extra_health: i32,
    pub no_unhappiness_count: i32,
    pub no_unhealthy_population_count: i32,
    pub building_only_healthy_count: i32,
    pub food: i32,
    pub food_kept: i32,
    pub max_food_kept_percent: i32,
    pub overflow_production: i32,
    pub feature_production: i32,
    pub military_production_modifier: i32,
    pub space_production_modifier: i32,
    pub extra_trade_routes: i32,
    pub trade_route_modifier: i32,
    pub foreign_trade_route_modifier: i32,
    pub building_defense: i32,
    pub building_bombard_defense: i32,
    pub free_experience: i32,
    pub curr_airlift: i32,
    pub max_airlift: i32,
    pub air_modifier: i32,
    /// C++ `m_iAirUnitCapacity` (own team's capacity; see `Game::city_air_unit_capacity`).
    pub air_unit_capacity: i32,
    pub nuke_modifier: i32,
    pub free_specialist: i32,
    pub power_count: i32,
    pub dirty_power_count: i32,
    pub defense_damage: i32,
    pub last_defense_damage: i32,
    pub occupation_timer: i32,
    pub culture_update_timer: i32,
    pub specialist_free_experience: i32,
    pub espionage_defense_modifier: i32,

    pub never_lost: bool,
    pub bombarded: bool,
    pub drafted: bool,
    pub airlift_targeted: bool,
    pub we_love_the_king_day: bool,
    pub citizens_automated: bool,
    pub production_automated: bool,
    pub plundered: bool,

    /// C++ `m_eOwner` (`getOwnerINLINE`).
    pub owner: PlayerId,
    pub previous_owner: Option<PlayerId>,
    pub original_owner: PlayerId,
    pub culture_level: Option<CultureLevelType>,

    // ---- FFH2 (Kael) ----
    pub settlement: bool,
    /// C++ `m_iCiv` (`getTrueCivilizationType`/`setCivilizationType`): the civilization the
    /// city was founded by; `getCivilizationType` uses it for assimilating owners.
    pub civ: Option<CivilizationType>,
    /// C++ `m_iCrime`: the city's own crime; `getCrime` adds the game's global crime.
    pub crime: i32,
    pub mutate_chance: i32,
    pub plot_radius: i32,
    pub resist_magic: i32,
    /// C++ `m_iUnhappyProduction` (a count; `isUnhappyProduction` = nonzero).
    pub unhappy_production: i32,

    // ---- per yield / commerce / domain ----
    pub sea_plot_yield: [i32; YieldType::NUM],
    pub river_plot_yield: [i32; YieldType::NUM],
    pub base_yield_rate: [i32; YieldType::NUM],
    pub yield_rate_modifier: [i32; YieldType::NUM],
    pub power_yield_rate_modifier: [i32; YieldType::NUM],
    pub bonus_yield_rate_modifier: [i32; YieldType::NUM],
    pub trade_yield: [i32; YieldType::NUM],
    pub corporation_yield: [i32; YieldType::NUM],
    pub extra_specialist_yield: [i32; YieldType::NUM],
    /// C++ `m_aiCommerceRate`, stored times 100 (read through `city_commerce_rate_times100`,
    /// which applies the no-espionage option).
    pub commerce_rate: [i32; CommerceType::NUM],
    pub production_to_commerce_modifier: [i32; CommerceType::NUM],
    pub building_commerce: [i32; CommerceType::NUM],
    pub specialist_commerce: [i32; CommerceType::NUM],
    pub religion_commerce: [i32; CommerceType::NUM],
    pub corporation_commerce: [i32; CommerceType::NUM],
    pub commerce_rate_modifier: [i32; CommerceType::NUM],
    pub commerce_happiness_per: [i32; CommerceType::NUM],
    pub domain_free_experience: [i32; DomainType::NUM],
    pub domain_production_modifier: [i32; DomainType::NUM],

    // ---- per player / team ----
    /// C++ `m_aiCulture`, stored times 100 (`getCultureTimes100`).
    pub culture: IdxVec<PlayerId, i32>,
    pub num_revolts: IdxVec<PlayerId, i32>,
    pub ever_owned: IdxVec<PlayerId, bool>,
    pub trade_route: IdxVec<PlayerId, bool>,
    pub revealed: IdxVec<TeamId, bool>,
    pub espionage_visibility: IdxVec<TeamId, bool>,

    /// C++ `m_szName` (`getNameKey`).
    pub name: String,
    pub script_data: String,

    // ---- per info type ----
    pub no_bonus: IdxVec<BonusType, i32>,
    pub free_bonus: IdxVec<BonusType, i32>,
    pub num_bonuses: IdxVec<BonusType, i32>,
    pub num_corp_produced_bonuses: IdxVec<BonusType, i32>,
    pub project_production: IdxVec<ProjectType, i32>,
    pub building_production: IdxVec<BuildingType, i32>,
    pub building_production_time: IdxVec<BuildingType, i32>,
    pub building_original_owner: IdxVec<BuildingType, Option<PlayerId>>,
    /// C++ `m_paiBuildingOriginalTime` (`MIN_INT` = none).
    pub building_original_time: IdxVec<BuildingType, i32>,
    pub unit_production: IdxVec<UnitType, i32>,
    pub unit_production_time: IdxVec<UnitType, i32>,
    pub great_people_unit_rate: IdxVec<UnitType, i32>,
    pub great_people_unit_progress: IdxVec<UnitType, i32>,
    pub specialist_count: IdxVec<SpecialistType, i32>,
    pub max_specialist_count: IdxVec<SpecialistType, i32>,
    pub force_specialist_count: IdxVec<SpecialistType, i32>,
    pub free_specialist_count: IdxVec<SpecialistType, i32>,
    pub improvement_free_specialists: IdxVec<ImprovementType, i32>,
    pub religion_influence: IdxVec<ReligionType, i32>,
    pub state_religion_happiness: IdxVec<ReligionType, i32>,
    pub unit_combat_free_experience: IdxVec<UnitCombatType, i32>,
    pub free_promotion_count: IdxVec<PromotionType, i32>,
    pub num_real_building: IdxVec<BuildingType, i32>,
    pub num_free_building: IdxVec<BuildingType, i32>,
    /// C++ `m_pabWorkingPlot`, indexed by city plot index (`city_plot_index`).
    pub working_plot: [bool; NUM_CITY_PLOTS],
    pub has_religion: IdxVec<ReligionType, bool>,
    pub has_corporation: IdxVec<CorporationType, bool>,
    /// C++ `m_paTradeCities` (`MAX_TRADE_ROUTES` entries). An id may name a city that no
    /// longer exists; read through `Game::city_trade_city`, which filters those out like C++
    /// `getCity(IDInfo)`.
    pub trade_cities: Vec<Option<CityId>>,

    /// C++ `m_orderQueue`: the build queue, head first.
    pub order_queue: Vec<OrderData>,

    /// C++ `m_aEventsOccured` (insertion order kept).
    pub events_occured: Vec<EventType>,
    pub building_yield_change: Vec<BuildingYieldChange>,
    pub building_commerce_change: Vec<BuildingCommerceChange>,
    pub building_happy_change: BuildingChangeArray,
    pub building_health_change: BuildingChangeArray,

    // ---- C++ rank caches (mutable in C++) ----
    pub population_rank: CacheI32,
    pub population_rank_valid: CacheBool,
    pub base_yield_rank: [CacheI32; YieldType::NUM],
    pub base_yield_rank_valid: [CacheBool; YieldType::NUM],
    pub yield_rank: [CacheI32; YieldType::NUM],
    pub yield_rank_valid: [CacheBool; YieldType::NUM],
    pub commerce_rank: [CacheI32; CommerceType::NUM],
    pub commerce_rank_valid: [CacheBool; CommerceType::NUM],

    // ---- CvCityAI bookkeeping read/written by rules code ----
    /// C++ `CvCityAI::m_bAssignWorkDirty` (`AI_setAssignWorkDirty`): citizens must be
    /// reassigned (by the governor) before they are next used.
    pub ai_assign_work_dirty: bool,
    /// C++ `CvCityAI::m_bChooseProductionDirty` (`AI_setChooseProductionDirty`).
    pub ai_choose_production_dirty: bool,
    /// C++ `CvCityAI::m_pbEmphasize` (`AI_isEmphasize`): player-settable emphasize flags.
    /// Change them through `Game::city_ai_set_emphasize`, which keeps the counts below.
    pub ai_emphasize: IdxVec<EmphasizeType, bool>,
    pub ai_emphasize_avoid_growth_count: i32,
    pub ai_emphasize_great_people_count: i32,
    pub ai_emphasize_yield_count: [i32; YieldType::NUM],
    pub ai_emphasize_commerce_count: [i32; CommerceType::NUM],
    /// FFH2 GrowthControl (`m_iEmphasizeAvoidAngryCitizensCount`).
    pub ai_emphasize_avoid_angry_citizens_count: i32,
    /// FFH2 GrowthControl (`m_iEmphasizeAvoidUnhealthyCitizensCount`).
    pub ai_emphasize_avoid_unhealthy_citizens_count: i32,
}

impl City {
    /// C++ `CvCity::reset(iID, eOwner, iX, iY, false)` (CvCity.cpp:389-713, which runs
    /// `uninit` first) together with the bookkeeping part of `CvCityAI::AI_reset`
    /// (CvCityAI.cpp:91-190): a city with every member at its C++ default.
    ///
    /// Per-type arrays are sized from `d`; per-player/per-team arrays from `num_players` /
    /// `num_teams` (`Game::num_players()` / `Game::num_teams()`).
    ///
    /// `crime` is C++ `m_iCrime = getSorenRandNum(20, "Crime")`: reset() draws from the game
    /// RNG, so the caller draws it (`Game::city_init` does, twice, as C++ does).
    #[allow(clippy::too_many_arguments)]
    pub fn new(d: &GameData, num_players: usize, num_teams: usize, id: CityId, owner: PlayerId, x: i32, y: i32, crime: i32) -> City {
        let nb = d.buildings.len();
        let nu = d.units.len();
        let ns = d.specialists.len();
        let nbonus = d.bonuses.len();
        let nr = d.religions.len();
        City {
            id,
            x,
            y,
            rally_x: INVALID_PLOT_COORD,
            rally_y: INVALID_PLOT_COORD,
            game_turn_founded: 0,
            game_turn_acquired: 0,
            population: 0,
            highest_population: 0,
            working_population: 0,
            specialist_population: 0,
            num_great_people: 0,
            base_great_people_rate: 0,
            great_people_rate_modifier: 0,
            great_people_progress: 0,
            num_world_wonders: 0,
            num_team_wonders: 0,
            num_national_wonders: 0,
            num_buildings: 0,
            government_center_count: 0,
            maintenance: 0,
            maintenance_modifier: 0,
            war_weariness_modifier: 0,
            hurry_anger_modifier: 0,
            heal_rate: 0,
            espionage_health_counter: 0,
            espionage_happiness_counter: 0,
            fresh_water_good_health: 0,
            fresh_water_bad_health: 0,
            feature_good_health: 0,
            feature_bad_health: 0,
            building_good_health: 0,
            building_bad_health: 0,
            power_good_health: 0,
            power_bad_health: 0,
            bonus_good_health: 0,
            bonus_bad_health: 0,
            hurry_anger_timer: 0,
            conscript_anger_timer: 0,
            defy_resolution_anger_timer: 0,
            happiness_timer: 0,
            military_happiness_units: 0,
            building_good_happiness: 0,
            building_bad_happiness: 0,
            extra_building_good_happiness: 0,
            extra_building_bad_happiness: 0,
            extra_building_good_health: 0,
            extra_building_bad_health: 0,
            feature_good_happiness: 0,
            feature_bad_happiness: 0,
            bonus_good_happiness: 0,
            bonus_bad_happiness: 0,
            religion_good_happiness: 0,
            religion_bad_happiness: 0,
            extra_happiness: 0,
            extra_health: 0,
            no_unhappiness_count: 0,
            no_unhealthy_population_count: 0,
            building_only_healthy_count: 0,
            food: 0,
            food_kept: 0,
            max_food_kept_percent: 0,
            overflow_production: 0,
            feature_production: 0,
            military_production_modifier: 0,
            space_production_modifier: 0,
            extra_trade_routes: 0,
            trade_route_modifier: 0,
            foreign_trade_route_modifier: 0,
            building_defense: 0,
            building_bombard_defense: 0,
            free_experience: 0,
            curr_airlift: 0,
            max_airlift: 0,
            air_modifier: 0,
            air_unit_capacity: 0,
            nuke_modifier: 0,
            free_specialist: 0,
            power_count: 0,
            dirty_power_count: 0,
            defense_damage: 0,
            last_defense_damage: 0,
            occupation_timer: 0,
            culture_update_timer: 0,
            specialist_free_experience: 0,
            espionage_defense_modifier: 0,

            never_lost: true,
            bombarded: false,
            drafted: false,
            airlift_targeted: false,
            we_love_the_king_day: false,
            citizens_automated: true,
            production_automated: false,
            plundered: false,

            owner,
            previous_owner: None,
            original_owner: owner,
            culture_level: None,

            settlement: false,
            civ: None,
            crime,
            mutate_chance: 0,
            plot_radius: 2,
            resist_magic: 0,
            unhappy_production: 0,

            sea_plot_yield: [0; YieldType::NUM],
            river_plot_yield: [0; YieldType::NUM],
            base_yield_rate: [0; YieldType::NUM],
            yield_rate_modifier: [0; YieldType::NUM],
            power_yield_rate_modifier: [0; YieldType::NUM],
            bonus_yield_rate_modifier: [0; YieldType::NUM],
            trade_yield: [0; YieldType::NUM],
            corporation_yield: [0; YieldType::NUM],
            extra_specialist_yield: [0; YieldType::NUM],
            commerce_rate: [0; CommerceType::NUM],
            production_to_commerce_modifier: [0; CommerceType::NUM],
            building_commerce: [0; CommerceType::NUM],
            specialist_commerce: [0; CommerceType::NUM],
            religion_commerce: [0; CommerceType::NUM],
            corporation_commerce: [0; CommerceType::NUM],
            commerce_rate_modifier: [0; CommerceType::NUM],
            commerce_happiness_per: [0; CommerceType::NUM],
            domain_free_experience: [0; DomainType::NUM],
            domain_production_modifier: [0; DomainType::NUM],

            culture: IdxVec::from_elem(0, num_players),
            num_revolts: IdxVec::from_elem(0, num_players),
            ever_owned: IdxVec::from_elem(false, num_players),
            trade_route: IdxVec::from_elem(false, num_players),
            revealed: IdxVec::from_elem(false, num_teams),
            espionage_visibility: IdxVec::from_elem(false, num_teams),

            name: String::new(),
            script_data: String::new(),

            no_bonus: IdxVec::from_elem(0, nbonus),
            free_bonus: IdxVec::from_elem(0, nbonus),
            num_bonuses: IdxVec::from_elem(0, nbonus),
            num_corp_produced_bonuses: IdxVec::from_elem(0, nbonus),
            project_production: IdxVec::from_elem(0, d.projects.len()),
            building_production: IdxVec::from_elem(0, nb),
            building_production_time: IdxVec::from_elem(0, nb),
            building_original_owner: IdxVec::from_elem(None, nb),
            building_original_time: IdxVec::from_elem(MIN_INT, nb),
            unit_production: IdxVec::from_elem(0, nu),
            unit_production_time: IdxVec::from_elem(0, nu),
            great_people_unit_rate: IdxVec::from_elem(0, nu),
            great_people_unit_progress: IdxVec::from_elem(0, nu),
            specialist_count: IdxVec::from_elem(0, ns),
            max_specialist_count: IdxVec::from_elem(0, ns),
            force_specialist_count: IdxVec::from_elem(0, ns),
            free_specialist_count: IdxVec::from_elem(0, ns),
            improvement_free_specialists: IdxVec::from_elem(0, d.improvements.len()),
            religion_influence: IdxVec::from_elem(0, nr),
            state_religion_happiness: IdxVec::from_elem(0, nr),
            unit_combat_free_experience: IdxVec::from_elem(0, d.unit_combats.len()),
            free_promotion_count: IdxVec::from_elem(0, d.promotions.len()),
            num_real_building: IdxVec::from_elem(0, nb),
            num_free_building: IdxVec::from_elem(0, nb),
            working_plot: [false; NUM_CITY_PLOTS],
            has_religion: IdxVec::from_elem(false, nr),
            has_corporation: IdxVec::from_elem(false, d.corporations.len()),
            trade_cities: vec![None; d.defines.max_trade_routes.max(0) as usize],

            order_queue: Vec::new(),

            events_occured: Vec::new(),
            building_yield_change: Vec::new(),
            building_commerce_change: Vec::new(),
            building_happy_change: Vec::new(),
            building_health_change: Vec::new(),

            population_rank: CacheI32::new(-1),
            population_rank_valid: CacheBool::new(false),
            base_yield_rank: std::array::from_fn(|_| CacheI32::new(-1)),
            base_yield_rank_valid: std::array::from_fn(|_| CacheBool::new(false)),
            yield_rank: std::array::from_fn(|_| CacheI32::new(-1)),
            yield_rank_valid: std::array::from_fn(|_| CacheBool::new(false)),
            commerce_rank: std::array::from_fn(|_| CacheI32::new(-1)),
            commerce_rank_valid: std::array::from_fn(|_| CacheBool::new(false)),

            ai_assign_work_dirty: false,
            ai_choose_production_dirty: false,
            ai_emphasize: IdxVec::from_elem(false, d.emphasizes.len()),
            ai_emphasize_avoid_growth_count: 0,
            ai_emphasize_great_people_count: 0,
            ai_emphasize_yield_count: [0; YieldType::NUM],
            ai_emphasize_commerce_count: [0; CommerceType::NUM],
            ai_emphasize_avoid_angry_citizens_count: 0,
            ai_emphasize_avoid_unhealthy_citizens_count: 0,
        }
    }

    /// C++ `headOrderQueueNode()->m_data`: the order being built, if any.
    #[inline]
    pub fn head_order(&self) -> Option<&OrderData> {
        self.order_queue.first()
    }
}

/// C++ `range(iNum, iLow, iHigh)` (CvGameCoreUtils.h). Unlike `i32::clamp` it does not panic
/// when `low > high` (the low bound wins, as in C++).
#[inline]
pub(crate) fn range(n: i32, low: i32, high: i32) -> i32 {
    if n < low {
        low
    } else if n > high {
        high
    } else {
        n
    }
}

/// The typed `iData1` of a queue entry (unit/building/project/process type).
#[inline]
pub(crate) fn order_type_data<T: Idx>(o: &OrderData) -> T {
    debug_assert!(o.data1 >= 0, "order without iData1");
    T::from_usize(o.data1 as usize)
}

/// C++ `GC.getUnitInfo(eUnit).getUnitClassType()` (every unit has a class; C++ would index
/// arrays with -1 otherwise).
#[inline]
fn unit_class_of(d: &GameData, unit: UnitType) -> UnitClassType {
    d.units[unit].unit_class_type.expect("CvUnitInfo without a unit class")
}

/// C++ `GC.getBuildingInfo(eBuilding).getBuildingClassType()`.
#[inline]
fn building_class_of(d: &GameData, building: BuildingType) -> BuildingClassType {
    d.buildings[building].building_class_type.expect("CvBuildingInfo without a building class")
}

/// C++ `(DomainTypes)GC.getUnitInfo(eUnit).getDomainType()`.
#[inline]
fn unit_domain_of(d: &GameData, unit: UnitType) -> DomainType {
    d.units[unit].domain_type.expect("CvUnitInfo without a domain")
}

/// C++ `CvCityAI::AI_chooseProduction()` as called by rules code (`doProduction`, `popOrder`,
/// `setProductionAutomated`): the owner's controller decides
/// (`Controller::choose_production`, which pushes orders through `city_push_order`).
///
/// C++ clears `m_bChooseProductionDirty` inside `AI_chooseProduction` when it re-decides
/// ("only clear the dirty bit if we actually do a check"). The controller replaces that
/// function, so the flag is cleared here before asking it; a controller that keeps the
/// current production and wants to be asked again next turn sets the flag again.
fn ai_choose_production(g: &mut Game, c: CityId) {
    g.cities[c].ai_choose_production_dirty = false;
    let owner = g.cities[c].owner;
    g.controller(owner).choose_production(g, c);
}

/// C++ `CvCityAI::AI_reset()` for the AI bookkeeping fields `City` keeps (the other
/// `CvCityAI` members are AI-only caches and are not ported).
fn ai_reset(city: &mut City) {
    city.ai_emphasize_avoid_growth_count = 0;
    city.ai_emphasize_great_people_count = 0;
    city.ai_assign_work_dirty = false;
    city.ai_choose_production_dirty = false;
    city.ai_emphasize_yield_count = [0; YieldType::NUM];
    city.ai_emphasize_commerce_count = [0; CommerceType::NUM];
    city.ai_emphasize_avoid_angry_citizens_count = 0;
    city.ai_emphasize_avoid_unhealthy_citizens_count = 0;
    city.ai_emphasize.fill(false);
}

/// A C++ `mutable int` cache slot, written from `&self` methods (`Cell`-like). Backed by a
/// relaxed atomic rather than `Cell` so `City` (and `Game`) stay `Sync`: the simulation is
/// single-threaded, and readers sharing a `&Game` can only race to store the same value.
#[derive(Default)]
pub struct CacheI32(AtomicI32);

impl CacheI32 {
    pub const fn new(v: i32) -> Self {
        CacheI32(AtomicI32::new(v))
    }
    #[inline]
    pub fn get(&self) -> i32 {
        self.0.load(Ordering::Relaxed)
    }
    #[inline]
    pub fn set(&self, v: i32) {
        self.0.store(v, Ordering::Relaxed)
    }
}

impl Clone for CacheI32 {
    fn clone(&self) -> Self {
        CacheI32::new(self.get())
    }
}

impl std::fmt::Debug for CacheI32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

/// A C++ `mutable bool` cache slot; see [`CacheI32`].
#[derive(Default)]
pub struct CacheBool(AtomicBool);

impl CacheBool {
    pub const fn new(v: bool) -> Self {
        CacheBool(AtomicBool::new(v))
    }
    #[inline]
    pub fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    #[inline]
    pub fn set(&self, v: bool) {
        self.0.store(v, Ordering::Relaxed)
    }
}

impl Clone for CacheBool {
    fn clone(&self) -> Self {
        CacheBool::new(self.get())
    }
}

impl std::fmt::Debug for CacheBool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_city_has_reset_defaults() {
        let d = ffh_data::global();
        let c = City::new(d, 3, 3, CityId(7), PlayerId(1), 4, 5, 13);
        assert_eq!(c.id, CityId(7));
        assert_eq!((c.x, c.y), (4, 5));
        assert_eq!((c.rally_x, c.rally_y), (INVALID_PLOT_COORD, INVALID_PLOT_COORD));
        assert_eq!(c.owner, PlayerId(1));
        assert_eq!(c.original_owner, PlayerId(1));
        assert_eq!(c.previous_owner, None);
        assert!(c.never_lost && c.citizens_automated && !c.production_automated);
        assert_eq!(c.crime, 13);
        assert_eq!(c.plot_radius, 2);
        assert_eq!(c.culture_level, None);
        assert_eq!(c.civ, None);
        assert_eq!(c.culture.len(), 3);
        assert_eq!(c.revealed.len(), 3);
        assert_eq!(c.num_real_building.len(), d.buildings.len());
        assert!(c.building_original_time.iter().all(|&t| t == MIN_INT));
        assert!(c.building_original_owner.iter().all(|o| o.is_none()));
        assert_eq!(c.unit_production.len(), d.units.len());
        assert_eq!(c.specialist_count.len(), d.specialists.len());
        assert_eq!(c.ai_emphasize.len(), d.emphasizes.len());
        assert_eq!(c.trade_cities.len(), d.defines.max_trade_routes as usize);
        assert!(c.order_queue.is_empty() && c.head_order().is_none());
        assert_eq!(c.population_rank.get(), -1);
        assert!(!c.yield_rank_valid[1].get());
    }

    #[test]
    fn rank_caches_are_interior_mutable() {
        let d = ffh_data::global();
        let c = City::new(d, 2, 2, CityId(0), PlayerId(0), 0, 0, 0);
        c.population_rank.set(3);
        c.population_rank_valid.set(true);
        let copy = c.clone();
        assert_eq!(copy.population_rank.get(), 3);
        assert!(copy.population_rank_valid.get());
        c.population_rank_valid.set(false);
        assert!(copy.population_rank_valid.get(), "clones do not share the cache");
        fn send_sync<T: Send + Sync>() {}
        send_sync::<City>();
    }

    #[test]
    fn range_matches_cpp() {
        assert_eq!(range(5, 0, 3), 3);
        assert_eq!(range(-2, 0, 3), 0);
        assert_eq!(range(2, 0, 3), 2);
        // low > high: C++ returns low for n < low, else high.
        assert_eq!(range(1, 2, 0), 2);
        assert_eq!(range(3, 2, 0), 0);
    }
}
