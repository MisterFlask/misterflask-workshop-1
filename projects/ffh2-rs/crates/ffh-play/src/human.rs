//! The human slot's `Controller` (design §2.3): what each rules decision does when the player is
//! human, following what the C++ does for a human (popups) rather than the AI.
//!
//! * **Deferred** (C++ queues a popup and the port's "no answer" equals an unanswered popup):
//!   production, events, diplomatic votes, elections, the research popup. The human answers at
//!   the next control point with the C++ net message's port (`command.rs`).
//! * **Blocking** (no answer would lose information): free tech, keep/raze/gift a conquered
//!   city, keep/disband an acquired city, city or settlement, take control, offer, religion. The
//!   [`Answerer`] answers inside the rules call.
//! * **Advisor**: AI work the C++ also does for humans (`AI_getAttitude`, `doResearch`'s
//!   `AI_chooseResearch`, automated groups) goes to a private `ffh_ai::Ai` (never the AI
//!   players' instance).
//! * **Fixed**: humans keep captured units and accept gifts without being asked.
//!
//! When the slot is not human (FfH AI autoplay: `setDisableHuman`), every method delegates to
//! the advisor, as a non-human `CvPlayer` is a `CvPlayerAI`.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use ffh_ai::Ai;
use ffh_core::controller::{ConqueredCityChoice, Controller};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::*;
use ffh_data::*;

use crate::decision::{self, Decision};
use crate::guard::QueryGuard;
use crate::journal::{Journal, Line, Stamp};
use crate::json::Json;
use crate::query::{Ctx, Query, QueryErr, UiState};

/// How the human answers one `Controller` method.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Answered later at a control point, as the C++ popup is.
    Deferred,
    /// Answered now, inside the rules call, by the `Answerer`.
    Blocking,
    /// The advisor's (AI) answer, as C++ computes it for humans too.
    Advisor,
    /// A fixed answer (what a C++ human always does).
    Fixed,
}

/// One entry per `Controller` method (the drift test T9 compares this with the trait).
pub const POLICIES: &[(&str, Policy)] = &[
    ("name", Policy::Fixed),
    ("play_turn", Policy::Advisor),
    ("choose_production", Policy::Deferred),
    ("choose_research", Policy::Deferred),
    ("ai_choose_research", Policy::Advisor),
    ("choose_free_tech", Policy::Blocking),
    ("raze_conquered_city", Policy::Blocking),
    ("conquered_city_choice", Policy::Blocking),
    ("disband_acquired_city", Policy::Blocking),
    ("confirm_settlement", Policy::Blocking),
    ("choose_event", Policy::Deferred),
    ("choose_election", Policy::Deferred),
    ("diplo_vote", Policy::Deferred),
    ("keep_captured_unit", Policy::Fixed),
    ("accept_unit", Policy::Fixed),
    ("consider_offer", Policy::Blocking),
    ("choose_religion", Policy::Blocking),
    ("attitude", Policy::Advisor),
    ("take_control", Policy::Blocking),
    ("update_automated_group", Policy::Advisor),
];

/// An answer to a blocking decision.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Index into `Decision::options` (an invalid or disabled index counts as `Advisor`).
    Option(usize),
    /// Take the advisor's pick ("AI default" button).
    Advisor,
}

/// Answers blocking decisions (design §2.4).
pub trait Answerer: Send + Sync {
    /// On the game thread, inside a rules call, with the `&Game` the rules handed the
    /// controller (under a `QueryGuard`: reads are free).
    fn ask(&self, g: &Game, d: &Decision) -> Answer;

    /// [`Answerer::ask`] with a reader that also answers the protocol's queries, as a client
    /// issues them while the decision's modal is open (the transports' answerers and the noisy
    /// client test use it). The default asks with the game alone.
    fn ask_with(&self, r: &DecisionReader, d: &Decision) -> Answer {
        self.ask(r.game(), d)
    }
}

/// What an [`Answerer`] can read while a blocking decision is open: the game (under the
/// decision's `QueryGuard`) and the queries of the protocol, from the deciding player's side.
/// The message log, the turn report and the saves list are not available inside a rules call
/// (`busy`); a query that panics is caught (design §2.4) and answered `busy`.
pub struct DecisionReader<'a> {
    g: &'a Game,
    shared: &'a SessionShared,
    ui: &'a Mutex<UiState>,
    decision: &'a Decision,
}

impl DecisionReader<'_> {
    /// The game, as the rules handed it to the controller (guarded).
    pub fn game(&self) -> &Game {
        self.g
    }

    pub fn decision(&self) -> &Decision {
        self.decision
    }

    /// Answers a protocol query from the deciding player's side.
    pub fn query(&self, q: &Query) -> Result<Json, QueryErr> {
        let events = self.shared.open_events_of(self.decision.player);
        let ctx = Ctx {
            h: self.decision.player,
            events: &events,
            decision: Some(self.decision),
            messages: None,
            report: None,
            in_decision: true,
            saves_dir: None,
            ui: self.ui,
        };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::query::run(self.g, &ctx, q)))
            .unwrap_or_else(|_| Err(QueryErr::Busy(format!("the {} query failed during the decision (see the panic message)", q.name()))))
    }
}

/// Always the advisor's pick (soak tests, EOF of an agent).
pub struct AdvisorAnswerer;

impl Answerer for AdvisorAnswerer {
    fn ask(&self, _g: &Game, _d: &Decision) -> Answer {
        Answer::Advisor
    }
}

/// A closure (tests, the bot).
pub struct FnAnswerer<F>(pub F);

impl<F: Fn(&Game, &Decision) -> Answer + Send + Sync> Answerer for FnAnswerer<F> {
    fn ask(&self, g: &Game, d: &Decision) -> Answer {
        (self.0)(g, d)
    }
}

/// Counts for tests and the status line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Blocking decisions answered, by kind name.
    pub decisions: Vec<(&'static str, usize)>,
    /// Event popups deferred.
    pub events_deferred: usize,
    /// The advisor's picks for the human at `doResearch`'s `AI_chooseResearch` call
    /// (`ai_choose_research`).
    pub research_ai_picks: usize,
    /// Times the rules moved the human to another slot (`reassignPlayerAdvanced`).
    pub reassigned: usize,
}

impl Stats {
    pub fn decisions_total(&self) -> usize {
        self.decisions.iter().map(|(_, n)| n).sum()
    }

    pub fn decisions_of(&self, kind: &str) -> usize {
        self.decisions.iter().find(|(k, _)| *k == kind).map_or(0, |(_, n)| *n)
    }
}

/// A deferred event popup (C++ BUTTONPOPUP_EVENT in the player's popup queue).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DeferredEvent {
    pub player: PlayerId,
    pub triggered: TriggeredId,
    /// Shown: the popup was launched at a control point (`launchEventPopup` found a doable
    /// event). Before that it is only queued; once launched it stays open (C++ fixes the popup
    /// when it is shown) until answered, closed by a forced end turn, or its data is gone.
    pub launched: bool,
}

/// Session state the controller shares with the session (design §3).
pub struct SessionShared {
    /// Deferred event popups in trigger order (C++ the player's popup queue; saved in the
    /// sidecar).
    pub pending_events: Mutex<Vec<DeferredEvent>>,
    pub next_decision: AtomicU32,
    /// The decision being asked, if any (saving is refused meanwhile).
    pub open_decision: Mutex<Option<Decision>>,
    /// The journal (answers are written in the order they are asked).
    pub journal: Mutex<Journal>,
    pub stats: Mutex<Stats>,
    /// A transport asked, while the game runs, to stop the AI autoplay (`autoplay {turns: 0}`,
    /// FfH `setAIAutoPlay(0)`): `Session::advance` applies it before its next turn slice.
    pub stop_autoplay: AtomicBool,
}

impl SessionShared {
    pub fn new(journal: Journal) -> SessionShared {
        SessionShared {
            pending_events: Mutex::new(Vec::new()),
            next_decision: AtomicU32::new(1),
            open_decision: Mutex::new(None),
            journal: Mutex::new(journal),
            stats: Mutex::new(Stats::default()),
            stop_autoplay: AtomicBool::new(false),
        }
    }

    /// Asks the running game to stop the AI autoplay before its next turn slice (a transport,
    /// while `advance` runs: `Session::advance` journals and applies it).
    pub fn request_stop_autoplay(&self) {
        self.stop_autoplay.store(true, Ordering::SeqCst);
    }

    /// Deferred events of `p` (queued or open), in order.
    pub fn events_of(&self, p: PlayerId) -> Vec<TriggeredId> {
        self.pending_events.lock().unwrap().iter().filter(|e| e.player == p).map(|e| e.triggered).collect()
    }

    /// The event popups of `p` on screen (launched), in order: what `pending` lists and the
    /// `event` command answers.
    pub fn open_events_of(&self, p: PlayerId) -> Vec<TriggeredId> {
        self.pending_events.lock().unwrap().iter().filter(|e| e.player == p && e.launched).map(|e| e.triggered).collect()
    }

    pub fn decision_open(&self) -> bool {
        self.open_decision.lock().unwrap().is_some()
    }
}

/// The `Controller` of the human slot.
pub struct HumanController {
    advisor: Arc<Ai>,
    answerer: Arc<dyn Answerer>,
    shared: Arc<SessionShared>,
    /// The session's client state (the interface path finder, debug reveal) for the queries
    /// answered during a decision.
    ui: Arc<Mutex<UiState>>,
}

impl HumanController {
    pub fn new(advisor: Arc<Ai>, answerer: Arc<dyn Answerer>, shared: Arc<SessionShared>) -> HumanController {
        HumanController::with_ui(advisor, answerer, shared, Arc::default())
    }

    /// A controller whose decisions answer queries with the session's [`UiState`].
    pub fn with_ui(advisor: Arc<Ai>, answerer: Arc<dyn Answerer>, shared: Arc<SessionShared>, ui: Arc<Mutex<UiState>>) -> HumanController {
        HumanController { advisor, answerer, shared, ui }
    }

    /// Asks the human `d` (a blocking decision) and returns the chosen option, journaled.
    fn block(&self, g: &Game, mut d: Decision) -> usize {
        d.id = self.shared.next_decision.fetch_add(1, Ordering::Relaxed);
        // The text keys ffh-core uses for names (city names, ...) are shown resolved, as in the
        // message log.
        d.resolve_keys(g.d);
        *self.shared.open_decision.lock().unwrap() = Some(d.clone());
        // The game changed since the last control point (the command or the AI turns that raised
        // the decision): the decision's view is a new one, and so is the view after it.
        UiState::lock(&self.ui).view_version += 1;
        let answer = {
            let guard = QueryGuard::new(g);
            let reader = DecisionReader { g: &guard, shared: &self.shared, ui: &self.ui, decision: &d };
            self.answerer.ask_with(&reader, &d)
        };
        *self.shared.open_decision.lock().unwrap() = None;
        UiState::lock(&self.ui).view_version += 1;
        let option = match answer {
            Answer::Option(i) if d.is_valid(i) => i,
            _ => d.default_option(),
        };
        let kind = d.kind.name();
        self.shared.journal.lock().unwrap().write(&Line::Answer { stamp: Stamp::of(g), kind: kind.to_string(), option });
        let mut stats = self.shared.stats.lock().unwrap();
        match stats.decisions.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => stats.decisions.push((kind, 1)),
        }
        option
    }

    /// An advisor computation shown to the human as a suggestion: run under a guard, so it is
    /// free of side effects on the game (C++ never computes it for a human).
    fn suggest<R>(&self, g: &Game, f: impl FnOnce(&Game) -> R) -> R {
        let guard = QueryGuard::new(g);
        f(&guard)
    }
}

impl Controller for HumanController {
    fn name(&self) -> &str {
        "human"
    }

    /// C++ `CvPlayerAI::AI_unitUpdate`, human branch: `AI_update` of each group in cycle order
    /// (only automated and FfH AI-controlled groups act) until one acts. Never blocks.
    fn play_turn(&self, g: &mut Game, p: PlayerId) {
        if !g.players[p].human {
            return self.advisor.play_turn(g, p);
        }
        if !g.player_has_busy_unit(p) {
            for grp in g.players[p].group_cycle.clone() {
                if g.groups.contains(grp) && decision::is_ai_group(g, grp) && self.advisor.update_automated_group(g, grp) {
                    break;
                }
            }
        }
    }

    /// C++ BUTTONPOPUP_CHOOSEPRODUCTION (deferred: the empty queue is `pending.idle_cities`);
    /// `AI_chooseProduction` when the city's production is automated.
    fn choose_production(&self, g: &mut Game, c: CityId) {
        let owner = g.cities[c].owner;
        if !g.players[owner].human || g.cities[c].production_automated {
            self.advisor.choose_production(g, c);
        }
    }

    /// C++ BUTTONPOPUP_CHOOSETECH (research, `chooseTech(0)`): deferred, the empty research is
    /// `pending.research_needed` and the human answers with `research` (CvNetResearch).
    fn choose_research(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        if !g.players[p].human {
            return self.advisor.choose_research(g, p);
        }
        None
    }

    /// `doResearch`'s `AI_chooseResearch` call, which C++ runs for humans too (empty research
    /// after turn 4: this turn's beakers go to the AI's pick, and the queue is cleared after).
    fn ai_choose_research(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        if g.players[p].human {
            self.shared.stats.lock().unwrap().research_ai_picks += 1;
        }
        self.advisor.ai_choose_research(g, p)
    }

    /// C++ CHOOSETECH popup with free techs: blocking (no answer = the tech is lost).
    fn choose_free_tech(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        if !g.players[p].human {
            return self.advisor.choose_free_tech(g, p);
        }
        let techs = decision::free_tech_options(g, p);
        if techs.is_empty() {
            return None;
        }
        let advice = self.suggest(g, |g| self.advisor.choose_free_tech(g, p));
        let i = self.block(g, decision::free_tech(g, p, &techs, advice));
        Some(techs[i])
    }

    /// `AI_conquerCity`, which rules code asks only for AI conquerors (a human conqueror gets
    /// `conquered_city_choice`): for a human, the same keep/raze modal without a gift button.
    fn raze_conquered_city(&self, g: &Game, p: PlayerId, c: CityId) -> bool {
        if !g.players[p].human {
            return self.advisor.raze_conquered_city(g, p, c);
        }
        let can_raze = g.player_can_raze(p, c);
        let advice = self.suggest(g, |g| self.advisor.conquered_city_choice(g, p, c, can_raze, None));
        let d = decision::keep_city(g, p, c, can_raze, None, advice);
        let i = self.block(g, d.clone());
        decision::keep_city_choice(&d, i) == ConqueredCityChoice::Raze
    }

    /// C++ BUTTONPOPUP_RAZECITY: blocking keep / raze / gift (the buttons offered).
    fn conquered_city_choice(&self, g: &Game, p: PlayerId, c: CityId, can_raze: bool, gift_to: Option<PlayerId>) -> ConqueredCityChoice {
        if !g.players[p].human {
            return self.advisor.conquered_city_choice(g, p, c, can_raze, gift_to);
        }
        let advice = self.suggest(g, |g| self.advisor.conquered_city_choice(g, p, c, can_raze, gift_to));
        let d = decision::keep_city(g, p, c, can_raze, gift_to, advice);
        let i = self.block(g, d.clone());
        decision::keep_city_choice(&d, i)
    }

    /// C++ BUTTONPOPUP_DISBANDCITY: blocking keep (0) / disband (1).
    fn disband_acquired_city(&self, g: &Game, p: PlayerId, c: CityId) -> bool {
        if !g.players[p].human {
            return self.advisor.disband_acquired_city(g, p, c);
        }
        let advice = self.suggest(g, |g| self.advisor.disband_acquired_city(g, p, c));
        self.block(g, decision::disband_city(g, p, c, advice)) == 1
    }

    /// FfH BUTTONPOPUP_CONFIRMSETTLEMENT: blocking city (0) / settlement (1). The advisor
    /// suggests what C++ gives an AI player: a city.
    fn confirm_settlement(&self, g: &Game, p: PlayerId, c: CityId) -> Option<bool> {
        if !g.players[p].human {
            return self.advisor.confirm_settlement(g, p, c);
        }
        let advice = self.suggest(g, |g| self.advisor.confirm_settlement(g, p, c)).unwrap_or(false);
        Some(self.block(g, decision::settlement(g, p, c, advice)) == 1)
    }

    /// C++ `CvPlayer::trigger` for a human queues BUTTONPOPUP_EVENT: deferred (F7), the
    /// triggered data stays pending until answered with `event`.
    fn choose_event(&self, g: &Game, p: PlayerId, t: &EventTriggeredData, options: &[EventType]) -> Option<EventType> {
        if !g.players[p].human {
            return self.advisor.choose_event(g, p, t, options);
        }
        let mut pending = self.shared.pending_events.lock().unwrap();
        if !pending.iter().any(|e| (e.player, e.triggered) == (p, t.id)) {
            pending.push(DeferredEvent { player: p, triggered: t.id, launched: false });
            self.shared.stats.lock().unwrap().events_deferred += 1;
        }
        None
    }

    /// C++ BUTTONPOPUP_CHOOSEELECTION: deferred. ffh-core keeps the selection while a human's
    /// answer is `None`; it is `pending.elections` until the human answers with `election`
    /// (CvNetChooseElection).
    fn choose_election(&self, g: &Game, p: PlayerId, vs: &VoteSelectionData) -> Option<usize> {
        if !g.players[p].human {
            return self.advisor.choose_election(g, p, vs);
        }
        None
    }

    /// C++ BUTTONPOPUP_DIPLOVOTE: deferred (F8); the vote is pending until `vote`.
    fn diplo_vote(&self, g: &Game, p: PlayerId, vote: &VoteSelectionSubData, source: VoteSourceType) -> PlayerVote {
        if !g.players[p].human {
            return self.advisor.diplo_vote(g, p, vote, source);
        }
        PlayerVote::NO_PLAYER_VOTE
    }

    fn keep_captured_unit(&self, g: &Game, p: PlayerId, unit: UnitType, plot: PlotIdx) -> bool {
        if !g.players[p].human {
            return self.advisor.keep_captured_unit(g, p, unit, plot);
        }
        true
    }

    fn accept_unit(&self, g: &Game, p: PlayerId, unit: UnitId) -> bool {
        if !g.players[p].human {
            return self.advisor.accept_unit(g, p, unit);
        }
        true
    }

    /// A deal offered to the human: blocking accept (0) / refuse (1).
    fn consider_offer(&self, g: &Game, p: PlayerId, from: PlayerId, they_give: &[TradeData], we_give: &[TradeData]) -> bool {
        if !g.players[p].human {
            return self.advisor.consider_offer(g, p, from, they_give, we_give);
        }
        let advice = self.suggest(g, |g| self.advisor.consider_offer(g, p, from, they_give, we_give));
        self.block(g, decision::offer(g, p, from, they_give, we_give, advice)) == 0
    }

    /// C++ BUTTONPOPUP_FOUND_RELIGION: blocking (unreachable while FfH locks PICK_RELIGION off).
    fn choose_religion(&self, g: &Game, p: PlayerId) -> Option<ReligionType> {
        if !g.players[p].human {
            return self.advisor.choose_religion(g, p);
        }
        let religions = decision::religion_options(g);
        if religions.is_empty() {
            return None;
        }
        let advice = self.suggest(g, |g| self.advisor.choose_religion(g, p));
        let i = self.block(g, decision::religion(g, p, &religions, advice));
        Some(religions[i])
    }

    /// C++ computes `AI_getAttitude` for humans too: a function of the game state.
    fn attitude(&self, g: &Game, p: PlayerId, towards: PlayerId) -> AttitudeType {
        self.advisor.attitude(g, p, towards)
    }

    /// FfH's take-control popup (Infernal Pact, Mercurian Gate): blocking yes (0) / no (1).
    fn take_control(&self, g: &Game, p: PlayerId, new: PlayerId) -> bool {
        if !g.players[p].human {
            return self.advisor.take_control(g, p, new);
        }
        let advice = self.suggest(g, |g| self.advisor.take_control(g, p, new));
        self.block(g, decision::take_control(g, p, new, advice)) == 0
    }

    fn update_automated_group(&self, g: &mut Game, grp: GroupId) -> bool {
        self.advisor.update_automated_group(g, grp)
    }
}
