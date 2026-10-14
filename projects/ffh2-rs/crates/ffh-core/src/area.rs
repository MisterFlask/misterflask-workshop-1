//! Port of `CvArea` (CvArea.h / CvArea.cpp): a connected land or water region.
//!
//! Areas live in `Game::map.areas` (`Slab<AreaId, Area>`); `CvMap::addArea` + `CvArea::init`
//! is `Game::map_add_area`. Methods are `impl Game` methods `area_*` taking the `AreaId`
//! first; trivial getters/setters are fields (see `docs/porting/map.md`).
//!
//! CvArea has no AI subclass: the AI bookkeeping that rules code maintains (area AI types,
//! target cities, unit AI counts, best found values) are plain fields here, as in C++.

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

/// C++ `CvArea`.
#[derive(Clone, Debug)]
pub struct Area {
    /// C++ `m_iID`.
    pub id: AreaId,
    /// C++ `m_iNumTiles`. Change with `area_change_num_tiles` (lake status side effects).
    pub num_tiles: i32,
    /// C++ `m_iNumOwnedTiles`.
    pub num_owned_tiles: i32,
    /// C++ `m_iNumRiverEdges`.
    pub num_river_edges: i32,
    /// C++ `m_iNumUnits` (changed together with `units_per_player`).
    pub num_units: i32,
    /// C++ `m_iNumCities` (changed together with `cities_per_player`).
    pub num_cities: i32,
    /// C++ `m_iTotalPopulation` (changed together with `population_per_player`).
    pub total_population: i32,
    /// C++ `m_iNumStartingPlots`.
    pub num_starting_plots: i32,
    /// C++ `m_bWater`.
    pub water: bool,
    /// C++ `m_aiUnitsPerPlayer`. Change: `area_change_units_per_player`.
    pub units_per_player: IdxVec<PlayerId, i32>,
    /// C++ `m_aiAnimalsPerPlayer`.
    pub animals_per_player: IdxVec<PlayerId, i32>,
    /// C++ `m_aiCitiesPerPlayer`. Change: `area_change_cities_per_player`.
    pub cities_per_player: IdxVec<PlayerId, i32>,
    /// C++ `m_aiPopulationPerPlayer`. Change: `area_change_population_per_player`.
    pub population_per_player: IdxVec<PlayerId, i32>,
    /// C++ `m_aiBuildingGoodHealth`. Change: `area_change_building_good_health`.
    pub building_good_health: IdxVec<PlayerId, i32>,
    /// C++ `m_aiBuildingBadHealth`. Change: `area_change_building_bad_health`.
    pub building_bad_health: IdxVec<PlayerId, i32>,
    /// C++ `m_aiBuildingHappiness`. Change: `area_change_building_happiness`.
    pub building_happiness: IdxVec<PlayerId, i32>,
    /// C++ `m_aiFreeSpecialist`. Change: `area_change_free_specialist`.
    pub free_specialist: IdxVec<PlayerId, i32>,
    /// C++ `m_aiPower`.
    pub power: IdxVec<PlayerId, i32>,
    /// C++ `m_aiBestFoundValue` (AI bookkeeping, raised by `plot_get_found_value`).
    pub best_found_value: IdxVec<PlayerId, i32>,
    /// C++ `m_aiNumRevealedTiles`.
    pub num_revealed_tiles: IdxVec<TeamId, i32>,
    /// C++ `m_aiCleanPowerCount`. Change: `area_change_clean_power_count`.
    pub clean_power_count: IdxVec<TeamId, i32>,
    /// C++ `m_aiBorderObstacleCount`. Change: `area_change_border_obstacle_count`.
    pub border_obstacle_count: IdxVec<TeamId, i32>,
    /// C++ `m_aeAreaAIType` (AI bookkeeping; NO_AREAAI = None).
    pub area_ai_type: IdxVec<TeamId, Option<AreaAIType>>,
    /// C++ `m_aTargetCities` (AI bookkeeping). Read with `area_get_target_city` (drops dead
    /// cities like C++ `getCity(IDInfo)`); `setTargetCity` is a plain assignment.
    pub target_cities: IdxVec<PlayerId, Option<CityId>>,
    /// C++ `m_aaiYieldRateModifier`. Change: `area_change_yield_rate_modifier`.
    pub yield_rate_modifier: IdxVec<PlayerId, [i32; YieldType::NUM]>,
    /// C++ `m_aaiNumTrainAIUnits` (AI bookkeeping: units being trained per UnitAI).
    pub num_train_ai_units: IdxVec<PlayerId, IdxVec<UnitAIType, i32>>,
    /// C++ `m_aaiNumAIUnits` (AI bookkeeping: units per UnitAI).
    pub num_ai_units: IdxVec<PlayerId, IdxVec<UnitAIType, i32>>,
    /// C++ `m_paiNumBonuses`.
    pub num_bonuses: IdxVec<BonusType, i32>,
    /// C++ `m_paiNumImprovements`.
    pub num_improvements: IdxVec<ImprovementType, i32>,
}

impl Area {
    /// C++ `CvArea::init(iID, bWater)` (= `reset(iID, bWater)`), arrays sized from the game
    /// data and the game's player/team counts.
    pub fn new(d: &GameData, id: AreaId, water: bool, num_players: usize, num_teams: usize) -> Area {
        Area::with_counts(id, water, num_players, num_teams, d.bonuses.len(), d.improvements.len())
    }

    /// C++ `CvArea::reset` (as `reset(iID, bWater)`) with explicit array sizes.
    pub fn with_counts(id: AreaId, water: bool, num_players: usize, num_teams: usize, num_bonuses: usize, num_improvements: usize) -> Area {
        Area {
            id,
            num_tiles: 0,
            num_owned_tiles: 0,
            num_river_edges: 0,
            num_units: 0,
            num_cities: 0,
            total_population: 0,
            num_starting_plots: 0,
            water,
            units_per_player: IdxVec::from_elem(0, num_players),
            animals_per_player: IdxVec::from_elem(0, num_players),
            cities_per_player: IdxVec::from_elem(0, num_players),
            population_per_player: IdxVec::from_elem(0, num_players),
            building_good_health: IdxVec::from_elem(0, num_players),
            building_bad_health: IdxVec::from_elem(0, num_players),
            building_happiness: IdxVec::from_elem(0, num_players),
            free_specialist: IdxVec::from_elem(0, num_players),
            power: IdxVec::from_elem(0, num_players),
            best_found_value: IdxVec::from_elem(0, num_players),
            num_revealed_tiles: IdxVec::from_elem(0, num_teams),
            clean_power_count: IdxVec::from_elem(0, num_teams),
            border_obstacle_count: IdxVec::from_elem(0, num_teams),
            area_ai_type: IdxVec::from_elem(None, num_teams),
            target_cities: IdxVec::from_elem(None, num_players),
            yield_rate_modifier: IdxVec::from_elem([0; YieldType::NUM], num_players),
            num_train_ai_units: IdxVec::from_fn(num_players, |_| IdxVec::from_elem(0, UnitAIType::NUM)),
            num_ai_units: IdxVec::from_fn(num_players, |_| IdxVec::from_elem(0, UnitAIType::NUM)),
            num_bonuses: IdxVec::from_elem(0, num_bonuses),
            num_improvements: IdxVec::from_elem(0, num_improvements),
        }
    }
}

impl Default for Area {
    /// C++ `CvArea()` (`reset(0, false, true)`: no per-player/per-team arrays).
    fn default() -> Self {
        Area::with_counts(AreaId(0), false, 0, 0, 0, 0)
    }
}

impl Game {
    // ------------------------------------------------------------------
    // Counting over the area's plots and cities
    // ------------------------------------------------------------------

    /// C++ `CvArea::calculateTotalBestNatureYield`.
    pub fn area_calculate_total_best_nature_yield(&self, a: AreaId) -> i32 {
        let mut count = 0;
        for p in self.map.plot_ids() {
            if self.map.plots[p].area == Some(a) {
                count += self.plot_calculate_total_best_nature_yield(p, None);
            }
        }
        count
    }

    /// C++ `CvArea::countCoastalLand`.
    pub fn area_count_coastal_land(&self, a: AreaId) -> i32 {
        if self.map.areas[a].water {
            return 0;
        }
        let mut count = 0;
        for p in self.map.plot_ids() {
            if self.map.plots[p].area == Some(a) && self.plot_is_coastal_land(p, -1) {
                count += 1;
            }
        }
        count
    }

    /// C++ `CvArea::countNumUniqueBonusTypes`: present bonuses flagged `bOneArea`.
    pub fn area_count_num_unique_bonus_types(&self, a: AreaId) -> i32 {
        let mut count = 0;

        for (b, bonus) in self.d.bonuses.iter_enumerated() {
            if self.map.areas[a].num_bonuses[b] > 0 && bonus.one_area {
                count += 1;
            }
        }

        count
    }

    /// C++ `CvArea::countHasReligion` (C++ default `eOwner = NO_PLAYER`).
    pub fn area_count_has_religion(&self, a: AreaId, religion: ReligionType, owner: Option<PlayerId>) -> i32 {
        let mut count = 0;

        for i in self.player_ids() {
            if self.players[i].alive && (owner.is_none() || owner == Some(i)) {
                for &c in &self.players[i].cities {
                    if self.city_area(c) == a && self.cities[c].has_religion[religion] {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// C++ `CvArea::countHasCorporation` (C++ default `eOwner = NO_PLAYER`).
    pub fn area_count_has_corporation(&self, a: AreaId, corporation: CorporationType, owner: Option<PlayerId>) -> i32 {
        let mut count = 0;

        for i in self.player_ids() {
            if self.players[i].alive && (owner.is_none() || owner == Some(i)) {
                for &c in &self.players[i].cities {
                    if self.city_area(c) == a && self.cities[c].has_corporation[corporation] {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    // ------------------------------------------------------------------
    // Tiles
    // ------------------------------------------------------------------

    /// C++ `CvArea::isLake`.
    #[inline]
    pub fn area_is_lake(&self, a: AreaId) -> bool {
        let area = &self.map.areas[a];
        area.water && area.num_tiles <= self.d.defines.lake_max_area_size
    }

    /// C++ `CvArea::changeNumTiles`: a change of lake status updates irrigation and yields.
    pub fn area_change_num_tiles(&mut self, a: AreaId, change: i32) {
        if change != 0 {
            let old_lake = self.area_is_lake(a);
            self.map.areas[a].num_tiles += change;
            debug_assert!(self.map.areas[a].num_tiles >= 0);
            if old_lake != self.area_is_lake(a) {
                self.map_update_irrigated();
                self.map_update_yield();
            }
        }
    }

    /// C++ `CvArea::getNumUnownedTiles`.
    #[inline]
    pub fn area_get_num_unowned_tiles(&self, a: AreaId) -> i32 {
        let area = &self.map.areas[a];
        area.num_tiles - area.num_owned_tiles
    }

    /// C++ `CvArea::getNumUnrevealedTiles`.
    #[inline]
    pub fn area_get_num_unrevealed_tiles(&self, a: AreaId, team: TeamId) -> i32 {
        let area = &self.map.areas[a];
        area.num_tiles - area.num_revealed_tiles[team]
    }

    // ------------------------------------------------------------------
    // Per-player counters with totals
    // ------------------------------------------------------------------

    /// C++ `CvArea::changeUnitsPerPlayer` (also changes the area's unit total).
    pub fn area_change_units_per_player(&mut self, a: AreaId, player: PlayerId, change: i32) {
        let area = &mut self.map.areas[a];
        area.num_units += change;
        debug_assert!(area.num_units >= 0);
        area.units_per_player[player] += change;
        debug_assert!(area.units_per_player[player] >= 0);
    }

    /// C++ `CvArea::changeCitiesPerPlayer` (also changes the area's city total).
    pub fn area_change_cities_per_player(&mut self, a: AreaId, player: PlayerId, change: i32) {
        let area = &mut self.map.areas[a];
        area.num_cities += change;
        debug_assert!(area.num_cities >= 0);
        area.cities_per_player[player] += change;
        debug_assert!(area.cities_per_player[player] >= 0);
    }

    /// C++ `CvArea::changePopulationPerPlayer` (also changes the area's population total).
    pub fn area_change_population_per_player(&mut self, a: AreaId, player: PlayerId, change: i32) {
        let area = &mut self.map.areas[a];
        area.total_population += change;
        debug_assert!(area.total_population >= 0);
        area.population_per_player[player] += change;
        debug_assert!(area.population_per_player[player] >= 0);
    }

    // ------------------------------------------------------------------
    // Building effects (the owner's citizens must be reassigned on change)
    // ------------------------------------------------------------------

    /// C++ `CvArea::changeBuildingGoodHealth` (+ `CvPlayer::AI_makeAssignWorkDirty`).
    pub fn area_change_building_good_health(&mut self, a: AreaId, player: PlayerId, change: i32) {
        if change != 0 {
            let area = &mut self.map.areas[a];
            area.building_good_health[player] += change;
            debug_assert!(area.building_good_health[player] >= 0);

            self.player_ai_make_assign_work_dirty(player);
        }
    }

    /// C++ `CvArea::changeBuildingBadHealth` (+ `CvPlayer::AI_makeAssignWorkDirty`).
    pub fn area_change_building_bad_health(&mut self, a: AreaId, player: PlayerId, change: i32) {
        if change != 0 {
            // C++: FAssert(getBuildingBadHealth(eIndex) >= 0), although `processArea` adds the
            // (negative or zero) `getAreaHealth()` of non-healthy buildings here.
            self.map.areas[a].building_bad_health[player] += change;

            self.player_ai_make_assign_work_dirty(player);
        }
    }

    /// C++ `CvArea::changeBuildingHappiness` (+ `CvPlayer::AI_makeAssignWorkDirty`).
    pub fn area_change_building_happiness(&mut self, a: AreaId, player: PlayerId, change: i32) {
        if change != 0 {
            self.map.areas[a].building_happiness[player] += change;

            self.player_ai_make_assign_work_dirty(player);
        }
    }

    /// C++ `CvArea::changeFreeSpecialist` (+ `CvPlayer::AI_makeAssignWorkDirty`).
    pub fn area_change_free_specialist(&mut self, a: AreaId, player: PlayerId, change: i32) {
        if change != 0 {
            let area = &mut self.map.areas[a];
            area.free_specialist[player] += change;
            // C++ quirk: the C++ FAssert can fail for a player added to a team mid-game (bTeamShare
            // buildings, see `player_change_trade_routes`).

            self.player_ai_make_assign_work_dirty(player);
        }
    }

    /// C++ `CvArea::changeYieldRateModifier` (invalidates the owner's yield rank cache,
    /// updates commerce, reassigns citizens).
    pub fn area_change_yield_rate_modifier(&mut self, a: AreaId, player: PlayerId, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.map.areas[a].yield_rate_modifier[player][yield_type.idx()] += change;

            self.player_invalidate_yield_rank_cache(player, Some(yield_type));

            if yield_type == YieldType::YIELD_COMMERCE {
                self.player_update_commerce(player);
            }

            self.player_ai_make_assign_work_dirty(player);

            // setDirty(CityInfo_DIRTY_BIT) for the active team: UI only.
        }
    }

    // ------------------------------------------------------------------
    // Per-team counters
    // ------------------------------------------------------------------

    /// C++ `CvArea::isCleanPower`.
    #[inline]
    pub fn area_is_clean_power(&self, a: AreaId, team: TeamId) -> bool {
        self.map.areas[a].clean_power_count[team] > 0
    }

    /// C++ `CvArea::changeCleanPowerCount` (updates the team's commerce and power health).
    pub fn area_change_clean_power_count(&mut self, a: AreaId, team: TeamId, change: i32) {
        if change != 0 {
            let old_clean_power = self.area_is_clean_power(a, team);

            self.map.areas[a].clean_power_count[team] += change;

            if old_clean_power != self.area_is_clean_power(a, team) {
                self.team_update_commerce(team);
                self.team_update_power_health(team);

                // setDirty(CityInfo_DIRTY_BIT) for the active team: UI only.
            }
        }
    }

    /// C++ `CvArea::isBorderObstacle`.
    #[inline]
    pub fn area_is_border_obstacle(&self, a: AreaId, team: TeamId) -> bool {
        self.map.areas[a].border_obstacle_count[team] > 0
    }

    /// C++ `CvArea::changeBorderObstacleCount`: the first obstacle bumps invalid units.
    pub fn area_change_border_obstacle_count(&mut self, a: AreaId, team: TeamId, change: i32) {
        let count = &mut self.map.areas[a].border_obstacle_count[team];
        *count += change;
        let first = change > 0 && *count == change;
        if first {
            self.map_verify_unit_valid_plot();
        }
    }

    // ------------------------------------------------------------------
    // AI bookkeeping and bonuses
    // ------------------------------------------------------------------

    /// C++ `CvArea::getTargetCity`: None if the stored city no longer exists (C++ `getCity(IDInfo)`).
    #[inline]
    pub fn area_get_target_city(&self, a: AreaId, player: PlayerId) -> Option<CityId> {
        self.map.areas[a].target_cities[player].filter(|&c| self.cities.contains(c))
    }

    /// C++ `CvArea::getNumTotalBonuses`.
    pub fn area_get_num_total_bonuses(&self, a: AreaId) -> i32 {
        self.map.areas[a].num_bonuses.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_defaults() {
        let a = Area::with_counts(AreaId(7), true, 3, 2, 4, 5);
        assert_eq!(a.id, AreaId(7));
        assert!(a.water);
        assert_eq!(a.units_per_player.len(), 3);
        assert_eq!(a.area_ai_type.len(), 2);
        assert_eq!(a.area_ai_type[TeamId(1)], None);
        assert_eq!(a.num_ai_units[PlayerId(2)].len(), UnitAIType::NUM);
        assert_eq!(a.num_bonuses.len(), 4);
        assert_eq!(a.num_improvements.len(), 5);
    }

    #[test]
    fn counters() {
        let mut g = crate::map::tests::game_from_ascii(&["##"], false, false);
        let a = g.map_add_area(false);

        // Only bonuses present in the area and flagged `bOneArea` count (FFH2 flags none).
        assert_eq!(g.area_count_num_unique_bonus_types(a), 0);
        g.map.areas[a].num_bonuses.fill(1);
        let one_area = g.d.bonuses.iter().filter(|b| b.one_area).count() as i32;
        assert_eq!(g.area_count_num_unique_bonus_types(a), one_area);

        // No cities: nothing has the religion / corporation.
        if !g.d.religions.is_empty() {
            assert_eq!(g.area_count_has_religion(a, ReligionType(0), None), 0);
        }
        if !g.d.corporations.is_empty() {
            assert_eq!(g.area_count_has_corporation(a, CorporationType(0), Some(PlayerId(0))), 0);
        }

        // Zero changes are no-ops; a clean power change that keeps the state updates nothing.
        let (p, t) = (PlayerId(0), TeamId(0));
        g.area_change_building_good_health(a, p, 0);
        g.area_change_building_bad_health(a, p, 0);
        g.area_change_building_happiness(a, p, 0);
        g.area_change_free_specialist(a, p, 0);
        g.area_change_yield_rate_modifier(a, p, YieldType::YIELD_COMMERCE, 0);
        g.area_change_clean_power_count(a, t, 0);
        assert_eq!(g.map.areas[a].yield_rate_modifier[p], [0; YieldType::NUM]);
        g.map.areas[a].clean_power_count[t] = 1;
        g.area_change_clean_power_count(a, t, 2);
        assert_eq!(g.map.areas[a].clean_power_count[t], 3);
        assert!(g.area_is_clean_power(a, t));
    }
}
