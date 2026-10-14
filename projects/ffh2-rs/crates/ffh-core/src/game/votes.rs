//! CvGame: diplomatic votes (FfH: Overcouncil / Undercouncil). Vote counting and
//! eligibility, secretary general, diplo vote sources, resolution validity, vote selections
//! and triggered votes, `doVoteResults`/`doVoteSelection`, `processVote` (with the FfH vote
//! effects: crime, gambling/smuggling rings, slave trade, no bonus, free units, PyResult),
//! vote source religions.

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

impl Game {
    /// C++ `CvGame::isTeamVote(eVote)` (secretary general or victory votes elect a team).
    pub fn is_team_vote(&self, vote: VoteType) -> bool {
        todo!("CvGame::isTeamVote")
    }

    /// C++ `CvGame::isChooseElection(eVote)` (not a secretary general vote).
    pub fn is_choose_election(&self, vote: VoteType) -> bool {
        todo!("CvGame::isChooseElection")
    }

    /// C++ `CvGame::isTeamVoteEligible(eTeam, eVoteSource)` (`TEAM_VOTE_MIN_CANDIDATES`).
    pub fn is_team_vote_eligible(&self, team: TeamId, vote_source: VoteSourceType) -> bool {
        todo!("CvGame::isTeamVoteEligible")
    }

    /// C++ `CvGame::countVote(kData, eChoice)`: votes of the alive players that chose `choice`.
    pub fn count_vote(&self, data: &VoteTriggeredData, choice: PlayerVote) -> i32 {
        todo!("CvGame::countVote")
    }

    /// C++ `CvGame::countPossibleVote(eVote, eVoteSource)`.
    pub fn count_possible_vote(&self, vote: VoteType, vote_source: VoteSourceType) -> i32 {
        todo!("CvGame::countPossibleVote")
    }

    /// C++ `CvGame::findHighestVoteTeam(kData)`.
    pub fn find_highest_vote_team(&self, data: &VoteTriggeredData) -> Option<TeamId> {
        todo!("CvGame::findHighestVoteTeam")
    }

    /// C++ `CvGame::getVoteRequired(eVote, eVoteSource)` (population threshold percent of the
    /// possible votes).
    pub fn vote_required(&self, vote: VoteType, vote_source: VoteSourceType) -> i32 {
        todo!("CvGame::getVoteRequired")
    }

    /// C++ `CvGame::getSecretaryGeneral(eVoteSource)`.
    pub fn secretary_general(&self, vote_source: VoteSourceType) -> Option<TeamId> {
        todo!("CvGame::getSecretaryGeneral")
    }

    /// C++ `CvGame::canHaveSecretaryGeneral(eVoteSource)`.
    pub fn can_have_secretary_general(&self, vote_source: VoteSourceType) -> bool {
        todo!("CvGame::canHaveSecretaryGeneral")
    }

    /// C++ `CvGame::clearSecretaryGeneral(eVoteSource)`.
    pub fn clear_secretary_general(&mut self, vote_source: VoteSourceType) {
        todo!("CvGame::clearSecretaryGeneral")
    }

    /// C++ `CvGame::updateSecretaryGeneral`: clears secretaries that are no longer full members.
    pub fn update_secretary_general(&mut self) {
        todo!("CvGame::updateSecretaryGeneral")
    }

    /// C++ `CvGame::isDiploVote(eVoteSource)`. FfH: also true when at least two players are
    /// full members of the vote source.
    pub fn is_diplo_vote(&self, vote_source: VoteSourceType) -> bool {
        todo!("CvGame::isDiploVote")
    }

    /// C++ `CvGame::changeDiploVote(eVoteSource, iChange)` (re-applies the players' vote source
    /// bonuses around the change).
    pub fn change_diplo_vote(&mut self, vote_source: VoteSourceType, change: i32) {
        todo!("CvGame::changeDiploVote")
    }

    /// C++ `CvGame::canDoResolution(eVoteSource, kData)`.
    pub fn can_do_resolution(&self, vote_source: VoteSourceType, data: &VoteSelectionSubData) -> bool {
        todo!("CvGame::canDoResolution")
    }

    /// C++ `CvGame::isValidVoteSelection(eVoteSource, kData)`.
    pub fn is_valid_vote_selection(&self, vote_source: VoteSourceType, data: &VoteSelectionSubData) -> bool {
        todo!("CvGame::isValidVoteSelection")
    }

    /// C++ `CvGame::isVotePassed(eIndex)`: a team vote passed if its outcome is a civ team, other
    /// votes if the outcome is PLAYER_VOTE_YES.
    pub fn is_vote_passed(&self, vote: VoteType) -> bool {
        todo!("CvGame::isVotePassed")
    }

    /// C++ `CvGame::setVoteOutcome(kData, eNewValue)`: `processVote` on a pass/fail change, then
    /// clears every alive player's vote on `data`.
    pub fn set_vote_outcome(&mut self, data: &VoteTriggeredData, new_value: PlayerVote) {
        todo!("CvGame::setVoteOutcome")
    }

    /// C++ `CvGame::getPlayerVote(eOwnerIndex, iVoteId)` (stored on the player).
    pub fn player_vote(&self, owner: PlayerId, vote_id: VoteId) -> PlayerVote {
        todo!("CvGame::getPlayerVote")
    }

    /// C++ `CvGame::setPlayerVote(eOwnerIndex, iVoteId, eNewValue)`.
    pub fn set_player_vote(&mut self, owner: PlayerId, vote_id: VoteId, new_value: PlayerVote) {
        todo!("CvGame::setPlayerVote")
    }

    /// C++ `CvGame::castVote(eOwnerIndex, iVoteId, ePlayerVote)` (AI memory: voted for/against us).
    pub fn cast_vote(&mut self, owner: PlayerId, vote_id: VoteId, player_vote: PlayerVote) {
        todo!("CvGame::castVote")
    }

    /// C++ `CvGame::setVoteChosen(iSelection, iVoteId)`: triggers the chosen option (`None` or out
    /// of range = no vote) and deletes the selection.
    pub fn set_vote_chosen(&mut self, selection: Option<usize>, vote_id: VoteId) {
        if let Some(data) = self.vote_selection(vote_id).cloned() {
            self.add_vote_triggered_from_selection(&data, selection);
        }
        self.delete_vote_selection(vote_id);
    }

    /// C++ `CvGame::doDiploVote`.
    pub fn do_diplo_vote(&mut self) {
        self.do_vote_results();
        self.do_vote_selection();
    }

    /// C++ `CvGame::processVote(kData, iChange)`: applies (+1) or reverts (-1) a passed vote.
    pub fn process_vote(&mut self, data: &VoteTriggeredData, change: i32) {
        todo!("CvGame::processVote")
    }

    /// C++ `CvGame::getVoteSelection(iID)`.
    pub fn vote_selection(&self, id: VoteId) -> Option<&VoteSelectionData> {
        self.vote_selections.iter().find(|v| v.id == id)
    }

    /// C++ `CvGame::addVoteSelection(eVoteSource)`: builds the electable options; `None` (and
    /// nothing stored) when there are none.
    pub fn add_vote_selection(&mut self, vote_source: VoteSourceType) -> Option<VoteId> {
        todo!("CvGame::addVoteSelection")
    }

    /// C++ `CvGame::deleteVoteSelection(iID)`.
    pub fn delete_vote_selection(&mut self, id: VoteId) {
        if let Some(i) = self.vote_selections.iter().position(|v| v.id == id) {
            self.vote_selections.remove(i);
        }
    }

    /// C++ `CvGame::getVoteTriggered(iID)`.
    pub fn vote_triggered(&self, id: VoteId) -> Option<&VoteTriggeredData> {
        self.votes_triggered.iter().find(|v| v.id == id)
    }

    /// C++ `CvGame::addVoteTriggered(const VoteSelectionData& kData, int iChoice)`.
    pub fn add_vote_triggered_from_selection(&mut self, data: &VoteSelectionData, choice: Option<usize>) -> Option<VoteId> {
        match choice {
            Some(i) if i < data.vote_options.len() => self.add_vote_triggered(data.vote_source, &data.vote_options[i]),
            _ => None,
        }
    }

    /// C++ `CvGame::addVoteTriggered(eVoteSource, kOptionData)`: stores the vote and collects
    /// the AI members' votes (`AI_diploVote` -> controller); humans vote through the UI.
    pub fn add_vote_triggered(&mut self, vote_source: VoteSourceType, option: &VoteSelectionSubData) -> Option<VoteId> {
        todo!("CvGame::addVoteTriggered")
    }

    /// C++ `CvGame::deleteVoteTriggered(iID)`.
    pub fn delete_vote_triggered(&mut self, id: VoteId) {
        if let Some(i) = self.votes_triggered.iter().position(|v| v.id == id) {
            self.votes_triggered.remove(i);
        }
    }

    /// C++ `CvGame::doVoteResults`: tallies triggered votes (one extra turn for missing votes,
    /// then abstain), defied/endorsed resolutions, outcomes.
    pub fn do_vote_results(&mut self) {
        todo!("CvGame::doVoteResults")
    }

    /// C++ `CvGame::doVoteSelection`: vote timers, members meet, secretary general elections,
    /// resolution choice by the secretary (`AI_chooseElection` -> controller).
    pub fn do_vote_selection(&mut self) {
        todo!("CvGame::doVoteSelection")
    }

    /// C++ `CvGame::setVoteSourceReligion(eVoteSource, eReligion, bAnnounce)`.
    pub fn set_vote_source_religion(&mut self, vote_source: VoteSourceType, religion: Option<ReligionType>, announce: bool) {
        self.vote_source_religions[vote_source] = religion;

        if announce {
            if let Some(r) = religion {
                // C++: TXT_KEY_VOTE_SOURCE_RELIGION message to every alive player.
                let d = self.d;
                let turn = self.game_turn();
                self.log.msg(turn, None, || {
                    format!(
                        "{} is now the religion of the {}",
                        d.text.name(&d.religions[r].base.description),
                        d.text.name(&d.vote_sources[vote_source].base.description)
                    )
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::Game;
    use crate::ids::VoteId;
    use crate::setup::GameSetup;
    use crate::structs::*;
    use ffh_data::*;

    #[test]
    fn vote_containers() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3));
        let source = VoteSourceType(0);
        let option = VoteSelectionSubData { vote: VoteType(0), player: None, city: None, other_player: None, text: String::new() };
        g.vote_selections.push(VoteSelectionData { id: VoteId(7), vote_source: source, vote_options: vec![option.clone()] });
        assert!(g.vote_selection(VoteId(7)).is_some());
        assert!(g.vote_selection(VoteId(8)).is_none());
        // no choice: nothing is triggered, the selection is deleted
        g.set_vote_chosen(None, VoteId(7));
        assert!(g.vote_selections.is_empty());
        assert!(g.votes_triggered.is_empty());

        g.votes_triggered.push(VoteTriggeredData { id: VoteId(3), vote_source: source, vote_option: option });
        assert!(g.vote_triggered(VoteId(3)).is_some());
        g.delete_vote_triggered(VoteId(3));
        assert!(g.vote_triggered(VoteId(3)).is_none());

        g.set_vote_source_religion(source, Some(ReligionType(1)), false);
        assert_eq!(g.vote_source_religions[source], Some(ReligionType(1)));
        g.set_vote_source_religion(source, None, true);
        assert_eq!(g.vote_source_religions[source], None);
    }
}
