//! Decisions and pending items (design §2.3).
//!
//! * A [`Decision`] is a **blocking** question a rules call asks the human (the C++ popups whose
//!   "no answer" would lose information): free tech, keep/raze/gift a conquered city, keep or
//!   disband an acquired city, city or settlement (Kuriotates), take control of a new
//!   civilization, offer, religion. It is answered inside the rules call through the session's
//!   `Answerer`.
//! * [`Pending`] lists what the human answers at a control point with a journaled command: the
//!   **deferred** popups (events, votes, elections, research, idle cities), plus promotions and
//!   ready groups. Everything but the deferred event ids is derived from `Game`.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::*;
use ffh_data::*;

use crate::json::Json;
use crate::names;
use ffh_core::controller::ConqueredCityChoice;

/// What a blocking decision is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionKind {
    /// C++ BUTTONPOPUP_CHOOSETECH with `iDiscover > 0`: the options are techs.
    FreeTech,
    /// C++ BUTTONPOPUP_RAZECITY (a human's conquest): keep, then raze when `can_raze`, then
    /// give to `gift_to` (the liberation player) when it is offered.
    KeepCity { city: CityId, can_raze: bool, gift_to: Option<PlayerId> },
    /// C++ BUTTONPOPUP_DISBANDCITY (a city that came by culture, a spell or an event): option 0
    /// keeps it, 1 disbands it.
    DisbandCity { city: CityId },
    /// FfH BUTTONPOPUP_CONFIRMSETTLEMENT (a Kuriotates city below the limit): option 0 makes a
    /// city (radius 3), 1 a settlement.
    Settlement { city: CityId },
    /// FfH's "take control" popup after the Infernal Pact or the Mercurian Gate: option 0 plays
    /// `player` from now on, 1 keeps the human's civilization.
    TakeControl { player: PlayerId },
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
            DecisionKind::DisbandCity { .. } => "disband_city",
            DecisionKind::Settlement { .. } => "settlement",
            DecisionKind::TakeControl { .. } => "take_control",
            DecisionKind::Offer { .. } => "offer",
            DecisionKind::Religion => "religion",
        }
    }
}

/// One button of a decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionOption {
    /// Stable key: a type name (`TECH_X`), or `keep` / `raze` / `gift` / `disband` / `city` /
    /// `settlement` / `yes` / `no` / `accept` / `refuse`.
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

fn city_context(g: &Game, c: CityId, other: Option<PlayerId>) -> DecisionContext {
    let city = &g.cities[c];
    DecisionContext { plot: Some((city.x, city.y)), city: Some(c), unit: None, other_player: other }
}

/// BUTTONPOPUP_RAZECITY (`launchRazeCityPopup`): keep (button 0), raze (button 1, when
/// `canRaze`), return the city to its liberation player (button 2, when offered). The options
/// are the buttons offered, in that order ([`keep_city_choice`] maps an answer back).
pub fn keep_city(g: &Game, p: PlayerId, c: CityId, can_raze: bool, gift_to: Option<PlayerId>, advisor: ConqueredCityChoice) -> Decision {
    let city = &g.cities[c];
    let mut options = vec![DecisionOption::new("keep", "Keep")];
    if can_raze {
        let mut o = DecisionOption::new("raze", "Raze");
        o.help = "The city is destroyed.".into();
        options.push(o);
    }
    if let Some(to) = gift_to {
        let mut o = DecisionOption::new("gift", format!("Return it to {}", crate::view::player_name(g, to)));
        o.help = "The city goes back to the civilization with the most culture in it, a teammate, vassal or open-borders partner of yours.".into();
        options.push(o);
    }
    let key = match advisor {
        ConqueredCityChoice::Keep => "keep",
        ConqueredCityChoice::Raze => "raze",
        ConqueredCityChoice::Gift => "gift",
    };
    Decision {
        id: 0,
        player: p,
        kind: DecisionKind::KeepCity { city: c, can_raze, gift_to },
        title: if can_raze { "Raze or keep?".into() } else { "Keep the city?".into() },
        text: format!("You have captured {} (size {}).", city.name, city.population),
        advisor: options.iter().position(|o| o.key == key),
        options,
        context: city_context(g, c, city.previous_owner),
    }
}

/// The popup button chosen in a [`keep_city`] decision.
pub fn keep_city_choice(d: &Decision, option: usize) -> ConqueredCityChoice {
    match d.options.get(option).map(|o| o.key.as_str()) {
        Some("raze") => ConqueredCityChoice::Raze,
        Some("gift") => ConqueredCityChoice::Gift,
        _ => ConqueredCityChoice::Keep,
    }
}

/// BUTTONPOPUP_DISBANDCITY (`launchDisbandCityPopup`): keep (button 0) or disband (button 1).
pub fn disband_city(g: &Game, p: PlayerId, c: CityId, advisor_disbands: bool) -> Decision {
    let city = &g.cities[c];
    let mut disband = DecisionOption::new("disband", "Disband");
    disband.help = "The city is removed.".into();
    Decision {
        id: 0,
        player: p,
        kind: DecisionKind::DisbandCity { city: c },
        title: "Keep the city?".into(),
        text: format!("{} (size {}) has joined your civilization. Keep it or disband it?", city.name, city.population),
        options: vec![DecisionOption::new("keep", "Keep"), disband],
        advisor: Some(advisor_disbands as usize),
        context: city_context(g, c, city.previous_owner),
    }
}

/// BUTTONPOPUP_CONFIRMSETTLEMENT (`launchConfirmSettlementPopup`): a city (button 0: radius 3)
/// or a settlement (button 1).
pub fn settlement(g: &Game, p: PlayerId, c: CityId, advisor_settlement: bool) -> Decision {
    let city = &g.cities[c];
    let mut as_city = DecisionOption::new("city", "City");
    as_city.help = format!("A full city that also works the outer ring of plots (radius 3); you may have {} cities.", g.players[p].max_cities);
    let mut as_settlement = DecisionOption::new("settlement", "Settlement");
    as_settlement.help = "Settlements produce no gold, research or culture, nothing can be built in them, and they cost no maintenance (they do not count against the city limit).".into();
    Decision {
        id: 0,
        player: p,
        kind: DecisionKind::Settlement { city: c },
        title: format!("{}: city or settlement?", city.name),
        text: names::text(g.d, "TXT_KEY_POPUP_SETTLEMENT_OR_CITY").to_string(),
        options: vec![as_city, as_settlement],
        advisor: Some(advisor_settlement as usize),
        context: city_context(g, c, None),
    }
}

/// FfH's take-control popup (TXT_KEY_POPUP_CONTROL_INFERNAL / _MERCURIANS): yes (button 0) or
/// no (button 1).
pub fn take_control(g: &Game, p: PlayerId, new: PlayerId, advisor_takes: bool) -> Decision {
    let name = crate::view::player_name(g, new);
    let mut yes = DecisionOption::new("yes", "Yes");
    yes.help = format!("You play {name} from now on; the AI plays your current civilization.");
    // The popup's text by the civilization that appeared (CvEventManager.py).
    let civ = g.player_get_civilization_type(new).map(|c| g.d.type_name(c));
    let text = match civ {
        Some("CIVILIZATION_INFERNAL") => names::text(g.d, "TXT_KEY_POPUP_CONTROL_INFERNAL").to_string(),
        Some("CIVILIZATION_MERCURIANS") => names::text(g.d, "TXT_KEY_POPUP_CONTROL_MERCURIANS").to_string(),
        _ => format!("{name} have entered the world. Would you like to abandon your civilization and play as them instead?"),
    };
    Decision {
        id: 0,
        player: p,
        kind: DecisionKind::TakeControl { player: new },
        title: "Take control?".into(),
        text,
        options: vec![yes, DecisionOption::new("no", "No")],
        advisor: Some(if advisor_takes { 0 } else { 1 }),
        context: DecisionContext { other_player: Some(new), ..Default::default() },
    }
}

/// A deal offered to the human.
pub fn offer(g: &Game, p: PlayerId, from: PlayerId, they_give: &[TradeData], we_give: &[TradeData], advisor_accepts: bool) -> Decision {
    let items = |v: &[TradeData]| if v.is_empty() { "nothing".to_string() } else { v.iter().map(|t| trade_item_text(g, t)).collect::<Vec<_>>().join(", ") };
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

/// A trade item as the diplomacy screen names it.
fn trade_item_text(g: &Game, t: &TradeData) -> String {
    let d = g.d;
    let data = t.data.max(0) as usize;
    match t.item_type {
        TradeableItem::TRADE_TECHNOLOGIES if data < d.techs.len() => names::display(d, TechType::from_usize(data)).into_owned(),
        TradeableItem::TRADE_RESOURCES if data < d.bonuses.len() => names::display(d, BonusType::from_usize(data)).into_owned(),
        TradeableItem::TRADE_CITIES if g.cities.contains(CityId(data as u32)) => crate::view::city_name(g, CityId(data as u32)),
        TradeableItem::TRADE_GOLD => format!("{} gold", t.data),
        TradeableItem::TRADE_GOLD_PER_TURN => format!("{} gold per turn", t.data),
        TradeableItem::TRADE_PEACE_TREATY => "a peace treaty".into(),
        TradeableItem::TRADE_OPEN_BORDERS => "open borders".into(),
        TradeableItem::TRADE_DEFENSIVE_PACT => "a defensive pact".into(),
        TradeableItem::TRADE_PERMANENT_ALLIANCE => "a permanent alliance".into(),
        TradeableItem::TRADE_MAPS => "the world map".into(),
        _ => {
            let name = TradeableItem::NAMES.get(t.item_type.0 as usize).copied().unwrap_or("TRADE_?");
            format!("{} {}", name.trim_start_matches("TRADE_").to_lowercase().replace('_', " "), t.data)
        }
    }
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
    /// The secretary general's elections waiting for the human (BUTTONPOPUP_CHOOSEELECTION).
    pub elections: Vec<PendingElection>,
    pub research_needed: bool,
    pub idle_cities: Vec<CityId>,
    pub promotions_ready: Vec<UnitId>,
    pub ready_groups: Vec<GroupId>,
    /// Groups the human cannot order that the AI moves (automated or FfH AI-controlled) waiting
    /// for orders ([`ai_groups_waiting`]). The session moves them before the human has control,
    /// so this is empty at a control point (kept for clients and the soak's invariants).
    pub ai_groups_idle: Vec<GroupId>,
}

impl Pending {
    /// Deferred popups still open (the end-turn gate's events, votes, elections and research).
    pub fn popups(&self) -> usize {
        self.events.len() + self.votes.len() + self.elections.len() + self.research_needed as usize
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
            "elections" => Json::Arr(self.elections.iter().map(|e| crate::jobj! {
                "selection" => e.selection,
                "source" => Json::idx(e.source),
                "options" => Json::Arr(e.options.iter().map(|o| crate::jobj! {
                    "vote" => Json::idx(o.vote),
                    "text" => o.text.as_str(),
                }).collect()),
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

/// An election waiting for the human secretary general (C++ BUTTONPOPUP_CHOOSEELECTION,
/// `launchChooseElectionPopup`): one button per resolution of the selection, then "none".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingElection {
    pub selection: VoteId,
    pub source: VoteSourceType,
    /// The resolutions (`aVoteOptions`): the answer is an index, or none.
    pub options: Vec<ElectionOption>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElectionOption {
    pub vote: VoteType,
    /// The option's text as the popup shows it (`szText`: the resolution, the votes needed).
    pub text: String,
}

/// The elections waiting for the human `p`: the vote selections ffh-core keeps because a human
/// secretary general's controller deferred them (an AI secretary's selection is decided and
/// deleted at once, and an empty one is never kept), while `p` is a human (FfH can move the
/// human to another slot; the popup goes with the human).
pub fn pending_elections(g: &Game, p: PlayerId) -> Vec<PendingElection> {
    if !g.players[p].human {
        return Vec::new();
    }
    let d = g.d;
    g.vote_selections
        .iter()
        .map(|vs| PendingElection {
            selection: vs.id,
            source: vs.vote_source,
            options: vs
                .vote_options
                .iter()
                .map(|o| ElectionOption { vote: o.vote, text: if o.text.is_empty() { names::display(d, o.vote).into_owned() } else { o.text.clone() } })
                .collect(),
        })
        .collect()
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

/// The AI-moved groups of `p` waiting for their next orders (ready, with moves left): the exe's
/// next frames would move them (`AI_unitUpdate` runs every frame of the human's turn), so the
/// session runs `update()` until there are none (`session::phase`).
pub fn ai_groups_waiting(g: &Game, p: PlayerId) -> Vec<GroupId> {
    g.players[p].groups.iter().copied().filter(|&grp| g.group_ready_to_move(grp, true) && is_ai_group(g, grp)).collect()
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
        elections: pending_elections(g, p),
        research_needed: research_needed(g, p),
        idle_cities: pl.cities.iter().copied().filter(|&c| !g.city_is_production(c) && !g.cities[c].production_automated).collect(),
        promotions_ready: pl.units.iter().copied().filter(|&u| g.units[u].promotion_ready).collect(),
        ready_groups: ready_groups(g, p),
        ai_groups_idle: ai_groups_waiting(g, p),
    }
}
