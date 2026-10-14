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
        // UI: "deal cancelled" messages to both players.
        let (first, second) = (self.deals[d].first_player, self.deals[d].second_player);

        for trade in self.deals[d].first_trades.clone() {
            self.deal_end_trade(d, trade, first, second, kill_team);
        }

        for trade in self.deals[d].second_trades.clone() {
            self.deal_end_trade(d, trade, second, first, kill_team);
        }

        // CvGame::deleteDeal
        self.deals.remove(d);
    }

    /// C++ `CvDeal::addTrades`: start the trades (`first` given by `first_player`), making peace
    /// first if the players are at war, and merge teams for a permanent alliance.
    pub fn deal_add_trades(&mut self, d: DealId, first: &[TradeData], second: &[TradeData], check_allowed: bool) {
        if Deal::is_vassal_trade(first) && Deal::is_vassal_trade(second) {
            return;
        }

        let (first_player, second_player) = (self.deals[d].first_player, self.deals[d].second_player);

        if check_allowed {
            for trade in first {
                if !self.player_can_trade_item(first_player, second_player, trade, false) {
                    return;
                }
            }
            for trade in second {
                if !self.player_can_trade_item(second_player, first_player, trade, false) {
                    return;
                }
            }
        }

        let first_team = self.players[first_player].team;
        let second_team = self.players[second_player].team;

        if self.at_war(first_team, second_team) {
            // free vassals of capitulating team before peace is signed
            if Deal::is_vassal_trade(second) {
                for loop_team in self.team_ids() {
                    if loop_team != first_team && loop_team != second_team && self.team_is_alive(loop_team) && self.teams[loop_team].vassal[second_team] {
                        self.team_free_vassal(second_team, loop_team);
                        let second_success = self.teams[first_team].ai_war_success[second_team]
                            + self.d.defines.war_success_city_capturing * self.teams[second_team].num_cities;
                        let v = second_success.max(self.teams[first_team].ai_war_success[loop_team]);
                        self.teams[first_team].ai_war_success[loop_team] = v; // AI_setWarSuccess (bookkeeping)
                    }
                }
            }

            if Deal::is_vassal_trade(first) {
                for loop_team in self.team_ids() {
                    if loop_team != first_team && loop_team != second_team && self.team_is_alive(loop_team) && self.teams[loop_team].vassal[first_team] {
                        self.team_free_vassal(first_team, loop_team);
                        let first_success = self.teams[second_team].ai_war_success[first_team]
                            + self.d.defines.war_success_city_capturing * self.teams[first_team].num_cities;
                        let v = first_success.max(self.teams[second_team].ai_war_success[loop_team]);
                        self.teams[second_team].ai_war_success[loop_team] = v; // AI_setWarSuccess (bookkeeping)
                    }
                }
            }

            let bump_units = !Deal::is_vassal_trade(first) && !Deal::is_vassal_trade(second);
            self.team_make_peace(first_team, second_team, bump_units);
        } else if !self.deal_is_peace_deal_between_others(d, first, second) {
            // AI: AI_dealVal -> AI_changePeacetimeTradeValue / AI_changePeacetimeGrantValue
            // (attitude memory of the C++ AI; the new AI keeps its own relations).
        }

        for trade in first {
            if self.deal_start_trade(d, trade.clone(), first_player, second_player) && self.deals.contains(d) {
                self.deals[d].first_trades.push(trade.clone());
            }
        }

        for trade in second {
            if self.deal_start_trade(d, trade.clone(), second_player, first_player) && self.deals.contains(d) {
                self.deals[d].second_trades.push(trade.clone());
            }
        }

        let alliance = first.iter().chain(second.iter()).any(|t| t.item_type == TradeableItem::TRADE_PERMANENT_ALLIANCE);

        if alliance {
            if first_team < second_team {
                self.team_add_team(first_team, second_team);
            } else if second_team < first_team {
                self.team_add_team(second_team, first_team);
            }
        }
    }

    /// C++ `CvDeal::doTurn` (accumulates AI peacetime trade/grant values).
    pub fn deal_do_turn(&mut self, d: DealId) {
        // AI: for non-peace deals C++ adds AI_dealVal(..) / PEACE_TREATY_LENGTH to each side's
        // AI peacetime trade/grant value (attitude memory of the C++ AI); nothing else.
        let _ = d;
    }

    /// C++ `CvDeal::verify`: cancel the deal if a traded resource is no longer available or
    /// a cancelable peace deal remains.
    pub fn deal_verify(&mut self, d: DealId) {
        let mut cancel_deal = false;

        let (first, second) = (self.deals[d].first_player, self.deals[d].second_player);
        let (first_team, second_team) = (self.players[first].team, self.players[second].team);

        for (from, to, list) in [(first, second, &self.deals[d].first_trades), (second, first, &self.deals[d].second_trades)] {
            for trade in list {
                if trade.item_type == TradeableItem::TRADE_RESOURCES {
                    let bonus = BonusType::from_usize(trade.data as usize);
                    // XXX embargoes?
                    if self.player_get_num_tradeable_bonuses(from, bonus) < 0
                        || !self.player_can_trade_network_with(from, to)
                        || self.team_is_bonus_obsolete(first_team, bonus)
                        || self.team_is_bonus_obsolete(second_team, bonus)
                    {
                        cancel_deal = true;
                    }
                }
            }
        }

        if self.deal_is_cancelable(d, None) && self.deal_is_peace_deal(d) {
            cancel_deal = true;
        }

        if cancel_deal {
            self.deal_kill(d, true);
        }
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
        let deal = &self.deals[d];
        let (first, second) = (deal.first_player, deal.second_player);

        for trade in &deal.first_trades {
            if Deal::is_vassal(trade.item_type) && by_player == Some(second) {
                return true;
            }
            if trade.item_type == TradeableItem::TRADE_SURRENDER {
                let vassal = self.players[first].team;
                let master = self.players[second].team;
                if !self.team_can_vassal_revolt(vassal, master) {
                    return true;
                }
            }
        }

        for trade in &deal.second_trades {
            if Deal::is_vassal(trade.item_type) && by_player == Some(first) {
                return true;
            }
            if trade.item_type == TradeableItem::TRADE_SURRENDER {
                let vassal = self.players[second].team;
                let master = self.players[first].team;
                if !self.team_can_vassal_revolt(vassal, master) {
                    return true;
                }
            }
        }

        false
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
        let mut save = false;
        let from_team = self.players[from_player].team;
        let to_team = self.players[to_player].team;

        match trade.item_type {
            TradeableItem::TRADE_TECHNOLOGIES => {
                let tech = TechType::from_usize(trade.data as usize);
                self.team_set_has_tech(to_team, tech, true, Some(to_player), true, true);
                // setNoTradeTech
                self.teams[to_team].no_trade_tech[tech] = true;

                for p in self.player_ids() {
                    if self.players[p].alive {
                        if self.players[p].team == to_team {
                            self.players[p].ai_memory_count[from_player][MemoryType::MEMORY_TRADED_TECH_TO_US] += 1;
                        } else if self.teams[self.players[p].team].has_met[to_team] {
                            self.players[p].ai_memory_count[to_player][MemoryType::MEMORY_RECEIVED_TECH_FROM_ANY] += 1;
                        }
                    }
                }
            }

            TradeableItem::TRADE_RESOURCES => {
                let bonus = BonusType::from_usize(trade.data as usize);
                self.player_change_bonus_export(from_player, bonus, 1);
                self.player_change_bonus_import(to_player, bonus, 1);
                save = true;
            }

            TradeableItem::TRADE_CITIES => {
                // C++ `GET_PLAYER(eFromPlayer).getCity(iData)`: the city must belong to the giver.
                let c = CityId(trade.data as u32);
                if self.cities.contains(c) && self.cities[c].owner == from_player {
                    self.city_do_task(c, TaskType::TASK_GIFT, to_player.i(), -1, false, false, false, false);
                }
            }

            TradeableItem::TRADE_GOLD => {
                self.players[from_player].gold -= trade.data;
                self.players[to_player].gold += trade.data;
                // AI_changeGoldTradedTo (AI bookkeeping)
                self.players[from_player].ai_gold_traded_to[to_player] += trade.data;

                self.ev_player_gold_trade(from_player, to_player, trade.data);
            }

            TradeableItem::TRADE_GOLD_PER_TURN => {
                self.player_change_gold_per_turn_by_player(from_player, to_player, -trade.data);
                self.player_change_gold_per_turn_by_player(to_player, from_player, trade.data);
                save = true;
            }

            TradeableItem::TRADE_MAPS => {
                for p in self.map.plot_ids() {
                    if self.map.plots[p].revealed[from_team] {
                        self.plot_set_revealed(p, to_team, true, false, Some(from_team), false);
                    }
                }

                for p in self.player_ids() {
                    if self.players[p].alive && self.players[p].team == to_team {
                        self.player_update_plot_groups(p);
                    }
                }
            }

            TradeableItem::TRADE_SURRENDER | TradeableItem::TRADE_VASSAL => {
                if trade.data == 0 {
                    self.deal_start_team_trade(d, trade.item_type, from_team, to_team, false);
                    let capitulated = trade.item_type == TradeableItem::TRADE_SURRENDER;
                    self.team_set_vassal(from_team, to_team, true, capitulated);
                } else {
                    save = true;
                }
            }

            TradeableItem::TRADE_PEACE => {
                // C++ default `bBumpUnits = true`.
                self.team_make_peace(from_team, TeamId(trade.data as u8), true);
            }

            TradeableItem::TRADE_WAR => {
                let target = TeamId(trade.data as u8);
                self.team_declare_war(from_team, target, true, None);

                for p in self.player_ids() {
                    if self.players[p].alive && self.players[p].team == target {
                        self.players[p].ai_memory_count[to_player][MemoryType::MEMORY_HIRED_WAR_ALLY] += 1;
                    }
                }
            }

            TradeableItem::TRADE_EMBARGO => {
                let target = TeamId(trade.data as u8);
                self.player_stop_trading_with_team(from_player, target);

                for p in self.player_ids() {
                    if self.players[p].alive && self.players[p].team == target {
                        self.players[p].ai_memory_count[to_player][MemoryType::MEMORY_HIRED_TRADE_EMBARGO] += 1;
                    }
                }
            }

            TradeableItem::TRADE_CIVIC => {
                let civic = CivicType::from_usize(trade.data as usize);
                let mut new_civics = self.players[from_player].civics.clone();
                if let Some(option) = self.d.civics[civic].civic_option_type {
                    new_civics[option] = Some(civic);
                }

                self.player_revolution(from_player, &new_civics, true);

                let length = self.d.defines.peace_treaty_length;
                if self.players[from_player].ai_civic_timer < length {
                    self.players[from_player].ai_civic_timer = length;
                }
            }

            TradeableItem::TRADE_RELIGION => {
                self.player_convert(from_player, Some(ReligionType::from_usize(trade.data as usize)));

                let length = self.d.defines.peace_treaty_length;
                if self.players[from_player].ai_religion_timer < length {
                    self.players[from_player].ai_religion_timer = length;
                }
            }

            TradeableItem::TRADE_OPEN_BORDERS => {
                if trade.data == 0 {
                    self.deal_start_team_trade(d, TradeableItem::TRADE_OPEN_BORDERS, from_team, to_team, true);
                    self.team_set_open_borders(from_team, to_team, true);
                } else {
                    save = true;
                }
            }

            TradeableItem::TRADE_DEFENSIVE_PACT => {
                if trade.data == 0 {
                    self.deal_start_team_trade(d, TradeableItem::TRADE_DEFENSIVE_PACT, from_team, to_team, true);
                    self.team_set_defensive_pact(from_team, to_team, true);
                } else {
                    save = true;
                }
            }

            TradeableItem::TRADE_PERMANENT_ALLIANCE => {}

            TradeableItem::TRADE_PEACE_TREATY => {
                self.team_set_force_peace(from_team, to_team, true);
                save = true;
            }

            _ => debug_assert!(false, "CvDeal::startTrade: unexpected item {:?}", trade.item_type),
        }

        save
    }

    /// C++ `CvDeal::endTrade`: undo an ongoing trade item (`team`: also end the matching
    /// team-wide trades).
    pub fn deal_end_trade(&mut self, d: DealId, trade: TradeData, from_player: PlayerId, to_player: PlayerId, team: bool) {
        let from_team = self.players[from_player].team;
        let to_team = self.players[to_player].team;

        match trade.item_type {
            TradeableItem::TRADE_RESOURCES => {
                let bonus = BonusType::from_usize(trade.data as usize);
                self.player_change_bonus_import(to_player, bonus, -1);
                self.player_change_bonus_export(from_player, bonus, -1);
            }

            TradeableItem::TRADE_GOLD_PER_TURN => {
                self.player_change_gold_per_turn_by_player(from_player, to_player, trade.data);
                self.player_change_gold_per_turn_by_player(to_player, from_player, -trade.data);
            }

            TradeableItem::TRADE_VASSAL => {
                self.team_set_vassal(from_team, to_team, false, false);
                if team {
                    self.deal_end_team_trade(d, TradeableItem::TRADE_VASSAL, from_team, to_team);
                }
            }

            TradeableItem::TRADE_SURRENDER => {
                self.team_set_vassal(from_team, to_team, false, true);
                if team {
                    self.deal_end_team_trade(d, TradeableItem::TRADE_SURRENDER, from_team, to_team);
                }
            }

            TradeableItem::TRADE_OPEN_BORDERS => {
                self.team_set_open_borders(from_team, to_team, false);
                if team {
                    self.deal_end_team_trade(d, TradeableItem::TRADE_OPEN_BORDERS, from_team, to_team);
                }

                for i in self.player_ids() {
                    if self.players[i].alive && self.players[i].team == to_team {
                        for j in self.player_ids() {
                            if self.players[j].alive && self.players[j].team == from_team {
                                self.players[i].ai_memory_count[j][MemoryType::MEMORY_CANCELLED_OPEN_BORDERS] += 1;
                            }
                        }
                    }
                }
            }

            TradeableItem::TRADE_DEFENSIVE_PACT => {
                self.team_set_defensive_pact(from_team, to_team, false);
                if team {
                    self.deal_end_team_trade(d, TradeableItem::TRADE_DEFENSIVE_PACT, from_team, to_team);
                }
            }

            TradeableItem::TRADE_PEACE_TREATY => {
                self.team_set_force_peace(from_team, to_team, false);
            }

            // TRADE_TECHNOLOGIES, TRADE_CITIES, TRADE_GOLD, TRADE_MAPS, TRADE_PEACE, TRADE_WAR,
            // TRADE_EMBARGO, TRADE_CIVIC, TRADE_RELIGION, TRADE_PERMANENT_ALLIANCE: FAssert(false)
            // (one-shot items are never kept in a deal).
            _ => debug_assert!(false, "CvDeal::endTrade: unexpected item {:?}", trade.item_type),
        }
    }

    /// C++ `CvDeal::startTeamTrade`: implement the item between every player pair of the two teams.
    pub fn deal_start_team_trade(&mut self, d: DealId, item: TradeableItem, from_team: TeamId, to_team: TeamId, dual: bool) {
        let _ = d;
        for i in self.player_ids() {
            if !self.players[i].alive || self.players[i].team != from_team {
                continue;
            }
            for j in self.player_ids() {
                if !self.players[j].alive || self.players[j].team != to_team {
                    continue;
                }
                let trade = crate::utils::set_trade_item(item, 1);
                let our_list = vec![trade.clone()];
                let their_list = if dual { vec![trade] } else { Vec::new() };
                // C++ default `bForce = false`.
                self.implement_deal(i, j, &our_list, &their_list, false);
            }
        }
    }

    /// C++ `CvDeal::endTeamTrade`: kill the other deals carrying the same item between the teams.
    pub fn deal_end_team_trade(&mut self, d: DealId, item: TradeableItem, from_team: TeamId, to_team: TeamId) {
        // C++ iterates the game's deal list in id order; deals killed here are removed from it.
        for other in self.deals.ids() {
            if other == d || !self.deals.contains(other) {
                continue;
            }
            let deal = &self.deals[other];
            let first_team = self.players[deal.first_player].team;
            let second_team = self.players[deal.second_player].team;

            let mut valid = true;

            if first_team == from_team && second_team == to_team && deal.first_trades.iter().any(|t| t.item_type == item) {
                valid = false;
            }

            if valid && first_team == to_team && second_team == from_team && deal.second_trades.iter().any(|t| t.item_type == item) {
                valid = false;
            }

            if !valid {
                self.deal_kill(other, false);
            }
        }
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
