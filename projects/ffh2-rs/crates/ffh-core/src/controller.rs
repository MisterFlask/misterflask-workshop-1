//! Player controllers: the boundary between rules and decision-making.
//!
//! The C++ rules code calls `AI_*` functions synchronously whenever an AI player
//! must decide something (raze a conquered city, pick an event option, choose
//! research, ...). Here every player has a [`Controller`]; rules code calls
//! `Game::controller(p)` and invokes the matching method. Controllers take
//! `&self` so they can be called re-entrantly while the game is `&mut`
//! borrowed; per-player AI memory belongs in game state (see `Player::ai`).
//!
//! The defaults implement simple, deterministic behaviour so the rules engine
//! runs without the AI crate (useful for tests).

use std::sync::Arc;

use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

pub trait Controller: Send + Sync {
    fn name(&self) -> &str {
        "default"
    }

    /// Play one turn for player `p` (C++ `AI_unitUpdate`, `AI_doTurnPre/Post`, city
    /// orders, research, civics, diplomacy...). Called while the player's turn is active.
    fn play_turn(&self, _g: &mut Game, _p: PlayerId) {}

    /// C++ `CvCityAI::AI_chooseProduction`: push at least one order if the queue is empty.
    fn choose_production(&self, _g: &mut Game, _c: CityId) {}

    /// C++ `CvPlayerAI::AI_chooseResearch`.
    fn choose_research(&self, _g: &Game, _p: PlayerId) -> Option<TechType> {
        None
    }

    /// C++ `CvPlayerAI::AI_chooseFreeTech` (free tech from great person/goody/etc.).
    fn choose_free_tech(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        self.choose_research(g, p)
    }

    /// C++ `CvPlayerAI::AI_conquerCity`: true = raze. Called before the city is kept.
    fn raze_conquered_city(&self, _g: &Game, _p: PlayerId, _c: CityId) -> bool {
        false
    }

    /// C++ `CvPlayerAI::AI_chooseEvent`: pick one of the trigger's events (None = no choice).
    fn choose_event(&self, _g: &Game, _p: PlayerId, _t: &EventTriggeredData, options: &[EventType]) -> Option<EventType> {
        options.first().copied()
    }

    /// C++ `CvPlayerAI::AI_chooseElection`: index into `vs.vote_options`, or None.
    fn choose_election(&self, _g: &Game, _p: PlayerId, _vs: &VoteSelectionData) -> Option<usize> {
        None
    }

    /// C++ `CvPlayerAI::AI_diploVote`.
    fn diplo_vote(&self, _g: &Game, _p: PlayerId, _vote: &VoteSelectionSubData, _source: VoteSourceType) -> PlayerVote {
        PlayerVote::PLAYER_VOTE_ABSTAIN
    }

    /// C++ `CvPlayerAI::AI_captureUnit`: keep a captured unit?
    fn keep_captured_unit(&self, _g: &Game, _p: PlayerId, _unit: UnitType, _plot: PlotIdx) -> bool {
        true
    }

    /// C++ `CvPlayerAI::AI_acceptUnit`: accept a gifted unit?
    fn accept_unit(&self, _g: &Game, _p: PlayerId, _unit: UnitId) -> bool {
        true
    }

    /// C++ `CvPlayerAI::AI_considerOffer` / deal evaluation: accept the trade?
    fn consider_offer(&self, _g: &Game, _p: PlayerId, _from: PlayerId, _they_give: &[TradeData], _we_give: &[TradeData]) -> bool {
        false
    }

    /// C++ `CvPlayerAI::AI_chooseReligion` (FFH: religion choice when founding/converting).
    fn choose_religion(&self, _g: &Game, _p: PlayerId) -> Option<ReligionType> {
        None
    }

    /// C++ `CvPlayerAI::AI_getAttitude(ePlayer)`: player `p`'s attitude towards `towards`.
    /// Rules code reads it in a few places (FfH event trigger `PrereqCivilizationPleased`;
    /// AI memory bookkeeping); team attitude (`CvTeamAI::AI_getAttitude`) averages members.
    fn attitude(&self, _g: &Game, _p: PlayerId, _towards: PlayerId) -> AttitudeType {
        AttitudeType::ATTITUDE_CAUTIOUS
    }

    /// C++ `CvSelectionGroupAI::AI_update` for automated (non-AI-controlled) groups of
    /// human players, e.g. automated workers/explorers. Return true if the group acted.
    fn update_automated_group(&self, _g: &mut Game, _grp: GroupId) -> bool {
        false
    }
}

/// Controller with all-default behaviour.
#[derive(Default)]
pub struct NullController;

impl Controller for NullController {
    fn name(&self) -> &str {
        "null"
    }
}

pub type ControllerRef = Arc<dyn Controller>;

pub fn null_controller() -> ControllerRef {
    Arc::new(NullController)
}
