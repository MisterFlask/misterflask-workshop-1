//! The server side of the browser's screens (design §7 P6): the map tile deltas, the city
//! screen's buttons against the commands they send, the civics screen's proposed revolution,
//! the group cycle, options and hotkeys the client reads, the trade screen's city names, and a
//! quick run of the T13 bench (its timings are `tests/bench.rs`, ignored).

mod common;

use std::sync::Arc;

use common::{apply_map, assert_same_map, bot_session, new_game};
use ffh_core::ids::*;
use ffh_data::*;
use ffh_play::command::{CmdErr, CmdOk, Command};
use ffh_play::human::AdvisorAnswerer;
use ffh_play::query::Query;
use ffh_play::{bot, Json, NewGame, Session, SessionOptions, Stop};

fn map_query(s: &Session, since: Option<i64>, epoch: Option<&str>) -> Json {
    s.query(&Query::Map { since, epoch: epoch.map(str::to_string) }).unwrap()
}

/// A client keeping the map with `map {since, epoch}` holds the full map after every turn of a
/// bot's game, also across debug reveal; a revision of another epoch, or none, gets the full map.
#[test]
fn map_deltas_keep_a_client_up_to_date() {
    let (mut s, mut b) = bot_session(&new_game(7), None);
    let mut client: Option<Json> = None;
    let (mut deltas, mut fulls, mut points) = (0, 0, 0);
    let until = s.g.game_turn() + 25;
    let stop = bot::run(&mut s, &mut b, until, |s, _| {
        points += 1;
        if points == 12 {
            s.set_reveal(true);
        }
        if points == 14 {
            s.set_reveal(false);
        }
        let reply = match &client {
            None => map_query(s, None, None),
            Some(c) => map_query(s, Some(c.req::<i64>("rev").unwrap()), Some(c.req_str("epoch").unwrap())),
        };
        if reply.req::<bool>("full") == Ok(false) {
            deltas += 1;
        } else {
            fulls += 1;
        }
        let held = match &client {
            Some(c) => apply_map(c, &reply),
            None => reply.clone(),
        };
        // The full map of the same revision (the tracker has nothing new to count).
        let full = map_query(s, None, None);
        assert_same_map(&held, &full, &format!("turn {}", s.g.game_turn()));
        client = Some(held);
    });
    assert_eq!(stop, Stop::Human);
    assert!(deltas >= 15, "{deltas} deltas, {fulls} full maps");
    let c = client.unwrap();
    // Out of range, another epoch, none: the full map.
    let rev = c.req::<i64>("rev").unwrap();
    let epoch = c.req_str("epoch").unwrap();
    for (since, ep) in [(Some(rev + 1), Some(epoch)), (Some(0), Some(epoch)), (Some(rev), Some("x")), (Some(rev), None), (None, Some(epoch))] {
        assert_eq!(map_query(&s, since, ep).req::<bool>("full"), Ok(true), "{since:?} {ep:?}");
    }
    // Nothing changed since the client's revision: an empty delta.
    let d = map_query(&s, Some(rev), Some(epoch));
    assert_eq!((d.req::<bool>("full"), d.req_arr("tiles").map(<[Json]>::len)), (Ok(false), Ok(0)));
}

/// A duel at the human's first control point, its capital founded.
fn duel(seed: u32) -> (Session, CityId) {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, seed);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
    let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), SessionOptions::default()).unwrap();
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let h = s.human;
    let settler = s.g.players[h].units.iter().copied().find(|&u| s.g.d.units[s.g.units[u].unit_type].found).unwrap();
    applied(&mut s, Command::Mission { units: vec![settler], mission: MissionType::MISSION_FOUND, data1: -1, data2: -1, flags: 0, append: false });
    let c = s.g.players[h].cities[0];
    (s, c)
}

fn applied(s: &mut Session, c: Command) {
    match s.execute(&c) {
        Ok(CmdOk::Applied { .. }) => {}
        other => panic!("{c:?} -> {other:?}"),
    }
}

fn task(city: CityId, task: TaskType, data1: i32, data2: i32, option: bool) -> Command {
    Command::DoTask { city, task, data1, data2, option, alt: false, shift: false, ctrl: false }
}

fn next_turn(s: &mut Session) {
    applied(s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
}

/// The city screen's specialist +/- flags are what `do_task CHANGE_SPECIALIST` accepts, a click
/// on a workable plot works it or stops working it, and the emphasize and automation toggles
/// show what the tasks set.
#[test]
fn the_city_screen_buttons_are_the_commands() {
    let (mut s, c) = duel(3);
    next_turn(&mut s);
    // The city grown to size 4 (test setup through the rules: the specialists need people).
    s.g.city_set_population(c, 4);
    let screen = |s: &Session| s.query(&Query::City { city: c }).unwrap();
    let before = screen(&s);
    let size = before.req::<i64>("size").unwrap();
    assert!(size >= 1);
    let mut tried = 0;
    for sp in before.req_arr("specialists").unwrap() {
        let k = sp.req::<i32>("specialist").unwrap();
        for (flag, change) in [("plus", 1), ("minus", -1)] {
            let now = screen(&s);
            let entry = now.req_arr("specialists").unwrap().iter().find(|x| x.req::<i32>("specialist") == Ok(k)).cloned().unwrap();
            let shown = entry.req::<bool>(flag).unwrap();
            let r = s.execute(&task(c, TaskType::TASK_CHANGE_SPECIALIST, k, change, false));
            match (shown, r) {
                (true, Ok(CmdOk::Applied { .. })) => tried += 1,
                (false, Err(CmdErr::Cannot(_))) => {}
                (shown, r) => panic!("specialist {k} {flag} shown {shown}: {r:?}"),
            }
        }
    }
    assert!(tried >= 2, "{tried} specialist changes");
    // A workable plot that is not the centre: a click toggles it.
    let now = screen(&s);
    let plot = now.req_arr("plots").unwrap().iter().find(|p| p.req::<i64>("i") != Ok(0) && p.req::<bool>("can_work") == Ok(true)).cloned().expect("a workable plot");
    let i = plot.req::<i32>("i").unwrap();
    let worked = plot.req::<bool>("worked").unwrap();
    applied(&mut s, task(c, TaskType::TASK_CHANGE_WORKING_PLOT, i, -1, false));
    let after = screen(&s);
    let p = after.req_arr("plots").unwrap().iter().find(|p| p.req::<i32>("i") == Ok(i)).cloned().unwrap();
    // Unless the city had nobody to put there (no extra population and nothing worse to drop).
    if worked || after.req::<i64>("extra_population").unwrap_or(0) > 0 || p.req::<bool>("worked") == Ok(true) {
        assert_ne!(p.req::<bool>("worked"), Ok(worked), "plot {i} toggled");
    }
    assert_eq!(after.req::<bool>("citizens_automated"), Ok(false), "working a plot by hand stops the automation");
    applied(&mut s, task(c, TaskType::TASK_SET_AUTOMATED_CITIZENS, -1, -1, true));
    assert_eq!(screen(&s).req::<bool>("citizens_automated"), Ok(true));
    // Emphasize: the flags follow the task.
    let e = EmphasizeType::from_usize(0);
    assert_eq!(screen(&s).req_arr("emphasize").unwrap().len(), s.g.d.emphasizes.len());
    applied(&mut s, task(c, TaskType::TASK_SET_EMPHASIZE, e.idx() as i32, -1, true));
    assert_eq!(screen(&s).req_arr("emphasize").unwrap()[0], Json::Bool(true));
    applied(&mut s, task(c, TaskType::TASK_SET_EMPHASIZE, e.idx() as i32, -1, false));
    assert_eq!(screen(&s).req_arr("emphasize").unwrap()[0], Json::Bool(false));
}

/// `civics {with}` proposes the revolution `update_civics` would make: valid exactly when the
/// command is applied, with the anarchy and upkeep of the merged set.
#[test]
fn the_civics_screen_proposes_what_the_command_does() {
    let (mut s, _) = duel(3);
    let d = s.g.d;
    // Every civic alone: the proposal's validity is the command's verdict (on a copy of the game).
    let screen = s.query(&Query::Civics { with: Vec::new() }).unwrap();
    assert!(screen.get("proposed").is_none());
    let mut valid = 0;
    let mut checked = 0;
    for o in screen.req_arr("options").unwrap() {
        for c in o.req_arr("civics").unwrap() {
            let civic = CivicType::from_usize(c.req::<usize>("civic").unwrap());
            let p = s.query(&Query::Civics { with: vec![civic] }).unwrap();
            let proposed = p.get("proposed").cloned().unwrap();
            let ok = proposed.req::<bool>("valid").unwrap();
            if c.req::<bool>("current") == Ok(true) {
                assert!(!ok && proposed.req::<bool>("changed") == Ok(false), "{proposed}");
                continue;
            }
            if ok {
                valid += 1;
                assert_eq!(proposed.req::<i64>("anarchy").unwrap(), c.req::<i64>("anarchy").unwrap(), "{}", ffh_play::names::name(d, civic));
            }
            // The command on a copy of the game.
            let (bytes, sidecar) = s.save_bytes().unwrap();
            let mut copy = Session::load(&bytes, Some(&sidecar), Arc::new(AdvisorAnswerer), SessionOptions { journal: false, ..Default::default() }).unwrap();
            let r = copy.execute(&Command::UpdateCivics { civics: vec![civic] });
            assert_eq!(matches!(r, Ok(CmdOk::Applied { .. })), ok, "{} proposed {proposed}: {r:?}", ffh_play::names::name(d, civic));
            checked += 1;
        }
    }
    assert!(checked >= 5 && valid >= 1, "{checked} civics, {valid} valid");
    // Two civics of one option: refused by the command, not valid here.
    let opt = &screen.req_arr("options").unwrap()[0];
    let two: Vec<CivicType> = opt.req_arr("civics").unwrap().iter().take(2).map(|c| CivicType::from_usize(c.req::<usize>("civic").unwrap())).collect();
    let p = s.query(&Query::Civics { with: two.clone() }).unwrap();
    assert_eq!(p.get("proposed").unwrap().req::<bool>("valid"), Ok(false));
    assert!(s.execute(&Command::UpdateCivics { civics: two }).is_err());
    // The query decodes names.
    let j = Json::parse(r#"{"q":"civics","with":["CIVIC_DESPOTISM"]}"#).unwrap();
    assert_eq!(Query::from_json(&j).unwrap(), Query::Civics { with: vec![d.t("CIVIC_DESPOTISM")] });
    assert!(Query::from_json(&Json::parse(r#"{"q":"civics","with":["UNIT_WARRIOR"]}"#).unwrap()).is_err());
}

/// What the client reads for cycling, options and hotkeys: the group cycle (the human's groups,
/// C++ `m_groupCycle`), whether a group has orders to continue, the player options, and the
/// action hotkeys in `/api/data`.
#[test]
fn the_view_has_the_cycle_the_options_and_the_data_the_hotkeys() {
    let (s, _) = duel(5);
    let view = s.query(&Query::View).unwrap();
    let groups: Vec<i64> = view.req_arr("groups").unwrap().iter().map(|g| g.req::<i64>("id").unwrap()).collect();
    let cycle: Vec<i64> = view.req_arr("group_cycle").unwrap().iter().map(|g| g.as_i64().unwrap()).collect();
    assert!(!cycle.is_empty() && cycle.iter().all(|g| groups.contains(g)), "{cycle:?} {groups:?}");
    let gc: Vec<i64> = s.g.players[s.human].group_cycle.iter().map(|g| g.0 as i64).collect();
    assert_eq!(cycle, gc);
    assert_eq!(view.req::<bool>("has_auto_unit"), Ok(s.g.player_has_auto_unit(s.human)));
    let me = view.get("me").unwrap();
    let options = me.req_arr("options").unwrap();
    assert_eq!(options.len(), PlayerOptionType::NUM);
    assert_eq!(options[PlayerOptionType::PLAYEROPTION_QUICK_MOVES.idx()], Json::Bool(true));
    let data = s.query(&Query::Data).unwrap();
    let missions = data.get("tables").unwrap().req_arr("MissionType").unwrap();
    let fortify = missions.iter().find(|m| m.req_str("type") == Ok("MISSION_FORTIFY")).unwrap();
    assert_eq!(fortify.req_arr("keys").unwrap()[0].req_str("key"), Ok("KB_F"));
    assert_eq!(fortify.req::<i64>("order"), Ok(15));
    let builds = data.get("tables").unwrap().req_arr("BuildType").unwrap();
    assert!(builds.iter().any(|b| b.req_str("type") == Ok("BUILD_ROAD") && b.req_arr("keys").unwrap()[0].req_str("key") == Ok("KB_R")));
    let modes = data.req_arr("interface_modes").unwrap();
    assert!(modes.iter().any(|m| m.req_str("type") == Ok("INTERFACEMODE_GO_TO") && m.req_arr("keys").unwrap()[0].req_str("key") == Ok("KB_G")));
    let step = data.get("defines").and_then(|d| d.get("commerce_percent_change_increments")).and_then(Json::as_i64);
    assert_eq!(step, Some(s.g.d.defines.commerce_percent_change_increments as i64));
}

/// The group cycle's test is C++ `readyToSelect()` (`bAny = false`): a stack in which one unit
/// has used its moves still waits for orders (`ready`, `pending.ready_groups`: the end-turn gate)
/// but the cycle skips it (`selectable` false); a group whose units can all move is both
/// (P6 review).
#[test]
fn a_partly_moved_stack_waits_but_is_not_selectable() {
    let (mut s, c) = duel(5);
    let h = s.human;
    let (x, y) = (s.g.cities[c].x, s.g.cities[c].y);
    // Test setup through the rules: two warriors in the capital, grouped by the command.
    let warrior: UnitType = s.g.d.t("UNIT_WARRIOR");
    let a = s.g.player_init_unit(h, warrior, x, y, None, None);
    let b = s.g.player_init_unit(h, warrior, x, y, None, None);
    applied(&mut s, Command::JoinGroup { unit: b, head: Some(a) });
    let grp = s.g.units[a].group.unwrap();
    assert_eq!(s.g.units[b].group, Some(grp));
    let flags = |s: &Session| {
        let view = s.query(&Query::View).unwrap();
        let j = view.req_arr("groups").unwrap().iter().find(|j| j.req::<i64>("id") == Ok(grp.0 as i64)).unwrap().clone();
        let pending: Vec<i64> = view.get("pending").unwrap().req_arr("ready_groups").unwrap().iter().map(|g| g.as_i64().unwrap()).collect();
        (j.req::<bool>("ready").unwrap(), j.req::<bool>("selectable").unwrap(), pending.contains(&(grp.0 as i64)))
    };
    assert_eq!(flags(&s), (true, true, true));
    // One of the two has no moves left (as after a Warrior and a Scout moved one plot).
    s.g.unit_finish_moves(b);
    assert!(s.g.group_ready_to_move(grp, true) && !s.g.group_ready_to_move(grp, false));
    assert_eq!(flags(&s), (true, false, true));
    // None can move: neither.
    s.g.unit_finish_moves(a);
    assert_eq!(flags(&s), (false, false, false));
}

/// The trade tables name the cities either side offers (the diplomacy screen lists them).
#[test]
fn trade_tables_name_the_cities() {
    let (mut s, capital) = duel(3);
    next_turn(&mut s);
    let h = s.human;
    let other = s.g.civ_player_ids().find(|&p| p != h && s.g.players[p].alive).unwrap();
    // Test setup through the rules: meet the other player, and a second city (a capital is not
    // tradeable) on a free land plot away from the capital.
    let (t1, t2) = (s.g.players[h].team, s.g.players[other].team);
    s.g.team_meet(t1, t2, false);
    let (cx, cy) = (s.g.cities[capital].x, s.g.cities[capital].y);
    let spot = (4..8)
        .flat_map(|d| [(d, 0), (-d, 0), (0, d), (0, -d)])
        .filter_map(|(dx, dy)| s.g.plot_xy(cx, cy, dx, dy))
        .find(|&p| !s.g.plot_is_water(p) && s.g.map.plots[p].owner.is_none_or(|o| o == h) && s.g.map.plots[p].units.is_empty())
        .expect("a free land plot");
    let (x, y) = s.g.map.xy(spot);
    let second = s.g.player_init_city(h, x, y, true, true);
    let r = s.query(&Query::Trade { with: other }).unwrap();
    assert_eq!(r.req::<bool>("contact"), Ok(true), "{r}");
    let city = TradeableItem::TRADE_CITIES.idx() as i64;
    let offered: Vec<&Json> = r.req_arr("we_give").unwrap().iter().filter(|i| i.req::<i64>("item") == Ok(city)).collect();
    assert!(offered.iter().any(|i| i.req::<i64>("data") == Ok(second.0 as i64)), "{r}");
    for i in offered {
        let c = CityId(i.req::<u32>("data").unwrap());
        assert_eq!(i.req_str("name"), Ok(ffh_play::view::city_name(&s.g, c).as_str()));
    }
}

/// `ffh-play bench --quick`: the session and the bare update loop play the same game (the
/// session's profile on: it changes nothing), the session's own work and the autosave are
/// measured inside its runs, commands are timed and the view is built (no timing budget here:
/// `tests/bench.rs`, ignored, has T13's).
#[test]
fn the_quick_bench_plays_the_same_game_three_ways() {
    let r = ffh_play::bench::run(&ffh_play::bench::BenchOptions { bot_turns: 0, ..ffh_play::bench::BenchOptions::quick() });
    assert!(r.same_game, "{r:?}");
    assert!(r.commands > 0 && r.view_plots > 0 && r.raw_ms_per_turn > 0.0, "{r:?}");
    assert!(r.update_ms_per_turn > 0.0 && r.own_ms_per_turn > 0.0 && r.autosave_ms_per_turn > 0.0, "{r:?}");
    assert!(r.own_share > 0.0 && r.own_share.is_finite() && r.raw_max_ms_per_turn >= r.raw_ms_per_turn, "{r:?}");
}

/// `Session::profile`: `advance` adds up its `update()` slices, its own work and the autosave
/// (only with a saves directory), and the profile does not change the game.
#[test]
fn the_advance_profile_splits_update_own_work_and_autosave() {
    let dir = common::TempDir::new("profile");
    let play = |saves: Option<std::path::PathBuf>, profile: bool| {
        let d = ffh_data::global();
        let mut ng = NewGame::new(d, 4);
        ng.opponents = 1;
        ng.world = d.t("WORLDSIZE_DUEL");
        let opts = SessionOptions { saves_dir: saves, ..SessionOptions::default() };
        let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), opts).unwrap();
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
        if profile {
            s.profile = Some(Default::default());
        }
        applied(&mut s, Command::AutoPlay { turns: 3 });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
        (s.g.calculate_sync_checksum(), s.g.game_turn(), s.profile)
    };
    let (sum, turn, none) = play(None, false);
    assert_eq!(none, None);
    let (sum2, turn2, p) = play(None, true);
    assert_eq!((sum2, turn2), (sum, turn));
    let p = p.unwrap();
    assert!(p.slices > 0 && p.update > std::time::Duration::ZERO && p.own > std::time::Duration::ZERO, "{p:?}");
    assert_eq!(p.autosave, std::time::Duration::ZERO, "no saves directory, no autosave: {p:?}");
    let (sum3, turn3, p) = play(Some(dir.0.clone()), true);
    assert_eq!((sum3, turn3), (sum, turn));
    let p = p.unwrap();
    assert!(p.autosave > std::time::Duration::ZERO, "three turn boundaries autosaved: {p:?}");
}
