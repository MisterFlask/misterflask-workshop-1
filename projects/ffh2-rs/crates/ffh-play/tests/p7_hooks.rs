//! The P7 controller hooks (docs/PLAY_DESIGN.md §7 P7) answered by the human through the
//! session: the raze popup's gift button, the disband popup, the Kuriotates city-or-settlement
//! popup, deferred elections, the take-control popup, and `doResearch`'s AI pick.

use std::sync::{Arc, Mutex};

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::*;
use ffh_data::*;
use ffh_play::command::{CmdErr, CmdOk, Command, Confirm};
use ffh_play::decision::{Decision, DecisionKind};
use ffh_play::human::{Answer, FnAnswerer};
use ffh_play::{NewGame, Phase, Session, SessionOptions, Stop};

/// The decisions asked, and the option key to answer each kind with (missing: option 0).
#[derive(Default)]
struct Script {
    asked: Vec<Decision>,
    answers: Vec<(&'static str, &'static str)>,
}

type Shared = Arc<Mutex<Script>>;

fn session(seed: u32, opponents: usize, world: &str, civ: Option<&str>) -> (Session, Shared) {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, seed);
    ng.opponents = opponents;
    ng.world = d.t(world);
    ng.civ = civ.map(|c| d.t::<CivilizationType>(c));
    let script: Shared = Arc::default();
    let sc = script.clone();
    let answerer = Arc::new(FnAnswerer(move |_g: &Game, dec: &Decision| {
        let mut sc = sc.lock().unwrap();
        sc.asked.push(dec.clone());
        let key = sc.answers.iter().find(|(k, _)| *k == dec.kind.name()).map(|(_, o)| *o);
        match key.and_then(|k| dec.options.iter().position(|o| o.key == k)) {
            Some(i) => Answer::Option(i),
            None => Answer::Option(0),
        }
    }));
    let mut s = Session::new_game(&ng, answerer, SessionOptions::default()).unwrap();
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    (s, script)
}

fn answer(script: &Shared, kind: &'static str, key: &'static str) {
    let mut sc = script.lock().unwrap();
    sc.answers.retain(|(k, _)| *k != kind);
    sc.answers.push((kind, key));
}

fn asked(script: &Shared) -> Vec<Decision> {
    std::mem::take(&mut script.lock().unwrap().asked)
}

fn ok(s: &mut Session, c: Command) {
    match s.execute(&c) {
        Ok(CmdOk::Applied { .. }) => {}
        other => panic!("{c:?} -> {other:?}"),
    }
}

fn found_capital(s: &mut Session) {
    let h = s.human;
    let settler = s.g.players[h].units.iter().copied().find(|&u| s.g.d.units[s.g.units[u].unit_type].found).expect("a settler");
    ok(s, Command::Mission { units: vec![settler], mission: MissionType::MISSION_FOUND, data1: -1, data2: -1, flags: 0, append: false });
    assert_eq!(s.g.players[h].cities.len(), 1);
}

fn to_turn(s: &mut Session, turn: i32) {
    if s.g.players[s.human].cities.is_empty() {
        found_capital(s);
    }
    while s.g.game_turn() < turn {
        ok(s, Command::EndTurn { force: true });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    }
}

/// A plot where `p` may found a city, at least `min` plots from `from`.
fn city_site(g: &Game, p: PlayerId, from: (i32, i32), min: i32) -> (i32, i32) {
    let mut best: Option<(i32, (i32, i32))> = None;
    for q in g.map.plot_ids() {
        let (x, y) = g.map.xy(q);
        let dist = g.plot_distance(x, y, from.0, from.1);
        if dist >= min && g.player_can_found(p, x, y, false) && best.map_or(true, |(b, _)| dist < b) {
            best = Some((dist, (x, y)));
        }
    }
    best.expect("a city site").1
}

/// A warrior of `h` on a free land plot next to `city_plot` that can attack it.
fn attacker_next_to(g: &mut Game, h: PlayerId, city_plot: PlotIdx) -> UnitId {
    let warrior = g.d.t::<UnitType>("UNIT_WARRIOR");
    let (cx, cy) = g.map.xy(city_plot);
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
        let Some(p) = g.plot_xy(cx, cy, dx, dy) else { continue };
        if g.plot_is_water(p) || g.plot_is_impassable(p) || !g.map.plots[p].units.is_empty() || g.map.plots[p].plot_city.is_some() {
            continue;
        }
        let (px, py) = g.map.xy(p);
        let u = g.player_init_unit(h, warrior, px, py, None, None);
        if g.unit_can_move_or_attack_into(u, city_plot, false) {
            return u;
        }
        g.unit_kill(u, false, None);
    }
    panic!("no plot to attack from");
}

/// An undefended non-capital city of `owner`, size 3, at war with `h` (at `at`, else on a site
/// at least 4 plots from its capital).
fn exposed_city(s: &mut Session, owner: PlayerId) -> CityId {
    let capital = s.g.player_get_capital_city(owner).expect("the AI has a capital");
    let at = city_site(&s.g, owner, (s.g.cities[capital].x, s.g.cities[capital].y), 4);
    exposed_city_at(s, owner, at)
}

fn exposed_city_at(s: &mut Session, owner: PlayerId, (x, y): (i32, i32)) -> CityId {
    let h = s.human;
    let g = &mut s.g;
    assert!(g.player_get_capital_city(owner).is_some(), "the AI has a capital");
    let c = g.player_init_city(owner, x, y, true, true);
    assert!(!g.city_is_capital(c));
    g.city_set_population(c, 3);
    let plot = g.city_plot(c);
    for u in g.map.plots[plot].units.clone() {
        g.unit_kill(u, false, None);
    }
    let (my, their) = (g.players[h].team, g.players[owner].team);
    g.team_meet(my, their, false);
    if !g.teams[my].at_war[their] {
        g.team_declare_war(my, their, false, None);
    }
    c
}

/// The raze popup offers the gift button when the city's liberation player is a friend
/// (`launchRazeCityPopup`: a teammate, open borders, or our vassal), and the gift gives the city
/// to that player (`cityAcquiredAndKept`, TASK_GIFT); keep and raze stay as they were.
#[test]
fn conquest_offers_the_gift_to_a_friendly_liberation_player() {
    let (mut s, script) = session(31, 2, "WORLDSIZE_TINY", Some("CIVILIZATION_BANNOR"));
    to_turn(&mut s, 4);
    let h = s.human;
    let others: Vec<PlayerId> = s.g.civ_player_ids().filter(|&p| p != h && s.g.players[p].alive).collect();
    let (friend, enemy) = (others[0], others[1]);
    // `getLiberationPlayer(true)` weighs culture by the distance to each player's capital (and
    // doubles the owner's team's share): the enemy's city stands near the friend's capital.
    let near = s.g.player_get_capital_city(friend).map(|c| (s.g.cities[c].x, s.g.cities[c].y)).unwrap();
    let ours = s.g.player_get_capital_city(h).map(|c| (s.g.cities[c].x, s.g.cities[c].y)).unwrap();
    let g = &s.g;
    let site = g
        .map
        .plot_ids()
        .filter(|&q| !g.plot_is_water(q) && !g.plot_is_impassable(q) && g.map.plots[q].units.is_empty())
        .map(|q| g.map.xy(q))
        .filter(|&(x, y)| {
            let df = g.plot_distance(x, y, near.0, near.1);
            (2..=3).contains(&df)
                && g.cities.iter().all(|(_, c)| g.plot_distance(x, y, c.x, c.y) > 1)
        })
        .max_by_key(|&(x, y)| (g.plot_distance(x, y, ours.0, ours.1) - 2 * g.plot_distance(x, y, near.0, near.1), -x, -y))
        .expect("a site near the friend's capital");
    let c = exposed_city_at(&mut s, enemy, site);
    // The friend has the most culture in the city and open borders with us.
    let (my, theirs) = (s.g.players[h].team, s.g.players[friend].team);
    s.g.team_meet(my, theirs, false);
    s.g.teams[my].open_borders[theirs] = true;
    s.g.teams[theirs].open_borders[my] = true;
    s.g.cities[c].culture[friend] = 100_000_000;
    let plot = s.g.city_plot(c);
    let u = attacker_next_to(&mut s.g, h, plot);
    answer(&script, "keep_city", "gift");
    let (x, y) = s.g.map.xy(plot);
    ok(&mut s, Command::Move { units: vec![u], x, y, append: false, declare_war: None });
    let asked = asked(&script);
    assert_eq!(asked.len(), 1, "{asked:?}");
    let d = &asked[0];
    assert!(matches!(d.kind, DecisionKind::KeepCity { can_raze: true, gift_to: Some(p), .. } if p == friend), "{:?}", d.kind);
    assert_eq!(d.options.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["keep", "raze", "gift"]);
    let now = s.g.map.plots[plot].plot_city.expect("the city stands");
    assert_eq!(s.g.cities[now].owner, friend, "given to the liberation player");
    assert_eq!(s.stats().decisions_of("keep_city"), 1);
    let lines = s.journal_lines().unwrap();
    assert!(lines.iter().any(|l| l.contains("\"kind\":\"keep_city\"") && l.contains("\"option\":2")), "the gift answer is journaled");
}

/// Without a friendly liberation player the popup has no gift button; "raze" razes.
#[test]
fn conquest_without_a_friend_offers_keep_or_raze() {
    let (mut s, script) = session(32, 1, "WORLDSIZE_DUEL", Some("CIVILIZATION_BANNOR"));
    to_turn(&mut s, 4);
    let h = s.human;
    let enemy = s.g.civ_player_ids().find(|&p| p != h && s.g.players[p].alive).unwrap();
    let c = exposed_city(&mut s, enemy);
    let plot = s.g.city_plot(c);
    let u = attacker_next_to(&mut s.g, h, plot);
    answer(&script, "keep_city", "raze");
    let (x, y) = s.g.map.xy(plot);
    ok(&mut s, Command::Move { units: vec![u], x, y, append: false, declare_war: None });
    let asked = asked(&script);
    assert_eq!(asked.len(), 1);
    assert!(matches!(asked[0].kind, DecisionKind::KeepCity { gift_to: None, .. }));
    assert_eq!(asked[0].options.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["keep", "raze"]);
    assert!(s.g.map.plots[plot].plot_city.is_none(), "razed");
}

/// A city that joins the human without conquest or trade (a culture flip, a spell) asks the
/// disband popup (`launchDisbandCityPopup`), not the raze popup: keep or disband.
#[test]
fn an_acquired_city_asks_keep_or_disband() {
    let (mut s, script) = session(33, 1, "WORLDSIZE_DUEL", Some("CIVILIZATION_BANNOR"));
    to_turn(&mut s, 4);
    let h = s.human;
    let ai = s.g.civ_player_ids().find(|&p| p != h && s.g.players[p].alive).unwrap();
    let a = exposed_city(&mut s, ai);
    let b = exposed_city(&mut s, ai);
    let (pa, pb) = (s.g.city_plot(a), s.g.city_plot(b));
    // CvPlayer::acquireCity(pCity, bConquest = false, bTrade = false): a culture flip.
    answer(&script, "disband_city", "keep");
    s.g.player_acquire_city(h, a, false, false, true);
    let kept = s.g.map.plots[pa].plot_city.expect("kept");
    assert_eq!(s.g.cities[kept].owner, h);
    assert!(s.pending().idle_cities.contains(&kept), "kept: the production popup is the human's");
    answer(&script, "disband_city", "disband");
    s.g.player_acquire_city(h, b, false, false, true);
    assert!(s.g.map.plots[pb].plot_city.is_none(), "disbanded");
    let asked = asked(&script);
    assert_eq!(asked.len(), 2, "{:?}", asked.iter().map(|d| (&d.kind, &d.title)).collect::<Vec<_>>());
    for d in &asked {
        assert!(matches!(d.kind, DecisionKind::DisbandCity { .. }), "{:?}", d.kind);
        assert_eq!(d.options.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["keep", "disband"]);
    }
    assert_eq!(s.stats().decisions_of("disband_city"), 2);
}

/// A human Kuriotates city below the city limit (not the first) asks city or settlement
/// (BUTTONPOPUP_CONFIRMSETTLEMENT): "city" gives radius 3, "settlement" a settlement; past the
/// limit a new city is a settlement without asking, as for the AI.
#[test]
fn kuriotates_cities_ask_city_or_settlement() {
    let (mut s, script) = session(34, 1, "WORLDSIZE_SMALL", Some("CIVILIZATION_KURIOTATES"));
    found_capital(&mut s);
    let h = s.human;
    let max = s.g.players[h].max_cities;
    assert!(max >= 2, "the Kuriotates have a city limit ({max})");
    let capital = s.g.players[h].cities[0];
    assert_eq!(s.g.cities[capital].plot_radius, 3, "the first city is not asked");
    assert!(asked(&script).is_empty());
    let home = (s.g.cities[capital].x, s.g.cities[capital].y);
    // A settlement.
    answer(&script, "settlement", "settlement");
    let (x, y) = city_site(&s.g, h, home, 5);
    let c2 = s.g.player_init_city(h, x, y, true, true);
    let d = asked(&script);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0].kind, DecisionKind::Settlement { city } if city == c2));
    assert_eq!(d[0].options.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["city", "settlement"]);
    assert_eq!(d[0].advisor, Some(0), "the advisor suggests what an AI gets: a city");
    assert!(s.g.cities[c2].settlement);
    assert_eq!(s.g.cities[c2].plot_radius, 2);
    // A city.
    answer(&script, "settlement", "city");
    let (x, y) = city_site(&s.g, h, home, 5);
    let c3 = s.g.player_init_city(h, x, y, true, true);
    assert_eq!(asked(&script).len(), 1);
    assert!(!s.g.cities[c3].settlement);
    assert_eq!(s.g.cities[c3].plot_radius, 3);
    // Past the limit: a settlement without asking.
    s.g.players[h].max_cities = 1;
    let (x, y) = city_site(&s.g, h, home, 5);
    let c4 = s.g.player_init_city(h, x, y, true, true);
    assert!(asked(&script).is_empty());
    assert!(s.g.cities[c4].settlement);
}

/// The human secretary general's election is deferred (BUTTONPOPUP_CHOOSEELECTION): ffh-core
/// keeps the vote selection, the session lists it in `pending.elections`, the end-turn gate
/// counts it, and `election` puts the chosen resolution to the vote (`setVoteChosen`); "none"
/// and a forced end turn put nothing to the vote.
#[test]
fn elections_wait_for_the_human_secretary_general() {
    let (mut s, _) = session(35, 2, "WORLDSIZE_TINY", Some("CIVILIZATION_BANNOR"));
    to_turn(&mut s, 3);
    let h = s.human;
    let d = s.g.d;
    let team = s.g.players[h].team;
    // Everyone joins a council the human can run, and the human's team is its secretary general.
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
    assert!(s.g.is_diplo_vote(vs));
    let sg = d.votes.keys().find(|&v| d.votes[v].secretary_general && d.votes[v].vote_source_type[vs]).expect("a secretary general vote");
    s.g.vote_outcome[sg] = PlayerVote(team.0 as i32);
    assert_eq!(s.g.secretary_general(vs), Some(team));
    let elect = |s: &mut Session| -> Option<VoteId> {
        s.g.vote_timer[vs] = 0;
        s.g.secretary_general_timer[vs] = 5;
        let before: Vec<VoteId> = s.g.vote_selections.iter().map(|v| v.id).collect();
        s.g.do_vote_selection();
        s.g.vote_selections.iter().map(|v| v.id).find(|id| !before.contains(id))
    };
    let Some(sel) = elect(&mut s) else {
        // No resolution can be proposed in this game state: nothing to test.
        panic!("no election options in this setup");
    };
    // Deferred: kept, pending, nothing triggered yet.
    let p = s.pending();
    let e = p.elections.iter().find(|e| e.selection == sel).expect("pending election");
    assert!(!e.options.is_empty());
    let triggered_before = s.g.votes_triggered.len();
    match s.execute(&Command::EndTurn { force: false }) {
        Ok(CmdOk::NeedsConfirm(Confirm::EndTurn { elections, .. })) => assert_eq!(elections, 1),
        other => panic!("{other:?}"),
    }
    assert!(matches!(s.execute(&Command::Election { selection: sel, option: Some(e.options.len()) }), Err(CmdErr::Invalid(_))));
    assert!(matches!(s.execute(&Command::Election { selection: VoteId(9999), option: None }), Err(CmdErr::Gone)));
    let chosen = e.options[0].vote;
    ok(&mut s, Command::Election { selection: sel, option: Some(0) });
    assert!(s.g.vote_selection(sel).is_none(), "setVoteChosen deletes the selection");
    assert_eq!(s.g.votes_triggered.len(), triggered_before + 1);
    assert_eq!(s.g.votes_triggered.last().unwrap().vote_option.vote, chosen, "the chosen resolution is put to the vote");
    assert!(s.pending().elections.is_empty());
    // "None": nothing is put to the vote.
    let sel = elect(&mut s).expect("another election");
    let n = s.g.votes_triggered.len();
    ok(&mut s, Command::Election { selection: sel, option: None });
    assert!(s.g.vote_selection(sel).is_none());
    assert_eq!(s.g.votes_triggered.len(), n);
    // A forced end turn closes it the same way.
    let sel = elect(&mut s).expect("a third election");
    let n = s.g.votes_triggered.len();
    ok(&mut s, Command::EndTurn { force: true });
    assert!(s.g.vote_selection(sel).is_none());
    assert_eq!(s.g.votes_triggered.len(), n);
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
}

/// The first to the Infernal Pact is offered control of the Infernals (CvEventManager's
/// TXT_KEY_POPUP_CONTROL_INFERNAL popup, `reassignPlayer`): "no" keeps the human where it is;
/// "yes" moves it (`reassignPlayerAdvanced`), and the session follows it to the new slot.
#[test]
fn the_infernal_pact_offers_control_of_the_infernals() {
    for take in [false, true] {
        let (mut s, script) = session(36, 1, "WORLDSIZE_DUEL", Some("CIVILIZATION_BANNOR"));
        to_turn(&mut s, 3);
        let h = s.human;
        let d = s.g.d;
        let pact = d.t::<TechType>("TECH_INFERNAL_PACT");
        let infernal = d.t::<CivilizationType>("CIVILIZATION_INFERNAL");
        answer(&script, "take_control", if take { "yes" } else { "no" });
        let team = s.g.players[h].team;
        s.g.team_set_has_tech(team, pact, true, Some(h), true, true);
        let asked = asked(&script);
        assert_eq!(asked.len(), 1, "{asked:?}");
        let DecisionKind::TakeControl { player: new } = asked[0].kind else { panic!("{:?}", asked[0].kind) };
        assert_eq!(s.g.player_get_civilization_type(new), Some(infernal));
        assert!(asked[0].text.contains("Hyborem"), "{}", asked[0].text);
        assert_eq!(asked[0].advisor, Some(1), "the advisor says no");
        assert_eq!(s.g.players[new].human, take);
        assert_eq!(s.g.players[h].human, !take);
        // The session follows the human; its next control point is the human's.
        ok_or_moved(&mut s);
        assert_eq!(s.human, if take { new } else { h });
        assert_eq!(s.phase(), Phase::Human(s.human));
        assert_eq!(s.stats().reassigned, take as usize);
    }
}

/// After a rules call outside a command: run the game to the human's next control point.
fn ok_or_moved(s: &mut Session) {
    if s.phase() != Phase::Human(s.human) {
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    } else {
        ok(s, Command::SetOption { option: PlayerOptionType::PLAYEROPTION_QUICK_MOVES, value: true });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    }
}

/// The Mercurian Gate's builder is offered control of the Mercurians the same way.
#[test]
fn the_mercurian_gate_offers_control_of_the_mercurians() {
    let (mut s, script) = session(37, 1, "WORLDSIZE_DUEL", Some("CIVILIZATION_BANNOR"));
    to_turn(&mut s, 3);
    let h = s.human;
    let d = s.g.d;
    let gate = d.t::<BuildingType>("BUILDING_MERCURIAN_GATE");
    let city = s.g.player_get_capital_city(h).unwrap();
    answer(&script, "take_control", "no");
    s.g.city_set_num_real_building(city, gate, 1);
    s.g.ev_building_built(city, gate);
    let asked = asked(&script);
    assert_eq!(asked.len(), 1);
    let DecisionKind::TakeControl { player: new } = asked[0].kind else { panic!() };
    assert_eq!(s.g.player_get_civilization_type(new), Some(d.t::<CivilizationType>("CIVILIZATION_MERCURIANS")));
    assert!(asked[0].text.contains("Basium"), "{}", asked[0].text);
    assert!(s.g.players[h].human && !s.g.players[new].human);
}

/// `doResearch`'s `AI_chooseResearch` call (empty research after turn 4) is the advisor's pick
/// through `ai_choose_research`, and the research popup (`chooseTech`) is deferred: the old
/// "research mark" rule is gone, the counts are the same.
#[test]
fn the_ai_research_pick_goes_through_its_own_hook() {
    let (mut s, script) = session(38, 1, "WORLDSIZE_DUEL", Some("CIVILIZATION_BANNOR"));
    to_turn(&mut s, 7);
    let h = s.human;
    ok(&mut s, Command::Research { tech: None, shift: false });
    let picks = s.stats().research_ai_picks;
    // The popup call alone (chooseTech(0)) never picks for the human.
    s.g.player_choose_tech(h, 0);
    assert!(s.g.player_get_current_research(h).is_none());
    assert_eq!(s.stats().research_ai_picks, picks);
    ok(&mut s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    assert_eq!(s.stats().research_ai_picks, picks + 1);
    assert!(asked(&script).is_empty(), "no blocking decision");
    assert!(s.pending().research_needed);
}

/// The leader's contact period of the AI `ai` for `contact` (ffh-ai `offers.rs`).
fn contact_period(g: &Game, ai: PlayerId, contact: &str) -> i32 {
    let l = &g.d.leader_heads[g.players[ai].personality_type.unwrap()];
    let c = g.d.t::<ContactType>(contact);
    (l.contact_delay[c] + l.contact_rand[c]).max(1)
}

/// An AI losing its war with the human offers peace (C++ AI_doDiplo's CONTACT_PEACE_TREATY): the
/// offer is a blocking decision; refused, the war goes on, accepted, peace is signed.
#[test]
fn a_losing_ai_offers_peace_to_the_human() {
    let (mut s, script) = session(39, 1, "WORLDSIZE_DUEL", Some("CIVILIZATION_BANNOR"));
    to_turn(&mut s, 2);
    let h = s.human;
    let ai = s.g.civ_player_ids().find(|&p| p != h && s.g.players[p].alive).unwrap();
    let (my, theirs) = (s.g.players[h].team, s.g.players[ai].team);
    s.g.team_meet(my, theirs, false);
    s.g.team_declare_war(my, theirs, false, None);
    // The human is strong and winning; the AI wants peace.
    let capital = s.g.players[h].cities[0];
    let (x, y) = (s.g.cities[capital].x, s.g.cities[capital].y);
    let archer = s.g.d.t::<UnitType>("UNIT_ARCHER");
    for _ in 0..8 {
        s.g.player_init_unit(h, archer, x, y, None, None);
    }
    let period = contact_period(&s.g, ai, "CONTACT_PEACE_TREATY");
    let mut offers = Vec::new();
    for _ in 0..(2 * period + 25) {
        s.g.teams[theirs].ai_at_war_counter[my] = s.g.teams[theirs].ai_at_war_counter[my].max(15);
        s.g.teams[my].ai_war_success[theirs] = 500;
        // Refuse the first offer, accept the second.
        answer(&script, "offer", if offers.is_empty() { "refuse" } else { "accept" });
        ok(&mut s, Command::EndTurn { force: true });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
        offers.extend(asked(&script).into_iter().filter(|d| matches!(d.kind, DecisionKind::Offer { from } if from == ai)));
        if offers.len() >= 2 || !s.g.at_war(my, theirs) {
            break;
        }
    }
    assert!(offers.len() >= 2, "the AI offered peace twice ({} offers)", offers.len());
    assert!(offers[0].text.contains("a peace treaty"), "{}", offers[0].text);
    assert!(!s.g.at_war(my, theirs), "accepted: peace");
}

/// An AI at peace with the human offers a tech swap of similar value (CONTACT_TRADE_TECH);
/// accepted, both sides get the other's tech.
#[test]
fn the_ai_offers_tech_trades_to_the_human() {
    let (mut s, script) = session(40, 1, "WORLDSIZE_DUEL", Some("CIVILIZATION_BANNOR"));
    to_turn(&mut s, 2);
    let h = s.human;
    let d = s.g.d;
    let ai = s.g.civ_player_ids().find(|&p| p != h && s.g.players[p].alive).unwrap();
    let (my, theirs) = (s.g.players[h].team, s.g.players[ai].team);
    s.g.team_meet(my, theirs, false);
    // Tech trading (TECH_TRADE), and an AI that does not dislike us (the same alignment).
    let trading = d.t::<TechType>("TECH_TRADE");
    s.g.team_set_has_tech(my, trading, true, Some(h), false, false);
    s.g.team_set_has_tech(theirs, trading, true, Some(ai), false, false);
    s.g.players[h].alignment = s.g.players[ai].alignment;
    // Each side has a tech the other lacks and can research.
    let lacks = |g: &Game, t: TeamId, p: PlayerId| d.techs.keys().filter(|&x| !g.team_is_has_tech(t, Some(x)) && g.player_can_research(p, x, false)).collect::<Vec<_>>();
    let mine = *lacks(&s.g, theirs, ai).iter().find(|&&x| s.g.player_can_research(h, x, false)).expect("a tech for the human");
    s.g.team_set_has_tech(my, mine, true, Some(h), false, false);
    let other = *lacks(&s.g, my, h)
        .iter()
        .find(|&&x| x != mine && s.g.player_can_research(ai, x, false) && s.g.team_get_research_cost(theirs, x) <= s.g.team_get_research_cost(theirs, mine))
        .expect("a tech for the AI");
    s.g.team_set_has_tech(theirs, other, true, Some(ai), false, false);
    answer(&script, "offer", "accept");
    let period = contact_period(&s.g, ai, "CONTACT_TRADE_TECH");
    let mut offer = None;
    // Techs one side has and the other does not (the trade reduces both counts).
    let only = |g: &Game, a: TeamId, b: TeamId| d.techs.keys().filter(|&x| g.team_is_has_tech(a, Some(x)) && !g.team_is_has_tech(b, Some(x))).count();
    let mut before = (0, 0);
    for _ in 0..(period + 3) {
        before = (only(&s.g, my, theirs), only(&s.g, theirs, my));
        ok(&mut s, Command::EndTurn { force: true });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
        if let Some(o) = asked(&script).into_iter().find(|d| matches!(d.kind, DecisionKind::Offer { from } if from == ai)) {
            offer = Some(o);
            break;
        }
    }
    let offer = offer.expect("the AI offered a trade");
    assert!(offer.text.contains("They give:") && !offer.text.contains("TECHNOLOGIES"), "named items: {}", offer.text);
    let after = (only(&s.g, my, theirs), only(&s.g, theirs, my));
    assert!(after.0 < before.0 && after.1 < before.1, "accepted: each side got a tech of the other's ({before:?} -> {after:?}; {})", offer.text);
}
