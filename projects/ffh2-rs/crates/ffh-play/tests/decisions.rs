//! T6: decision semantics against the C++ (design §8), on games set up with rules APIs, and the
//! command checks that port the exe's gates.

use std::sync::{Arc, Mutex};

use ffh_core::controller::Controller;
use ffh_core::defs::INVALID_PLOT_COORD;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::*;
use ffh_data::*;
use ffh_play::command::{CmdErr, CmdOk, Command, Confirm};
use ffh_play::decision::{Decision, DecisionKind};
use ffh_play::human::{AdvisorAnswerer, Answer, Answerer, FnAnswerer, HumanController, SessionShared};
use ffh_play::journal::Journal;
use ffh_core::log::LogEntry;
use ffh_play::view::fog::LogFog;
use ffh_play::{NewGame, Phase, Session, SessionOptions, Stop};

/// What the answerer saw: (kind, the decision, a probe of the game at that moment).
type Asked = Arc<Mutex<Vec<(DecisionKind, Decision, i64)>>>;

/// A duel game at the human's first control point; the answerer records every decision with
/// `probe(g, d)` and answers option 0.
fn duel(seed: u32, probe: impl Fn(&Game, &Decision) -> i64 + Send + Sync + 'static) -> (Session, Asked) {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, seed);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
    let asked: Asked = Arc::default();
    let a = asked.clone();
    let answerer = Arc::new(FnAnswerer(move |g: &Game, dec: &Decision| {
        a.lock().unwrap().push((dec.kind.clone(), dec.clone(), probe(g, dec)));
        Answer::Option(0)
    }));
    let mut s = Session::new_game(&ng, answerer, SessionOptions::default()).unwrap();
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    (s, asked)
}

fn ok(s: &mut Session, c: Command) {
    match s.execute(&c) {
        Ok(CmdOk::Applied { .. }) => {}
        other => panic!("{c:?} -> {other:?}"),
    }
}

/// Founds the human's first city with its settler (research needs a city).
fn found_capital(s: &mut Session) {
    let h = s.human;
    let settler = s.g.players[h].units.iter().copied().find(|&u| s.g.d.units[s.g.units[u].unit_type].found).expect("a settler");
    ok(s, Command::Mission { units: vec![settler], mission: MissionType::MISSION_FOUND, data1: -1, data2: -1, flags: 0, append: false });
    assert_eq!(s.g.players[h].cities.len(), 1);
}

/// Ends turns (forced) until the human's control point of game turn `turn` or later.
fn to_turn(s: &mut Session, turn: i32) {
    if s.g.players[s.human].cities.is_empty() {
        found_capital(s);
    }
    while s.g.game_turn() < turn {
        ok(s, Command::EndTurn { force: true });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    }
}

fn research_progress(g: &Game, team: TeamId) -> i64 {
    g.teams[team].research_progress.iter().map(|&p| p as i64).sum()
}

/// A human attack captures a city: the keep/raze decision is asked inside `execute`, before
/// the city chooses production (C++ RAZECITY: `chooseProduction` only on "keep"); kept, the
/// city is idle and waits in `pending.idle_cities`.
#[test]
fn capture_asks_keep_or_raze_inside_the_command() {
    let (mut s, asked) = duel(11, |g, d| match d.kind {
        DecisionKind::KeepCity { city, .. } => g.cities[city].order_queue.len() as i64 * 1000 + g.cities[city].owner.0 as i64,
        _ => -1,
    });
    to_turn(&mut s, 4);
    let h = s.human;
    let d = s.g.d;
    let (c, owner) = s.g.cities.iter().find(|(_, c)| c.owner != h).map(|(id, c)| (id, c.owner)).expect("an AI city");
    // Set up: a size-3 city without defenders, at war, a warrior of ours next to it.
    let g = &mut s.g;
    g.city_set_population(c, 3);
    let city_plot = g.city_plot(c);
    for u in g.map.plots[city_plot].units.clone() {
        g.unit_kill(u, false, None);
    }
    let (my, their) = (g.players[h].team, g.players[owner].team);
    g.team_meet(my, their, false);
    g.team_declare_war(my, their, false, None);
    let (cx, cy) = (g.cities[c].x, g.cities[c].y);
    let warrior = d.t::<UnitType>("UNIT_WARRIOR");
    let mut attacker = None;
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
        let Some(p) = g.plot_xy(cx, cy, dx, dy) else { continue };
        if g.plot_is_water(p) || g.plot_is_impassable(p) || !g.map.plots[p].units.is_empty() {
            continue;
        }
        let (px, py) = (g.map.plots[p].x, g.map.plots[p].y);
        let u = g.player_init_unit(h, warrior, px, py, None, None);
        if g.unit_can_move_or_attack_into(u, city_plot, false) {
            attacker = Some(u);
            break;
        }
        g.unit_kill(u, false, None);
    }
    let u = attacker.expect("a plot to attack from");
    assert!(asked.lock().unwrap().is_empty());
    assert_eq!(s.phase(), Phase::Human(h));
    let log = match s.execute(&Command::Move { units: vec![u], x: cx, y: cy, append: false, declare_war: None }) {
        Ok(CmdOk::Applied { log, .. }) => log,
        other => panic!("{other:?}"),
    };
    // The reply carries the human's messages the command produced, including the untagged
    // capture message (C++ sends it to the players who revealed the city: we did).
    assert!(log.iter().any(|e| e.text.contains("captured")), "{log:?}");
    assert!(log.iter().any(|e| e.player.is_none() && e.text.contains(") was captured by the ")), "{log:?}");
    assert!(log.iter().all(|e| e.player.is_none() || e.player == Some(h.0)));
    let asked = asked.lock().unwrap();
    assert_eq!(asked.len(), 1, "one decision, asked during the command");
    // Acquiring a city makes a new city object on the plot (CvPlayer::acquireCity).
    let c = s.g.map.plots[city_plot].plot_city.expect("the city stands");
    assert!(matches!(asked[0].0, DecisionKind::KeepCity { city, can_raze: true, .. } if city == c), "{:?} (city {c:?})", asked[0].1);
    assert_eq!(asked[0].2, h.0 as i64, "asked with the city already ours and its queue empty");
    assert_eq!(asked[0].1.options.len(), 2);
    // Option 0 = keep.
    assert_eq!(s.g.cities[c].owner, h);
    assert!(s.g.cities[c].order_queue.is_empty(), "C++ queues the production popup: no silent choice");
    assert!(s.pending().idle_cities.contains(&c));
    // The answer is journaled after the command.
    let lines = s.journal_lines().unwrap();
    assert!(lines[lines.len() - 1].contains("\"k\":\"answer\"") || lines.iter().rev().take(5).any(|l| l.contains("keep_city")), "{:?}", &lines[lines.len().saturating_sub(5)..]);
}

/// Research, case 1: empty research after turn 4, end turn: the turn's beakers go to the AI pick
/// of `doResearch` (`AI_chooseResearch`, which C++ runs for humans), the queue is cleared, and
/// the next control point asks for research again.
#[test]
fn research_empty_after_turn_four_takes_the_ai_pick_once() {
    let (mut s, _) = duel(12, |_, _| 0);
    to_turn(&mut s, 7);
    let h = s.human;
    ok(&mut s, Command::Research { tech: None, shift: false });
    assert!(s.g.player_get_current_research(h).is_none());
    assert!(s.pending().research_needed);
    let team = s.g.players[h].team;
    let (before, picks) = (research_progress(&s.g, team), s.stats().research_ai_picks);
    let techs_before = s.g.teams[team].has_tech.iter().filter(|&&t| t).count();
    ok(&mut s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert_eq!(s.stats().research_ai_picks, picks + 1, "exactly the AI_chooseResearch call answers");
    let techs_after = s.g.teams[team].has_tech.iter().filter(|&&t| t).count();
    assert!(research_progress(&s.g, team) > before || techs_after > techs_before, "the beakers went to the pick");
    assert!(s.g.player_get_current_research(h).is_none() && s.g.players[h].research_queue.is_empty());
    assert!(s.pending().research_needed);
}

/// Research, case 2: research set, the tech completes at end of turn; the "research next"
/// popup is deferred: no tech is chosen silently.
#[test]
fn research_next_after_a_completed_tech_is_deferred() {
    let (mut s, _) = duel(13, |_, _| 0);
    to_turn(&mut s, 7);
    let h = s.human;
    let team = s.g.players[h].team;
    let tech = s.g.d.techs.keys().find(|&t| s.g.player_can_research(h, t, false)).unwrap();
    ok(&mut s, Command::Research { tech: Some(tech), shift: false });
    assert_eq!(s.g.player_get_current_research(h), Some(tech));
    let cost = s.g.team_get_research_cost(team, tech);
    s.g.teams[team].research_progress[tech] = cost - 1;
    let picks = s.stats().research_ai_picks;
    ok(&mut s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert!(s.g.team_is_has_tech(team, Some(tech)));
    assert_eq!(s.stats().research_ai_picks, picks, "no silent pick");
    assert!(s.g.player_get_current_research(h).is_none());
    assert!(s.pending().research_needed);
}

/// Fires the first trigger accepted by `which` that defers an event popup for `h`; returns its
/// triggered id (queued: the popup is shown at the next control point).
fn queue_event(s: &mut Session, which: impl Fn(&GameData, EventTriggerType) -> bool) -> Option<TriggeredId> {
    let h = s.human;
    let d = s.g.d;
    for t in d.event_triggers.keys().filter(|&t| which(d, t)) {
        let before = s.shared.events_of(h);
        s.g.player_init_triggered_data(h, t, true, None, INVALID_PLOT_COORD, INVALID_PLOT_COORD, None, None, None, None, None, None);
        if let Some(&id) = s.shared.events_of(h).iter().find(|id| !before.contains(id)) {
            return Some(id);
        }
    }
    None
}

/// Fires the first trigger that defers an event popup for `h` and shows it (a control point);
/// returns its triggered id.
fn fire_event(s: &mut Session) -> TriggeredId {
    let id = queue_event(s, |_, _| true).expect("no trigger fires for the human");
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert!(s.shared.open_events_of(s.human).contains(&id), "shown at the control point");
    id
}

/// Events: pending, not applied, after the trigger; applied by `event` (with `bUpdateTrigger`);
/// dropped by a forced end turn; re-filtered at control points.
#[test]
fn events_are_deferred_until_answered() {
    let (mut s, _) = duel(14, |_, _| 0);
    to_turn(&mut s, 3);
    let h = s.human;
    let id = fire_event(&mut s);
    let data = s.g.player_get_event_triggered(h, id).cloned().expect("the triggered data stays");
    let events = &s.g.d.event_triggers[data.trigger].events;
    assert!(events.iter().all(|&e| s.g.player_get_event_occured(h, e).is_none()), "nothing applied yet");
    let p = s.pending();
    let pe = p.events.iter().find(|e| e.triggered == id).expect("pending");
    let event = pe.options.iter().find(|o| o.enabled).unwrap().event;
    assert_eq!(pe.options.iter().map(|o| o.event).collect::<Vec<_>>(), *events, "every event of the trigger, in order");
    let j = p.to_json();
    let opts = j.req_arr("events").unwrap()[0].req_arr("options").unwrap();
    assert!(opts.iter().all(|o| o.req_arr("effects").is_ok() && o.req::<bool>("enabled").is_ok()));
    // The end-turn gate counts it.
    match s.execute(&Command::EndTurn { force: false }) {
        Ok(CmdOk::NeedsConfirm(Confirm::EndTurn { events, .. })) => assert!(events >= 1),
        other => panic!("{other:?}"),
    }
    // A disabled or foreign option is refused.
    let foreign = s.g.d.events.keys().find(|e| !events.contains(e)).unwrap();
    assert!(matches!(s.execute(&Command::Event { triggered: id, event: foreign }), Err(CmdErr::Invalid(_))));
    ok(&mut s, Command::Event { triggered: id, event });
    assert!(s.g.player_get_event_occured(h, event).is_some() || s.g.player_get_event_triggered(h, id).is_none());
    assert!(s.pending().events.iter().all(|e| e.triggered != id));
    assert!(matches!(s.execute(&Command::Event { triggered: id, event }), Err(CmdErr::Gone)));

    // A second one, dropped by a forced end turn (C++ closing the popup does nothing).
    let id2 = fire_event(&mut s);
    assert!(s.pending().events.iter().any(|e| e.triggered == id2));
    ok(&mut s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert!(!s.shared.events_of(h).contains(&id2));

    // Re-filtered: a pending event whose data is gone is dropped at the next control point.
    let id3 = fire_event(&mut s);
    s.g.player_delete_event_triggered(h, id3);
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert!(!s.shared.events_of(h).contains(&id3));
}

/// `launchEventPopup` checks an event popup once, when it is shown: one with nothing doable then
/// is dropped unshown; once shown it stays open (C++ popups are modal, their buttons fixed) even
/// when the human's own actions leave no option doable, and an option that becomes doable again
/// can still be chosen (P1 review: the check used to run at every control point).
#[test]
fn event_popups_are_checked_once_when_shown() {
    let (mut s, _) = duel(14, |_, _| 0);
    to_turn(&mut s, 3);
    let h = s.human;
    // Triggers whose events all act on the event's city: pointing the data at a city that does
    // not exist makes every option impossible (as losing that city would), and back again.
    let city_trigger = |d: &GameData, t: EventTriggerType| {
        let events = &d.event_triggers[t].events;
        !events.is_empty() && events.iter().all(|&e| d.events[e].city_effect)
    };
    let set_city = |s: &mut Session, id: TriggeredId, c: Option<CityId>| -> Option<CityId> {
        let data = s.g.players[h].events_triggered.iter_mut().find(|d| d.id == id).unwrap();
        std::mem::replace(&mut data.city, c)
    };
    let gone = Some(CityId(u32::MAX));

    // Nothing doable when it would be shown: dropped unshown.
    let id = queue_event(&mut s, city_trigger).expect("a city event fires");
    assert!(!s.shared.open_events_of(h).contains(&id), "queued, not yet shown");
    let city = set_city(&mut s, id, gone);
    assert!(city.is_some());
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert!(!s.shared.events_of(h).contains(&id), "dropped when it would be shown");

    // Shown, then nothing doable: still open, with every option disabled.
    let id = queue_event(&mut s, city_trigger).expect("another city event fires");
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert!(s.shared.open_events_of(h).contains(&id));
    let city = set_city(&mut s, id, gone);
    ok(&mut s, Command::SetOption { option: PlayerOptionType::PLAYEROPTION_QUICK_MOVES, value: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let pe = s.pending().events.into_iter().find(|e| e.triggered == id).expect("the shown popup stays open");
    assert!(pe.options.iter().all(|o| !o.enabled));
    let event = pe.options[0].event;
    assert!(matches!(s.execute(&Command::Event { triggered: id, event }), Err(CmdErr::Cannot(_))));
    match s.execute(&Command::EndTurn { force: false }) {
        Ok(CmdOk::NeedsConfirm(Confirm::EndTurn { events, .. })) => assert!(events >= 1),
        other => panic!("{other:?}"),
    }
    // Doable again: answered.
    set_city(&mut s, id, city);
    let event = s.pending().events.into_iter().find(|e| e.triggered == id).unwrap().options.into_iter().find(|o| o.enabled).expect("an option is doable again").event;
    ok(&mut s, Command::Event { triggered: id, event });
    assert!(!s.shared.events_of(h).contains(&id));
}

/// The game log reaches the human as C++ `addMessage` would: untagged entries only under their
/// C++ condition (war and peace: both teams met; captured / razed / liberated: the city
/// revealed; circumnavigation: anonymous unless met), unknown untagged entries held back (P1
/// review: every untagged entry used to reach the human).
#[test]
fn the_log_keeps_the_fog() {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, 22);
    ng.opponents = 2;
    ng.world = d.t("WORLDSIZE_DUEL");
    let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), SessionOptions::default()).unwrap();
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let h = s.human;
    while s.g.cities.iter().all(|(_, c)| c.owner == h) {
        ok(&mut s, Command::EndTurn { force: true });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    }
    let my = s.g.players[h].team;
    let others: Vec<TeamId> = s.g.civ_team_ids().filter(|&t| t != my && s.g.team_is_alive(t)).collect();
    let (a, b) = (others[0], others[1]);
    for t in [a, b] {
        s.g.teams[my].has_met[t] = false;
        s.g.teams[t].has_met[my] = false;
    }
    let reply = |s: &mut Session| match s.execute(&Command::SetOption { option: PlayerOptionType::PLAYEROPTION_QUICK_MOVES, value: true }) {
        Ok(CmdOk::Applied { log, .. }) => log,
        other => panic!("{other:?}"),
    };
    // A war between two civilizations we have not met.
    s.g.team_meet(a, b, false);
    s.g.team_declare_war(a, b, false, None);
    assert!(s.g.log.entries.iter().any(|e| e.text.contains("declared war")), "logged");
    reply(&mut s);
    assert!(s.messages().iter().all(|e| !e.text.contains("declared war")), "{:?}", s.messages());
    // Met both: their peace is news.
    s.g.team_meet(my, a, false);
    s.g.team_meet(my, b, false);
    s.g.team_make_peace(a, b, true);
    // (Logged before the command, so drained into the store, not into its reply.)
    reply(&mut s);
    let peace = format!("Team {a} made peace with team {b}");
    assert!(s.messages().iter().any(|e| e.player.is_none() && e.text == peace), "{:?}", s.messages());

    // The filter itself, on the formats of the ffh-core log sites.
    let fog = LogFog::new(&s.g, h);
    let untagged = |text: String| LogEntry { turn: 1, player: None, text };
    let shown = |text: String| fog.filter(&s.g, h, untagged(text)).map(|e| e.text);
    let (c, city) = s.g.cities.iter().find(|(_, c)| c.owner != h).map(|(c, city)| (c, d.text.name(&city.name).to_string())).unwrap();
    let revealed = s.g.cities[c].revealed[my];
    for text in [format!("{city} (Someone) was captured by the Elves"), format!("{city} was razed by the Elves"), format!("{city} liberated: player 1 -> player 2")] {
        assert_eq!(shown(text.clone()).is_some(), revealed, "{text}");
    }
    s.g.cities[c].revealed[my] = !revealed;
    let fog = LogFog::new(&s.g, h);
    assert_eq!(fog.filter(&s.g, h, untagged(format!("{city} was razed by the Elves"))).is_some(), !revealed);
    // A city revealed at the previous drain and gone since (razed: here renamed) still counts.
    s.g.cities[c].revealed[my] = true;
    let mut fog = LogFog::new(&s.g, h);
    let name = std::mem::replace(&mut s.g.cities[c].name, "Nowhere Else".into());
    assert!(fog.filter(&s.g, h, untagged(format!("{city} was razed by the Elves"))).is_some());
    assert!(LogFog::new(&s.g, h).filter(&s.g, h, untagged(format!("{city} was razed by the Elves"))).is_none());
    fog.update(&s.g, h);
    assert!(fog.filter(&s.g, h, untagged(format!("{city} was razed by the Elves"))).is_none(), "forgotten at the next drain");
    s.g.cities[c].name = name;
    s.g.cities[c].revealed[my] = revealed;
    let fog = LogFog::new(&s.g, h);
    let shown = |text: String| fog.filter(&s.g, h, untagged(text)).map(|e| e.text);
    // Circumnavigation: anonymous for a team we have not met.
    let unmet = s.g.civ_team_ids().find(|&t| t != my && !s.g.teams[my].has_met[t] && s.g.team_is_alive(t));
    if let Some(t) = unmet {
        assert_eq!(shown(format!("Team {t} has circumnavigated the globe (+1 sea moves)")).as_deref(), Some("An unknown civilization has circumnavigated the globe!"));
    }
    assert_eq!(shown(format!("Team {a} has circumnavigated the globe (+1 sea moves)")), Some(format!("Team {a} has circumnavigated the globe (+1 sea moves)")));
    // Untagged text nobody recognizes (event world news) is held back; tagged entries go to
    // their player only.
    assert_eq!(shown("The Elves have discovered a strange artifact".into()), None);
    assert!(fog.filter(&s.g, h, LogEntry { turn: 1, player: Some(h.0), text: "mine".into() }).is_some());
    assert!(fog.filter(&s.g, h, LogEntry { turn: 1, player: Some(h.0 + 1), text: "theirs".into() }).is_none());
}

/// Votes: unanswered, the vote gets C++'s extra turn and is then counted as abstain; answered,
/// `castVote` stores the choice; only the popup's buttons are accepted.
#[test]
fn votes_wait_an_extra_turn_then_abstain() {
    let (mut s, _) = duel(15, |_, _| 0);
    to_turn(&mut s, 3);
    let h = s.human;
    let d = s.g.d;
    // Membership: the council's civic, which the human must be able to run (else `verifyCivics`
    // takes it away at the end of the turn): its tech, and the council our alignment allows.
    let team = s.g.players[h].team;
    let (vs, civic) = ["DIPLOVOTE_OVERCOUNCIL", "DIPLOVOTE_UNDERCOUNCIL"]
        .iter()
        .map(|n| d.t::<VoteSourceType>(n))
        .map(|vs| (vs, d.vote_sources[vs].civic.unwrap()))
        .find(|&(_, civic)| {
            if let Some(t) = d.civics[civic].tech_prereq {
                s.g.team_set_has_tech(team, t, true, Some(h), false, false);
            }
            s.g.player_can_do_civics(h, Some(civic))
        })
        .expect("a council the human can join");
    let option = d.civics[civic].civic_option_type.unwrap();
    for p in s.g.civ_player_ids().collect::<Vec<_>>() {
        if s.g.players[p].alive {
            s.g.player_set_civics(p, option, Some(civic));
        }
    }
    assert!(s.g.player_is_voting_member(h, vs));
    let vote = d
        .votes
        .keys()
        .map(|v| VoteSelectionSubData { vote: v, player: None, city: None, other_player: None, text: "test".into() })
        .find(|o| d.votes[o.vote].vote_source_type[vs] && !s.g.is_team_vote(o.vote) && s.g.is_valid_vote_selection(vs, o))
        .expect("a yes/no resolution");
    let id = s.g.add_vote_triggered(vs, &vote).unwrap();
    assert_eq!(s.g.player_vote(h, id), PlayerVote::NO_PLAYER_VOTE, "deferred");
    let pv = s.pending().votes.into_iter().find(|v| v.vote == id).expect("pending vote");
    assert!(pv.choices.contains(&PlayerVote::PLAYER_VOTE_YES) && pv.choices.contains(&PlayerVote::PLAYER_VOTE_ABSTAIN));
    assert!(matches!(s.execute(&Command::Vote { vote: id, choice: PlayerVote(0) }), Err(CmdErr::Cannot(_))), "a team id in a yes/no vote");
    // Unanswered: one turn later the vote is checked (extra turn), the next turn it is decided.
    ok(&mut s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert!(s.g.vote_triggered(id).is_some(), "the vote waits for the human");
    assert_eq!(s.g.player_vote(h, id), PlayerVote::NO_PLAYER_VOTE_CHECKED);
    assert!(s.pending().votes.iter().any(|v| v.vote == id && v.last_turn));
    ok(&mut s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert!(s.g.vote_triggered(id).is_none(), "decided with the human abstaining");
    // Answered.
    assert!(
        s.g.player_is_voting_member(h, vs),
        "after {}: civic {:?} is {} pop {} religion {:?} votes {}",
        d.type_name(vote.vote),
        s.g.players[h].civics[option],
        s.g.player_is_civic(h, civic),
        s.g.players[h].total_population,
        s.g.vote_source_religions[vs],
        s.g.player_get_votes(h, None, vs)
    );
    let id = s.g.add_vote_triggered(vs, &vote).unwrap();
    ok(&mut s, Command::Vote { vote: id, choice: PlayerVote::PLAYER_VOTE_YES });
    assert_eq!(s.g.player_vote(h, id), PlayerVote::PLAYER_VOTE_YES);
    assert!(s.pending().votes.iter().all(|v| v.vote != id));
}

/// Free techs block, one decision per tech; the answers are journaled.
#[test]
fn free_techs_block_once_per_tech() {
    let (mut s, asked) = duel(16, |g, d| match d.kind {
        DecisionKind::FreeTech => g.d.techs.keys().filter(|&t| g.team_is_has_tech(g.players[d.player].team, Some(t))).count() as i64,
        _ => -1,
    });
    found_capital(&mut s);
    let h = s.human;
    let team = s.g.players[h].team;
    let before = s.g.teams[team].has_tech.iter().filter(|&&t| t).count();
    s.g.player_choose_tech(h, 2);
    let asked = asked.lock().unwrap();
    assert_eq!(asked.len(), 2);
    assert!(asked.iter().all(|a| a.0 == DecisionKind::FreeTech));
    assert_eq!(asked[1].2, asked[0].2 + 1, "the second is asked after the first tech is granted");
    assert_eq!(s.g.teams[team].has_tech.iter().filter(|&&t| t).count(), before + 2);
    let lines = s.journal_lines().unwrap();
    assert_eq!(lines.iter().filter(|l| l.contains("\"kind\":\"free_tech\"")).count(), 2);
    assert_eq!(s.stats().decisions_of("free_tech"), 2);
}

/// With AI autoplay the slot is not human, and every controller method answers as the advisor.
#[test]
fn autoplay_delegates_to_the_advisor() {
    let (mut s, asked) = duel(17, |_, _| 0);
    to_turn(&mut s, 2);
    let h = s.human;
    let start = s.g.game_turn();
    ok(&mut s, Command::AutoPlay { turns: 3 });
    assert!(!s.g.players[h].human);
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert!(s.g.players[h].human);
    assert!(s.g.game_turn() >= start + 3);
    assert!(asked.lock().unwrap().is_empty());

    // Directly: a non-human slot never defers or asks.
    let g = &mut s.g;
    g.player_set_disable_human(h, true);
    let advisor = Arc::new(ffh_ai::Ai::new());
    let shared = Arc::new(SessionShared::new(Journal::memory()));
    struct Never;
    impl Answerer for Never {
        fn ask(&self, _: &Game, d: &Decision) -> Answer {
            panic!("asked {:?} for a non-human slot", d.kind)
        }
    }
    let hc = HumanController::new(advisor.clone(), Arc::new(Never), shared.clone());
    let opt = VoteSelectionSubData { vote: VoteType(0), player: None, city: None, other_player: None, text: String::new() };
    assert_eq!(hc.diplo_vote(g, h, &opt, VoteSourceType(0)), advisor.diplo_vote(g, h, &opt, VoteSourceType(0)));
    assert_ne!(hc.diplo_vote(g, h, &opt, VoteSourceType(0)), PlayerVote::NO_PLAYER_VOTE);
    assert_eq!(hc.choose_research(g, h), advisor.choose_research(g, h));
    assert_eq!(hc.choose_free_tech(g, h), advisor.choose_free_tech(g, h));
    let c = g.players[h].cities[0];
    assert_eq!(hc.raze_conquered_city(g, h, c), advisor.raze_conquered_city(g, h, c));
    let data = EventTriggeredData {
        id: TriggeredId(999),
        trigger: EventTriggerType(0),
        turn: 0,
        player: h,
        city: None,
        plot_x: INVALID_PLOT_COORD,
        plot_y: INVALID_PLOT_COORD,
        unit: None,
        other_player: None,
        other_player_city: None,
        religion: None,
        corporation: None,
        building: None,
        text: String::new(),
        global_text: String::new(),
    };
    let options = [EventType(0), EventType(1)];
    assert_eq!(hc.choose_event(g, h, &data, &options), advisor.choose_event(g, h, &data, &options));
    assert!(hc.choose_event(g, h, &data, &options).is_some());
    assert!(shared.pending_events.lock().unwrap().is_empty());
    g.player_set_disable_human(h, false);
}

/// Take over (a debugging aid): an AI-only save at a turn boundary gets a human in slot `p`.
#[test]
fn take_over_an_ai_save() {
    let d = ffh_data::global();
    let o = ffh_core::setup::SetupOptions::new(d, 2, "WORLDSIZE_DUEL", 21);
    let mut g = Game::new_with_options(d, &o);
    g.map_script = Arc::new(ffh_core::mapscript::FractalMapScript::default());
    let ai = ffh_ai::ai_controller();
    for p in g.player_ids() {
        g.controllers[p] = ai.clone();
    }
    g.autorun = true;
    g.game_init();
    while g.game_turn() < 6 || !g.at_turn_boundary() {
        g.update();
    }
    let save = g.save();
    let p = PlayerId(1);
    let mut s = Session::take_over(&save, p, Arc::new(AdvisorAnswerer), SessionOptions::default()).unwrap();
    assert_eq!(s.human, p);
    assert!(s.g.players[p].human && s.g.active_player == Some(p) && !s.g.autorun);
    assert!(s.journal_lines().unwrap()[0].contains("\"k\":\"take_over\""));
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert_eq!(s.phase(), Phase::Human(p));
    ok(&mut s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    // A save with a human (here also mid-turn) is refused.
    let (human_save, _) = s.save_bytes().unwrap();
    assert!(Session::take_over(&human_save, PlayerId(0), Arc::new(AdvisorAnswerer), SessionOptions::default()).is_err());
    assert!(Session::take_over(&save, PlayerId(200), Arc::new(AdvisorAnswerer), SessionOptions::default()).is_err());
}

/// The commands' gates (the exe offers only legal actions): refusals change nothing.
#[test]
fn commands_are_checked_like_the_exe() {
    let (mut s, _) = duel(18, |_, _| 0);
    to_turn(&mut s, 2);
    let h = s.human;
    let before = s.g.save();
    let other = s.g.civ_player_ids().find(|&p| p != h && s.g.players[p].alive).unwrap();
    let foreign_unit = s.g.players[other].units[0];
    let my_unit = s.g.players[h].units[0];
    let my_city = s.g.players[h].cities[0];
    let mission = |units: Vec<UnitId>, m: MissionType| Command::Mission { units, mission: m, data1: -1, data2: -1, flags: 0, append: false };
    assert_eq!(s.execute(&mission(vec![foreign_unit], MissionType::MISSION_SKIP)), Err(CmdErr::NotOwner));
    assert_eq!(s.execute(&mission(vec![UnitId(999_999)], MissionType::MISSION_SKIP)), Err(CmdErr::Gone));
    assert!(matches!(s.execute(&mission(vec![], MissionType::MISSION_SKIP)), Err(CmdErr::Invalid(_))));
    assert!(matches!(s.execute(&mission(vec![my_unit, my_unit], MissionType::MISSION_SKIP)), Err(CmdErr::Invalid(_))));
    assert!(matches!(s.execute(&mission(vec![my_unit], MissionType::MISSION_DIE)), Err(CmdErr::Invalid(_))));
    assert!(matches!(s.execute(&mission(vec![my_unit], MissionType::MISSION_FOUND)), Err(CmdErr::Cannot(_))), "no settler");
    let off_map = Command::Move { units: vec![my_unit], x: -1, y: 0, append: false, declare_war: None };
    assert!(matches!(s.execute(&off_map), Err(CmdErr::Invalid(_))));
    let (ux, uy) = (s.g.units[my_unit].x, s.g.units[my_unit].y);
    assert!(matches!(s.execute(&Command::Move { units: vec![my_unit], x: ux, y: uy, append: false, declare_war: None }), Err(CmdErr::Cannot(_))));
    let raze = Command::DoTask { city: my_city, task: TaskType::TASK_RAZE, data1: -1, data2: -1, option: false, alt: false, shift: false, ctrl: false };
    assert!(matches!(s.execute(&raze), Err(CmdErr::Invalid(_))));
    let show = Command::SetOption { option: PlayerOptionType::PLAYEROPTION_SHOW_ENEMY_MOVES, value: true };
    assert!(matches!(s.execute(&show), Err(CmdErr::Cannot(_))));
    let automate = Command::DoCommand { units: vec![my_unit], command: CommandType::COMMAND_AUTOMATE, data1: 0, data2: -1, alt: false, confirmed: false };
    assert!(matches!(s.execute(&automate), Err(CmdErr::Cannot(_))));
    // The last order of a queue cannot be popped (the exe sends POP_ORDER only for longer queues).
    if s.g.cities[my_city].order_queue.len() == 1 {
        assert!(matches!(s.execute(&Command::PopOrder { city: my_city, index: 0 }), Err(CmdErr::Cannot(_))));
    }
    // Delete asks first.
    let delete = Command::DoCommand { units: vec![my_unit], command: CommandType::COMMAND_DELETE, data1: -1, data2: -1, alt: false, confirmed: false };
    assert_eq!(s.execute(&delete), Ok(CmdOk::NeedsConfirm(Confirm::Command { command: CommandType::COMMAND_DELETE })));
    assert!(s.g.save() == before, "a refused or unconfirmed command changed the game");
    // Not the human's turn.
    ok(&mut s, Command::EndTurn { force: true });
    assert_eq!(s.phase(), Phase::Running);
    assert_eq!(s.execute(&mission(vec![my_unit], MissionType::MISSION_SKIP)), Err(CmdErr::NotYourTurn));
    // The advisor answerer exists for soak runs.
    let _ = AdvisorAnswerer;
}

/// The AutoMoves rule (`updateTestEndTurn`) fires at most once per control episode: when the
/// orders it continues leave a group ready to auto again, `advance` returns instead of sending
/// AutoMoves forever; a save there resumes without sending it again; the human's next command
/// starts a new episode.
#[test]
fn auto_moves_fires_once_per_control_episode() {
    let (mut s, _) = duel(20, |_, _| 0);
    to_turn(&mut s, 3);
    let h = s.human;
    // No barbarians or animals around: a human group in danger drops its orders (autoMission).
    let barbarian = s.g.barbarian_player();
    for u in s.g.players[barbarian].units.clone() {
        s.g.unit_kill(u, false, None);
    }
    // A land group of ours and a destination two or more turns away (found with a finder of
    // our own, as the interface does).
    let mut target = None;
    'search: for &grp in &s.g.players[h].groups {
        let head = s.g.groups[grp].units[0];
        if s.g.unit_get_domain_type(head) != DomainType::DOMAIN_LAND || !s.g.group_ready_to_move(grp, true) {
            continue;
        }
        let (x, y) = (s.g.units[head].x, s.g.units[head].y);
        let mut finder = ffh_core::path::PathFinder::new(ffh_core::path::FinderKind::Path);
        finder.set_group(Some(grp));
        for r in 3..8 {
            for (dx, dy) in [(r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, -r), (r, -r), (-r, r)] {
                let Some(p) = s.g.plot_xy(x, y, dx, dy) else { continue };
                let (tx, ty) = (s.g.map.plots[p].x, s.g.map.plots[p].y);
                if finder.generate_path(&s.g, x, y, tx, ty, false, 0, false) && finder.path_turns().is_some_and(|t| t >= 2) {
                    target = Some((grp, tx, ty));
                    break 'search;
                }
            }
        }
    }
    let (grp, tx, ty) = target.expect("a land group with a destination two turns away");
    for other in s.g.players[h].groups.clone() {
        if other != grp && s.g.group_ready_to_move(other, true) {
            s.g.group_set_activity_type(other, ActivityType::ACTIVITY_SLEEP);
        }
    }
    let turn = s.g.game_turn();
    s.g.group_insert_at_end_mission_queue(grp, MissionData { mission_type: MissionType::MISSION_MOVE_TO, data1: tx, data2: ty, flags: 0, push_turn: turn }, false);
    let auto_moves = |s: &Session| s.journal_lines().unwrap().iter().filter(|l| l.contains("\"c\":\"auto_moves\"")).count();
    let idle_with_orders = |s: &Session| ffh_play::decision::ready_groups(&s.g, h).is_empty() && s.g.player_has_auto_unit(h);
    let refill = |s: &mut Session| {
        for u in s.g.groups[grp].units.clone() {
            s.g.units[u].moves = 0;
        }
    };
    assert!(idle_with_orders(&s));
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert_eq!(auto_moves(&s), 1, "the queued order is continued");
    assert!(!s.g.groups[grp].mission_queue.is_empty(), "the move takes more than one turn");
    // As if the order had not progressed: the group can move and still has it queued.
    refill(&mut s);
    assert!(idle_with_orders(&s));
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert_eq!(auto_moves(&s), 1, "AutoMoves at most once per control episode");
    // A save here resumes in the same episode.
    let (bytes, sidecar) = s.save_bytes().unwrap();
    assert!(sidecar.auto_moves_tried);
    let mut r = Session::load(&bytes, Some(&sidecar), Arc::new(AdvisorAnswerer), SessionOptions::default()).unwrap();
    assert_eq!(r.advance(&mut |_| {}), Stop::Human);
    assert_eq!(auto_moves(&r), 0, "the resumed session sends it again");
    // The human's next command starts a new episode.
    ok(&mut s, Command::SetOption { option: PlayerOptionType::PLAYEROPTION_QUICK_MOVES, value: true });
    refill(&mut s);
    assert!(idle_with_orders(&s));
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert_eq!(auto_moves(&s), 2);
}

/// Parameters no exe path sends are refused before any rules call (review of P1): specialist
/// changes other than +/-1 (a huge one looped the governor for hours), slider steps other than
/// COMMERCE_PERCENT_CHANGE_INCREMENTS (overflow), mission flags (AI movement modes),
/// MOVE_TO_UNIT (follows and probes for foreign units), mission data the action does not carry,
/// and liberation with the conquest flag.
#[test]
fn parameters_the_exe_never_sends_are_refused() {
    let (mut s, _) = duel(19, |_, _| 0);
    to_turn(&mut s, 3);
    let h = s.human;
    let d = s.g.d;
    let my_city = s.g.players[h].cities[0];
    let my_unit = s.g.players[h].units[0];
    let other = s.g.civ_player_ids().find(|&p| p != h && s.g.players[p].alive).unwrap();
    let foreign_unit = s.g.players[other].units[0];
    let citizen = d.t::<SpecialistType>("SPECIALIST_CITIZEN");
    let specialist = |data2: i32| Command::DoTask {
        city: my_city,
        task: TaskType::TASK_CHANGE_SPECIALIST,
        data1: citizen.idx() as i32,
        data2,
        option: false,
        alt: false,
        shift: false,
        ctrl: false,
    };
    let percent = |change: i32| Command::PercentChange { commerce: CommerceType::COMMERCE_RESEARCH, change };
    let mission = |m: MissionType, data1: i32, data2: i32, flags: i32| Command::Mission { units: vec![my_unit], mission: m, data1, data2, flags, append: false };
    let (ux, uy) = (s.g.units[my_unit].x, s.g.units[my_unit].y);
    let (tx, ty) = if ux > 0 { (ux - 1, uy) } else { (ux + 1, uy) };
    let liberate = Command::DoTask { city: my_city, task: TaskType::TASK_LIBERATE, data1: 1, data2: -1, option: false, alt: false, shift: false, ctrl: false };
    let before = s.g.save();
    for (c, what) in [
        (specialist(i32::MAX), "specialist +MAX"),
        (specialist(i32::MIN), "specialist MIN"),
        (specialist(2), "specialist +2"),
        (specialist(0), "specialist 0"),
        (percent(i32::MAX), "percent +MAX"),
        (percent(i32::MIN), "percent MIN"),
        (percent(5), "percent 5"),
        (mission(MissionType::MISSION_MOVE_TO, tx, ty, 0x20), "MOVE_THROUGH_ENEMY"),
        (mission(MissionType::MISSION_MOVE_TO, tx, ty, -1), "flags -1"),
        (mission(MissionType::MISSION_MOVE_TO_UNIT, other.i(), foreign_unit.0 as i32, 0), "MOVE_TO_UNIT"),
        (mission(MissionType::MISSION_SKIP, 5, -1, 0), "SKIP data1"),
        (mission(MissionType::MISSION_FORTIFY, -1, 7, 0), "FORTIFY data2"),
        (mission(MissionType::MISSION_GOLDEN_AGE, 0, -1, 0), "golden age animation"),
        (mission(MissionType::MISSION_BUILD, 0, 3, 0), "BUILD data2"),
        (liberate, "liberate with the conquest flag"),
    ] {
        assert!(matches!(s.execute(&c), Err(CmdErr::Invalid(_))), "{what}: {:?}", s.execute(&c));
    }
    // A citizen specialist cannot be removed when there is none.
    if s.g.cities[my_city].specialist_count[citizen] == 0 && s.g.cities[my_city].force_specialist_count[citizen] == 0 {
        assert!(matches!(s.execute(&specialist(-1)), Err(CmdErr::Cannot(_))));
    }
    assert!(s.g.save() == before, "a refused command changed the game");
    // What the buttons send is applied.
    let research = s.g.players[h].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()];
    let step = d.defines.commerce_percent_change_increments;
    ok(&mut s, percent(-step));
    assert_eq!(s.g.players[h].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()], research - step);
    ok(&mut s, percent(step));
    assert_eq!(s.g.players[h].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()], research);
    ok(&mut s, specialist(1));
    let city = &s.g.cities[my_city];
    assert!(city.specialist_count[citizen].max(city.force_specialist_count[citizen]) >= 1);
    ok(&mut s, specialist(-1));
    // A decoded liberate without data1 is the button's (0).
    let j = ffh_play::json::Json::parse(&format!(r#"{{"c":"do_task","city":{},"task":"TASK_LIBERATE"}}"#, my_city.0)).unwrap();
    assert!(matches!(Command::from_json(d, &j), Ok(Command::DoTask { data1: 0, .. })));
}

/// FfH moves the human to another slot with `reassignPlayerAdvanced`; here the Impersonate
/// Leader spell (no `bAllowAI`: a human-only path), cast with `do_command` by a Gibbon Goetia
/// standing in the AI's city. The session follows the human and moves the active player with it
/// (as C++ does); the human plays the AI civilization until the spell's timer runs out, then is
/// moved back. Before P2 the session kept waiting for the old slot (the watchdog's `Stuck`).
#[test]
fn the_session_follows_the_human_to_another_slot() {
    let dir = std::env::temp_dir().join(format!("ffh-play-reassign-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, 21);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
    let opts = SessionOptions { saves_dir: Some(dir.clone()), ..Default::default() };
    let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), opts).unwrap();
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    to_turn(&mut s, 3);
    let h = s.human;
    let (c, ai) = s.g.cities.iter().find(|(_, c)| c.owner != h).map(|(id, c)| (id, c.owner)).expect("an AI city");
    let (cx, cy) = (s.g.cities[c].x, s.g.cities[c].y);
    let gibbon = d.units.keys().find(|&u| d.units[u].unit_class_type == d.ty("UNITCLASS_GIBBON")).expect("the Gibbon Goetia");
    let u = s.g.player_init_unit(h, gibbon, cx, cy, None, None);
    let spell = d.t::<SpellType>("SPELL_IMPERSONATE_LEADER");
    assert!(s.g.unit_can_cast(u, spell, false));
    ok(&mut s, Command::DoCommand { units: vec![u], command: CommandType::COMMAND_CAST, data1: spell.0 as i32, data2: -1, alt: false, confirmed: false });
    assert_eq!((s.human, s.g.active_player), (ai, Some(ai)));
    assert!(s.g.players[ai].human && !s.g.players[h].human);
    assert_eq!(s.g.players[ai].real_player, Some(h));
    let timer = s.g.players[ai].temp_player_timer;
    assert!((5..15).contains(&timer), "{timer}");
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert_eq!(s.phase(), Phase::Human(ai));
    // A save of the impersonation resumes as the impersonated player.
    s.save("impersonating").unwrap();
    let (bytes, sidecar) = ffh_play::save::read_bundle(&dir.join("impersonating.ffhsave")).unwrap();
    assert_eq!(sidecar.as_ref().unwrap().human, ai);
    let resumed = Session::load(&bytes, sidecar.as_ref(), Arc::new(AdvisorAnswerer), SessionOptions::default()).unwrap();
    assert_eq!((resumed.human, resumed.phase()), (ai, Phase::Human(ai)));
    // Turns as the AI civilization, until the timer brings the human back.
    let mut turns = 0;
    while s.human == ai {
        ok(&mut s, Command::EndTurn { force: true });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
        turns += 1;
        assert!(turns <= 15, "still impersonating after {turns} turns");
    }
    assert_eq!((s.human, s.g.active_player, s.phase()), (h, Some(h), Phase::Human(h)));
    assert!(s.g.players[h].human && !s.g.players[ai].human);
    assert_eq!(s.stats().reassigned, 2);
    std::fs::remove_dir_all(&dir).unwrap();
}
