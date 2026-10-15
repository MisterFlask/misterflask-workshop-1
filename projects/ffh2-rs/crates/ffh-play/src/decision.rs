//! Decisions and pending items (design §2.3).
//!
//! * A [`Decision`] is a **blocking** question a rules call asks the human (the C++ popups whose
//!   "no answer" would lose information): free tech, keep/abandon a city, election, offer,
//!   religion. It is answered inside the rules call through the session's `Answerer`.
//! * [`Pending`] lists what the human answers at a control point with a journaled command: the
//!   **deferred** popups (events, votes, research, idle cities), plus promotions and ready
//!   groups. Everything but the deferred event ids is derived from `Game`.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::*;
use ffh_data::*;

use crate::json::Json;
use crate::names;

/// What a blocking decision is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionKind {
    /// C++ BUTTONPOPUP_CHOOSETECH with `iDiscover > 0`: the options are techs.
    FreeTech,
    /// C++ BUTTONPOPUP_RAZECITY / DISBANDCITY: option 0 keeps the city, 1 abandons it (raze,
    /// or disband when `can_raze` is false or the city came by culture or a spell).
    KeepCity { city: CityId, can_raze: bool },
    /// C++ BUTTONPOPUP_CHOOSEELECTION: one option per resolution, then "none".
    Election { selection: VoteId },
    /// A deal offered to the human (C++ diplomacy screen): option 0 accepts, 1 refuses.
    Offer { from: PlayerId },
    /// C++ BUTTONPOPUP_FOUND_RELIGION: the options are religions.
    Religion,
}

impl DecisionKind {
    /// The protocol name (`status.decision.kind`, journal `answer` lines).
    pub fn name(&self) -> &'static str {
        match self {
            DecisionKind::FreeTech => "free_tech",
            DecisionKind::KeepCity { .. } => "keep_city",
            DecisionKind::Election { .. } => "election",
            DecisionKind::Offer { .. } => "offer",
            DecisionKind::Religion => "religion",
        }
    }
}

/// One button of a decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionOption {
    /// Stable key: a type name (`TECH_X`), or `keep` / `abandon` / `accept` / `refuse` / `none`.
    pub key: String,
    pub label: String,
    pub help: String,
    pub enabled: bool,
}

impl DecisionOption {
    fn new(key: impl Into<String>, label: impl Into<String>) -> DecisionOption {
        DecisionOption { key: key.into(), label: label.into(), help: String::new(), enabled: true }
    }
}

/// Where a decision happens, for the client to show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionContext {
    pub plot: Option<(i32, i32)>,
    pub city: Option<CityId>,
    pub unit: Option<UnitId>,
    pub other_player: Option<PlayerId>,
}

/// A blocking question (design §3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    /// Session-local id (answers name it; not stable across save/load).
    pub id: u32,
    pub player: PlayerId,
    pub kind: DecisionKind,
    pub title: String,
    pub text: String,
    pub options: Vec<DecisionOption>,
    /// The advisor's pick (index into `options`), if it has one.
    pub advisor: Option<usize>,
    pub context: DecisionContext,
}

impl Decision {
    /// Is `i` an enabled option?
    pub fn is_valid(&self, i: usize) -> bool {
        self.options.get(i).is_some_and(|o| o.enabled)
    }

    /// The option taken when the answer is "advisor" (or not a valid option): the advisor's pick,
    /// else the first enabled option.
    pub fn default_option(&self) -> usize {
        self.advisor.filter(|&i| self.is_valid(i)).or_else(|| self.options.iter().position(|o| o.enabled)).unwrap_or(0)
    }

    /// Resolves the `TXT_KEY_*` tokens of the title, the text and the options' labels and help
    /// (ffh-core keeps some names as text keys: city names, ...); the option keys stay as they
    /// are.
    pub fn resolve_keys(&mut self, d: &GameData) {
        let resolve = |s: &mut String| {
            if let std::borrow::Cow::Owned(t) = names::resolve_keys(d, s) {
                *s = t;
            }
        };
        resolve(&mut self.title);
        resolve(&mut self.text);
        for o in &mut self.options {
            resolve(&mut o.label);
            resolve(&mut o.help);
        }
    }

    pub fn to_json(&self) -> Json {
        let c = &self.context;
        crate::jobj! {
            "id" => self.id,
            "player" => self.player,
            "kind" => self.kind.name(),
            "title" => self.title.as_str(),
            "text" => self.text.as_str(),
            "options" => Json::Arr(self.options.iter().map(|o| crate::jobj! {
                "key" => o.key.as_str(), "label" => o.label.as_str(), "help" => o.help.as_str(), "enabled" => o.enabled,
            }).collect()),
            "advisor" => self.advisor,
            "context" => crate::jobj! {
                "plot" => c.plot.map(|(x, y)| crate::jarr![x, y]),
                "city" => c.city,
                "unit" => c.unit,
                "other_player" => c.other_player,
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Decision builders: the options each blocking popup offers, and how an answer maps back.
// ---------------------------------------------------------------------------------------------

/// Free tech: the techs `canResearch(eTech, false)` accepts (`launchChooseTechPopup`).
pub fn free_tech_options(g: &Game, p: PlayerId) -> Vec<TechType> {
    g.d.techs.keys().filter(|&t| g.player_can_research(p, t, false)).collect()
}

pub fn free_tech(g: &Game, p: PlayerId, techs: &[TechType], advisor: Option<TechType>) -> Decision {
    let d = g.d;
    Decision {
        id: 0,
        player: p,
        kind: DecisionKind::FreeTech,
        title: "Free technology".into(),
        text: "Choose a technology to receive for free.".into(),
        options: techs.iter().map(|&t| DecisionOption::new(names::name(d, t), names::display(d, t))).collect(),
        advisor: advisor.and_then(|a| techs.iter().position(|&t| t == a)),
        context: DecisionContext::default(),
    }
}

/// Keep or abandon an acquired city (F10: the conquest and culture/spell popups ask the same
/// controller question, so one neutral modal serves both).
pub fn keep_city(g: &Game, p: PlayerId, c: CityId, advisor_razes: bool) -> Decision {
    let city = &g.cities[c];
    let can_raze = g.player_can_raze(p, c);
    let (title, abandon, help) = if can_raze {
        ("Raze or keep?", "Raze", "Raze: the city is destroyed. (Also asked when a city joins you by culture or a spell; then abandoning disbands it.)")
    } else {
        ("Disband?", "Disband", "Disband: the city is removed.")
    };
    let mut abandon_opt = DecisionOption::new("abandon", abandon);
    abandon_opt.help = help.into();
    Decision {
        id: 0,
        player: p,
        kind: DecisionKind::KeepCity { city: c, can_raze },
        title: title.into(),
        text: format!("{} (size {}) is now yours.", city.name, city.population),
        options: vec![DecisionOption::new("keep", "Keep"), abandon_opt],
        advisor: Some(advisor_razes as usize),
        context: DecisionContext { plot: Some((city.x, city.y)), city: Some(c), unit: None, other_player: city.previous_owner },
    }
}

/// Election: one option per resolution of the selection, then "none" (the popup's
/// TXT_KEY_NONE button). The answer `None` = no election (C++ sends nothing).
pub fn election(g: &Game, p: PlayerId, vs: &VoteSelectionData, advisor: Option<usize>) -> Decision {
    let d = g.d;
    let mut options: Vec<DecisionOption> = vs
        .vote_options
        .iter()
        .map(|o| DecisionOption::new(names::name(d, o.vote), if o.text.is_empty() { names::display(d, o.vote).into_owned() } else { o.text.clone() }))
        .collect();
    options.push(DecisionOption::new("none", "None"));
    let none = options.len() - 1;
    Decision {
        id: 0,
        player: p,
        kind: DecisionKind::Election { selection: vs.id },
        title: names::display(d, vs.vote_source).into_owned(),
        text: "As secretary general, choose the resolution to put to the vote.".into(),
        options,
        advisor: Some(advisor.filter(|&i| i < none).unwrap_or(none)),
        context: DecisionContext::default(),
    }
}

/// A deal offered to the human.
pub fn offer(g: &Game, p: PlayerId, from: PlayerId, they_give: &[TradeData], we_give: &[TradeData], advisor_accepts: bool) -> Decision {
    let items = |v: &[TradeData]| v.iter().map(trade_item_text).collect::<Vec<_>>().join(", ");
    Decision {
        id: 0,
        player: p,
        kind: DecisionKind::Offer { from },
        title: format!("Offer from {}", crate::view::player_name(g, from)),
        text: format!("They give: {}. You give: {}.", items(they_give), items(we_give)),
        options: vec![DecisionOption::new("accept", "Accept"), DecisionOption::new("refuse", "Refuse")],
        advisor: Some(if advisor_accepts { 0 } else { 1 }),
        context: DecisionContext { other_player: Some(from), ..Default::default() },
    }
}

fn trade_item_text(t: &TradeData) -> String {
    let name = TradeableItem::NAMES.get(t.item_type.0 as usize).copied().unwrap_or("TRADE_?");
    format!("{} {}", name.trim_start_matches("TRADE_"), t.data)
}

/// Religion choice (BUTTONPOPUP_FOUND_RELIGION): religions not founded yet.
pub fn religion_options(g: &Game) -> Vec<ReligionType> {
    g.d.religions.keys().filter(|&r| !g.is_religion_founded(r)).collect()
}

pub fn religion(g: &Game, p: PlayerId, religions: &[ReligionType], advisor: Option<ReligionType>) -> Decision {
    let d = g.d;
    Decision {
        id: 0,
        player: p,
        kind: DecisionKind::Religion,
        title: "Found a religion".into(),
        text: "Choose the religion to found.".into(),
        options: religions.iter().map(|&r| DecisionOption::new(names::name(d, r), names::display(d, r))).collect(),
        advisor: advisor.and_then(|a| religions.iter().position(|&r| r == a)),
        context: DecisionContext::default(),
    }
}

// ---------------------------------------------------------------------------------------------
// Pending items (deferred popups and other things waiting for the human)
// ---------------------------------------------------------------------------------------------

/// A deferred event popup (C++ BUTTONPOPUP_EVENT).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingEvent {
    pub triggered: TriggeredId,
    pub trigger: EventTriggerType,
    pub text: String,
    /// The trigger's events in order (disabled ones are shown greyed, as the popup does).
    pub options: Vec<EventOption>,
    pub city: Option<CityId>,
    pub plot: Option<(i32, i32)>,
}

/// One button of an event popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventOption {
    pub event: EventType,
    /// `canDoEvent`.
    pub enabled: bool,
    /// What it does, from the `EventInfo` fields (the help text `CvGameTextMgr::setEventHelp`
    /// would show is not ported).
    pub effects: Vec<String>,
}

/// A summary of an event's effects for player `p` (gold with inflation and tech cost, techs,
/// units, buildings, promotions, happiness and health, culture, attitude, population, ...).
pub fn event_effects(g: &Game, p: PlayerId, event: EventType, data: &EventTriggeredData) -> Vec<String> {
    let d = g.d;
    let e = &d.events[event];
    let mut out = Vec::new();
    let signed = |n: i32| format!("{n:+}");
    let (gold, gold_max) = (g.player_get_event_cost(p, event, data.other_player, false), g.player_get_event_cost(p, event, data.other_player, true));
    if gold != 0 || gold_max != 0 {
        out.push(if gold == gold_max { format!("{} gold", signed(gold)) } else { format!("{}..{} gold", signed(gold), signed(gold_max)) });
    }
    if e.tech_percent != 0 {
        let tech = g.player_get_best_event_tech(p, event, data.other_player).map_or_else(|| "a tech".to_string(), |t| names::display(d, t).into_owned());
        out.push(format!("{}% of {tech}", e.tech_percent));
    }
    if let (Some(class), n) = (e.unit_class, e.num_units) {
        if n != 0 {
            out.push(format!("{n} x {}", names::display(d, class)));
        }
    }
    if let Some(class) = e.building_class {
        if e.building_change != 0 {
            out.push(format!("{} {}", if e.building_change > 0 { "build" } else { "lose" }, names::display(d, class)));
        }
    }
    if let Some(promo) = e.unit_promotion {
        out.push(format!("promotion {}", names::display(d, promo)));
    }
    if e.unit_experience != 0 {
        out.push(format!("{} experience", signed(e.unit_experience)));
    }
    let lasting = |n: i32, what: &str| {
        if e.happy_turns > 0 {
            format!("{} {what} for {} turns", signed(n), e.happy_turns)
        } else {
            format!("{} {what}", signed(n))
        }
    };
    if e.happy != 0 {
        out.push(lasting(e.happy, "happiness"));
    }
    if e.health != 0 {
        out.push(lasting(e.health, "health"));
    }
    if e.hurry_anger != 0 {
        out.push(format!("{} hurry anger", signed(e.hurry_anger)));
    }
    for (n, what) in [
        (e.culture, "culture"),
        (e.food, "food"),
        (e.food_percent, "% food"),
        (e.population_change, "population"),
        (e.espionage_points, "espionage"),
        (e.free_unit_support, "unit support"),
        (e.global_counter, "Armageddon counter"),
        (e.crime, "crime"),
    ] {
        if n != 0 {
            out.push(format!("{} {what}", signed(n)));
        }
    }
    if e.revolt_turns > 0 {
        out.push(format!("{} turns of revolt", e.revolt_turns));
    }
    if e.our_attitude_modifier != 0 || e.attitude_modifier != 0 {
        out.push(format!("attitude {}", signed(e.our_attitude_modifier + e.attitude_modifier)));
    }
    if e.golden_age {
        out.push("golden age".into());
    }
    if e.declare_war {
        out.push("war".into());
    }
    if e.disband_unit {
        out.push("the unit is lost".into());
    }
    if let Some(f) = e.feature.filter(|_| e.feature_change != 0) {
        out.push(format!("{} {}", if e.feature_change > 0 { "add" } else { "remove" }, names::display(d, f)));
    }
    if let Some(i) = e.improvement.filter(|_| e.improvement_change != 0) {
        out.push(format!("{} {}", if e.improvement_change > 0 { "add" } else { "remove" }, names::display(d, i)));
    }
    if let Some(b) = e.bonus.filter(|_| e.bonus_change != 0) {
        out.push(format!("{} {}", if e.bonus_change > 0 { "add" } else { "remove" }, names::display(d, b)));
    }
    if let Some(r) = e.route.filter(|_| e.route_change != 0) {
        out.push(format!("{} {}", if e.route_change > 0 { "add" } else { "remove" }, names::display(d, r)));
    }
    if let Some(b) = e.bonus_revealed {
        out.push(format!("reveal {}", names::display(d, b)));
    }
    if let Some(b) = e.bonus_gift {
        out.push(format!("gift of {}", names::display(d, b)));
    }
    if !e.python_help.is_empty() {
        let help = names::text(d, &e.python_help);
        if !help.is_empty() && help != e.python_help {
            out.push(help.to_string());
        }
    }
    if !e.python_callback.is_empty() {
        // FfH implements many events in Python (ported to `ffh::events`): not summarized.
        out.push(format!("scripted effect ({})", e.python_callback));
    }
    out
}

/// A diplomatic vote waiting for the human (C++ BUTTONPOPUP_DIPLOVOTE).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingVote {
    pub vote: VoteId,
    pub source: VoteSourceType,
    pub resolution: VoteType,
    pub text: String,
    /// The popup's buttons (`launchDiploVotePopup`).
    pub choices: Vec<PlayerVote>,
    /// Already given its extra turn (`NO_PLAYER_VOTE_CHECKED`): abstains if unanswered this turn.
    pub last_turn: bool,
}

/// Everything the human answers or acts on at a control point (design §3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pending {
    pub events: Vec<PendingEvent>,
    pub votes: Vec<PendingVote>,
    pub research_needed: bool,
    pub idle_cities: Vec<CityId>,
    pub promotions_ready: Vec<UnitId>,
    pub ready_groups: Vec<GroupId>,
    /// Groups the human cannot order that wait for AI moves (automated or FfH AI-controlled;
    /// they idle until ffh-ai implements `update_automated_group`, F15).
    pub ai_groups_idle: Vec<GroupId>,
}

impl Pending {
    /// Deferred popups still open (the end-turn gate's events, votes and research).
    pub fn popups(&self) -> usize {
        self.events.len() + self.votes.len() + self.research_needed as usize
    }

    pub fn to_json(&self) -> Json {
        crate::jobj! {
            "events" => Json::Arr(self.events.iter().map(|e| crate::jobj! {
                "triggered" => e.triggered,
                "trigger" => Json::idx(e.trigger),
                "text" => e.text.as_str(),
                "options" => Json::Arr(e.options.iter().map(|o| crate::jobj! {
                    "event" => Json::idx(o.event),
                    "enabled" => o.enabled,
                    "effects" => Json::Arr(o.effects.iter().map(Json::from).collect()),
                }).collect()),
                "city" => e.city,
                "plot" => e.plot.map(|(x, y)| crate::jarr![x, y]),
            }).collect()),
            "votes" => Json::Arr(self.votes.iter().map(|v| crate::jobj! {
                "vote" => v.vote,
                "source" => Json::idx(v.source),
                "resolution" => Json::idx(v.resolution),
                "text" => v.text.as_str(),
                "choices" => Json::Arr(v.choices.iter().map(|c| Json::Int(c.0 as i64)).collect()),
                "last_turn" => v.last_turn,
            }).collect()),
            "research_needed" => self.research_needed,
            "idle_cities" => self.idle_cities.clone(),
            "promotions_ready" => self.promotions_ready.clone(),
            "ready_groups" => self.ready_groups.clone(),
            "ai_groups_idle" => self.ai_groups_idle.clone(),
        }
    }
}

/// Would `launchEventPopup` show the popup for triggered data `id` of `p`: the data exists and
/// at least one of the trigger's events can be done (else the popup is dropped unshown).
pub fn event_available(g: &Game, p: PlayerId, id: TriggeredId) -> bool {
    g.player_get_event_triggered(p, id).is_some_and(|data| g.d.event_triggers[data.trigger].events.iter().any(|&e| g.player_can_do_event(p, e, data)))
}

/// The event popup for triggered data `id` of `p` as `launchEventPopup` builds it, with each
/// option enabled as `canDoEvent` says now (`None` when the data is gone). An open popup stays
/// listed when no option can be done at the moment.
pub fn pending_event(g: &Game, p: PlayerId, id: TriggeredId) -> Option<PendingEvent> {
    let data = g.player_get_event_triggered(p, id)?;
    let trigger = &g.d.event_triggers[data.trigger];
    let enabled: Vec<bool> = trigger.events.iter().map(|&e| g.player_can_do_event(p, e, data)).collect();
    let options = trigger.events.iter().zip(enabled).map(|(&event, enabled)| EventOption { event, enabled, effects: event_effects(g, p, event, data) }).collect();
    let plot = (data.plot_x != ffh_core::defs::INVALID_PLOT_COORD && data.plot_y != ffh_core::defs::INVALID_PLOT_COORD).then_some((data.plot_x, data.plot_y));
    Some(PendingEvent { triggered: id, trigger: data.trigger, text: data.text.clone(), options, city: data.city, plot })
}

/// The buttons of the DIPLOVOTE popup for `p` (`CvDLLButtonPopup::launchDiploVotePopup`): in a
/// team vote, the alive eligible teams (only the first eligible master and `p`'s own team for a
/// vassal of an eligible team), plus ABSTAIN unless `p` is such a vassal (or no team was offered);
/// otherwise YES, NO, NEVER (if `canDefyResolution`) and ABSTAIN.
pub fn vote_choices(g: &Game, p: PlayerId, data: &VoteTriggeredData) -> Vec<PlayerVote> {
    let vote = data.vote_option.vote;
    let source = data.vote_source;
    let team = g.players[p].team;
    let mut vassal_of = None;
    let mut eligible = false;
    let mut out = Vec::new();
    if g.is_team_vote(vote) {
        for t in g.civ_team_ids() {
            if g.team_is_alive(t) && g.is_team_vote_eligible(t, source) && g.teams[team].vassal[t] {
                vassal_of = Some(t);
                break;
            }
        }
        for t in g.civ_team_ids() {
            if g.team_is_alive(t) && g.is_team_vote_eligible(t, source) && (vassal_of.is_none() || vassal_of == Some(t) || t == team) {
                out.push(PlayerVote::for_team(t));
                eligible = true;
            }
        }
    } else {
        out.push(PlayerVote::PLAYER_VOTE_YES);
        out.push(PlayerVote::PLAYER_VOTE_NO);
        if g.player_can_defy_resolution(p, source, &data.vote_option) {
            out.push(PlayerVote::PLAYER_VOTE_NEVER);
        }
        eligible = true;
    }
    if vassal_of.is_none() || !eligible {
        out.push(PlayerVote::PLAYER_VOTE_ABSTAIN);
    }
    out
}

/// The votes waiting for `p`: triggered votes of a source `p` votes in, not yet answered.
pub fn pending_votes(g: &Game, p: PlayerId) -> Vec<PendingVote> {
    let mut out = Vec::new();
    if p.idx() >= g.num_civ_players() {
        return out;
    }
    for data in &g.votes_triggered {
        if !g.player_is_voting_member(p, data.vote_source) {
            continue;
        }
        let v = g.player_vote(p, data.id);
        if v != PlayerVote::NO_PLAYER_VOTE && v != PlayerVote::NO_PLAYER_VOTE_CHECKED {
            continue;
        }
        out.push(PendingVote {
            vote: data.id,
            source: data.vote_source,
            resolution: data.vote_option.vote,
            text: data.vote_option.text.clone(),
            choices: vote_choices(g, p, data),
            last_turn: v == PlayerVote::NO_PLAYER_VOTE_CHECKED,
        });
    }
    out
}

/// The research popup is due: no current research and something can be researched.
pub fn research_needed(g: &Game, p: PlayerId) -> bool {
    g.game_state != GameStateType::GAMESTATE_OVER
        && g.player_get_current_research(p).is_none()
        && g.d.techs.keys().any(|t| g.player_can_research(p, t, false))
}

/// A group the AI would move (automated, or holding an FfH AI-controlled unit).
pub fn is_ai_group(g: &Game, grp: GroupId) -> bool {
    g.group_is_automated(grp) || g.group_is_ai_control(grp)
}

/// Groups ready for orders: C++ `readyToMove(bAny = true)` (what `hasReadyUnit(true)` and the
/// end-turn gate count), without the AI-moved groups (C++ moves those in `AI_unitUpdate`).
pub fn ready_groups(g: &Game, p: PlayerId) -> Vec<GroupId> {
    g.players[p].groups.iter().copied().filter(|&grp| g.group_ready_to_move(grp, true) && !is_ai_group(g, grp)).collect()
}

/// A group Civ4's group cycle selects: C++ `CvSelectionGroup::readyToSelect()` (`bAny = false`:
/// every unit can move, `canAllMove`), as `CvPlayer::cycleSelectionGroups`, `cyclePlotUnits(..,
/// bAuto)` and `CvMap::findSelectionGroup` test it, without the AI-moved groups. A stack in
/// which one unit has moved all its moves is ready (`ready_groups`) but not selectable; only
/// `updateSelectionList`'s "does the selected group still wait" check uses `readyToSelect(true)`.
pub fn is_selectable_group(g: &Game, grp: GroupId) -> bool {
    g.group_ready_to_select(grp, false) && !is_ai_group(g, grp)
}

/// [`Pending`] for `p`, with the open event popups in `events` (the session's launched ones).
pub fn pending(g: &Game, p: PlayerId, events: &[TriggeredId]) -> Pending {
    let pl = &g.players[p];
    Pending {
        events: events.iter().filter_map(|&id| pending_event(g, p, id)).collect(),
        votes: pending_votes(g, p),
        research_needed: research_needed(g, p),
        idle_cities: pl.cities.iter().copied().filter(|&c| !g.city_is_production(c) && !g.cities[c].production_automated).collect(),
        promotions_ready: pl.units.iter().copied().filter(|&u| g.units[u].promotion_ready).collect(),
        ready_groups: ready_groups(g, p),
        ai_groups_idle: pl.groups.iter().copied().filter(|&grp| g.group_ready_to_move(grp, true) && is_ai_group(g, grp)).collect(),
    }
}
