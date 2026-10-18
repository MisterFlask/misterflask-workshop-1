//! Defensive pacts: a weaker civ's check against a stronger, warlike neighbour.
//!
//! The rules already do the rest (C++ `CvTeam`): a pact member that is attacked brings its
//! partners into the war (`team_declare_war`, `WARPLAN_DOGPILE`), and a team's defensive power
//! (`team_get_defensive_power`) counts its pact partners, which is the number every war gate
//! (this AI's `diplomacy::manage`, `victory_war`) divides by -- so a pact deters before it ever
//! fires. A civ that declares a war loses its own pacts. Pacts need Honor on either side
//! (`bDefensivePactTrading`) and both teams at peace with everyone.
//!
//! The AI only reads what a human could see: power, distance, and wars fought in the open. It
//! draws no RNG and adds no saved state.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::*;
use ffh_data::*;

use crate::diplomacy::{is_team_leader, preparing_target, reach_distance, relation};

/// A neighbour is a threat when its power is at least this share (%) of our defensive power:
/// around the war bar of the median leader (`diplomacy::manage`: 85-180%).
const THREAT_RATIO: i64 = 120;
/// Plots between their cities and ours: the land reach of the war gate.
const THREAT_REACH: i32 = 16;
/// A team that fought a civ within this many turns (or is fighting now) is warlike.
const WARLIKE_TURNS: i32 = 50;
/// Pacts a team keeps at most: enough to deter, not so many that one war drags in everyone.
const MAX_PACTS: i32 = 2;

fn pact_item() -> TradeData {
    TradeData { item_type: TradeableItem::TRADE_DEFENSIVE_PACT, data: 0, offering: false, hidden: false }
}

/// Is team `t` at war with a civ now, or did it fight one in the last `WARLIKE_TURNS` turns?
fn warlike(g: &Game, t: TeamId) -> bool {
    let k = &g.teams[t];
    g.team_ids().into_iter().any(|o| {
        o != t
            && g.team_is_alive(o)
            && !g.team_is_barbarian(o)
            && (k.at_war[o] || (k.has_met[o] && k.ai_at_peace_counter[o] < WARLIKE_TURNS && k.ai_has_met_counter[o] > k.ai_at_peace_counter[o] + 5))
    })
}

/// The stronger, warlike neighbour that could make us its next target (the strongest of them),
/// with its power as a share of our defensive power.
pub fn threat(g: &Game, p: PlayerId) -> Option<(TeamId, i64)> {
    let team = g.players[p].team;
    let ours = g.team_get_defensive_power(team).max(1) as i64;
    let mut best: Option<(TeamId, i64)> = None;
    for t in g.team_ids() {
        if t == team || !g.team_is_alive(t) || g.team_is_barbarian(t) || !g.teams[team].has_met[t] || g.teams[t].num_cities == 0 {
            continue;
        }
        let ratio = g.team_get_power(t, true) as i64 * 100 / ours;
        if ratio < THREAT_RATIO || !reach_distance(g, t, team).is_some_and(|d| d <= THREAT_REACH) || !warlike(g, t) {
            continue;
        }
        if best.map_or(true, |(_, r)| ratio > r) {
            best = Some((t, ratio));
        }
    }
    best
}

/// Pacts team `t` has with live teams.
fn pacts(g: &Game, t: TeamId) -> i32 {
    g.team_ids().into_iter().filter(|&o| o != t && g.team_is_alive(o) && g.teams[t].defensive_pact[o]).count() as i32
}

/// Seek a pact against the neighbour that threatens us (called on the trade cadence, at peace).
/// The partner is the AI leader we get on with best among those the same neighbour threatens,
/// else the strongest one we do not dislike: its power counts toward our defence at once.
pub fn seek(g: &mut Game, p: PlayerId) {
    let team = g.players[p].team;
    if pacts(g, team) >= MAX_PACTS || preparing_target(g, team).is_some() {
        return;
    }
    let Some((foe, ratio)) = threat(g, p) else { return };
    let mut best: Option<(PlayerId, (bool, i32, i32))> = None;
    for other in g.player_ids() {
        let op = &g.players[other];
        let ot = op.team;
        if other == p || !op.alive || op.human || ot == team || ot == foe || g.player_is_barbarian(other) || !is_team_leader(g, other) {
            continue;
        }
        if !g.teams[team].has_met[ot] || g.teams[team].defensive_pact[ot] || relation(g, p, other) < 0 || pacts(g, ot) >= MAX_PACTS {
            continue;
        }
        if !g.player_can_trade_item(p, other, &pact_item(), true) || !g.player_can_trade_item(other, p, &pact_item(), true) {
            continue;
        }
        let shared = threat(g, other).is_some_and(|(t, _)| t == foe);
        let key = (shared, relation(g, p, other), g.team_get_power(ot, true));
        if best.map_or(true, |(_, k)| key > k) {
            best = Some((other, key));
        }
    }
    let Some((other, _)) = best else { return };
    let (ours, theirs) = (vec![pact_item()], vec![pact_item()]);
    if g.controller(other).consider_offer(g, other, p, &ours, &theirs) {
        crate::trace!(g, p, "defensive pact with {:?} against team {:?} (its power {}% of our defence)", other, foe, ratio);
        g.implement_deal(p, other, &ours, &theirs, false);
    }
}

/// `consider_offer` for a pact: accept from a civ we do not dislike when we are
/// threatened ourselves or like them, unless we are about to attack them, already hold our
/// pacts, or they are the victory rival we would have to stop.
pub fn accept(g: &Game, p: PlayerId, from: PlayerId) -> bool {
    let (team, their_team) = (g.players[p].team, g.players[from].team);
    if g.at_war(team, their_team) || pacts(g, team) >= MAX_PACTS || preparing_target(g, team) == Some(their_team) {
        return false;
    }
    if crate::victory::rival_team(g, &crate::victory::plan(g, p)) == Some(their_team) {
        return false;
    }
    let rel = relation(g, p, from);
    rel >= 0 && (threat(g, p).is_some_and(|(t, _)| t != their_team) || rel >= 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    // At the start nobody has fought a war, so no neighbour is a threat and no pact is sought.
    #[test]
    fn no_threat_before_any_war() {
        let d = ffh_data::global();
        let mut g = Game::new(d, ffh_core::setup::GameSetup::standard(d, 6, "WORLDSIZE_SMALL", 5));
        g.map_script = std::sync::Arc::new(ffh_core::mapscript::FractalMapScript::default());
        g.game_init();
        for p in g.player_ids().filter(|&p| g.players[p].alive && !g.player_is_barbarian(p)) {
            assert_eq!(threat(&g, p), None);
            assert!(!warlike(&g, g.players[p].team));
        }
    }
}
