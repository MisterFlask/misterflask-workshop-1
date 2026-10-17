//! Deals the AI offers to human players (C++ `CvPlayerAI::AI_doDiplo`'s contacts with a human:
//! `gDLL->beginDiplomacy` with an offer the human accepts or refuses). The human's controller
//! answers through `Controller::consider_offer` (ffh-play turns it into a modal); an accepted
//! deal is implemented as the AI's own deals are.
//!
//! * **Peace** (`CONTACT_PEACE_TREATY`): at war with a human team for more than 10 turns, when
//!   the AI wants peace (`diplomacy::want_peace`: the war goes badly, or drags on).
//! * **Technologies** (`CONTACT_TRADE_TECH`): at peace with a human it does not dislike, one of
//!   the human's techs it can research for one of its own of similar cost, a deal the AI's own
//!   `consider_offer` would accept.
//!
//! As in C++, only a team leader contacts, only a human team leader is contacted, and a human
//! is contacted at most once per AI turn. C++ waits the leader's contact delay after each
//! contact and then draws `1 / iContactRand` per turn; the AI draws no random numbers and keeps
//! no contact timers (no state outside `Game`), so each pair is contacted on a fixed schedule:
//! every `iContactDelay + iContactRand` turns. AI-only games are untouched (no human).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::TradeData;
use ffh_data::*;

use crate::view::View;

fn tech_item(t: TechType) -> TradeData {
    TradeData { item_type: TradeableItem::TRADE_TECHNOLOGIES, data: t.i(), offering: false, hidden: false }
}

fn peace_item() -> TradeData {
    TradeData { item_type: TradeableItem::TRADE_PEACE_TREATY, data: 0, offering: false, hidden: false }
}

/// The alive leader player of team `t` (C++ `CvTeam::getLeaderID`).
fn leader(g: &Game, t: TeamId) -> Option<PlayerId> {
    g.player_ids().into_iter().find(|&q| g.players[q].alive && g.players[q].team == t)
}

/// Is this turn the AI `p`'s turn to contact `human` about `contact` (every `iContactDelay +
/// iContactRand` turns, staggered by the two players)?
pub(crate) fn contact_due(g: &Game, p: PlayerId, human: PlayerId, contact: &str) -> bool {
    let Some(c) = g.d.ty::<ContactType>(contact) else { return false };
    let Some(l) = g.players[p].personality_type.map(|l| &g.d.leader_heads[l]) else { return false };
    let period = (l.contact_delay[c] + l.contact_rand[c]).max(1);
    (g.game_turn() + 7 * p.0 as i32 + 3 * human.0 as i32).rem_euclid(period) == 0
}

/// The AI player `v.p` offers its deals to the human players (once per turn, on its first
/// `play_turn` call).
pub fn to_humans(g: &mut Game, v: &View) {
    let p = v.p;
    let team = v.team;
    if v.barbarian || g.players[p].human || g.team_is_human(team) || leader(g, team) != Some(p) {
        return;
    }
    for human in g.player_ids() {
        let hp = &g.players[human];
        if !hp.alive || !hp.human || hp.team == team || leader(g, hp.team) != Some(human) {
            continue;
        }
        let their = hp.team;
        if !g.teams[team].has_met[their] || !g.player_can_contact(p, human) {
            continue;
        }
        // abContacted: one contact per human a turn, peace first, then a tech or a resource offer.
        if g.at_war(team, their) {
            offer_peace(g, p, human);
        } else if !offer_tech_trade(g, p, human) {
            offer_resource_trade(g, p, human);
        }
    }
}

fn offer_peace(g: &mut Game, p: PlayerId, human: PlayerId) {
    let (team, their) = (g.players[p].team, g.players[human].team);
    if g.teams[team].ai_at_war_counter[their] <= 10 || !contact_due(g, p, human, "CONTACT_PEACE_TREATY") {
        return;
    }
    let plan = crate::victory::plan_ours(g, p);
    let our_want = crate::diplomacy::end_war_val(g, team, their);
    let their_want = crate::diplomacy::end_war_val(g, their, team);
    // When clearly winning (they want peace more than we do), initiate a peace that extracts
    // reparations -- unless we are pursuing conquest/domination against them, in which case we press
    // on. Otherwise offer only when we want peace (losing / stalemated), as before.
    let victory_war = crate::victory::wants_conquest(&plan) || crate::victory::rival_team(g, &plan) == Some(their);
    let winner_initiated = their_want > our_want && !victory_war;
    if !crate::diplomacy::want_peace(g, team, their, &plan) && !winner_initiated {
        return;
    }
    let item = peace_item();
    if !g.player_can_trade_item(p, human, &item, true) || !g.player_can_trade_item(human, p, &item, true) {
        return;
    }
    let mut ours = vec![item.clone()];
    let mut theirs = vec![item];
    if their_want > our_want {
        theirs.extend(crate::trade::peace_tribute(g, p, human, their_want - our_want)); // reparations from them
    } else if our_want > their_want {
        ours.extend(crate::trade::peace_tribute(g, human, p, our_want - their_want)); // sweeten so they accept
    }
    crate::trace!(g, p, "offers peace to the human {:?}", human);
    if g.controller(human).consider_offer(g, human, p, &ours, &theirs) {
        g.implement_deal(p, human, &ours, &theirs, false);
    }
}

/// Returns whether the AI contacted the human with a tech offer this turn (so `to_humans` can fall
/// back to a resource offer when it did not).
fn offer_tech_trade(g: &mut Game, p: PlayerId, human: PlayerId) -> bool {
    let (team, their) = (g.players[p].team, g.players[human].team);
    if crate::diplomacy::relation(g, p, human) < 0 || crate::diplomacy::preparing_target(g, team) == Some(their) || !contact_due(g, p, human, "CONTACT_TRADE_TECH") {
        return false;
    }
    let cost = |t: TechType| g.team_get_research_cost(team, t).max(1);
    // The human's tech we value most (the costliest we can research).
    let get = g
        .d
        .techs
        .keys()
        .filter(|&t| !g.team_is_has_tech(team, Some(t)) && g.player_can_research(p, t, false) && g.player_can_trade_item(human, p, &tech_item(t), true))
        .max_by_key(|&t| (cost(t), std::cmp::Reverse(t)));
    let Some(get) = get else { return false };
    // Ours closest in cost.
    let give = g
        .d
        .techs
        .keys()
        .filter(|&t| g.team_is_has_tech(team, Some(t)) && !g.team_is_has_tech(their, Some(t)) && g.player_can_trade_item(p, human, &tech_item(t), true))
        .min_by_key(|&t| ((cost(t) - cost(get)).abs(), t));
    let Some(give) = give else { return false };
    let (ours, theirs) = (vec![tech_item(give)], vec![tech_item(get)]);
    // Only a deal the AI would accept itself.
    if !crate::diplomacy::consider_offer(g, p, human, &theirs, &ours) {
        return false;
    }
    crate::trace!(g, p, "offers {} for {} to the human {:?}", g.d.techs[give].base.type_, g.d.techs[get].base.type_, human);
    if g.controller(human).consider_offer(g, human, p, &ours, &theirs) {
        g.implement_deal(p, human, &ours, &theirs, false);
    }
    true
}

/// Offer the human a resource of theirs we want, paid for with a surplus of ours or with gold
/// (`trade::buy_resource_offer`). Gated like the tech offer and only when the AI would take the
/// deal itself.
fn offer_resource_trade(g: &mut Game, p: PlayerId, human: PlayerId) {
    let (team, their) = (g.players[p].team, g.players[human].team);
    if crate::diplomacy::relation(g, p, human) < 0 || crate::diplomacy::preparing_target(g, team) == Some(their) || !contact_due(g, p, human, "CONTACT_TRADE_BONUS") {
        return;
    }
    let Some((ours, theirs)) = crate::trade::buy_resource_offer(g, p, human) else { return };
    // Only a deal the AI would accept itself.
    if !crate::diplomacy::consider_offer(g, p, human, &theirs, &ours) {
        return;
    }
    let got = BonusType::from_usize(theirs[0].data as usize);
    crate::trace!(g, p, "offers to buy {} from the human {:?}", g.d.bonuses[got].base.type_, human);
    if g.controller(human).consider_offer(g, human, p, &ours, &theirs) {
        g.implement_deal(p, human, &ours, &theirs, false);
    }
}
