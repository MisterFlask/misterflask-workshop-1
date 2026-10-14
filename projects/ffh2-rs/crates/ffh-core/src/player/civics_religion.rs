//! CvPlayer: civics (revolution, anarchy, upkeep, civic effects), state religion and
//! conversion, holy cities and religion founding, corporations, vote sources (resolutions,
//! membership, votes), and FFH2 leader traits (`setHasTrait`) and hidden civics/religions.

#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------ civics

    /// C++ `CvPlayer::isCivic(eCivic)`.
    pub fn player_is_civic(&self, p: PlayerId, civic: CivicType) -> bool {
        self.players[p].civics.iter().any(|&c| c == Some(civic))
    }

    /// C++ `CvPlayer::canDoCivics(eCivic)` (true for NO_CIVIC; forced civics, Python
    /// `canDoCivic`/`cannotDoCivic`, FFH prereqs: war, alignment, civilization, religion).
    pub fn player_can_do_civics(&self, p: PlayerId, civic: Option<CivicType>) -> bool {
        todo!("CvPlayer::canDoCivics")
    }

    /// C++ `CvPlayer::canRevolution(paeNewCivics)` (None = any civic change possible).
    pub fn player_can_revolution(&self, p: PlayerId, new_civics: Option<&IdxVec<CivicOptionType, Option<CivicType>>>) -> bool {
        todo!("CvPlayer::canRevolution")
    }

    /// C++ `CvPlayer::revolution(paeNewCivics, bForce = false)`.
    pub fn player_revolution(&mut self, p: PlayerId, new_civics: &IdxVec<CivicOptionType, Option<CivicType>>, force: bool) {
        todo!("CvPlayer::revolution")
    }

    /// C++ `CvPlayer::verifyCivics`: replaces civics that can no longer be run.
    pub fn player_verify_civics(&mut self, p: PlayerId) {
        todo!("CvPlayer::verifyCivics")
    }

    /// C++ `CvPlayer::setCivics(eIndex, eNewValue)`: processes the old and new civic.
    pub fn player_set_civics(&mut self, p: PlayerId, civic_option: CivicOptionType, civic: Option<CivicType>) {
        todo!("CvPlayer::setCivics")
    }

    /// C++ `CvPlayer::processCivics(eCivic, iChange)`: applies (+1) or removes (-1) a civic's effects.
    pub fn player_process_civics(&mut self, p: PlayerId, civic: CivicType, change: i32) {
        todo!("CvPlayer::processCivics")
    }

    /// C++ `CvPlayer::getCivicPercentAnger(eCivic, bIgnore = false)`.
    pub fn player_get_civic_percent_anger(&self, p: PlayerId, civic: CivicType, ignore: bool) -> i32 {
        todo!("CvPlayer::getCivicPercentAnger")
    }

    /// C++ `CvPlayer::getCivicAnarchyLength(paeNewCivics)`.
    pub fn player_get_civic_anarchy_length(&self, p: PlayerId, new_civics: &IdxVec<CivicOptionType, Option<CivicType>>) -> i32 {
        todo!("CvPlayer::getCivicAnarchyLength")
    }

    /// C++ `CvPlayer::getSingleCivicUpkeep(eCivic, bIgnoreAnarchy = false)`.
    pub fn player_get_single_civic_upkeep(&self, p: PlayerId, civic: Option<CivicType>, ignore_anarchy: bool) -> i32 {
        todo!("CvPlayer::getSingleCivicUpkeep")
    }

    /// C++ `CvPlayer::getCivicUpkeep(paeCivics = NULL, bIgnoreAnarchy = false)` (None = current civics).
    pub fn player_get_civic_upkeep(&self, p: PlayerId, civics: Option<&IdxVec<CivicOptionType, Option<CivicType>>>, ignore_anarchy: bool) -> i32 {
        let civics = civics.unwrap_or(&self.players[p].civics);
        let mut total_upkeep = 0;
        for civic_option in self.d.civic_options.keys() {
            total_upkeep += self.player_get_single_civic_upkeep(p, civics[civic_option], ignore_anarchy);
        }
        total_upkeep
    }

    /// C++ `CvPlayer::isHasCivicOption`.
    pub fn player_is_has_civic_option(&self, p: PlayerId, civic_option: CivicOptionType) -> bool {
        self.players[p].has_civic_option_count[civic_option] > 0
    }

    /// C++ `CvPlayer::isNoCivicUpkeep`.
    pub fn player_is_no_civic_upkeep(&self, p: PlayerId, civic_option: CivicOptionType) -> bool {
        self.players[p].no_civic_upkeep_count[civic_option] > 0
    }

    /// C++ `CvPlayer::canSeeCivic(iCivic)` (FFH: hidden civics are only visible to their holder).
    pub fn player_can_see_civic(&self, p: PlayerId, civic: CivicType) -> bool {
        todo!("CvPlayer::canSeeCivic")
    }

    // ------------------------------------------------------------------ anarchy

    /// C++ `CvPlayer::isAnarchy`.
    pub fn player_is_anarchy(&self, p: PlayerId) -> bool {
        self.players[p].anarchy_turns > 0
    }

    /// C++ `CvPlayer::changeAnarchyTurns`: commerce, maintenance, trade routes and
    /// corporations are recomputed when anarchy starts or ends.
    pub fn player_change_anarchy_turns(&mut self, p: PlayerId, change: i32) {
        todo!("CvPlayer::changeAnarchyTurns")
    }

    /// C++ `CvPlayer::updateMaxAnarchyTurns` (MAX_ANARCHY_TURNS lowered by traits).
    pub fn player_update_max_anarchy_turns(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateMaxAnarchyTurns")
    }

    /// C++ `CvPlayer::changeAnarchyModifier`: also rescales the running revolution and
    /// conversion timers.
    pub fn player_change_anarchy_modifier(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            let pl = &mut self.players[p];
            pl.anarchy_modifier += change;
            pl.revolution_timer = 0.max(((100 + change) * pl.revolution_timer) / 100);
            pl.conversion_timer = 0.max(((100 + change) * pl.conversion_timer) / 100);
        }
    }

    /// C++ `CvPlayer::getReligionAnarchyLength`.
    pub fn player_get_religion_anarchy_length(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::getReligionAnarchyLength")
    }

    // ------------------------------------------------------------------ state religion

    /// C++ `CvPlayer::isStateReligion`.
    pub fn player_is_state_religion(&self, p: PlayerId) -> bool {
        self.players[p].state_religion_count > 0
    }

    /// C++ `CvPlayer::getStateReligion`.
    pub fn player_get_state_religion(&self, p: PlayerId) -> Option<ReligionType> {
        if self.player_is_state_religion(p) {
            self.players[p].last_state_religion
        } else {
            None
        }
    }

    /// C++ `CvPlayer::changeStateReligionCount`: maintenance, religion happiness/commerce and
    /// the game's citizen assignment are updated.
    pub fn player_change_state_religion_count(&mut self, p: PlayerId, change: i32) {
        todo!("CvPlayer::changeStateReligionCount")
    }

    /// C++ `CvPlayer::setLastStateReligion`: city sight, maintenance, religion happiness and
    /// commerce, secretary general, messages and the Python `playerChangeStateReligion` event.
    pub fn player_set_last_state_religion(&mut self, p: PlayerId, religion: Option<ReligionType>) {
        todo!("CvPlayer::setLastStateReligion")
    }

    /// C++ `CvPlayer::isNoNonStateReligionSpread`.
    pub fn player_is_no_non_state_religion_spread(&self, p: PlayerId) -> bool {
        self.players[p].no_non_state_religion_spread_count > 0
    }

    /// C++ `CvPlayer::changeStateReligionHappiness`.
    pub fn player_change_state_religion_happiness(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].state_religion_happiness += change;
            self.player_update_religion_happiness(p);
        }
    }

    /// C++ `CvPlayer::changeNonStateReligionHappiness`.
    pub fn player_change_non_state_religion_happiness(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].non_state_religion_happiness += change;
            self.player_update_religion_happiness(p);
        }
    }

    /// C++ `CvPlayer::canDoReligion(eReligion)` (FFH: agnostics and leader religion weights).
    pub fn player_can_do_religion(&self, p: PlayerId, religion: ReligionType) -> bool {
        todo!("CvPlayer::canDoReligion")
    }

    /// C++ `CvPlayer::canChangeReligion`.
    pub fn player_can_change_religion(&self, p: PlayerId) -> bool {
        for religion in self.d.religions.keys() {
            if self.player_can_convert(p, Some(religion)) {
                return true;
            }
        }
        false
    }

    /// C++ `CvPlayer::canConvert(eReligion)` (None = drop the state religion).
    pub fn player_can_convert(&self, p: PlayerId, religion: Option<ReligionType>) -> bool {
        todo!("CvPlayer::canConvert")
    }

    /// C++ `CvPlayer::convert(eReligion)` (FFH: alignment pulled into the religion's range).
    pub fn player_convert(&mut self, p: PlayerId, religion: Option<ReligionType>) {
        todo!("CvPlayer::convert")
    }

    /// C++ `CvPlayer::canSeeReligion(iReligion, pCity)` (FFH: hidden religions).
    pub fn player_can_see_religion(&self, p: PlayerId, religion: ReligionType, city: Option<CityId>) -> bool {
        todo!("CvPlayer::canSeeReligion")
    }

    /// C++ `CvPlayer::AI_doFavoriteReligion` (BETTER AI, called from `doTurn` for AI players):
    /// picks a favorite religion (game RNG) and converts to it when possible.
    pub fn player_ai_do_favorite_religion(&mut self, p: PlayerId) {
        todo!("CvPlayer::AI_doFavoriteReligion")
    }

    // ------------------------------------------------------------------ religions in the empire

    /// C++ `CvPlayer::hasHolyCity(eReligion)`.
    pub fn player_has_holy_city(&self, p: PlayerId, religion: ReligionType) -> bool {
        todo!("CvPlayer::hasHolyCity")
    }

    /// C++ `CvPlayer::countHolyCities`.
    pub fn player_count_holy_cities(&self, p: PlayerId) -> i32 {
        let mut count = 0;
        for religion in self.d.religions.keys() {
            if self.player_has_holy_city(p, religion) {
                count += 1;
            }
        }
        count
    }

    /// C++ `CvPlayer::foundReligion(eReligion, eSlotReligion, bAward)` (no-op for NO_RELIGION,
    /// disabled or already founded religions).
    pub fn player_found_religion(&mut self, p: PlayerId, religion: Option<ReligionType>, slot_religion: ReligionType, award: bool) {
        todo!("CvPlayer::foundReligion")
    }

    /// C++ `CvPlayer::hasShrine(eReligion)`.
    pub fn player_has_shrine(&self, p: PlayerId, religion: Option<ReligionType>) -> bool {
        todo!("CvPlayer::hasShrine")
    }

    /// C++ `CvPlayer::countTotalHasReligion`.
    pub fn player_count_total_has_religion(&self, p: PlayerId) -> i32 {
        self.players[p].has_religion_count.iter().sum()
    }

    /// C++ `CvPlayer::findHighestHasReligionCount`.
    pub fn player_find_highest_has_religion_count(&self, p: PlayerId) -> i32 {
        let mut best_value = 0;
        for &value in self.players[p].has_religion_count.iter() {
            if value > best_value {
                best_value = value;
            }
        }
        best_value
    }

    /// C++ `CvPlayer::changeHasReligionCount` (updates building commerce and citizen
    /// assignment game-wide).
    pub fn player_change_has_religion_count(&mut self, p: PlayerId, religion: ReligionType, change: i32) {
        todo!("CvPlayer::changeHasReligionCount")
    }

    /// C++ `CvPlayer::getReligionPopulation(eReligion)`.
    pub fn player_get_religion_population(&self, p: PlayerId, religion: ReligionType) -> i32 {
        todo!("CvPlayer::getReligionPopulation")
    }

    // ------------------------------------------------------------------ corporations

    /// C++ `CvPlayer::hasHeadquarters(eCorporation)`.
    pub fn player_has_headquarters(&self, p: PlayerId, corporation: CorporationType) -> bool {
        todo!("CvPlayer::hasHeadquarters")
    }

    /// C++ `CvPlayer::countHeadquarters`.
    pub fn player_count_headquarters(&self, p: PlayerId) -> i32 {
        let mut count = 0;
        for corporation in self.d.corporations.keys() {
            if self.player_has_headquarters(p, corporation) {
                count += 1;
            }
        }
        count
    }

    /// C++ `CvPlayer::countCorporations(eCorporation)` (cities with the corporation).
    pub fn player_count_corporations(&self, p: PlayerId, corporation: CorporationType) -> i32 {
        todo!("CvPlayer::countCorporations")
    }

    /// C++ `CvPlayer::foundCorporation(eCorporation)`.
    pub fn player_found_corporation(&mut self, p: PlayerId, corporation: CorporationType) {
        todo!("CvPlayer::foundCorporation")
    }

    /// C++ `CvPlayer::isActiveCorporation(eIndex)`.
    pub fn player_is_active_corporation(&self, p: PlayerId, corporation: CorporationType) -> bool {
        if self.player_is_no_corporations(p) {
            return false;
        }

        if self.player_is_no_foreign_corporations(p) && !self.player_has_headquarters(p, corporation) {
            return false;
        }

        true
    }

    /// C++ `CvPlayer::getHasCorporationCount(eIndex)` (0 unless the corporation is active).
    pub fn player_get_has_corporation_count(&self, p: PlayerId, corporation: CorporationType) -> i32 {
        if !self.player_is_active_corporation(p, corporation) {
            return 0;
        }
        self.players[p].has_corporation_count[corporation]
    }

    /// C++ `CvPlayer::countTotalHasCorporation`.
    pub fn player_count_total_has_corporation(&self, p: PlayerId) -> i32 {
        let mut count = 0;
        for corporation in self.d.corporations.keys() {
            count += self.player_get_has_corporation_count(p, corporation);
        }
        count
    }

    /// C++ `CvPlayer::changeHasCorporationCount` (updates building commerce and citizen
    /// assignment game-wide).
    pub fn player_change_has_corporation_count(&mut self, p: PlayerId, corporation: CorporationType, change: i32) {
        todo!("CvPlayer::changeHasCorporationCount")
    }

    // ------------------------------------------------------------------ vote sources

    /// C++ `CvPlayer::getVote(iId)` (`NO_PLAYER_VOTE` if this player has not voted).
    pub fn player_get_vote(&self, p: PlayerId, id: VoteId) -> PlayerVote {
        for &(vote_id, vote) in &self.players[p].votes {
            if vote_id == id {
                return vote;
            }
        }
        PlayerVote::NO_PLAYER_VOTE
    }

    /// C++ `CvPlayer::setVote(iId, ePlayerVote)` (`NO_PLAYER_VOTE` removes the entry).
    pub fn player_set_vote(&mut self, p: PlayerId, id: VoteId, vote: PlayerVote) {
        let votes = &mut self.players[p].votes;
        if let Some(i) = votes.iter().position(|&(vote_id, _)| vote_id == id) {
            if vote == PlayerVote::NO_PLAYER_VOTE {
                votes.remove(i);
            } else {
                votes[i].1 = vote;
            }
            return;
        }

        if vote != PlayerVote::NO_PLAYER_VOTE {
            votes.push((id, vote));
        }
    }

    /// C++ `CvPlayer::setLoyalMember(eVoteSource, bNewValue)`.
    pub fn player_set_loyal_member(&mut self, p: PlayerId, vote_source: VoteSourceType, new_value: bool) {
        todo!("CvPlayer::setLoyalMember")
    }

    /// C++ `CvPlayer::processVoteSourceBonus(eVoteSource, bActive)` (per city).
    pub fn player_process_vote_source_bonus(&mut self, p: PlayerId, vote_source: VoteSourceType, active: bool) {
        todo!("CvPlayer::processVoteSourceBonus")
    }

    /// C++ `CvPlayer::getVotes(eVote, eVoteSource)` (`vote` None = C++ NO_VOTE).
    pub fn player_get_votes(&self, p: PlayerId, vote: Option<VoteType>, vote_source: VoteSourceType) -> i32 {
        todo!("CvPlayer::getVotes")
    }

    /// C++ `CvPlayer::canDoResolution(eVoteSource, kData)`.
    pub fn player_can_do_resolution(&self, p: PlayerId, vote_source: VoteSourceType, data: &VoteSelectionSubData) -> bool {
        todo!("CvPlayer::canDoResolution")
    }

    /// C++ `CvPlayer::canDefyResolution(eVoteSource, kData)`.
    pub fn player_can_defy_resolution(&self, p: PlayerId, vote_source: VoteSourceType, data: &VoteSelectionSubData) -> bool {
        todo!("CvPlayer::canDefyResolution")
    }

    /// C++ `CvPlayer::setDefiedResolution(eVoteSource, kData)`: defiance anger in cities,
    /// loses loyal membership.
    pub fn player_set_defied_resolution(&mut self, p: PlayerId, vote_source: VoteSourceType, data: &VoteSelectionSubData) {
        todo!("CvPlayer::setDefiedResolution")
    }

    /// C++ `CvPlayer::setEndorsedResolution(eVoteSource, kData)`.
    pub fn player_set_endorsed_resolution(&mut self, p: PlayerId, vote_source: VoteSourceType, data: &VoteSelectionSubData) {
        self.player_set_loyal_member(p, vote_source, true);
    }

    /// C++ `CvPlayer::isFullMember(eVoteSource)`.
    pub fn player_is_full_member(&self, p: PlayerId, vote_source: VoteSourceType) -> bool {
        todo!("CvPlayer::isFullMember")
    }

    /// C++ `CvPlayer::isVotingMember(eVoteSource)` (FFH: vote source civic required).
    pub fn player_is_voting_member(&self, p: PlayerId, vote_source: VoteSourceType) -> bool {
        todo!("CvPlayer::isVotingMember")
    }

    /// C++ `CvPlayer::isGamblingRing` (FFH: full member of a vote source with the gambling ring resolution).
    pub fn player_is_gambling_ring(&self, p: PlayerId) -> bool {
        todo!("CvPlayer::isGamblingRing")
    }

    /// C++ `CvPlayer::isSlaveTrade` (FFH vote source resolution).
    pub fn player_is_slave_trade(&self, p: PlayerId) -> bool {
        todo!("CvPlayer::isSlaveTrade")
    }

    /// C++ `CvPlayer::isSmugglingRing` (FFH vote source resolution).
    pub fn player_is_smuggling_ring(&self, p: PlayerId) -> bool {
        todo!("CvPlayer::isSmugglingRing")
    }

    // ------------------------------------------------------------------ traits (FFH)

    /// C++ `CvPlayer::setHasTrait(eTrait, bNewValue)` (FFH scriptable leader traits): applies
    /// or removes the trait's health, happiness, modifiers, commerce, civic upkeep, FFH flags
    /// (adaptive, agnostic, barbarian ally, ...), max cities and gold/summon modifiers.
    pub fn player_set_has_trait(&mut self, p: PlayerId, trait_type: TraitType, new_value: bool) {
        todo!("CvPlayer::setHasTrait")
    }
}
