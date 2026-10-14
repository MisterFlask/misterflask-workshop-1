//! CvTeam: research and techs (cost, progress, `setHasTech`, `processTech`, tech sharing,
//! no-trade techs), bonuses (obsolete, force-revealed), terrain/river trade, irrigation,
//! water work, extra water sight, improvement yields, building effects, turn processing
//! and circumnavigation.

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{civ_team_ids, is_option};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Turn processing
    // ------------------------------------------------------------------

    /// C++ `CvTeam::doTurn`. Its `AI_doTurnPre()` is `team_ai_do_counter` (bookkeeping);
    /// `AI_doTurnPost()` is `team_ai_update_worst_enemy` + AI-only area strategies and war
    /// decisions (`AI_doWar` -> controller).
    pub fn team_do_turn(&mut self, t: TeamId) {
        todo!("CvTeam::doTurn")
    }

    /// C++ `CvTeam::updateYield` (updates every alive member player).
    pub fn team_update_yield(&mut self, t: TeamId) {
        todo!("CvTeam::updateYield")
    }

    /// C++ `CvTeam::updatePowerHealth`.
    pub fn team_update_power_health(&mut self, t: TeamId) {
        todo!("CvTeam::updatePowerHealth")
    }

    /// C++ `CvTeam::updateCommerce`.
    pub fn team_update_commerce(&mut self, t: TeamId) {
        todo!("CvTeam::updateCommerce")
    }

    /// C++ `CvTeam::processBuilding`: team-wide effects of a building (vote source
    /// eligibility, map centering, enemy war weariness, FfH no civic anger).
    pub fn team_process_building(&mut self, t: TeamId, building: BuildingType, change: i32) {
        todo!("CvTeam::processBuilding")
    }

    /// C++ `CvTeam::testCircumnavigated` (protected).
    pub fn team_test_circumnavigated(&mut self, t: TeamId) {
        todo!("CvTeam::testCircumnavigated")
    }

    // ------------------------------------------------------------------
    // Research
    // ------------------------------------------------------------------

    /// C++ `CvTeam::getResearchCost`.
    pub fn team_get_research_cost(&self, t: TeamId, tech: TechType) -> i32 {
        todo!("CvTeam::getResearchCost")
    }

    /// C++ `CvTeam::getResearchLeft`.
    pub fn team_get_research_left(&self, t: TeamId, tech: TechType) -> i32 {
        (self.team_get_research_cost(t, tech) - self.team_get_research_progress(t, Some(tech))).max(0)
    }

    /// C++ `CvTeam::getResearchProgress`: 0 for NO_TECH (`tech = None`).
    pub fn team_get_research_progress(&self, t: TeamId, tech: Option<TechType>) -> i32 {
        match tech {
            Some(tech) => self.teams[t].research_progress[tech],
            None => 0,
        }
    }

    /// C++ `CvTeam::setResearchProgress` (grants the tech when complete).
    pub fn team_set_research_progress(&mut self, t: TeamId, tech: TechType, value: i32, player: PlayerId) {
        todo!("CvTeam::setResearchProgress")
    }

    /// C++ `CvTeam::changeResearchProgress`.
    pub fn team_change_research_progress(&mut self, t: TeamId, tech: TechType, change: i32, player: PlayerId) {
        let v = self.team_get_research_progress(t, Some(tech)) + change;
        self.team_set_research_progress(t, tech, v, player);
    }

    /// C++ `CvTeam::changeResearchProgressPercent`: returns the beakers added.
    pub fn team_change_research_progress_percent(&mut self, t: TeamId, tech: TechType, percent: i32, player: PlayerId) -> i32 {
        let mut beakers = 0;

        if 0 != percent && !self.team_is_has_tech(t, Some(tech)) {
            if percent > 0 {
                beakers = self
                    .team_get_research_left(t, tech)
                    .min((self.team_get_research_cost(t, tech) * percent) / 100);
            } else {
                beakers = (self.team_get_research_left(t, tech) - self.team_get_research_cost(t, tech))
                    .max((self.team_get_research_cost(t, tech) * percent) / 100);
            }

            self.team_change_research_progress(t, tech, beakers, player);
        }

        beakers
    }

    // ------------------------------------------------------------------
    // Techs
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isHasTech`: true for NO_TECH (`tech = None`).
    pub fn team_is_has_tech(&self, t: TeamId, tech: Option<TechType>) -> bool {
        match tech {
            Some(tech) => self.teams[t].has_tech[tech],
            None => true,
        }
    }

    /// C++ `CvTeam::setHasTech`. C++ returns immediately for NO_TECH, so callers holding an
    /// optional tech skip the call. `player = None` (C++ `NO_PLAYER`) means the team leader.
    pub fn team_set_has_tech(&mut self, t: TeamId, tech: TechType, value: bool, player: Option<PlayerId>, first: bool, announce: bool) {
        todo!("CvTeam::setHasTech")
    }

    /// C++ `CvTeam::processTech` (protected): apply (`change = 1`) or remove (`-1`) a tech's effects.
    pub fn team_process_tech(&mut self, t: TeamId, tech: TechType, change: i32) {
        todo!("CvTeam::processTech")
    }

    /// C++ `CvTeam::shareItems`: on a permanent alliance, take over `other`'s techs, forced
    /// revealed bonuses, espionage points and team-shared buildings.
    pub fn team_share_items(&mut self, t: TeamId, other: TeamId) {
        todo!("CvTeam::shareItems")
    }

    /// C++ `CvTeam::isTechShare`.
    pub fn team_is_tech_share(&self, t: TeamId, index: usize) -> bool {
        self.teams[t].tech_share_count[index] > 0
    }

    /// C++ `CvTeam::changeTechShareCount`.
    pub fn team_change_tech_share_count(&mut self, t: TeamId, index: usize, change: i32) {
        if change != 0 {
            self.teams[t].tech_share_count[index] += change;

            if self.team_is_tech_share(t, index) {
                self.team_update_tech_share(t);
            }
        }
    }

    /// C++ `CvTeam::updateTechShare(TechTypes)` (protected overload for one tech).
    pub fn team_update_tech_share_tech(&mut self, t: TeamId, tech: TechType) {
        if self.team_is_has_tech(t, Some(tech)) {
            return;
        }

        let mut best_share = MAX_INT;

        for i in 0..self.num_teams() {
            if self.team_is_tech_share(t, i) {
                best_share = best_share.min(i as i32 + 1);
            }
        }

        if best_share != MAX_INT {
            let mut count = 0;

            for o in civ_team_ids(self) {
                if self.team_is_alive(o) {
                    if self.team_is_has_tech(o, Some(tech)) {
                        if self.teams[t].has_met[o] {
                            count += 1;
                        }
                    }
                }
            }

            if count >= best_share {
                self.team_set_has_tech(t, tech, true, None, true, true);
            }
        }
    }

    /// C++ `CvTeam::updateTechShare()` (protected overload for all techs).
    pub fn team_update_tech_share(&mut self, t: TeamId) {
        for tech in self.d.techs.keys() {
            self.team_update_tech_share_tech(t, tech);
        }
    }

    // ------------------------------------------------------------------
    // Bonuses
    // ------------------------------------------------------------------

    /// C++ `CvTeam::hasBonus` (any member player has the bonus).
    pub fn team_has_bonus(&self, t: TeamId, bonus: BonusType) -> bool {
        todo!("CvTeam::hasBonus")
    }

    /// C++ `CvTeam::isBonusObsolete`.
    pub fn team_is_bonus_obsolete(&self, t: TeamId, bonus: BonusType) -> bool {
        todo!("CvTeam::isBonusObsolete")
    }

    /// C++ `CvTeam::setForceRevealedBonus`.
    pub fn team_set_force_revealed_bonus(&mut self, t: TeamId, bonus: BonusType, revealed: bool) {
        todo!("CvTeam::setForceRevealedBonus")
    }

    /// C++ `CvTeam::isForceRevealedBonus`.
    pub fn team_is_force_revealed_bonus(&self, t: TeamId, bonus: BonusType) -> bool {
        self.teams[t].revealed_bonuses.contains(&bonus)
    }

    // ------------------------------------------------------------------
    // Terrain, routes, irrigation, sight
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isTerrainTrade`.
    pub fn team_is_terrain_trade(&self, t: TeamId, terrain: TerrainType) -> bool {
        self.teams[t].terrain_trade_count[terrain] > 0
    }

    /// C++ `CvTeam::changeTerrainTradeCount` (updates member players' plot groups).
    pub fn team_change_terrain_trade_count(&mut self, t: TeamId, terrain: TerrainType, change: i32) {
        todo!("CvTeam::changeTerrainTradeCount")
    }

    /// C++ `CvTeam::isRiverTrade`.
    pub fn team_is_river_trade(&self, t: TeamId) -> bool {
        self.teams[t].river_trade_count > 0
    }

    /// C++ `CvTeam::changeRiverTradeCount` (updates member players' plot groups).
    pub fn team_change_river_trade_count(&mut self, t: TeamId, change: i32) {
        todo!("CvTeam::changeRiverTradeCount")
    }

    /// C++ `CvTeam::isBridgeBuilding`. (`changeBridgeBuildingCount` is a plain
    /// `bridge_building_count += n`; its only side effect is a graphics refresh.)
    pub fn team_is_bridge_building(&self, t: TeamId) -> bool {
        self.teams[t].bridge_building_count > 0
    }

    /// C++ `CvTeam::isIrrigation`.
    pub fn team_is_irrigation(&self, t: TeamId) -> bool {
        self.teams[t].irrigation_count > 0
    }

    /// C++ `CvTeam::changeIrrigationCount` (calls `CvMap::updateIrrigated`).
    pub fn team_change_irrigation_count(&mut self, t: TeamId, change: i32) {
        todo!("CvTeam::changeIrrigationCount")
    }

    /// C++ `CvTeam::isIgnoreIrrigation`.
    pub fn team_is_ignore_irrigation(&self, t: TeamId) -> bool {
        self.teams[t].ignore_irrigation_count > 0
    }

    /// C++ `CvTeam::isWaterWork`.
    pub fn team_is_water_work(&self, t: TeamId) -> bool {
        self.teams[t].water_work_count > 0
    }

    /// C++ `CvTeam::changeWaterWorkCount`.
    pub fn team_change_water_work_count(&mut self, t: TeamId, change: i32) {
        if change != 0 {
            self.teams[t].water_work_count += change;

            self.team_ai_make_assign_work_dirty(t);
        }
    }

    /// C++ `CvTeamAI::AI_makeAssignWorkDirty`: mark every alive member player's cities for
    /// citizen re-assignment (governor bookkeeping, see `governor.rs`).
    pub fn team_ai_make_assign_work_dirty(&mut self, t: TeamId) {
        todo!("CvTeamAI::AI_makeAssignWorkDirty")
    }

    /// C++ `CvTeam::isExtraWaterSeeFrom`.
    pub fn team_is_extra_water_see_from(&self, t: TeamId) -> bool {
        self.teams[t].extra_water_see_from_count > 0
    }

    /// C++ `CvTeam::changeExtraWaterSeeFromCount` (recomputes sight around the change).
    pub fn team_change_extra_water_see_from_count(&mut self, t: TeamId, change: i32) {
        todo!("CvTeam::changeExtraWaterSeeFromCount")
    }

    /// C++ `CvTeam::changeImprovementYieldChange`.
    pub fn team_change_improvement_yield_change(&mut self, t: TeamId, improvement: ImprovementType, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.teams[t].improvement_yield_change[improvement][yield_type.idx()] += change;

            self.team_update_yield(t);
        }
    }

    // ------------------------------------------------------------------
    // Buildings
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isObsoleteBuilding`.
    pub fn team_is_obsolete_building(&self, t: TeamId, building: BuildingType) -> bool {
        self.teams[t].obsolete_building_count[building] > 0
    }

    /// C++ `CvTeam::changeObsoleteBuildingCount` (re-processes the building in member cities).
    pub fn team_change_obsolete_building_count(&mut self, t: TeamId, building: BuildingType, change: i32) {
        todo!("CvTeam::changeObsoleteBuildingCount")
    }
}
