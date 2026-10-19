//! FFH2 AI: a new design (see docs/AI.md), not a port of the C++ AI.
//!
//! [`Ai`] implements [`ffh_core::Controller`]. Each turn (`play_turn`) it builds a
//! [`view::View`] (enemies, danger map), manages the empire (research, civics, religion,
//! sliders), diplomacy, then gives every group that needs orders one decision through the
//! rules' mission API, and finally casts end-of-turn spells as FfH's AI does. City
//! production is chosen when the rules ask (`choose_production`).
//!
//! All decisions are deterministic functions of the game state: the AI draws no random
//! numbers and its caches are recomputed whenever the turn changes.

use std::sync::{Arc, Mutex};

use ffh_core::controller::{Controller, ControllerRef};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::*;
use ffh_data::*;

pub mod advanced_start;
pub mod automation;
pub mod campaign;
pub mod casting;
pub mod civs;
pub mod diplomacy;
pub mod economy;
pub mod empire;
pub mod events;
pub mod expansion;
pub mod finance;
pub mod military;
pub mod naval;
pub mod offers;
pub mod pacts;
pub mod production;
pub mod reach;
pub mod research;
pub mod sites;
pub mod tech_plan;
pub mod trade;
pub mod units;
pub mod values;
pub mod victory;
pub mod view;
pub mod workers;
pub mod world_spells;

/// Debug tracing: `FFH_AI_TRACE=all` or `FFH_AI_TRACE=<player id>` prints AI decisions to
/// stderr. Read once; it only prints, so it cannot change the game.
pub fn trace_on(p: PlayerId) -> bool {
    static TRACE: std::sync::OnceLock<Option<i32>> = std::sync::OnceLock::new();
    let t = TRACE.get_or_init(|| match std::env::var("FFH_AI_TRACE") {
        Ok(v) if v == "all" => Some(-1),
        Ok(v) => v.parse().ok(),
        Err(_) => None,
    });
    matches!(*t, Some(-1)) || *t == Some(p.0 as i32)
}

#[macro_export]
macro_rules! trace {
    ($g:expr, $p:expr, $($arg:tt)*) => {
        if $crate::trace_on($p) {
            eprintln!("[t{} p{}] {}", $g.game_turn(), $p.0, format!($($arg)*));
        }
    };
}

/// The AI controller. One instance may serve any number of players.
#[derive(Default)]
pub struct Ai {
    pub(crate) sites: Mutex<sites::SiteCaches>,
    /// Game turn of each player's last full `play_turn` (C++ calls `AI_unitUpdate` on every
    /// `update` until the turn ends; empire-level decisions are made once per turn). Only a
    /// shortcut: `play_turn` also derives "already played" from the game state.
    played: Mutex<std::collections::BTreeMap<PlayerId, i32>>,
}

impl Ai {
    pub fn new() -> Ai {
        Ai::default()
    }
}

pub fn ai_controller() -> ControllerRef {
    Arc::new(Ai::new())
}

impl Controller for Ai {
    fn name(&self) -> &str {
        "ffh-ai"
    }

    fn play_turn(&self, g: &mut Game, p: PlayerId) {
        if !g.players[p].alive {
            return;
        }
        // C++ setTurnActive -> AI_doAdvancedStart while the player still has start points.
        if g.players[p].advanced_start_points >= 0 && !g.players[p].human && !g.player_is_barbarian(p) {
            advanced_start::run(g, p);
        }
        let turn = g.game_turn();
        // The empire decisions are made on the first call of the player's turn. A controller
        // that did not see the earlier calls (a game saved mid-turn, resumed with a fresh AI)
        // recognises them from the game state: the rules call again within a turn only while
        // some unit is still moving, i.e. has spent moves this turn.
        let seen = self.played.lock().unwrap().insert(p, turn) == Some(turn);
        let first_call = !seen && !g.players[p].units.iter().any(|&u| g.units.contains(u) && g.unit_has_moved(u));
        let mut v = view::View::new(g, p);
        if first_call && !v.barbarian {
            military::report_exposure(g, &v, p);
        }
        if !first_call {
            // Called again in the same turn: only groups that still need orders.
            units::play(self, g, &mut v);
            return;
        }
        if !v.barbarian {
            if trace_on(p) {
                trace!(g, p, "{}", victory::describe(g, p, &victory::plan(g, p)));
            }
            empire::manage(g, p);
            production::emergency(self, g, p);
            expansion::prepare_escorts(g, p); // early-expansion
            diplomacy::manage(g, &v);
            offers::to_humans(g, &v); // ai-offers: deals offered to human players
            // War declarations change the picture.
            v = view::View::new(g, p);
            civs::turn(self, g, &mut v); // civ-mechanics
        }
        units::play(self, g, &mut v);
        casting::end_turn(g, &v);
        // Spells may have summoned units or used moves: make sure nothing is left ready.
        units::finalize(g, p);
    }

    fn choose_production(&self, g: &mut Game, c: CityId) {
        production::choose(self, g, c);
    }

    fn choose_research(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        research::choose(g, p)
    }

    fn raze_conquered_city(&self, g: &Game, p: PlayerId, c: CityId) -> bool {
        diplomacy::raze_conquered_city(g, p, c)
    }

    fn consider_offer(&self, g: &Game, p: PlayerId, from: PlayerId, they_give: &[TradeData], we_give: &[TradeData]) -> bool {
        diplomacy::consider_offer(g, p, from, they_give, we_give)
    }

    fn attitude(&self, g: &Game, p: PlayerId, towards: PlayerId) -> AttitudeType {
        diplomacy::attitude(g, p, towards)
    }

    fn update_automated_group(&self, g: &mut Game, grp: GroupId) -> bool { automation::update(self, g, grp) }

    fn choose_event(&self, g: &Game, p: PlayerId, t: &EventTriggeredData, options: &[EventType]) -> Option<EventType> {
        events::choose(g, p, t, options)
    }

    fn choose_religion(&self, g: &Game, p: PlayerId) -> Option<ReligionType> {
        g.players[p].personality_type.and_then(|l| g.d.leader_heads[l].favorite_religion)
    }
}
