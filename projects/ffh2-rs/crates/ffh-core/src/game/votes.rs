//! CvGame: diplomatic votes (FfH: Overcouncil / Undercouncil). Vote counting and
//! eligibility, secretary general, diplo vote sources, resolution validity, vote selections
//! and triggered votes, `doVoteResults`/`doVoteSelection`, `processVote` (with the FfH vote
//! effects: crime, gambling/smuggling rings, slave trade, no bonus, free units, PyResult),
//! vote source religions.
//!
//! Decisions: the secretary general's choice of resolution (C++ `CvTeamAI::AI_chooseElection`,
//! or the `BUTTONPOPUP_CHOOSEELECTION` popup for a human secretary) and each member's vote
//! (`CvPlayerAI::AI_diploVote`, or the `BUTTONPOPUP_DIPLOVOTE` popup for humans) go to the
//! controllers (`choose_election`, `diplo_vote`), answered at once.
//!
//! Vote containers: C++ `m_voteSelections` / `m_votesTriggered` are `FFreeListTrashArray`s.
//! Their ids are reproduced exactly (`m_iCurrentID + slot`, see [`flta_add`]) and the
//! `Vec`s are kept in slot order, which is the C++ iteration order (`doVoteResults`).
//! `Game::next_vote_selection_id` / `next_vote_triggered_id` (0 after `reset`) hold the array
//! state as `m_iCurrentID - FLTA_MAX_BUCKETS + m_iLastIndex + 1`.

use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

// ---------------------------------------------------------------------------------------------
// FFreeListTrashArray bookkeeping of the vote containers (FFreeListTrashArray.h)
// ---------------------------------------------------------------------------------------------

/// C++ `FLTA_ID_SHIFT`.
const FLTA_ID_SHIFT: u32 = 13;
/// C++ `FLTA_MAX_BUCKETS`: `m_iCurrentID` starts at this value and grows by it per `add()`.
const FLTA_MAX_BUCKETS: u32 = 1 << FLTA_ID_SHIFT;
/// C++ `FLTA_INDEX_MASK`: the slot part of an id.
const FLTA_INDEX_MASK: u32 = FLTA_MAX_BUCKETS - 1;

/// Slot of a vote container entry (C++ `iID & FLTA_INDEX_MASK`): the iteration order.
fn flta_slot(id: VoteId) -> u32 {
    id.0 & FLTA_INDEX_MASK
}

/// C++ `FFreeListTrashArray::add()`: the id of a new entry, `m_iCurrentID + slot`, where the
/// slot is the head of the free list, else `++m_iLastIndex`. `next_id` is the container state
/// (`m_iCurrentID - FLTA_MAX_BUCKETS + m_iLastIndex + 1`, 0 = freshly `init`ed), `live` the
/// ids of the entries in the container.
///
/// The free list is LIFO over the freed slots. Only `doVoteResults` frees triggered votes (in
/// slot order, so the last freed slot is the highest) and vote selections never outlive the
/// call that creates them, so the head of the C++ free list is the highest free slot up to
/// `m_iLastIndex` -- except after a triggered vote that waited an extra turn for a missing vote
/// is freed later than a higher slot, where C++ would reuse that lower slot first.
// SIGNATURE: exact parity in that case needs the free list itself, which `Game` does not store
// (only `next_vote_triggered_id`): it would need a field for C++ m_votesTriggered's free-list
// order (game.rs).
fn flta_add(next_id: &mut u32, live: &[VoteId]) -> VoteId {
    let adds = *next_id >> FLTA_ID_SHIFT;
    let index_after_last = *next_id & FLTA_INDEX_MASK;

    let free = (0..index_after_last).rev().find(|&slot| !live.iter().any(|&id| flta_slot(id) == slot));
    let (slot, index_after_last) = match free {
        Some(slot) => (slot, index_after_last),
        None => (index_after_last, index_after_last + 1),
    };
    debug_assert!(index_after_last < FLTA_MAX_BUCKETS, "FFreeListTrashArray<T>::add() is full");

    // m_iCurrentID = FLTA_MAX_BUCKETS * (adds + 1); m_iCurrentID += FLTA_MAX_BUCKETS
    let current_id = (adds + 1) << FLTA_ID_SHIFT;
    *next_id = current_id | index_after_last;
    VoteId(current_id + slot)
}

/// Position of a new entry in a container kept in slot order.
fn flta_insert_pos(ids: impl Iterator<Item = VoteId>, id: VoteId) -> usize {
    ids.take_while(|&other| flta_slot(other) < flta_slot(id)).count()
}

/// C++ `GET_PLAYER(p).getCity(iID)`: the city if it still exists and belongs to `p`.
fn player_city(g: &Game, p: PlayerId, city: Option<CityId>) -> Option<CityId> {
    city.filter(|&c| g.cities.get(c).is_some_and(|ct| ct.owner == p))
}

/// The "(required of possible votes)" part of the option texts (C++ `TXT_KEY_POPUP_ELECTION_*`
/// with `getVoteRequired` and `countPossibleVote`; display only).
fn votes_needed_text(g: &Game, vote: VoteType, vote_source: VoteSourceType) -> String {
    format!(
        " ({} of {} votes needed)",
        g.vote_required(vote, vote_source),
        g.count_possible_vote(vote, vote_source)
    )
}

impl Game {
    /// C++ `CvGame::isTeamVote(eVote)` (secretary general or victory votes elect a team).
    pub fn is_team_vote(&self, vote: VoteType) -> bool {
        self.d.votes[vote].secretary_general || self.d.votes[vote].victory
    }

    /// C++ `CvGame::isChooseElection(eVote)` (not a secretary general vote).
    pub fn is_choose_election(&self, vote: VoteType) -> bool {
        !self.d.votes[vote].secretary_general
    }

    /// C++ `CvGame::isTeamVoteEligible(eTeam, eVoteSource)` (`TEAM_VOTE_MIN_CANDIDATES`).
    pub fn is_team_vote_eligible(&self, team: TeamId, vote_source: VoteSourceType) -> bool {
        if self.team_is_force_team_vote_eligible(team, vote_source) {
            return true;
        }

        if !self.team_is_full_member(team, vote_source) {
            return false;
        }

        let mut count = 0;
        for t in self.civ_team_ids() {
            if self.team_is_alive(t) {
                if self.team_is_force_team_vote_eligible(t, vote_source) {
                    count += 1;
                }
            }
        }

        let mut extra_eligible = self.d.defines.team_vote_min_candidates - count;
        if extra_eligible <= 0 {
            return false;
        }

        for t in self.civ_team_ids() {
            if t != team {
                if self.team_is_alive(t) {
                    if !self.team_is_force_team_vote_eligible(t, vote_source) {
                        if self.team_is_full_member(t, vote_source) {
                            let loop_votes = self.team_get_votes(t, None, vote_source);
                            let votes = self.team_get_votes(team, None, vote_source);
                            if loop_votes > votes || (loop_votes == votes && t < team) {
                                extra_eligible -= 1;
                            }
                        }
                    }
                }
            }
        }

        extra_eligible > 0
    }

    /// C++ `CvGame::countVote(kData, eChoice)`: votes of the alive players that chose `choice`.
    pub fn count_vote(&self, data: &VoteTriggeredData, choice: PlayerVote) -> i32 {
        let mut count = 0;

        for p in self.civ_player_ids() {
            if self.players[p].alive {
                if self.player_vote(p, data.id) == choice {
                    count += self.player_get_votes(p, Some(data.vote_option.vote), data.vote_source);
                }
            }
        }

        count
    }

    /// C++ `CvGame::countPossibleVote(eVote, eVoteSource)`.
    pub fn count_possible_vote(&self, vote: VoteType, vote_source: VoteSourceType) -> i32 {
        let mut count = 0;

        for p in self.civ_player_ids() {
            count += self.player_get_votes(p, Some(vote), vote_source);
        }

        count
    }

    /// C++ `CvGame::findHighestVoteTeam(kData)`.
    pub fn find_highest_vote_team(&self, data: &VoteTriggeredData) -> Option<TeamId> {
        let mut best_team = None;
        let mut best_count = 0;

        if self.is_team_vote(data.vote_option.vote) {
            for t in self.civ_team_ids() {
                if self.team_is_alive(t) {
                    let count = self.count_vote(data, PlayerVote::for_team(t));

                    if count > best_count {
                        best_count = count;
                        best_team = Some(t);
                    }
                }
            }
        }

        best_team
    }

    /// C++ `CvGame::getVoteRequired(eVote, eVoteSource)` (population threshold percent of the
    /// possible votes).
    pub fn vote_required(&self, vote: VoteType, vote_source: VoteSourceType) -> i32 {
        (self.count_possible_vote(vote, vote_source) * self.d.votes[vote].population_threshold) / 100
    }

    /// C++ `CvGame::getSecretaryGeneral(eVoteSource)`: the team elected by the source's
    /// secretary general vote; for a source without one, the first alive player owning a building
    /// of the source (with the source's religion as state religion, if it has one).
    pub fn secretary_general(&self, vote_source: VoteSourceType) -> Option<TeamId> {
        let d = self.d;

        if !self.can_have_secretary_general(vote_source) {
            for building in d.buildings.keys() {
                if d.buildings[building].vote_source_type == Some(vote_source) {
                    for p in self.civ_player_ids() {
                        if self.players[p].alive {
                            let building_class = d.buildings[building].building_class_type.expect("CvBuildingInfo without a building class");
                            if self.players[p].building_class_count[building_class] > 0 {
                                let religion = self.vote_source_religions[vote_source];
                                if religion.is_none() || self.player_get_state_religion(p) == religion {
                                    return Some(self.players[p].team);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            for vote in d.votes.keys() {
                if d.votes[vote].vote_source_type[vote_source] {
                    if d.votes[vote].secretary_general {
                        if self.is_vote_passed(vote) {
                            return self.vote_outcome[vote].team();
                        }
                    }
                }
            }
        }

        None
    }

    /// C++ `CvGame::canHaveSecretaryGeneral(eVoteSource)`.
    pub fn can_have_secretary_general(&self, vote_source: VoteSourceType) -> bool {
        let d = self.d;
        d.votes
            .keys()
            .any(|vote| d.votes[vote].vote_source_type[vote_source] && d.votes[vote].secretary_general)
    }

    /// C++ `CvGame::clearSecretaryGeneral(eVoteSource)`.
    pub fn clear_secretary_general(&mut self, vote_source: VoteSourceType) {
        let d = self.d;

        for vote in d.votes.keys() {
            if d.votes[vote].vote_source_type[vote_source] {
                if d.votes[vote].secretary_general {
                    // C++ quirk: `kData.iId` and `kData.kVoteOption.eOtherPlayer` are left
                    // uninitialized; setVoteOutcome clears the players' votes under that stack
                    // garbage id, taken here as an id no triggered vote has.
                    let data = VoteTriggeredData {
                        id: VoteId(0),
                        vote_source,
                        vote_option: VoteSelectionSubData {
                            vote,
                            player: None,
                            city: None,
                            other_player: None,
                            text: String::new(),
                        },
                    };
                    self.set_vote_outcome(&data, PlayerVote::NO_PLAYER_VOTE);
                    self.secretary_general_timer[vote_source] = 0;
                }
            }
        }
    }

    /// C++ `CvGame::updateSecretaryGeneral`: clears secretaries that are no longer full members.
    pub fn update_secretary_general(&mut self) {
        for vote_source in self.d.vote_sources.keys() {
            if let Some(secretary_general) = self.secretary_general(vote_source) {
                if !self.team_is_full_member(secretary_general, vote_source) {
                    self.clear_secretary_general(vote_source);
                }
            }
        }
    }

    /// C++ `CvGame::isDiploVote(eVoteSource)`. FfH: also true when at least two players are
    /// full members of the vote source.
    pub fn is_diplo_vote(&self, vote_source: VoteSourceType) -> bool {
        // FfH: Added by Kael 03/19/2008
        let mut count = 0;
        for p in self.player_ids() {
            if self.player_is_full_member(p, vote_source) {
                count += 1;
            }
        }
        if count >= 2 {
            return true;
        }
        // FfH: End Add

        self.diplo_vote[vote_source] > 0
    }

    /// C++ `CvGame::changeDiploVote(eVoteSource, iChange)` (re-applies the players' vote source
    /// bonuses around the change).
    pub fn change_diplo_vote(&mut self, vote_source: VoteSourceType, change: i32) {
        if change != 0 {
            for p in self.player_ids() {
                self.player_process_vote_source_bonus(p, vote_source, false);
            }

            self.diplo_vote[vote_source] += change;
            debug_assert!(self.diplo_vote[vote_source] >= 0);

            for p in self.player_ids() {
                self.player_process_vote_source_bonus(p, vote_source, true);
            }
        }
    }

    /// C++ `CvGame::canDoResolution(eVoteSource, kData)`.
    pub fn can_do_resolution(&self, vote_source: VoteSourceType, data: &VoteSelectionSubData) -> bool {
        let victory = self.d.votes[data.vote].victory;

        if victory {
            for t in self.civ_team_ids() {
                if self.team_is_voting_member(t, vote_source) {
                    if self.team_get_votes(t, Some(data.vote), vote_source) >= self.vote_required(data.vote, vote_source) {
                        // Can't vote on a winner if one team already has all the votes necessary to win
                        return false;
                    }
                }
            }
        }

        for p in self.civ_player_ids() {
            if self.player_is_voting_member(p, vote_source) {
                if !self.player_can_do_resolution(p, vote_source, data) {
                    return false;
                }
            } else if self.players[p].alive && !self.player_is_barbarian(p) && !self.player_is_minor_civ(p) {
                // all players need to be able to vote for a diplo victory
                if victory {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvGame::isValidVoteSelection(eVoteSource, kData)`.
    pub fn is_valid_vote_selection(&self, vote_source: VoteSourceType, data: &VoteSelectionSubData) -> bool {
        let d = self.d;

        if let Some(p) = data.player {
            if !self.players[p].alive || self.player_is_barbarian(p) || self.player_is_minor_civ(p) {
                return false;
            }
        }

        if let Some(p) = data.other_player {
            if !self.players[p].alive || self.player_is_barbarian(p) || self.player_is_minor_civ(p) {
                return false;
            }
        }

        let mut num_voters = 0;
        for t in self.civ_team_ids() {
            if self.team_is_voting_member(t, vote_source) {
                num_voters += 1;
            }
        }
        if num_voters < d.votes[data.vote].min_voters {
            return false;
        }

        let info = &d.votes[data.vote];
        // C++ dereferences GET_PLAYER(kData.ePlayer) for these resolutions without a check.
        let vote_player = || data.player.expect("isValidVoteSelection: resolution without a player");

        if info.open_borders {
            let mut open_with_everyone = true;
            for t1 in self.civ_team_ids() {
                if self.team_is_full_member(t1, vote_source) {
                    for t2 in self.civ_team_ids().filter(|&t2| t2 > t1) {
                        if self.team_is_full_member(t2, vote_source) {
                            if !self.team_is_open_borders(t2, t1) {
                                open_with_everyone = false;
                                break;
                            }
                        }
                    }
                }
            }
            if open_with_everyone {
                return false;
            }
        } else if info.defensive_pact {
            let mut pact_with_everyone = true;
            for t1 in self.civ_team_ids() {
                if self.team_is_full_member(t1, vote_source) {
                    for t2 in self.civ_team_ids().filter(|&t2| t2 > t1) {
                        if self.team_is_full_member(t2, vote_source) {
                            if !self.teams[t2].defensive_pact[t1] {
                                pact_with_everyone = false;
                                break;
                            }
                        }
                    }
                }
            }
            if pact_with_everyone {
                return false;
            }
        } else if info.force_peace {
            let p = vote_player();
            let team = self.players[p].team;

            if self.team_is_a_vassal(team) {
                return false;
            }

            if !self.player_is_full_member(p, vote_source) {
                return false;
            }

            let mut valid = false;

            for t2 in self.civ_team_ids() {
                if self.at_war(team, t2) {
                    if self.team_is_voting_member(t2, vote_source) {
                        valid = true;
                        break;
                    }
                }
            }

            if !valid {
                return false;
            }
        } else if info.force_no_trade {
            let p = vote_player();
            let team = self.players[p].team;

            if self.player_is_full_member(p, vote_source) {
                return false;
            }

            let mut no_trade_with_everyone = true;
            for p2 in self.civ_player_ids() {
                if self.players[p2].team != team {
                    if self.player_is_full_member(p2, vote_source) {
                        if self.player_can_stop_trading_with_team(p2, team, false) {
                            no_trade_with_everyone = false;
                            break;
                        }
                    }
                }
            }
            // Not an option if already at war with everyone
            if no_trade_with_everyone {
                return false;
            }
        } else if info.force_war {
            let p = vote_player();
            let team = self.players[p].team;

            if self.team_is_a_vassal(team) {
                return false;
            }

            if self.player_is_full_member(p, vote_source) {
                return false;
            }

            let mut at_war_with_everyone = true;
            for t2 in self.civ_team_ids() {
                if t2 != team {
                    if self.team_is_full_member(t2, vote_source) {
                        if !self.teams[t2].at_war[team] && self.team_can_change_war_peace(t2, team, false) {
                            at_war_with_everyone = false;
                            break;
                        }
                    }
                }
            }
            // Not an option if already at war with everyone
            if at_war_with_everyone {
                return false;
            }

            // Can be passed against a non-member only if he is already at war with a member
            if !self.player_is_voting_member(p, vote_source) {
                let mut valid = false;
                for t2 in self.civ_team_ids() {
                    if self.at_war(team, t2) {
                        if self.team_is_full_member(t2, vote_source) {
                            valid = true;
                            break;
                        }
                    }
                }

                if !valid {
                    return false;
                }
            }
        } else if info.assign_city {
            let p = vote_player();
            if self.player_is_full_member(p, vote_source) || !self.player_is_voting_member(p, vote_source) {
                return false;
            }

            // (C++ also FAsserts that the city exists.)
            let city = player_city(self, p, data.city);
            if city.is_none() {
                return false;
            }

            let Some(other) = data.other_player else {
                return false;
            };

            let team = self.players[p].team;
            let other_team = self.players[other].team;
            if other_team == team {
                return false;
            }

            if self.at_war(team, other_team) {
                return false;
            }

            if !self.player_is_full_member(other, vote_source) {
                return false;
            }

            if self.players[other].human && self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) {
                return false;
            }
        }

        if !self.can_do_resolution(vote_source, data) {
            return false;
        }

        true
    }

    /// C++ `CvGame::isVotePassed(eIndex)`: a team vote passed if its outcome is a civ team, other
    /// votes if the outcome is PLAYER_VOTE_YES.
    pub fn is_vote_passed(&self, vote: VoteType) -> bool {
        let player_vote = self.vote_outcome[vote];

        if self.is_team_vote(vote) {
            // C++ `ePlayerVote >= 0 && ePlayerVote < MAX_CIV_TEAMS`
            player_vote.team().is_some_and(|t| t.idx() < self.num_civ_teams())
        } else {
            player_vote == PlayerVote::PLAYER_VOTE_YES
        }
    }

    /// C++ `CvGame::setVoteOutcome(kData, eNewValue)`: `processVote` on a pass/fail change, then
    /// clears every alive player's vote on `data`.
    pub fn set_vote_outcome(&mut self, data: &VoteTriggeredData, new_value: PlayerVote) {
        let vote = data.vote_option.vote;

        if self.vote_outcome[vote] != new_value {
            let old_passed = self.is_vote_passed(vote);

            self.vote_outcome[vote] = new_value;

            if old_passed != self.is_vote_passed(vote) {
                let change = if self.is_vote_passed(vote) { 1 } else { -1 };
                self.process_vote(data, change);
            }
        }

        for p in self.civ_player_ids() {
            if self.players[p].alive {
                self.player_set_vote(p, data.id, PlayerVote::NO_PLAYER_VOTE);
            }
        }
    }

    /// C++ `CvGame::getPlayerVote(eOwnerIndex, iVoteId)` (stored on the player).
    pub fn player_vote(&self, owner: PlayerId, vote_id: VoteId) -> PlayerVote {
        debug_assert!(owner.idx() < self.num_civ_players());
        debug_assert!(self.vote_triggered(vote_id).is_some());

        self.player_get_vote(owner, vote_id)
    }

    /// C++ `CvGame::setPlayerVote(eOwnerIndex, iVoteId, eNewValue)`.
    pub fn set_player_vote(&mut self, owner: PlayerId, vote_id: VoteId, new_value: PlayerVote) {
        debug_assert!(owner.idx() < self.num_civ_players());
        debug_assert!(self.vote_triggered(vote_id).is_some());

        self.player_set_vote(owner, vote_id, new_value);
    }

    /// C++ `CvGame::castVote(eOwnerIndex, iVoteId, ePlayerVote)` (AI memory: voted for/against us).
    pub fn cast_vote(&mut self, owner: PlayerId, vote_id: VoteId, player_vote: PlayerVote) {
        let Some(vote_option) = self.vote_triggered(vote_id).map(|v| v.vote_option.clone()) else {
            return;
        };

        let owner_team = self.players[owner].team;

        if self.d.votes[vote_option.vote].assign_city {
            let city_player = vote_option.player.expect("castVote: assign city vote without a player");

            if owner_team != self.players[city_player].team {
                match player_vote {
                    PlayerVote::PLAYER_VOTE_YES => {
                        self.players[city_player].ai_memory_count[owner][MemoryType::MEMORY_VOTED_AGAINST_US] += 1;
                    }
                    PlayerVote::PLAYER_VOTE_NO => {
                        self.players[city_player].ai_memory_count[owner][MemoryType::MEMORY_VOTED_FOR_US] += 1;
                    }
                    _ => {}
                }
            }
        } else if self.is_team_vote(vote_option.vote) {
            if PlayerVote::for_team(owner_team) != player_vote {
                for p in self.civ_player_ids() {
                    if self.players[p].alive {
                        let team = self.players[p].team;
                        if team != owner_team && PlayerVote::for_team(team) == player_vote {
                            self.players[p].ai_memory_count[owner][MemoryType::MEMORY_VOTED_FOR_US] += 1;
                        }
                    }
                }
            }
        }

        self.set_player_vote(owner, vote_id, player_vote);
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
        let d = self.d;
        let info = &d.votes[data.vote_option.vote];

        self.change_trade_routes(info.trade_routes * change);
        self.change_free_trade_count(if info.free_trade { change } else { 0 });
        // changeNoNukesCount
        self.no_nukes_count += if info.no_nukes { change } else { 0 };
        debug_assert!(self.no_nukes_count >= 0);

        for civic in d.civics.keys() {
            self.change_force_civic_count(civic, if info.force_civic[civic] { change } else { 0 });
        }

        // FfH: Added by Kael 11/14/2007
        let b_change = change == 1;
        if let Some(bonus) = info.no_bonus {
            self.no_bonus[bonus] = b_change;
        }
        if info.gambling_ring {
            self.gambling_ring[data.vote_source] = b_change;
        }
        if info.no_outside_tech_trades {
            self.no_outside_tech_trades[data.vote_source] = b_change;
        }
        if info.slave_trade {
            self.slave_trade[data.vote_source] = b_change;
        }
        if info.smuggling_ring {
            self.smuggling_ring[data.vote_source] = b_change;
        }
        if info.free_units > 0 && b_change {
            for p in self.civ_player_ids() {
                if self.players[p].alive && self.player_is_full_member(p, data.vote_source) && info.cost < self.players[p].gold {
                    let civ = self
                        .player_get_civilization_type(p)
                        .expect("CvPlayer::getCivilizationType() is NO_CIVILIZATION");
                    // (FFH2 data always sets FreeUnitClass along with iFreeUnits.)
                    let free_unit = info.free_unit_class.and_then(|unit_class| d.civ_unit(civ, unit_class));
                    let city = self.player_get_capital_city(p);
                    if let (Some(city), Some(free_unit)) = (city, free_unit) {
                        // changeGold(-kVote.getCost())
                        self.players[p].gold += -info.cost;
                        let (x, y) = (self.cities[city].x, self.cities[city].y);
                        for _ in 0..info.free_units {
                            self.player_init_unit(p, free_unit, x, y, None, None);
                        }
                    }
                }
            }
        }
        if info.crime != 0 {
            // changeCrime. C++ quirk: also when the vote is reverted (iChange -1).
            self.crime += info.crime;
        }
        if !info.py_result.is_empty() {
            // Python CvSpellInterface.vote(eVote, 1). C++ quirk: the argument is always 1, and the
            // result also runs when the vote is reverted.
            self.py_vote(data.vote_option.vote, 1);
        }
        // FfH: End Add

        if change > 0 {
            // C++ loops these teams up to MAX_CIV_PLAYERS (= MAX_CIV_TEAMS).
            if info.open_borders {
                for t1 in self.civ_team_ids() {
                    if self.team_is_voting_member(t1, data.vote_source) {
                        for t2 in self.civ_team_ids().filter(|&t2| t2 > t1) {
                            if self.team_is_voting_member(t2, data.vote_source) {
                                self.team_sign_open_borders(t1, t2);
                            }
                        }
                    }
                }

                self.set_vote_outcome(data, PlayerVote::NO_PLAYER_VOTE);
            } else if info.defensive_pact {
                for t1 in self.civ_team_ids() {
                    if self.team_is_voting_member(t1, data.vote_source) {
                        for t2 in self.civ_team_ids().filter(|&t2| t2 > t1) {
                            if self.team_is_voting_member(t2, data.vote_source) {
                                self.team_sign_defensive_pact(t1, t2);
                            }
                        }
                    }
                }

                self.set_vote_outcome(data, PlayerVote::NO_PLAYER_VOTE);
            } else if info.force_peace {
                let vote_player = data.vote_option.player.expect("processVote: force peace without a player");
                let team = self.players[vote_player].team;

                for p in self.civ_player_ids() {
                    if self.players[p].team != team {
                        if self.player_is_voting_member(p, data.vote_source) {
                            self.player_force_peace(p, vote_player);
                        }
                    }
                }

                self.set_vote_outcome(data, PlayerVote::NO_PLAYER_VOTE);
            } else if info.force_no_trade {
                let vote_player = data.vote_option.player.expect("processVote: force no trade without a player");
                let team = self.players[vote_player].team;

                for p in self.civ_player_ids() {
                    if self.player_is_voting_member(p, data.vote_source) {
                        if self.player_can_stop_trading_with_team(p, team, false) {
                            self.player_stop_trading_with_team(p, team);
                        }
                    }
                }

                self.set_vote_outcome(data, PlayerVote::NO_PLAYER_VOTE);
            } else if info.force_war {
                let vote_player = data.vote_option.player.expect("processVote: force war without a player");
                let team = self.players[vote_player].team;

                for p in self.civ_player_ids() {
                    if self.player_is_voting_member(p, data.vote_source) {
                        let loop_team = self.players[p].team;
                        if self.team_can_change_war_peace(loop_team, team, false) {
                            self.team_declare_war(loop_team, team, false, Some(WarPlanType::WARPLAN_DOGPILE));
                        }
                    }
                }

                self.set_vote_outcome(data, PlayerVote::NO_PLAYER_VOTE);
            } else if info.assign_city {
                let vote_player = data.vote_option.player.expect("processVote: assign city without a player");
                // (C++ FAsserts that the city exists: it may have been lost since the vote.)
                let city = player_city(self, vote_player, data.vote_option.city);

                if let Some(city) = city {
                    if let Some(other) = data.vote_option.other_player {
                        if other != self.cities[city].owner {
                            self.player_acquire_city(other, city, false, true, true);
                        }
                    }
                }

                self.set_vote_outcome(data, PlayerVote::NO_PLAYER_VOTE);
            }
        }
    }

    /// C++ `CvGame::getVoteSelection(iID)`.
    pub fn vote_selection(&self, id: VoteId) -> Option<&VoteSelectionData> {
        self.vote_selections.iter().find(|v| v.id == id)
    }

    /// C++ `CvGame::addVoteSelection(eVoteSource)`: builds the electable options; `None` (and
    /// nothing stored) when there are none.
    pub fn add_vote_selection(&mut self, vote_source: VoteSourceType) -> Option<VoteId> {
        let d = self.d;

        // m_voteSelections.add() (the slot is taken and released again when there is no option)
        let live: Vec<VoteId> = self.vote_selections.iter().map(|v| v.id).collect();
        let id = flta_add(&mut self.next_vote_selection_id, &live);

        let mut vote_options = Vec::new();

        for vote in d.votes.keys() {
            if d.votes[vote].vote_source_type[vote_source] {
                if self.is_choose_election(vote) {
                    let info = &d.votes[vote];
                    let mut data = VoteSelectionSubData {
                        vote,
                        player: None,
                        city: None,
                        other_player: None,
                        text: String::new(),
                    };

                    if info.open_borders {
                        if self.is_valid_vote_selection(vote_source, &data) {
                            data.text = format!("Open borders between all members{}", votes_needed_text(self, vote, vote_source));
                            vote_options.push(data.clone());
                        }
                    } else if info.defensive_pact {
                        if self.is_valid_vote_selection(vote_source, &data) {
                            data.text = format!("Defensive pact between all members{}", votes_needed_text(self, vote, vote_source));
                            vote_options.push(data.clone());
                        }
                    } else if info.force_peace || info.force_no_trade || info.force_war {
                        // (three C++ branches, identical but for the text)
                        for t1 in self.civ_team_ids() {
                            if self.team_is_alive(t1) {
                                data.player = self.team_get_leader_id(t1);

                                if self.is_valid_vote_selection(vote_source, &data) {
                                    let what = if info.force_peace {
                                        "Force peace with"
                                    } else if info.force_no_trade {
                                        "Stop trading with"
                                    } else {
                                        "Declare war on"
                                    };
                                    data.text = format!("{} {}{}", what, self.team_get_name(t1), votes_needed_text(self, vote, vote_source));
                                    vote_options.push(data.clone());
                                }
                            }
                        }
                    } else if info.assign_city {
                        for p1 in self.civ_player_ids() {
                            for &c in &self.players[p1].cities {
                                let new_owner = self.plot_find_highest_culture_player(self.city_plot(c));
                                if let Some(new_owner) = new_owner {
                                    data.player = Some(p1);
                                    data.city = Some(c);
                                    data.other_player = Some(new_owner);

                                    if self.is_valid_vote_selection(vote_source, &data) {
                                        data.text = format!(
                                            "Assign the {} city of {} to {}{}",
                                            self.player_get_civilization_adjective(p1),
                                            self.cities[c].name,
                                            self.player_get_name(new_owner),
                                            votes_needed_text(self, vote, vote_source)
                                        );
                                        vote_options.push(data.clone());
                                    }
                                }
                            }
                        }
                    } else {
                        data.text = format!("{}{}", d.text.name(&info.base.description), votes_needed_text(self, vote, vote_source));
                        if self.is_vote_passed(vote) {
                            data.text += " (passed)";
                        }

                        if self.can_do_resolution(vote_source, &data) {
                            vote_options.push(data.clone());
                        }
                    }
                }
            }
        }

        if vote_options.is_empty() {
            // deleteVoteSelection(pData->getID())
            return None;
        }

        let pos = flta_insert_pos(self.vote_selections.iter().map(|v| v.id), id);
        self.vote_selections.insert(pos, VoteSelectionData { id, vote_source, vote_options });
        Some(id)
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
    /// the members' votes (`AI_diploVote`, or the human's `BUTTONPOPUP_DIPLOVOTE` answer ->
    /// controller).
    pub fn add_vote_triggered(&mut self, vote_source: VoteSourceType, option: &VoteSelectionSubData) -> Option<VoteId> {
        // m_votesTriggered.add()
        let live: Vec<VoteId> = self.votes_triggered.iter().map(|v| v.id).collect();
        let id = flta_add(&mut self.next_vote_triggered_id, &live);
        let pos = flta_insert_pos(self.votes_triggered.iter().map(|v| v.id), id);
        self.votes_triggered.insert(
            pos,
            VoteTriggeredData {
                id,
                vote_source,
                vote_option: option.clone(),
            },
        );

        for p in self.civ_player_ids() {
            if self.player_is_voting_member(p, vote_source) {
                // Humans: BUTTONPOPUP_DIPLOVOTE, whose answer comes back as castVote; AI:
                // castVote(AI_diploVote(kOptionData, eVoteSource, false)). Both: the controller.
                let player_vote = self.controller(p).diplo_vote(self, p, option, vote_source);
                self.cast_vote(p, id, player_vote);
            }
        }

        Some(id)
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
        let d = self.d;

        // m_votesTriggered.beginIter / nextIter: slot order (the Vec order). Nothing in the loop
        // adds a triggered vote, and only the current one is deleted.
        let ids: Vec<VoteId> = self.votes_triggered.iter().map(|v| v.id).collect();
        for id in ids {
            let Some(data) = self.vote_triggered(id).cloned() else {
                continue;
            };
            let vote = data.vote_option.vote;
            let vote_source = data.vote_source;
            let mut passed = false;

            if !self.can_do_resolution(vote_source, &data.vote_option) {
                // C++: "TXT_KEY_ELECTION_CANCELLED: <vote>" to every voting member (UI).
                if self.log.enabled {
                    let turn = self.game_turn();
                    for p in self.civ_player_ids() {
                        if self.player_is_voting_member(p, vote_source) {
                            self.log.msg(turn, Some(p), || {
                                format!("Election cancelled: {}", d.text.name(&d.votes[vote].base.description))
                            });
                        }
                    }
                }
            } else {
                let mut all_voted = true;
                for p in self.civ_player_ids() {
                    if self.player_is_voting_member(p, vote_source) {
                        if self.player_vote(p, id) == PlayerVote::NO_PLAYER_VOTE {
                            //give player one more turn to submit vote
                            self.set_player_vote(p, id, PlayerVote::NO_PLAYER_VOTE_CHECKED);
                            all_voted = false;
                            break;
                        } else if self.player_vote(p, id) == PlayerVote::NO_PLAYER_VOTE_CHECKED {
                            //default player vote to abstain
                            self.set_player_vote(p, id, PlayerVote::PLAYER_VOTE_ABSTAIN);
                        }
                    }
                }

                if !all_voted {
                    // (the vote stays for one more turn; the secretary general timer is kept)
                    continue;
                }

                if self.is_team_vote(vote) {
                    let team = self.find_highest_vote_team(&data);

                    if let Some(team) = team {
                        passed = self.count_vote(&data, PlayerVote::for_team(team)) >= self.vote_required(vote, vote_source);
                    }

                    // C++ builds the result text (TXT_KEY_POPUP_DIPLOMATIC_VOTING_VICTORY, the
                    // members' votes and abstentions) for the human members' popups: UI.

                    match team {
                        Some(team) if passed => self.set_vote_outcome(&data, PlayerVote::for_team(team)),
                        _ => self.set_vote_outcome(&data, PlayerVote::PLAYER_VOTE_ABSTAIN),
                    }
                } else {
                    passed = self.count_vote(&data, PlayerVote::PLAYER_VOTE_YES) >= self.vote_required(vote, vote_source);

                    // Defying resolution
                    if passed {
                        for p in self.civ_player_ids() {
                            if self.player_vote(p, id) == PlayerVote::PLAYER_VOTE_NEVER {
                                passed = false;

                                self.player_set_defied_resolution(p, vote_source, &data.vote_option);
                            }
                        }
                    }

                    if passed {
                        for p in self.civ_player_ids() {
                            if self.player_is_voting_member(p, vote_source) {
                                if self.player_vote(p, id) == PlayerVote::PLAYER_VOTE_YES {
                                    self.player_set_endorsed_resolution(p, vote_source, &data.vote_option);
                                }
                            }
                        }
                    }

                    // C++ builds the result text (TXT_KEY_POPUP_DIPLOMATIC_VOTING_SUCCEEDS /
                    // _FAILURE and every member's vote) for the human members' popups: UI.

                    self.set_vote_outcome(&data, if passed { PlayerVote::PLAYER_VOTE_YES } else { PlayerVote::PLAYER_VOTE_NO });
                }

                // C++: result popup to the human voting members and, when passed,
                // TXT_KEY_VOTE_RESULTS to the humans concerned (UI). Logged once.
                let turn = self.game_turn();
                self.log.msg(turn, None, || {
                    format!(
                        "{}: {} {}",
                        d.text.name(&d.vote_sources[vote_source].base.description),
                        data.vote_option.text,
                        if passed { "passed" } else { "failed" }
                    )
                });
            }

            if !passed && d.votes[vote].secretary_general {
                self.secretary_general_timer[vote_source] = 0;
            }

            self.delete_vote_triggered(id);
        }
    }

    /// C++ `CvGame::doVoteSelection`: vote timers, members meet, secretary general elections,
    /// resolution choice by the secretary (`AI_chooseElection` -> controller).
    pub fn do_vote_selection(&mut self) {
        let d = self.d;

        for vote_source in d.vote_sources.keys() {
            if self.is_diplo_vote(vote_source) {
                if self.vote_timer[vote_source] > 0 {
                    self.vote_timer[vote_source] -= 1;
                } else {
                    self.vote_timer[vote_source] =
                        (d.vote_sources[vote_source].vote_interval * d.game_speeds[self.game_speed_type()].victory_delay_percent) / 100;

                    for t1 in self.civ_team_ids() {
                        if self.team_is_alive(t1) && self.team_is_voting_member(t1, vote_source) {
                            for t2 in self.civ_team_ids().filter(|&t2| t2 > t1) {
                                if self.team_is_alive(t2) && self.team_is_voting_member(t2, vote_source) {
                                    self.team_meet(t1, t2, true);
                                }
                            }
                        }
                    }

                    let secretary_general = self.secretary_general(vote_source);
                    let secretary_player = match secretary_general {
                        Some(team) => self.team_get_secretary_id(team),
                        None => None,
                    };

                    let mut secretary_general_vote = false;
                    if self.can_have_secretary_general(vote_source) {
                        if self.secretary_general_timer[vote_source] > 0 {
                            self.secretary_general_timer[vote_source] -= 1;
                        } else {
                            self.secretary_general_timer[vote_source] = d.defines.diplo_vote_secretary_general_interval;

                            for vote in d.votes.keys() {
                                if d.votes[vote].secretary_general && d.votes[vote].vote_source_type[vote_source] {
                                    // C++ quirk: kOptionData.eOtherPlayer is left uninitialized
                                    // (unused for secretary general votes).
                                    let option = VoteSelectionSubData {
                                        vote,
                                        player: None,
                                        city: None,
                                        other_player: None,
                                        text: format!("{}{}", d.text.name(&d.votes[vote].base.description), votes_needed_text(self, vote, vote_source)),
                                    };
                                    self.add_vote_triggered(vote_source, &option);
                                    secretary_general_vote = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !secretary_general_vote && secretary_general.is_some() {
                        if let Some(secretary_player) = secretary_player {
                            if let Some(selection_id) = self.add_vote_selection(vote_source) {
                                // Human secretary: BUTTONPOPUP_CHOOSEELECTION, answered with
                                // CvNetChooseElection (setVoteChosen); AI:
                                // setVoteChosen(GET_TEAM(eSecretaryGeneral).AI_chooseElection(..)).
                                // Both: the secretary player's controller. A human's `None` is the
                                // popup not answered yet: the selection stays until the human's
                                // answer (`set_vote_chosen`; the popup's "none" sends nothing).
                                let selection = self.vote_selection(selection_id).cloned().expect("vote selection just added");
                                let choice = self.controller(secretary_player).choose_election(self, secretary_player, &selection);
                                if choice.is_some() || !self.players[secretary_player].human {
                                    self.set_vote_chosen(choice, selection_id);
                                }
                            } else {
                                self.vote_timer[vote_source] = 0;
                            }
                        }
                    }
                }
            }
        }
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
    use crate::ids::*;
    use crate::setup::GameSetup;
    use crate::structs::*;
    use ffh_data::*;

    #[test]
    fn vote_containers() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3));
        let source = VoteSourceType(0);
        let option = VoteSelectionSubData {
            vote: VoteType(0),
            player: None,
            city: None,
            other_player: None,
            text: String::new(),
        };
        g.vote_selections.push(VoteSelectionData {
            id: VoteId(7),
            vote_source: source,
            vote_options: vec![option.clone()],
        });
        assert!(g.vote_selection(VoteId(7)).is_some());
        assert!(g.vote_selection(VoteId(8)).is_none());
        // no choice: nothing is triggered, the selection is deleted
        g.set_vote_chosen(None, VoteId(7));
        assert!(g.vote_selections.is_empty());
        assert!(g.votes_triggered.is_empty());

        g.votes_triggered.push(VoteTriggeredData {
            id: VoteId(3),
            vote_source: source,
            vote_option: option,
        });
        assert!(g.vote_triggered(VoteId(3)).is_some());
        g.delete_vote_triggered(VoteId(3));
        assert!(g.vote_triggered(VoteId(3)).is_none());

        g.set_vote_source_religion(source, Some(ReligionType(1)), false);
        assert_eq!(g.vote_source_religions[source], Some(ReligionType(1)));
        g.set_vote_source_religion(source, None, true);
        assert_eq!(g.vote_source_religions[source], None);
    }

    #[test]
    fn free_list_ids_and_slots() {
        use super::{flta_add, flta_insert_pos, flta_slot};
        const B: u32 = 1 << 13;

        // a fresh array: slots 0, 1 with m_iCurrentID 8192, 16384
        let mut next = 0;
        let a = flta_add(&mut next, &[]);
        assert_eq!(a, VoteId(B));
        let b = flta_add(&mut next, &[a]);
        assert_eq!(b, VoteId(2 * B + 1));
        // both freed (slot 0, then slot 1): LIFO, slot 1 is reused first
        let c = flta_add(&mut next, &[]);
        assert_eq!(c, VoteId(3 * B + 1));
        let e = flta_add(&mut next, &[c]);
        assert_eq!(e, VoteId(4 * B));
        // no free slot: the array grows
        let f = flta_add(&mut next, &[c, e]);
        assert_eq!((f, flta_slot(f)), (VoteId(5 * B + 2), 2));

        // containers are kept in slot order (the C++ iteration order)
        assert_eq!(flta_insert_pos([e, f].into_iter(), c), 1);
        assert_eq!(flta_insert_pos([c, f].into_iter(), e), 0);
        assert_eq!(flta_insert_pos([e, c].into_iter(), f), 2);
    }

    fn game_with_players() -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 3, "WORLDSIZE_DUEL", 3));
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = crate::player::Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        for i in 0..3 {
            g.players[PlayerId(i)].alive = true;
        }
        g
    }

    fn triggered(id: u32, vote_source: VoteSourceType, vote: VoteType) -> VoteTriggeredData {
        VoteTriggeredData { id: VoteId(id), vote_source, vote_option: VoteSelectionSubData { vote, player: None, city: None, other_player: None, text: String::new() } }
    }

    #[test]
    fn secretary_general_outcome() {
        let mut g = game_with_players();
        let d = g.d;
        let over = d.t::<VoteSourceType>("DIPLOVOTE_OVERCOUNCIL");
        let under = d.t::<VoteSourceType>("DIPLOVOTE_UNDERCOUNCIL");
        let sg = d.t::<VoteType>("VOTE_SECRETARY_GENERAL");
        let gambling = d.t::<VoteType>("VOTE_GAMBLING_RING");

        assert!(g.is_team_vote(sg) && !g.is_choose_election(sg));
        assert!(!g.is_team_vote(gambling) && g.is_choose_election(gambling));
        assert!(g.can_have_secretary_general(over) && g.can_have_secretary_general(under));

        let data = triggered(8192, over, sg);
        g.votes_triggered.push(data.clone());
        g.set_player_vote(PlayerId(0), data.id, PlayerVote::for_team(TeamId(1)));
        assert_eq!(g.secretary_general(over), None);

        // team 1 elected: a team vote passes with a civ team as outcome; votes are cleared
        g.set_vote_outcome(&data, PlayerVote::for_team(TeamId(1)));
        assert!(g.is_vote_passed(sg));
        assert_eq!(g.secretary_general(over), Some(TeamId(1)));
        assert_eq!(g.secretary_general(under), None);
        assert_eq!(g.player_vote(PlayerId(0), data.id), PlayerVote::NO_PLAYER_VOTE);

        g.secretary_general_timer[over] = 4;
        g.clear_secretary_general(over);
        assert_eq!(g.vote_outcome[sg], PlayerVote::NO_PLAYER_VOTE);
        assert!(!g.is_vote_passed(sg));
        assert_eq!(g.secretary_general(over), None);
        assert_eq!(g.secretary_general_timer[over], 0);

        // abstention or the barbarian team elect nobody
        g.set_vote_outcome(&data, PlayerVote::PLAYER_VOTE_ABSTAIN);
        assert!(!g.is_vote_passed(sg));
        g.set_vote_outcome(&data, PlayerVote::for_team(g.barbarian_team()));
        assert!(!g.is_vote_passed(sg));
    }

    #[test]
    fn ffh_resolution_effects() {
        let mut g = game_with_players();
        let d = g.d;
        let under = d.t::<VoteSourceType>("DIPLOVOTE_UNDERCOUNCIL");
        let over = d.t::<VoteSourceType>("DIPLOVOTE_OVERCOUNCIL");

        let gambling = triggered(8192, under, d.t::<VoteType>("VOTE_GAMBLING_RING"));
        let crime = g.crime;
        g.set_vote_outcome(&gambling, PlayerVote::PLAYER_VOTE_YES);
        assert!(g.is_vote_passed(gambling.vote_option.vote));
        assert!(g.gambling_ring[under] && !g.gambling_ring[over]);
        assert_eq!(g.crime, crime + 5);
        g.set_vote_outcome(&gambling, PlayerVote::PLAYER_VOTE_NO);
        assert!(!g.gambling_ring[under]);
        // C++ quirk: repealing adds the crime again
        assert_eq!(g.crime, crime + 10);
        // a failed vote that never passed changes nothing
        g.set_vote_outcome(&gambling, PlayerVote::PLAYER_VOTE_ABSTAIN);
        assert_eq!(g.crime, crime + 10);

        let no_death_mana = triggered(16384, over, d.t::<VoteType>("VOTE_NO_DEATH_MANA"));
        let death_mana = d.t::<BonusType>("BONUS_MANA_DEATH");
        g.set_vote_outcome(&no_death_mana, PlayerVote::PLAYER_VOTE_YES);
        assert!(g.no_bonus[death_mana]);
        g.set_vote_outcome(&no_death_mana, PlayerVote::PLAYER_VOTE_NO);
        assert!(!g.no_bonus[death_mana]);
    }

    #[test]
    fn cast_vote_memories() {
        let mut g = game_with_players();
        let d = g.d;
        let over = d.t::<VoteSourceType>("DIPLOVOTE_OVERCOUNCIL");
        let (p0, p1, p2) = (PlayerId(0), PlayerId(1), PlayerId(2));

        // team vote: voting for another team is remembered by that team's players
        let sg = triggered(8192, over, d.t::<VoteType>("VOTE_SECRETARY_GENERAL"));
        g.votes_triggered.push(sg.clone());
        g.cast_vote(p0, sg.id, PlayerVote::for_team(TeamId(1)));
        assert_eq!(g.player_vote(p0, sg.id), PlayerVote::for_team(TeamId(1)));
        assert_eq!(g.players[p1].ai_memory_count[p0][MemoryType::MEMORY_VOTED_FOR_US], 1);
        assert_eq!(g.players[p2].ai_memory_count[p0][MemoryType::MEMORY_VOTED_FOR_US], 0);
        g.cast_vote(p1, sg.id, PlayerVote::for_team(TeamId(1)));
        assert_eq!(g.players[p1].ai_memory_count[p1][MemoryType::MEMORY_VOTED_FOR_US], 0);

        // assign city: the city's owner remembers yes (against us) and no (for us) votes
        let mut assign = triggered(16385, over, d.t::<VoteType>("VOTE_ASSIGN_CITY"));
        assign.vote_option.player = Some(p2);
        g.votes_triggered.push(assign.clone());
        g.cast_vote(p0, assign.id, PlayerVote::PLAYER_VOTE_YES);
        g.cast_vote(p1, assign.id, PlayerVote::PLAYER_VOTE_NO);
        g.cast_vote(p2, assign.id, PlayerVote::PLAYER_VOTE_NO);
        assert_eq!(g.players[p2].ai_memory_count[p0][MemoryType::MEMORY_VOTED_AGAINST_US], 1);
        assert_eq!(g.players[p2].ai_memory_count[p1][MemoryType::MEMORY_VOTED_FOR_US], 1);
        assert_eq!(g.players[p2].ai_memory_count[p2][MemoryType::MEMORY_VOTED_FOR_US], 0);
        assert_eq!(g.player_vote(p1, assign.id), PlayerVote::PLAYER_VOTE_NO);

        // no such vote: nothing happens
        g.cast_vote(p0, VoteId(5), PlayerVote::PLAYER_VOTE_YES);
        assert!(g.players[p0].votes.iter().all(|&(id, _)| id != VoteId(5)));

        // an empty container: no results to tally
        g.votes_triggered.clear();
        g.do_vote_results();
        assert!(g.votes_triggered.is_empty());
    }

    /// A non-negative `PlayerVoteTypes` is a team (C++ `(TeamTypes)ePlayerVote`), not a player:
    /// with player and team ids apart, a team vote reaches the players of that team.
    #[test]
    fn team_votes_name_teams_not_players() {
        let mut g = game_with_players();
        let d = g.d;
        let over = d.t::<VoteSourceType>("DIPLOVOTE_OVERCOUNCIL");
        let (p0, p1, p2) = (PlayerId(0), PlayerId(1), PlayerId(2));
        // Player 1 plays for team 2 and player 2 for team 1.
        g.players[p1].team = TeamId(2);
        g.players[p2].team = TeamId(1);

        assert_eq!(PlayerVote::for_team(TeamId(1)).team(), Some(TeamId(1)));
        for sentinel in [PlayerVote::NO_PLAYER_VOTE, PlayerVote::PLAYER_VOTE_YES, PlayerVote::PLAYER_VOTE_NO, PlayerVote::PLAYER_VOTE_ABSTAIN, PlayerVote::PLAYER_VOTE_NEVER, PlayerVote::NO_PLAYER_VOTE_CHECKED] {
            assert_eq!(sentinel.team(), None);
        }

        let sg = triggered(8192, over, d.t::<VoteType>("VOTE_SECRETARY_GENERAL"));
        g.votes_triggered.push(sg.clone());
        g.cast_vote(p0, sg.id, PlayerVote::for_team(TeamId(1)));
        // Team 1's player (2) remembers the vote, player 1 does not.
        assert_eq!(g.players[p2].ai_memory_count[p0][MemoryType::MEMORY_VOTED_FOR_US], 1);
        assert_eq!(g.players[p1].ai_memory_count[p0][MemoryType::MEMORY_VOTED_FOR_US], 0);
        // A vote for one's own team is not remembered.
        g.cast_vote(p2, sg.id, PlayerVote::for_team(TeamId(1)));
        assert_eq!(g.players[p2].ai_memory_count[p2][MemoryType::MEMORY_VOTED_FOR_US], 0);

        // The elected secretary general is the team of the outcome.
        g.set_vote_outcome(&sg, PlayerVote::for_team(TeamId(2)));
        assert!(g.is_vote_passed(sg.vote_option.vote));
        assert_eq!(g.secretary_general(over), Some(TeamId(2)));
        assert_eq!(g.players[p1].team, TeamId(2));
    }
}
