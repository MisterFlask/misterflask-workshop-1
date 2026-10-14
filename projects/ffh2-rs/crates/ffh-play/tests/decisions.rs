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
    // The reply carries the human's messages the command produced.
    assert!(log.iter().any(|e| e.text.contains("captured")), "{log:?}");
    assert!(log.iter().all(|e| e.player.is_none() || e.player == Some(h.0)));
    let asked = asked.lock().unwrap();
    assert_eq!(asked.len(), 1, "one decision, asked during the command");
    // Acquiring a city makes a new city object on the plot (CvPlayer::acquireCity).
    let c = s.g.map.plots[city_plot].plot_city.expect("the city stands");
    assert!(matches!(asked[0].0, DecisionKind::KeepCity { city, can_raze: true } if city == c), "{:?} (city {c:?})", asked[0].1);
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

/// Fires the first trigger that defers an event popup for `h`; returns its triggered id.
fn fire_event(s: &mut Session) -> TriggeredId {
    let h = s.human;
    let d = s.g.d;
    for t in d.event_triggers.keys() {
        let before = s.shared.events_of(h);
        s.g.player_init_triggered_data(h, t, true, None, INVALID_PLOT_COORD, INVALID_PLOT_COORD, None, None, None, None, None, None);
        if let Some(&id) = s.shared.events_of(h).iter().find(|id| !before.contains(id)) {
            return id;
        }
    }
    panic!("no trigger fires for the human");
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
    assert_eq!(hc.choose_event(g, h, &data, &options), Some(EventType(0)));
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
