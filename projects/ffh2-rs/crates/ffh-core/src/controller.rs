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

    /// The research choice of `CvPlayer::chooseTech(0)`: for a human the BUTTONPOPUP_CHOOSETECH
    /// popup (`None` = not answered now; the popup's answer arrives later as `CvNetResearch`),
    /// for an AI player `CvPlayerAI::AI_chooseResearch` (`chooseTech` deletes an AI player's
    /// popup, see `player_choose_tech`, so the rules ask only humans here). Also the default of
    /// [`Controller::ai_choose_research`] and [`Controller::choose_free_tech`].
    fn choose_research(&self, _g: &Game, _p: PlayerId) -> Option<TechType> {
        None
    }

    /// C++ `CvPlayerAI::AI_chooseResearch` as `CvPlayer::doResearch` calls it for every player,
    /// humans included, when research is empty after turn 4 (no popup: C++ silently picks
    /// the AI's tech for a human too). Default: [`Controller::choose_research`].
    fn ai_choose_research(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        self.choose_research(g, p)
    }

    /// C++ `CvPlayerAI::AI_chooseFreeTech` (free tech from great person/goody/etc.).
    fn choose_free_tech(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        self.choose_research(g, p)
    }

    /// C++ `CvPlayerAI::AI_conquerCity`: true = raze. Called before the city is kept. Also the
    /// default of [`Controller::conquered_city_choice`] and [`Controller::disband_acquired_city`].
    fn raze_conquered_city(&self, _g: &Game, _p: PlayerId, _c: CityId) -> bool {
        false
    }

    /// The human conqueror's BUTTONPOPUP_RAZECITY (`CvDLLButtonPopup::launchRazeCityPopup` /
    /// `OnOkClicked`; only humans are asked, AI conquerors go through `AI_conquerCity`): keep
    /// city `c` (button 0), raze it (button 1, offered when `can_raze`), or give it to its
    /// liberation player `gift_to` (button 2, offered when `Some`: the player has the city's
    /// highest culture and is a teammate, has open borders with us, or is our vassal). An answer
    /// whose button is not offered keeps the city. Default: raze when `can_raze` and
    /// [`Controller::raze_conquered_city`] says so, else keep (never a gift).
    fn conquered_city_choice(&self, g: &Game, p: PlayerId, c: CityId, can_raze: bool, _gift_to: Option<PlayerId>) -> ConqueredCityChoice {
        if can_raze && self.raze_conquered_city(g, p, c) {
            ConqueredCityChoice::Raze
        } else {
            ConqueredCityChoice::Keep
        }
    }

    /// The human's BUTTONPOPUP_DISBANDCITY (`launchDisbandCityPopup`): city `c` came to human
    /// `p` without conquest or trade (a culture flip, an FfH spell or event); true = disband it
    /// (button 1, TASK_DISBAND), false = keep it (button 0). Default:
    /// [`Controller::raze_conquered_city`].
    fn disband_acquired_city(&self, g: &Game, p: PlayerId, c: CityId) -> bool {
        self.raze_conquered_city(g, p, c)
    }

    /// FfH's BUTTONPOPUP_CONFIRMSETTLEMENT (`launchConfirmSettlementPopup`): human `p`'s new
    /// city `c` (Kuriotates, below their city limit, not the first city, single player) becomes
    /// a city of radius 3 (button 0, `Some(false)`) or a settlement (button 1, `Some(true)`).
    /// `None` = not answered: the city stays as founded (radius 2, not a settlement), as with a
    /// popup never answered. AI players are not asked (C++ gives them radius 3). Default: `None`.
    fn confirm_settlement(&self, _g: &Game, _p: PlayerId, _c: CityId) -> Option<bool> {
        None
    }

    /// C++ `CvPlayerAI::AI_chooseEvent`: pick one of the trigger's events (None = no choice).
    fn choose_event(&self, _g: &Game, _p: PlayerId, _t: &EventTriggeredData, options: &[EventType]) -> Option<EventType> {
        options.first().copied()
    }

    /// The secretary general's election: for an AI secretary `CvTeamAI::AI_chooseElection`
    /// (`setVoteChosen` at once: `None` deletes the selection); for a human secretary the
    /// BUTTONPOPUP_CHOOSEELECTION popup, answered with `CvNetChooseElection` (`setVoteChosen`),
    /// so `None` from a human's controller keeps the selection (`Game::vote_selections`) until
    /// the human answers through `Game::set_vote_chosen`. Index into `vs.vote_options`.
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

    /// FfH's "take control" popup (Python `CvEventManager` with the `reassignPlayer` callback of
    /// `CvScreensInterface`): human `p` founded the Infernals (first to the Infernal Pact) or
    /// built the Mercurian Gate, and the new civilization `new` appeared; true = play it from
    /// now on (`reassignPlayerAdvanced(p, new, -1)`, button 0), false = keep playing `p`. Only
    /// humans are asked. Default: false.
    fn take_control(&self, _g: &Game, _p: PlayerId, _new: PlayerId) -> bool {
        false
    }

    /// C++ `CvSelectionGroupAI::AI_update` for a human player's group that the AI moves: an
    /// automated group (`AUTOMATE_BUILD`, `NETWORK`, `CITY`, `EXPLORE`, `RELIGION`,
    /// `TERRAFORMING`) or one with an FfH AI-controlled unit (`isAIControl`, e.g. an enraged
    /// unit). Called by the human's `play_turn` (the human branch of `CvPlayerAI::AI_unitUpdate`)
    /// for each such group in cycle order until one returns true (the group is busy: wait for
    /// the next update). An implementation must leave a group it was given with a mission or
    /// without moves (`MISSION_SKIP` as the fallback), or the human's turn waits for it.
    fn update_automated_group(&self, _g: &mut Game, _grp: GroupId) -> bool {
        false
    }
}

/// The human's answer to BUTTONPOPUP_RAZECITY ([`Controller::conquered_city_choice`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConqueredCityChoice {
    /// Button 0: `chooseProduction`, `cityAcquiredAndKept`.
    Keep,
    /// Button 1: TASK_RAZE.
    Raze,
    /// Button 2: `cityAcquiredAndKept`, then TASK_GIFT to the liberation player.
    Gift,
}

impl Game {
    /// C++ `CvPlayerAI::AI_getAttitude(ePlayer, bForced)`: the `bForced` overrides are rules
    /// (a teammate, or the master of our non-capitulated vassal team, is FRIENDLY; anything
    /// involving the barbarians is FURIOUS); otherwise the controller's attitude.
    pub fn player_ai_get_attitude(&self, p: PlayerId, towards: PlayerId, forced: bool) -> AttitudeType {
        if forced {
            let team = self.players[p].team;
            let their_team = self.players[towards].team;
            if team == their_team || (self.teams[team].vassal[their_team] && !self.teams[team].capitulated) {
                return AttitudeType::ATTITUDE_FRIENDLY;
            }
            if self.player_is_barbarian(p) || self.player_is_barbarian(towards) {
                return AttitudeType::ATTITUDE_FURIOUS;
            }
        }
        self.controller(p).attitude(self, p, towards)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::GameSetup;

    /// Answers only the AI-side questions the new hooks default to.
    struct Razes;

    impl Controller for Razes {
        fn choose_research(&self, _g: &Game, _p: PlayerId) -> Option<TechType> {
            Some(TechType(3))
        }

        fn raze_conquered_city(&self, _g: &Game, _p: PlayerId, _c: CityId) -> bool {
            true
        }
    }

    /// The P7 hooks default to the decisions they split from, so an AI (or a controller that
    /// predates them) behaves as before; the human-only popups default to "no answer" / "no".
    #[test]
    fn new_hooks_default_to_the_old_decisions() {
        let d = ffh_data::global();
        let g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3));
        let (p, c) = (PlayerId(0), CityId(0));
        let r = Razes;
        assert_eq!(r.ai_choose_research(&g, p), Some(TechType(3)));
        assert!(r.disband_acquired_city(&g, p, c));
        assert_eq!(r.conquered_city_choice(&g, p, c, true, Some(PlayerId(1))), ConqueredCityChoice::Raze);
        assert_eq!(r.conquered_city_choice(&g, p, c, false, Some(PlayerId(1))), ConqueredCityChoice::Keep, "no raze button");
        assert_eq!(r.confirm_settlement(&g, p, c), None);
        assert!(!r.take_control(&g, p, PlayerId(1)));
        let n = NullController;
        assert_eq!(n.ai_choose_research(&g, p), None);
        assert!(!n.disband_acquired_city(&g, p, c));
        assert_eq!(n.conquered_city_choice(&g, p, c, true, None), ConqueredCityChoice::Keep);
    }
}
