//! Port of `CvDeal` (CvDeal.h / CvDeal.cpp): a diplomatic deal between two players, with
//! the trade items each side gives (`first_trades` = given by `first_player`).
//!
//! C++ static helpers (`isAnnual`, `isDual`, `isVassal`, `getPeaceItem`, ...) are associated
//! functions on [`Deal`]; everything else is an `impl Game` method `deal_*`. Trade lists are
//! `Vec<TradeData>` (C++ `CLinkList<TradeData>`; a NULL list is an empty slice), so
//! `clearFirstTrades` / `insertAtEndFirstTrades` / `getLengthFirstTrades` /
//! `headFirstTradesNode` ... are plain `Vec` operations on `first_trades` / `second_trades`.
//! C++ `pszReason` (UI text) out-parameters are dropped.

#![allow(unused_variables)]
#![allow(unused_imports)]
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

/// C++ `CvDeal`.
#[derive(Clone, Debug, Default)]
pub struct Deal {
    /// C++ `m_iID`.
    pub id: DealId,
    /// C++ `m_iInitialGameTurn` (set by `deal_init`).
    pub initial_game_turn: i32,
    /// C++ `m_eFirstPlayer`.
    pub first_player: PlayerId,
    /// C++ `m_eSecondPlayer`.
    pub second_player: PlayerId,
    /// C++ `m_firstTrades`: items `first_player` gives (only those kept by `startTrade`).
    pub first_trades: Vec<TradeData>,
    /// C++ `m_secondTrades`: items `second_player` gives.
    pub second_trades: Vec<TradeData>,
}

impl Deal {
    /// C++ `CvDeal::reset(iID, eFirstPlayer, eSecondPlayer)`.
    pub fn new(id: DealId, first_player: PlayerId, second_player: PlayerId) -> Deal {
        Deal { id, initial_game_turn: 0, first_player, second_player, first_trades: Vec::new(), second_trades: Vec::new() }
    }

    /// C++ static `CvDeal::isAnnual`.
    pub fn is_annual(item: TradeableItem) -> bool {
        item == TradeableItem::TRADE_RESOURCES
            || item == TradeableItem::TRADE_GOLD_PER_TURN
            || item == TradeableItem::TRADE_VASSAL
            || item == TradeableItem::TRADE_SURRENDER
            || item == TradeableItem::TRADE_OPEN_BORDERS
            || item == TradeableItem::TRADE_DEFENSIVE_PACT
            || item == TradeableItem::TRADE_PERMANENT_ALLIANCE
    }

    /// C++ static `CvDeal::isDual` (C++ default `bExcludePeace = false`).
    pub fn is_dual(item: TradeableItem, exclude_peace: bool) -> bool {
        if item == TradeableItem::TRADE_OPEN_BORDERS
            || item == TradeableItem::TRADE_DEFENSIVE_PACT
            || item == TradeableItem::TRADE_PERMANENT_ALLIANCE
        {
            return true;
        }
        if item == TradeableItem::TRADE_PEACE_TREATY {
            return !exclude_peace;
        }
        false
    }

    /// C++ static `CvDeal::hasData`.
    pub fn has_data(item: TradeableItem) -> bool {
        item != TradeableItem::TRADE_MAPS
            && item != TradeableItem::TRADE_VASSAL
            && item != TradeableItem::TRADE_SURRENDER
            && item != TradeableItem::TRADE_OPEN_BORDERS
            && item != TradeableItem::TRADE_DEFENSIVE_PACT
            && item != TradeableItem::TRADE_PERMANENT_ALLIANCE
            && item != TradeableItem::TRADE_PEACE_TREATY
    }

    /// C++ static `CvDeal::isGold`.
    pub fn is_gold(item: TradeableItem) -> bool {
        item == Deal::get_gold_item() || item == Deal::get_gold_per_turn_item()
    }

    /// C++ static `CvDeal::isVassal`.
    pub fn is_vassal(item: TradeableItem) -> bool {
        item == TradeableItem::TRADE_VASSAL || item == TradeableItem::TRADE_SURRENDER
    }

    /// C++ static `CvDeal::isEndWar`.
    pub fn is_end_war(item: TradeableItem) -> bool {
        if item == Deal::get_peace_item() {
            return true;
        }

        if Deal::is_vassal(item) {
            return true;
        }

        false
    }

    /// C++ static `CvDeal::getPeaceItem`.
    pub fn get_peace_item() -> TradeableItem {
        TradeableItem::TRADE_PEACE_TREATY
    }

    /// C++ static `CvDeal::getGoldItem`.
    pub fn get_gold_item() -> TradeableItem {
        TradeableItem::TRADE_GOLD
    }

    /// C++ static `CvDeal::getGoldPerTurnItem`.
    pub fn get_gold_per_turn_item() -> TradeableItem {
        TradeableItem::TRADE_GOLD_PER_TURN
    }

    /// C++ static protected `CvDeal::isVassalTrade`: the list contains a vassal/surrender item.
    pub fn is_vassal_trade(list: &[TradeData]) -> bool {
        list.iter().any(|t| Deal::is_vassal(t.item_type))
    }

    /// C++ static `CvDeal::isVassalTributeDeal`: every item is a resource (true for an empty list).
    pub fn is_vassal_tribute_deal(list: &[TradeData]) -> bool {
        list.iter().all(|t| t.item_type == TradeableItem::TRADE_RESOURCES)
    }
}

impl Game {
    // ------------------------------------------------------------------
    // Lifetime
    // ------------------------------------------------------------------

    /// C++ `CvDeal::init(iID, eFirstPlayer, eSecondPlayer)`: `reset` + initial game turn.
    /// The deal must already be in `self.deals` (C++ `CvGame::addDeal`).
    pub fn deal_init(&mut self, d: DealId, first_player: PlayerId, second_player: PlayerId) {
        self.deals[d] = Deal::new(d, first_player, second_player);
        // C++ GC.getGameINLINE().getGameTurn() == GC.getInitCore().getGameTurn().
        self.deals[d].initial_game_turn = self.setup.game_turn;
    }

    /// C++ `CvDeal::kill` (C++ default `bKillTeam = true`): end every trade and delete the deal
    /// (`CvGame::deleteDeal`).
    pub fn deal_kill(&mut self, d: DealId, kill_team: bool) {
        todo!("CvDeal::kill")
    }

    /// C++ `CvDeal::addTrades`: start the trades (`first` given by `first_player`), making peace
    /// first if the players are at war, and merge teams for a permanent alliance.
    pub fn deal_add_trades(&mut self, d: DealId, first: &[TradeData], second: &[TradeData], check_allowed: bool) {
        todo!("CvDeal::addTrades")
    }

    /// C++ `CvDeal::doTurn` (accumulates AI peacetime trade/grant values).
    pub fn deal_do_turn(&mut self, d: DealId) {
        todo!("CvDeal::doTurn")
    }

    /// C++ `CvDeal::verify`: cancel the deal if a traded resource is no longer available or
    /// a cancelable peace deal remains.
    pub fn deal_verify(&mut self, d: DealId) {
        todo!("CvDeal::verify")
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// C++ `CvDeal::isPeaceDeal`.
    pub fn deal_is_peace_deal(&self, d: DealId) -> bool {
        let deal = &self.deals[d];
        deal.first_trades.iter().any(|t| t.item_type == Deal::get_peace_item())
            || deal.second_trades.iter().any(|t| t.item_type == Deal::get_peace_item())
    }

    /// C++ `CvDeal::isPeaceDealBetweenOthers`: either list contains TRADE_PEACE (making peace
    /// with a third party). Does not read the deal itself.
    pub fn deal_is_peace_deal_between_others(&self, d: DealId, first: &[TradeData], second: &[TradeData]) -> bool {
        first.iter().any(|t| t.item_type == TradeableItem::TRADE_PEACE)
            || second.iter().any(|t| t.item_type == TradeableItem::TRADE_PEACE)
    }

    /// C++ `CvDeal::isVassalDeal`.
    pub fn deal_is_vassal_deal(&self, d: DealId) -> bool {
        let deal = &self.deals[d];
        Deal::is_vassal_trade(&deal.first_trades) || Deal::is_vassal_trade(&deal.second_trades)
    }

    /// C++ `CvDeal::isUncancelableVassalDeal` (`by_player = None` is NO_PLAYER).
    pub fn deal_is_uncancelable_vassal_deal(&self, d: DealId, by_player: Option<PlayerId>) -> bool {
        todo!("CvDeal::isUncancelableVassalDeal")
    }

    /// C++ `CvDeal::isCancelable` (C++ default `eByPlayer = NO_PLAYER`; non-const in C++,
    /// read-only).
    pub fn deal_is_cancelable(&self, d: DealId, by_player: Option<PlayerId>) -> bool {
        if self.deal_is_uncancelable_vassal_deal(d, by_player) {
            return false;
        }

        let turns = self.deal_turns_to_cancel(d, by_player);
        // UI: pszReason = TXT_KEY_MISC_DEAL_NO_CANCEL when turns > 0.

        turns <= 0
    }

    /// C++ `CvDeal::turnsToCancel` (C++ default `eByPlayer = NO_PLAYER`, unused).
    pub fn deal_turns_to_cancel(&self, d: DealId, by_player: Option<PlayerId>) -> i32 {
        // C++ GC.getGameINLINE().getGameTurn() == GC.getInitCore().getGameTurn().
        self.deals[d].initial_game_turn + self.d.defines.peace_treaty_length - self.setup.game_turn
    }

    // ------------------------------------------------------------------
    // Trades (protected in C++)
    // ------------------------------------------------------------------

    /// C++ `CvDeal::startTrade`: apply one trade item; returns true if it must be kept in the
    /// deal (ongoing item).
    pub fn deal_start_trade(&mut self, d: DealId, trade: TradeData, from_player: PlayerId, to_player: PlayerId) -> bool {
        todo!("CvDeal::startTrade")
    }

    /// C++ `CvDeal::endTrade`: undo an ongoing trade item (`team`: also end the matching
    /// team-wide trades).
    pub fn deal_end_trade(&mut self, d: DealId, trade: TradeData, from_player: PlayerId, to_player: PlayerId, team: bool) {
        todo!("CvDeal::endTrade")
    }

    /// C++ `CvDeal::startTeamTrade`: implement the item between every player pair of the two teams.
    pub fn deal_start_team_trade(&mut self, d: DealId, item: TradeableItem, from_team: TeamId, to_team: TeamId, dual: bool) {
        todo!("CvDeal::startTeamTrade")
    }

    /// C++ `CvDeal::endTeamTrade`: kill the other deals carrying the same item between the teams.
    pub fn deal_end_team_trade(&mut self, d: DealId, item: TradeableItem, from_team: TeamId, to_team: TeamId) {
        todo!("CvDeal::endTeamTrade")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(i: TradeableItem) -> TradeData {
        TradeData { item_type: i, data: 0, offering: false, hidden: false }
    }

    #[test]
    fn static_item_predicates_match_cpp() {
        // (item, isAnnual, isDual(false), isDual(true), hasData, isGold, isVassal, isEndWar) from CvDeal.cpp.
        let (t, f) = (true, false);
        let expect = [
            ("TRADE_GOLD", f, f, f, t, t, f, f),
            ("TRADE_GOLD_PER_TURN", t, f, f, t, t, f, f),
            ("TRADE_MAPS", f, f, f, f, f, f, f),
            ("TRADE_VASSAL", t, f, f, f, f, t, t),
            ("TRADE_SURRENDER", t, f, f, f, f, t, t),
            ("TRADE_OPEN_BORDERS", t, t, t, f, f, f, f),
            ("TRADE_DEFENSIVE_PACT", t, t, t, f, f, f, f),
            ("TRADE_PERMANENT_ALLIANCE", t, t, t, f, f, f, f),
            ("TRADE_PEACE_TREATY", f, t, f, f, f, f, t),
            ("TRADE_TECHNOLOGIES", f, f, f, t, f, f, f),
            ("TRADE_RESOURCES", t, f, f, t, f, f, f),
            ("TRADE_CITIES", f, f, f, t, f, f, f),
            ("TRADE_PEACE", f, f, f, t, f, f, f),
            ("TRADE_WAR", f, f, f, t, f, f, f),
            ("TRADE_EMBARGO", f, f, f, t, f, f, f),
            ("TRADE_CIVIC", f, f, f, t, f, f, f),
            ("TRADE_RELIGION", f, f, f, t, f, f, f),
        ];
        assert_eq!(expect.len(), TradeableItem::NUM);
        for (i, e) in expect.iter().enumerate() {
            let it = TradeableItem(i as u16);
            assert_eq!(TradeableItem::NAMES[i], e.0);
            let got = (
                Deal::is_annual(it),
                Deal::is_dual(it, false),
                Deal::is_dual(it, true),
                Deal::has_data(it),
                Deal::is_gold(it),
                Deal::is_vassal(it),
                Deal::is_end_war(it),
            );
            assert_eq!(got, (e.1, e.2, e.3, e.4, e.5, e.6, e.7), "{}", e.0);
        }
    }

    #[test]
    fn list_predicates() {
        let res = item(TradeableItem::TRADE_RESOURCES);
        let vassal = item(TradeableItem::TRADE_VASSAL);
        assert!(Deal::is_vassal_tribute_deal(&[]));
        assert!(Deal::is_vassal_tribute_deal(&[res.clone(), res.clone()]));
        assert!(!Deal::is_vassal_tribute_deal(&[res.clone(), item(TradeableItem::TRADE_GOLD)]));
        assert!(!Deal::is_vassal_trade(&[res.clone()]));
        assert!(Deal::is_vassal_trade(&[res, vassal]));
    }
}
