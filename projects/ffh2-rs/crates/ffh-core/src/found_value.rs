//! C++ `CvPlayerAI::AI_foundValue` (CvPlayerAI.cpp:1847-2993) and the bonus valuation it uses
//! (`AI_bonusVal` 7690-7716, `AI_baseBonusVal` 7719-8038, `AI_corporationBonusVal` 8040-8086).
//!
//! These are AI functions in C++, but rules code depends on them: starting plots
//! (`CvPlot::getFoundValue` -> `CvPlayer::findStartingPlot`, `CvGame::normalizeStartingPlotLocations`
//! and the other `CvGame` starting-plot passes) and barbarian city placement
//! (`CvGame::createBarbarianCities`, FfH's barbarian city spawning). So they are ported
//! faithfully here; the new AI (`ffh-ai`) also uses them to score city sites.
//!
//! C++ `AI_isPlotCitySite`/`AI_getCitySite` read the AI's cached city-site list
//! (`m_aiAICitySites`, filled by `AI_updateCitySites` for non-barbarian AI players). Rules
//! callers (barbarians, starting locations) never see a non-empty list, so the list is an
//! explicit `city_sites` argument: rules code passes `&[]`, the new AI passes its own sites.
//! C++ caches `AI_baseBonusVal` per player (`m_aiBonusValue`, reset each turn by
//! `AI_updateBonusValue`); here it is recomputed on each call (same values within a turn).

#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    /// C++ `CvPlayerAI::AI_foundValue(iX, iY, iMinRivalRange = -1, bStartingLoc = false)`,
    /// with the AI's city-site list passed explicitly (see the module docs).
    pub fn player_ai_found_value(&self, p: PlayerId, x: i32, y: i32, min_rival_range: i32, starting_loc: bool, city_sites: &[PlotIdx]) -> i32 {
        todo!("CvPlayerAI::AI_foundValue")
    }

    /// C++ `CvPlayerAI::AI_bonusVal(eBonus, iChange)` (C++ default `iChange = 0`).
    pub fn player_ai_bonus_val(&self, p: PlayerId, bonus: BonusType, change: i32) -> i32 {
        todo!("CvPlayerAI::AI_bonusVal")
    }

    /// C++ `CvPlayerAI::AI_baseBonusVal` (value of a bonus without corporations).
    pub fn player_ai_base_bonus_val(&self, p: PlayerId, bonus: BonusType) -> i32 {
        todo!("CvPlayerAI::AI_baseBonusVal")
    }

    /// C++ `CvPlayerAI::AI_corporationBonusVal`.
    pub fn player_ai_corporation_bonus_val(&self, p: PlayerId, bonus: BonusType) -> i32 {
        todo!("CvPlayerAI::AI_corporationBonusVal")
    }

    /// C++ `CvPlayerAI::AI_getMinFoundValue` (used by the AI's site filtering).
    pub fn player_ai_min_found_value(&self, p: PlayerId) -> i32 {
        todo!("CvPlayerAI::AI_getMinFoundValue")
    }
}
