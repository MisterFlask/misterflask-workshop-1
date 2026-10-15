//! The views and queries (design §4.4-4.7): every query answers at a control point, reads leave
//! the game as it was, and the replies keep to their documented shapes.

mod common;

use std::sync::Arc;
use std::time::Instant;

use common::{bot_session, new_game};
use ffh_core::ids::*;
use ffh_core::structs::TradeData;
use ffh_data::*;
use ffh_play::command::{CmdErr, CmdOk, Command};
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
        Query::Map { since: None },
        Query::Techs,
        Query::Civics,
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
        qs.push(Query::Inspect { kind: "unit".into(), id: units[0].0 as i64 });
    }
    for &c in &g.players[h].cities {
        qs.push(Query::City { city: c });
    }
    for p in g.player_ids() {
        if p != h && g.players[p].alive && !g.player_is_barbarian(p) && g.teams[g.players[h].team].has_met[g.players[p].team] {
            qs.push(Query::Trade { with: p });
        }
    }
    qs.push(Query::Inspect { kind: "UnitType".into(), id: 0 });
    qs
}

#[test]
fn every_query_answers_and_reads_change_nothing() {
    let ng = new_game(4);
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
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, seed);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
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
/// (`consider_offer`): a refusal changes nothing, an accepted peace treaty makes peace.
#[test]
fn propose_deal_follows_the_trade_tables_and_the_ai_answer() {
    let mut s = duel(21);
    let h = s.human;
    let ai = s.g.civ_player_ids().find(|&p| p != h).unwrap();
    let (my, their) = (s.g.players[h].team, s.g.players[ai].team);
    let peace = || vec![item(TradeableItem::TRADE_PEACE_TREATY, 0)];
    // Unmet: no contact, no trade query.
    let deal = |give: Vec<TradeData>, get: Vec<TradeData>| Command::ProposeDeal { to: ai, give, get };
    assert!(matches!(s.execute(&deal(peace(), peace())), Err(CmdErr::Cannot(_))));
    assert!(s.query(&Query::Trade { with: ai }).is_err());
    s.g.team_meet(my, their, false);
    let t = s.query(&Query::Trade { with: ai }).unwrap();
    assert_eq!(t.get("contact"), Some(&Json::Bool(true)));
    // At peace there is no peace treaty to offer.
    assert!(matches!(s.execute(&deal(peace(), peace())), Err(CmdErr::Cannot(_))));
    applied(&mut s, Command::DeclareWar { team: their });
    let t = s.query(&Query::Trade { with: ai }).unwrap();
    let has_peace = |side: &str| t.get(side).unwrap().as_arr().unwrap().iter().any(|i| i.get("item") == Some(&Json::idx(TradeableItem::TRADE_PEACE_TREATY)));
    assert!(has_peace("we_give") && has_peace("they_give"));
    // A dual item goes on both sides; nonsense is refused before the AI is asked.
    assert!(matches!(s.execute(&deal(peace(), vec![])), Err(CmdErr::Invalid(e)) if e.contains("both sides")));
    assert!(matches!(s.execute(&deal(vec![], vec![])), Err(CmdErr::Invalid(_))));
    assert!(matches!(s.execute(&deal(vec![item(TradeableItem::TRADE_GOLD, i32::MAX)], vec![])), Err(CmdErr::Cannot(_))));
    assert!(matches!(s.execute(&deal(vec![item(TradeableItem::TRADE_CITIES, 999_999)], vec![])), Err(CmdErr::Cannot(_))));
    assert!(matches!(s.execute(&Command::ProposeDeal { to: h, give: peace(), get: peace() }), Err(CmdErr::Cannot(_))));
    assert!(matches!(s.execute(&Command::ProposeDeal { to: PlayerId(200), give: peace(), get: peace() }), Err(CmdErr::Invalid(_))));
    // Fresh war: the AI declines (its war counter is under 10); nothing changes, nothing is
    // journaled.
    let (before, lines) = (s.g.save(), s.journal_lines().unwrap().len());
    assert_eq!(applied(&mut s, deal(peace(), peace())), Some(false));
    assert!(before == s.g.save());
    assert_eq!(s.journal_lines().unwrap().len(), lines);
    assert!(s.g.at_war(my, their));
    // After ten turns of war it takes peace.
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
#[test]
fn the_turn_report_lists_what_changed() {
    let mut s = duel(2);
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
    assert!(reports >= 79 && techs_seen >= 3 && units_seen >= 3, "{reports} reports, {techs_seen} techs, {units_seen} units");
    // The report and the pending snapshot are in the sidecar.
    let (bytes, sidecar) = s.save_bytes().unwrap();
    let r = s.query(&Query::Report).unwrap();
    let loaded = Session::load(&bytes, Some(&sidecar), Arc::new(AdvisorAnswerer), SessionOptions { journal: false, ..Default::default() }).unwrap();
    assert_eq!(loaded.query(&Query::Report).unwrap(), r);
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
