//! The views and queries (design §4.4-4.7): every query answers at a control point, reads leave
//! the game as it was, and the replies keep to their documented shapes.

mod common;

use std::sync::Arc;
use std::time::Instant;

use common::{bot_session, new_game, peaceful_game};
use ffh_core::ids::*;
use ffh_core::structs::TradeData;
use ffh_data::*;
use ffh_play::command::{CmdErr, CmdOk, Command, Confirm};
use ffh_play::human::AdvisorAnswerer;
use ffh_play::query::{Layer, Query};
use ffh_play::{bot, Json, NewGame, Session, SessionOptions, Stop};

/// Every query of a control point (the noisy client's list, design §8 T10).
fn all_queries(s: &ffh_play::Session) -> Vec<Query> {
    let h = s.human;
    let g = &s.g;
    let mut qs = vec![
        Query::Status,
        Query::Pending,
        Query::View,
        Query::Map { since: None, epoch: None },
        Query::Techs,
        Query::Civics { with: Vec::new() },
        Query::Religions,
        Query::Players,
        Query::Log { since: 0 },
        Query::Report,
        Query::Text,
        Query::Data,
        Query::Setup,
        Query::Saves,
    ];
    for layer in Layer::ALL {
        qs.push(Query::Ascii { x: 0, y: 0, w: 0, h: 0, layer });
    }
    for &grp in &g.players[h].groups {
        let units = g.groups[grp].units.clone();
        let (x, y) = (g.units[units[0]].x, g.units[units[0]].y);
        qs.push(Query::Group { units: units.clone() });
        qs.push(Query::Path { units: units.clone(), x: (x + 3).min(g.map.grid_width - 1), y, flags: 0 });
        qs.push(Query::Combat { units: units.clone(), x: (x + 1).min(g.map.grid_width - 1), y });
        qs.push(Query::Plot { x, y });
        qs.push(Query::Inspect { kind: "unit".into(), oid: units[0].0 as i64 });
    }
    for &c in &g.players[h].cities {
        qs.push(Query::City { city: c });
    }
    for p in g.player_ids() {
        if p != h && g.players[p].alive && !g.player_is_barbarian(p) && g.teams[g.players[h].team].has_met[g.players[p].team] {
            qs.push(Query::Trade { with: p });
        }
    }
    qs.push(Query::Inspect { kind: "UnitType".into(), oid: 0 });
    qs
}

/// Pattern A (a long varied game): a peaceful game (`GAMEOPTION_ALWAYS_PEACE` +
/// `GAMEOPTION_NO_BARBARIANS`) cannot end early, so for ANY seed the bot reaches the until-turn
/// with `Stop::Human` -- the `assert_eq!(stop, Stop::Human)` below is true by construction, not by
/// a seed happening to produce a game that lasts. The test then asserts invariants (every query
/// answers with valid JSON, and the reads leave the save unchanged), never a seed-specific outcome.
#[test]
fn every_query_answers_and_reads_change_nothing() {
    let ng = peaceful_game(4);
    let (mut s, mut b) = bot_session(&ng, None);
    let until = s.g.game_turn() + 40;
    let mut checked = 0;
    let stop = bot::run(&mut s, &mut b, until, |s, _| {
        if s.g.game_turn() % 10 != 0 {
            return;
        }
        let before = s.g.save();
        let t0 = Instant::now();
        for q in all_queries(s) {
            let t = Instant::now();
            let r = s.query(&q).unwrap_or_else(|e| panic!("{q:?}: {e}"));
            let text = r.to_string();
            assert!(Json::parse(&text).is_ok(), "{q:?} gives invalid JSON");
            let ms = t.elapsed().as_millis();
            if ms > 200 {
                eprintln!("slow query {} ({ms} ms, {} bytes)", q.name(), text.len());
            }
        }
        eprintln!("turn {}: all queries in {} ms", s.g.game_turn(), t0.elapsed().as_millis());
        assert!(before == s.g.save(), "turn {}: the queries changed the save", s.g.game_turn());
        checked += 1;
    });
    assert_eq!(stop, Stop::Human);
    assert!(checked >= 3);
}

/// A duel at the human's first control point, its capital founded.
fn duel(seed: u32) -> Session {
    duel_with(seed, &[])
}

/// [`duel`] with the game unable to end early (`GAMEOPTION_ALWAYS_PEACE` +
/// `GAMEOPTION_NO_BARBARIANS`): no AI can attack or eliminate the human and no victory fires in
/// under the run lengths here, so the duel lasts its full length for ANY seed -- the basis of the
/// pattern-A report test, which asserts coverage (techs and units reported over the run), never a
/// seed producing a particular game.
fn peaceful_duel(seed: u32) -> Session {
    duel_with(seed, &[GameOptionType::GAMEOPTION_ALWAYS_PEACE, GameOptionType::GAMEOPTION_NO_BARBARIANS])
}

fn duel_with(seed: u32, options: &[GameOptionType]) -> Session {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, seed);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
    ng.options.extend_from_slice(options);
    let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), SessionOptions::default()).unwrap();
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let h = s.human;
    let settler = s.g.players[h].units.iter().copied().find(|&u| s.g.d.units[s.g.units[u].unit_type].found).unwrap();
    applied(&mut s, Command::Mission { units: vec![settler], mission: MissionType::MISSION_FOUND, data1: -1, data2: -1, flags: 0, append: false });
    s
}

fn applied(s: &mut Session, c: Command) -> Option<bool> {
    match s.execute(&c) {
        Ok(CmdOk::Applied { accepted, .. }) => accepted,
        other => panic!("{c:?} -> {other:?}"),
    }
}

fn next_turn(s: &mut Session) {
    applied(s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
}

fn item(item_type: TradeableItem, data: i32) -> TradeData {
    TradeData { item_type, data, offering: false, hidden: false }
}

/// `propose_deal`: the trade tables' items only, both sides of a dual item, the AI's answer
/// (`consider_offer`): a refusal changes nothing, an accepted peace treaty makes peace. At war the
/// AI refuses to talk until its war counter reaches its leader's threshold
/// (`AI_isWillingToTalk`), and a deal must be a peace deal: a gift cannot end the war.
#[test]
fn propose_deal_follows_the_trade_tables_and_the_ai_answer() {
    let mut s = duel(21);
    let h = s.human;
    let ai = s.g.civ_player_ids().find(|&p| p != h).unwrap();
    let (my, their) = (s.g.players[h].team, s.g.players[ai].team);
    let peace = || vec![item(TradeableItem::TRADE_PEACE_TREATY, 0)];
    let gold = |n: i32| vec![item(TradeableItem::TRADE_GOLD, n)];
    // Unmet: no contact, no trade query.
    let deal = |give: Vec<TradeData>, get: Vec<TradeData>| Command::ProposeDeal { to: ai, give, get };
    assert!(matches!(s.execute(&deal(peace(), peace())), Err(CmdErr::Cannot(_))));
    assert!(s.query(&Query::Trade { with: ai }).is_err());
    s.g.team_meet(my, their, false);
    // Gold trading on for both, and gold on both sides, for the gift cases below.
    s.g.teams[my].gold_trading_count += 1;
    s.g.players[h].gold = s.g.players[h].gold.max(50);
    s.g.players[ai].gold = s.g.players[ai].gold.max(50);
    let t = s.query(&Query::Trade { with: ai }).unwrap();
    assert_eq!(t.get("contact"), Some(&Json::Bool(true)));
    // At peace there is no peace treaty to offer; nonsense is refused before the AI is asked.
    assert!(matches!(s.execute(&deal(peace(), peace())), Err(CmdErr::Cannot(_))));
    assert!(matches!(s.execute(&deal(vec![], vec![])), Err(CmdErr::Invalid(_))));
    assert!(matches!(s.execute(&deal(vec![item(TradeableItem::TRADE_GOLD, i32::MAX)], vec![])), Err(CmdErr::Cannot(_))));
    assert!(matches!(s.execute(&deal(vec![item(TradeableItem::TRADE_CITIES, 999_999)], vec![])), Err(CmdErr::Cannot(_))));
    assert!(matches!(s.execute(&Command::ProposeDeal { to: h, give: peace(), get: peace() }), Err(CmdErr::Cannot(_))));
    assert!(matches!(s.execute(&Command::ProposeDeal { to: PlayerId(200), give: peace(), get: peace() }), Err(CmdErr::Invalid(_))));
    applied(&mut s, Command::DeclareWar { team: their });

    // Fresh war: the AI refuses to talk (its war counter is under its leader's threshold). No
    // trade table, no deal: neither peace nor a gift (which would make peace, CvDeal::addTrades).
    assert!(!ffh_play::view::diplo::ai_willing_to_talk(&s.g, ai, h));
    let t = s.query(&Query::Trade { with: ai }).unwrap();
    assert_eq!(t.get("contact"), Some(&Json::Bool(false)));
    assert!(t.get("why_not").and_then(Json::as_str).is_some_and(|w| w.contains("refuses to talk")), "{t}");
    let (before, lines) = (s.g.save(), s.journal_lines().unwrap().len());
    for (give, get) in [(peace(), peace()), (gold(1), vec![]), (vec![], gold(1))] {
        let r = s.execute(&deal(give, get));
        assert!(matches!(&r, Err(CmdErr::Cannot(e)) if e.contains("refuses to talk")), "{r:?}");
    }
    assert!(before == s.g.save());
    assert_eq!(s.journal_lines().unwrap().len(), lines);
    assert!(s.g.at_war(my, their));

    // The AI talks once its war counter reaches the threshold.
    while !ffh_play::view::diplo::ai_willing_to_talk(&s.g, ai, h) {
        next_turn(&mut s);
        assert!(s.g.at_war(my, their));
    }
    assert!(s.g.teams[their].ai_at_war_counter[my] > 0);
    s.g.players[h].gold = s.g.players[h].gold.max(50);
    s.g.players[ai].gold = s.g.players[ai].gold.max(50);
    let t = s.query(&Query::Trade { with: ai }).unwrap();
    let has_peace = |side: &str| t.get(side).unwrap().as_arr().unwrap().iter().any(|i| i.get("item") == Some(&Json::idx(TradeableItem::TRADE_PEACE_TREATY)));
    assert!(has_peace("we_give") && has_peace("they_give"));
    // A dual item goes on both sides; at war a deal is a peace deal: a gift alone is refused.
    assert!(matches!(s.execute(&deal(peace(), vec![])), Err(CmdErr::Invalid(e)) if e.contains("both sides")));
    let (before, lines) = (s.g.save(), s.journal_lines().unwrap().len());
    for (give, get) in [(gold(1), vec![]), (vec![], gold(1))] {
        let r = s.execute(&deal(give, get));
        assert!(matches!(&r, Err(CmdErr::Invalid(e)) if e.contains("peace treaty")), "{r:?}");
    }
    // A peace treaty the AI does not take (it would give gold too): declined, nothing changes,
    // nothing is journaled.
    let mut get = peace();
    get.extend(gold(1));
    assert_eq!(applied(&mut s, deal(peace(), get)), Some(false));
    assert!(before == s.g.save());
    assert_eq!(s.journal_lines().unwrap().len(), lines);
    assert!(s.g.at_war(my, their));
    // After ten turns of war it takes peace (ffh-ai's judgement).
    while s.g.teams[their].ai_at_war_counter[my] < 10 {
        next_turn(&mut s);
    }
    assert_eq!(applied(&mut s, deal(peace(), peace())), Some(true));
    assert!(!s.g.at_war(my, their));
    let lines = s.journal_lines().unwrap();
    assert!(lines.iter().any(|l| l.contains("\"c\":\"propose_deal\"")), "the accepted deal is journaled");
    // The peace treaty is a deal between them now.
    let players = s.query(&Query::Players).unwrap();
    let them = players.get("players").unwrap().as_arr().unwrap().iter().find(|p| p.get("id") == Some(&Json::from(ai))).unwrap().clone();
    assert_eq!(them.get("war"), Some(&Json::Bool(false)));
    assert!(!them.get("deals").unwrap().as_arr().unwrap().is_empty(), "{them}");
}

/// A lazy human for the report test: research, production, settlers found, the rest skips.
fn lazy_turn(s: &mut Session) {
    let h = s.human;
    let p = s.pending();
    if p.research_needed {
        let tech = s.read(|g| g.d.techs.keys().find(|&t| g.player_can_research(h, t, false)));
        applied(s, Command::Research { tech, shift: false });
    }
    for c in p.idle_cities {
        let unit = s.read(|g| {
            let n = g.players[h].cities.len();
            let want = if n < 3 && g.cities[c].population > 1 { "UNITCLASS_SETTLER" } else { "UNITCLASS_WARRIOR" };
            let civ = g.player_get_civilization_type(h)?;
            g.d.civ_unit(civ, g.d.t(want)).filter(|&u| g.city_can_train(c, u, false, false, false, false))
        });
        if let Some(u) = unit {
            applied(s, Command::PushOrder { city: c, order: OrderType::ORDER_TRAIN, data: u.idx() as i32, alt: false, shift: false, ctrl: false });
        }
    }
    for grp in s.pending().ready_groups {
        let units = s.g.groups[grp].units.clone();
        let found = s.read(|g| g.unit_can_found(units[0], g.unit_plot(units[0]), false));
        let mission = if found { MissionType::MISSION_FOUND } else { MissionType::MISSION_SKIP };
        let _ = s.execute(&Command::Mission { units, mission, data1: -1, data2: -1, flags: 0, append: false });
    }
}

/// What the human has at the end of its turn (to check the report against).
type EndState = (i32, Vec<TechType>, Vec<CityId>, Vec<UnitId>);

fn end_state(s: &Session) -> EndState {
    let h = s.human;
    let team = s.g.players[h].team;
    (s.g.game_turn(), s.g.d.techs.keys().filter(|&t| s.g.teams[team].has_tech[t]).collect(), s.g.players[h].cities.clone(), s.g.players[h].units.clone())
}

/// The turn report agrees with the game: every tech the team gained, every city and unit of the
/// human's gained or lost between its end of turn and its next control point is reported; the
/// report survives a save.
///
/// Pattern A on a duel: a peaceful duel (`GAMEOPTION_ALWAYS_PEACE` + `GAMEOPTION_NO_BARBARIANS`)
/// cannot end early, so it lasts all 80 turns for ANY seed and the report fires at all 79 control
/// points after the first. The lazy human researches and trains each turn, so over 80 peaceful
/// turns it always gains several techs and units -- the test asserts that coverage (techs and
/// units reported), never a seed-specific count.
#[test]
fn the_turn_report_lists_what_changed() {
    let mut s = peaceful_duel(7);
    let mut prev: Option<EndState> = None;
    let (mut techs_seen, mut units_seen, mut reports) = (0, 0, 0);
    for _ in 0..80 {
        let (techs, cities, units) = { let e = end_state(&s); (e.1, e.2, e.3) };
        let r = s.query(&Query::Report).unwrap();
        if let Some((turn, ptechs, pcities, punits)) = &prev {
            reports += 1;
            assert_eq!(r.get("turn"), Some(&Json::from(s.g.game_turn())));
            assert_eq!(r.get("from_turn"), Some(&Json::from(*turn)));
            let listed = |k: &str| r.get(k).unwrap().as_arr().unwrap().to_vec();
            for t in techs.iter().filter(|t| !ptechs.contains(t)) {
                techs_seen += 1;
                assert!(listed("techs_gained").contains(&Json::idx(*t)), "turn {}: {t:?} not reported", s.g.game_turn());
            }
            assert_eq!(listed("techs_gained").len(), techs.iter().filter(|t| !ptechs.contains(t)).count());
            for c in cities.iter().filter(|c| !pcities.contains(c)) {
                assert!(listed("cities_founded").iter().chain(&listed("cities_acquired")).any(|j| j.get("city") == Some(&Json::from(*c))));
            }
            for u in units.iter().filter(|u| !punits.contains(u)) {
                units_seen += 1;
                assert!(listed("units_gained").iter().any(|j| j.get("unit") == Some(&Json::from(*u))), "turn {}: unit {u:?}", s.g.game_turn());
            }
            for u in punits.iter().filter(|u| !units.contains(u)) {
                assert!(listed("units_lost").iter().any(|j| j.get("unit") == Some(&Json::from(*u))));
            }
            assert_eq!(listed("units_gained").len(), units.iter().filter(|u| !punits.contains(u)).count());
        }
        lazy_turn(&mut s);
        prev = Some(end_state(&s));
        next_turn(&mut s);
    }
    // Coverage, not a golden count: the report fires at all 79 control points after the first,
    // and over the peaceful run the lazy human gains at least one tech and one unit, so the
    // per-turn exactness checks above ran against real techs_gained and units_gained entries.
    assert!(reports >= 79 && techs_seen >= 1 && units_seen >= 1, "{reports} reports, {techs_seen} techs, {units_seen} units");
    // The report and the pending snapshot are in the sidecar.
    let (bytes, sidecar) = s.save_bytes().unwrap();
    let r = s.query(&Query::Report).unwrap();
    let loaded = Session::load(&bytes, Some(&sidecar), Arc::new(AdvisorAnswerer), SessionOptions { journal: false, ..Default::default() }).unwrap();
    assert_eq!(loaded.query(&Query::Report).unwrap(), r);
}

/// A team the human meets during the AI turns brings its wars along: they are not new wars
/// (P3 review). The report lists the contact, and no war of the new team.
#[test]
fn a_team_met_during_the_ai_turns_is_a_contact_not_a_war() {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, 21);
    ng.opponents = 2;
    ng.world = d.t("WORLDSIZE_DUEL");
    let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), SessionOptions::default()).unwrap();
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let h = s.human;
    let my = s.g.players[h].team;
    let others: Vec<TeamId> = s.g.civ_team_ids().filter(|&t| t != my).collect();
    let (a, c) = (others[0], others[1]);
    s.g.team_meet(my, a, false);
    if !s.g.at_war(a, c) {
        s.g.team_declare_war(a, c, false, None);
    }
    assert!(s.g.at_war(a, c) && !s.g.teams[my].has_met[c]);
    // The human ends its turn (the snapshot: teams met, wars among them), then meets the other
    // team before its next control point, as during the AI turns.
    applied(&mut s, Command::EndTurn { force: true });
    s.g.team_meet(my, c, false);
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let r = s.query(&Query::Report).unwrap();
    let wars = r.get("wars").unwrap().as_arr().unwrap();
    assert!(wars.iter().all(|w| !w.as_arr().unwrap().contains(&Json::from(c))), "{r}");
    assert_eq!(r.get("contacts"), Some(&Json::from(vec![c])), "{r}");
    assert!(s.g.at_war(a, c), "the war is still on");
}

/// A sidecar's report snapshot whose ids do not fit its save (a corrupted or mismatched
/// sidecar) is dropped when loaded, not used at the next control point (P3 review: a foreign
/// owner, plot or team out of range panicked there).
#[test]
fn a_sidecar_snapshot_that_does_not_fit_is_dropped() {
    let mut s = duel(21);
    next_turn(&mut s);
    let (bytes, sidecar) = s.save_bytes().unwrap();
    assert!(sidecar.report_snapshot.is_none(), "a control point has no pending snapshot");
    let saved_report = s.query(&Query::Report).unwrap();
    let good = ffh_play::report::snapshot(&s.g, s.human, 0);
    let load = |snapshot: Json| {
        let mut sc = sidecar.clone();
        sc.report_snapshot = Some(snapshot);
        let mut l = Session::load(&bytes, Some(&sc), Arc::new(AdvisorAnswerer), SessionOptions { journal: false, ..Default::default() }).unwrap();
        assert_eq!(l.advance(&mut |_| {}), Stop::Human);
        l.query(&Query::Report).unwrap()
    };
    // A snapshot that fits is diffed at the control point.
    let fresh = load(good.to_json());
    assert_ne!(fresh, saved_report);
    assert_eq!(fresh.get("from_turn"), Some(&Json::from(good.turn)));
    // Out of range: dropped, the saved report stays.
    let bad = |key: &str, value: Json| {
        let mut j = good.to_json();
        j.set(key, value);
        j
    };
    for j in [
        bad("foreign", Json::parse(r#"[[999, 200, "Nowhere", 0, 0]]"#).unwrap()),
        bad("plots", Json::parse("[4000000000]").unwrap()),
        bad("wars", Json::parse("[[200, 201]]").unwrap()),
        bad("met", Json::parse("[200]").unwrap()),
    ] {
        assert_eq!(load(j.clone()), saved_report, "{j}");
    }
}

/// When the game ends during the AI turns after the human's end of turn, the report covers that
/// last turn (P3 review: it kept the previous turn's, without what ended the game).
///
/// Pattern B (the ending is constructed, not waited for): the human has a single city (its
/// capital); it ends its turn, so the report's snapshot is taken with that city still the
/// human's; then, as if during the AI turns of the same game turn, an AI takes the human's last
/// city (`player_acquire_city`, a conquest), eliminating the human (`count_human_players_alive`
/// hits 0) and ending the game. `advance` then reaches `Stop::Over` and builds the report of the
/// ending turn -- cities_lost and messages non-empty -- for any seed, with no dependence on the
/// AI ever conquering the human on its own.
#[test]
fn the_report_covers_the_turn_that_ends_the_game() {
    let mut s = duel(65);
    let h = s.human;
    // The human ends its turn with its single city: the report's snapshot captures that city.
    applied(&mut s, Command::EndTurn { force: true });
    let from_turn = s.g.game_turn();
    // As if during the AI turns of this same game turn, an AI conquers the human's last city and
    // the human is eliminated. Built directly (no AI RNG, no turn roll-over): the AI takes the
    // city, the human's remaining units are cleared, and `test_alive` drops it from the alive
    // humans -- exactly the condition under which `update()` ends the game (turn.rs:59-62).
    let city = s.g.players[h].cities[0];
    let ai = s.g.civ_player_ids().find(|&p| p != h).unwrap();
    s.g.player_acquire_city(ai, city, true, false, true);
    for u in s.g.players[h].units.clone() {
        s.g.unit_kill(u, false, None);
    }
    s.g.test_alive();
    assert_eq!(s.g.count_human_players_alive(), 0, "the human is eliminated this turn");
    s.g.set_game_state(GameStateType::GAMESTATE_OVER);
    // `advance` sees the game is over and builds the report of the turn that ended it.
    assert_eq!(s.advance(&mut |_| {}), Stop::Over);
    let turn = s.g.game_turn();
    assert_eq!(turn, from_turn, "the game ended in the AI turns of the human's own game turn");
    let r = s.query(&Query::Report).unwrap();
    // The game ended in the AI turns of the game turn the human ended: the report covers it.
    assert_eq!(r.get("turn"), Some(&Json::from(turn)), "{r}");
    assert_eq!(r.get("from_turn"), Some(&Json::from(turn)), "{r}");
    assert!(!r.get("cities_lost").unwrap().as_arr().unwrap().is_empty(), "{r}");
    assert!(!r.get("messages").unwrap().as_arr().unwrap().is_empty(), "{r}");
}

/// A panic while the UI state is locked (a query that fails inside a decision is answered
/// `busy`) poisons its mutex; queries and the game go on (P3 review: every later lock
/// panicked, the game thread included).
#[test]
fn a_poisoned_ui_state_does_not_stop_the_session() {
    let mut s = duel(21);
    let ui = s.ui.clone();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _held = ui.lock().unwrap();
        panic!("a query failed while holding the UI state (expected by the test)");
    }));
    assert!(r.is_err() && s.ui.is_poisoned());
    let h = s.human;
    let unit = s.g.players[h].units[0];
    let (x, y) = (s.g.units[unit].x, s.g.units[unit].y);
    s.query(&Query::Status).unwrap();
    s.query(&Query::View).unwrap();
    let w = s.g.map.grid_width;
    s.query(&Query::Path { units: vec![unit], x: (x + 1) % w, y, flags: 0 }).unwrap();
    s.set_reveal(true);
    s.set_reveal(false);
    next_turn(&mut s);
    s.query(&Query::View).unwrap();
}

/// The `target` actions are the exe's interface modes (`canDoInterfaceMode`): GO_TO is a `move`
/// (the declare-war question), ROUTE_TO only for workers (`UNITAI_WORKER`) that can build a
/// route. A `mission` MOVE_TO into a rival's territory asks too (P3 review: it moved without
/// asking, and an adept was offered route-to).
///
/// Pattern B (the situation is constructed, so it needs a real declare-war move -- NOT a peaceful
/// game): a met rival is given a city, whose culture claims the ring of plots around it, and a
/// human warrior is placed on a free plot just beyond that ring. For ANY seed the warrior then
/// has a declare-war move into the rival's territory, so `move`, the MOVE_TO mission and the GO_TO
/// action all ask to declare war. The human's own capital (founded by `duel`) holds the units for
/// the ROUTE_TO check, with the road tech granted so a worker can build a route.
#[test]
fn target_actions_follow_the_interface_modes() {
    let mut s = duel(19);
    let d = s.g.d;
    let h = s.human;
    let target_actions = |s: &Session, units: &[UnitId]| -> Vec<Json> {
        let gq = s.query(&Query::Group { units: units.to_vec() }).unwrap();
        gq.get("actions").unwrap().as_arr().unwrap().iter().filter(|a| a.get("kind") == Some(&Json::from("target"))).cloned().collect()
    };
    let mode = |a: &Json| a.get("mode").and_then(Json::as_str).unwrap().to_string();

    // Construct a human warrior next to a met rival's border: meet the rival, grant the road tech
    // (for the ROUTE_TO check below), give the rival a city so it has territory, and stand a human
    // warrior on a free plot just outside that territory.
    let rival = s.g.civ_player_ids().find(|&p| p != h).unwrap();
    let (my, their) = (s.g.players[h].team, s.g.players[rival].team);
    s.g.team_meet(my, their, false);
    s.g.team_set_has_tech(my, d.t::<TechType>("TECH_EXPLORATION"), true, None, false, false);
    let (capx, capy) = s.read(|g| {
        let c = g.players[h].cities[0];
        (g.cities[c].x, g.cities[c].y)
    });
    // A land plot for the rival's city, away from the human's capital, with a land ring plot
    // (`tp`, rival territory once the city is founded) and a free land plot beyond it (`up`, where
    // the warrior stands, just outside the border).
    let dirs: [(i32, i32); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)];
    let (rc, tp, up) = s
        .read(|g| {
            let open = |p: PlotIdx| {
                !g.plot_is_water(p) && !g.plot_is_impassable(p) && g.map.plots[p].plot_city.is_none() && g.map.plots[p].units.is_empty()
            };
            g.map.plot_ids().find_map(|rc| {
                let (rx, ry) = g.map.xy(rc);
                if !open(rc) || g.plot_distance(rx, ry, capx, capy) < 5 {
                    return None;
                }
                dirs.iter().find_map(|&(dx, dy)| {
                    let (tp, up) = (g.plot_xy(rx, ry, dx, dy)?, g.plot_xy(rx, ry, 2 * dx, 2 * dy)?);
                    (open(tp) && open(up)).then_some((rc, tp, up))
                })
            })
        })
        .expect("a land spot for the rival's city with a free plot next to its border");
    let (rx, ry) = s.g.map.xy(rc);
    s.g.player_init_city(rival, rx, ry, true, true);
    let (ux, uy) = s.g.map.xy(up);
    let u = s.g.player_init_unit(h, d.t::<UnitType>("UNIT_WARRIOR"), ux, uy, None, None);
    let (tx, ty) = s.g.map.xy(tp);
    assert!(
        s.read(|g| g.unit_can_move(u) && g.unit_get_declare_war_move(u, tp).is_some()),
        "the warrior must have a declare-war move into the rival's territory"
    );

    // A move that needs a war: `move` asks, and so does the mission and the GO_TO action.
    let before = s.g.save();
    let asks = |r: Result<CmdOk, CmdErr>| matches!(r, Ok(CmdOk::NeedsConfirm(Confirm::DeclareWar { .. })));
    assert!(asks(s.execute(&Command::Move { units: vec![u], x: tx, y: ty, append: false, declare_war: None })));
    let mission = Command::Mission { units: vec![u], mission: MissionType::MISSION_MOVE_TO, data1: tx, data2: ty, flags: 0, append: false };
    assert!(asks(s.execute(&mission)), "the mission must ask like move");
    let go_to = target_actions(&s, &[u]).into_iter().find(|a| mode(a) == "INTERFACEMODE_GO_TO").expect("GO_TO");
    assert_eq!(go_to.get("any"), Some(&Json::Bool(true)));
    let mut cmd = go_to.get("cmd").unwrap().clone();
    assert_eq!(cmd.get("c"), Some(&Json::from("move")));
    cmd.set("x", tx);
    cmd.set("y", ty);
    assert!(asks(s.execute(&Command::from_json(d, &cmd).unwrap())));
    assert!(before == s.g.save(), "the questions changed the game");

    // ROUTE_TO: a worker that can build a route, not an adept (its builds are not routes and
    // its AI type is not a worker's), nor a warrior.
    let (x, y) = s.read(|g| {
        let c = g.players[h].cities[0];
        (g.cities[c].x, g.cities[c].y)
    });
    let civ = s.g.player_get_civilization_type(h).unwrap();
    let make = |s: &mut Session, class: &str| {
        let t = s.g.d.civ_unit(civ, s.g.d.t(class)).unwrap_or_else(|| panic!("{class}"));
        s.g.player_init_unit(h, t, x, y, None, None)
    };
    let worker = make(&mut s, "UNITCLASS_WORKER");
    let adept = make(&mut s, "UNITCLASS_ADEPT");
    let warrior = make(&mut s, "UNITCLASS_WARRIOR");
    let modes = |s: &Session, u: UnitId| target_actions(s, &[u]).iter().map(mode).collect::<Vec<_>>();
    assert_eq!(s.g.units[worker].ai_type, Some(UnitAIType::UNITAI_WORKER));
    assert!(s.read(|g| g.unit_can_build_route(worker)), "the human has the road tech, so a worker can build a route");
    assert_eq!(modes(&s, worker), ["INTERFACEMODE_GO_TO", "INTERFACEMODE_ROUTE_TO"]);
    assert!(s.g.d.units[s.g.units[adept].unit_type].builds.iter().any(|&b| b), "adepts have builds (mana nodes)");
    assert_eq!(modes(&s, adept), ["INTERFACEMODE_GO_TO"]);
    assert_eq!(modes(&s, warrior), ["INTERFACEMODE_GO_TO"]);
    // A stack offers route-to when one of its units can.
    assert_eq!(target_actions(&s, &[warrior, worker]).iter().map(mode).collect::<Vec<_>>(), ["INTERFACEMODE_GO_TO", "INTERFACEMODE_ROUTE_TO"]);
}

/// Prints every reply of a control point (`cargo test --release -p ffh-play --test views --
/// --ignored --nocapture`), to read them.
#[test]
#[ignore]
fn print_queries() {
    let ng = new_game(4);
    let (mut s, mut b) = bot_session(&ng, None);
    let until = s.g.game_turn() + 30;
    bot::run(&mut s, &mut b, until, |_, _| {});
    for q in all_queries(&s) {
        if matches!(q, Query::Data | Query::Map { .. }) {
            continue;
        }
        let r = s.query(&q).unwrap();
        let t = r.to_pretty();
        println!("==== {q:?}\n{}", if t.len() > 6000 { &t[..6000] } else { &t });
        if let Some(Json::Str(text)) = r.get("text").or(r.get("map")) {
            println!("{text}");
        }
    }
}
