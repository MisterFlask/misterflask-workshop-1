//! Automated groups of the human (P7, `Controller::update_automated_group` in ffh-ai's
//! `automation.rs`): the automate commands, the AI moving the groups during the human's turns,
//! FfH's AI-controlled units, and the journal replaying such a game.

use std::sync::Arc;

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;
use ffh_play::command::{CmdErr, CmdOk, Command};
use ffh_play::human::AdvisorAnswerer;
use ffh_play::journal;
use ffh_play::session::replay;
use ffh_play::{NewGame, Phase, Session, SessionOptions, Stop};

/// A duel at the human's first control point, its capital founded; `techs`: the human also gets
/// the techs for roads, farms and mines (not journaled: no replay then).
fn session(seed: u32, techs: bool) -> Session {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, seed);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
    ng.civ = Some(d.t::<CivilizationType>("CIVILIZATION_BANNOR"));
    let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), SessionOptions::default()).unwrap();
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    if techs {
        let team = s.g.players[s.human].team;
        for t in ["TECH_EXPLORATION", "TECH_AGRICULTURE", "TECH_MINING"] {
            s.g.team_set_has_tech(team, d.t::<TechType>(t), true, Some(s.human), false, false);
        }
    }
    let settler = s.g.players[s.human].units.iter().copied().find(|&u| s.g.d.units[s.g.units[u].unit_type].found).expect("a settler");
    ok(&mut s, Command::Mission { units: vec![settler], mission: MissionType::MISSION_FOUND, data1: -1, data2: -1, flags: 0, append: false });
    s
}

fn ok(s: &mut Session, c: Command) {
    match s.execute(&c) {
        Ok(CmdOk::Applied { .. }) => {}
        other => panic!("{c:?} -> {other:?}"),
    }
    if s.auto_moves_due() {
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    }
}

/// A new unit of the human in its capital.
fn unit(s: &mut Session, name: &str) -> UnitId {
    let h = s.human;
    let c = s.g.players[h].cities[0];
    let (x, y) = (s.g.cities[c].x, s.g.cities[c].y);
    let t = s.g.d.t::<UnitType>(name);
    s.g.player_init_unit(h, t, x, y, None, None)
}

fn automate(s: &mut Session, u: UnitId, a: AutomateType) {
    ok(s, Command::DoCommand { units: vec![u], command: CommandType::COMMAND_AUTOMATE, data1: a.idx() as i32, data2: -1, alt: false, confirmed: false });
}

/// Ends the turn (forced) and runs to the human's next control point; at every control point no
/// AI-moved group is left without orders.
fn next_turn(s: &mut Session) {
    // (After a rules call outside a command: the AI moves its groups first.)
    if s.phase() != Phase::Human(s.human) {
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    }
    ok(s, Command::EndTurn { force: true });
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let idle = s.pending().ai_groups_idle;
    let describe = |s: &Session, grp: GroupId| {
        let gr = &s.g.groups[grp];
        format!(
            "{grp:?} automate {:?} activity {:?} missions {:?} force {} head moves {:?} ai {:?}",
            gr.automate_type,
            gr.activity_type,
            gr.mission_queue.iter().map(|m| (m.mission_type, m.data1, m.data2)).collect::<Vec<_>>(),
            gr.force_update,
            gr.units.first().map(|&u| (s.g.units[u].moves, s.g.unit_moves_left(u))),
            gr.units.first().map(|&u| s.g.units[u].ai_type),
        )
    };
    assert!(idle.is_empty(), "an automated group was left ready at turn {}: {:?}", s.g.game_turn(), idle.iter().map(|&g| describe(s, g)).collect::<Vec<_>>());
}

fn group_of(g: &Game, u: UnitId) -> GroupId {
    g.units[u].group.expect("a group")
}

/// The automate buttons are offered for what can be automated (no terraforming), the command
/// automates the group, the AI moves it in the human's turns (it never waits for orders), and
/// the workers improve the capital's land.
#[test]
fn automated_workers_improve_the_land() {
    let mut s = session(41, true);
    let h = s.human;
    let w = unit(&mut s, "UNIT_WORKER");
    ok(&mut s, Command::SetOption { option: PlayerOptionType::PLAYEROPTION_QUICK_MOVES, value: true });
    let actions = s.query_json(&ffh_play::jobj! { "q" => "group", "units" => vec![w] }).unwrap();
    let automates: Vec<i64> = actions.req_arr("actions").unwrap().iter().filter(|a| a.req_str("kind") == Ok("automate")).map(|a| a.req::<i64>("automate").unwrap()).collect();
    let d = s.g.d;
    let names: Vec<&str> = automates.iter().map(|&a| d.type_name(AutomateType::from_usize(a as usize))).collect();
    assert!(names.contains(&"AUTOMATE_BUILD") && names.contains(&"AUTOMATE_NETWORK") && names.contains(&"AUTOMATE_CITY"), "{names:?}");
    assert!(!names.contains(&"AUTOMATE_EXPLORE") && !names.contains(&"AUTOMATE_TERRAFORMING"), "{names:?}");
    // Terraforming is refused (no AI for it).
    let t = Command::DoCommand { units: vec![w], command: CommandType::COMMAND_AUTOMATE, data1: AutomateType::AUTOMATE_TERRAFORMING.idx() as i32, data2: -1, alt: false, confirmed: false };
    assert!(matches!(s.execute(&t), Err(CmdErr::Cannot(_))));

    automate(&mut s, w, AutomateType::AUTOMATE_BUILD);
    let grp = group_of(&s.g, w);
    assert_eq!(s.g.groups[grp].automate_type, Some(AutomateType::AUTOMATE_BUILD));
    assert!(!s.pending().ready_groups.contains(&grp), "the human gives it no orders");
    assert!(!s.g.groups[grp].mission_queue.is_empty() || !s.g.group_ready_to_move(grp, true), "the AI gave it orders in the command's settle");
    let improved = |g: &Game| g.map.plot_ids().filter(|&q| g.map.plots[q].owner == Some(h) && (g.map.plots[q].improvement_type.is_some() || g.map.plots[q].route_type.is_some()) && g.map.plots[q].plot_city.is_none()).count();
    let before = improved(&s.g);
    for _ in 0..25 {
        next_turn(&mut s);
        if !s.g.units.contains(w) {
            break;
        }
        assert!(s.g.group_is_automated(group_of(&s.g, w)), "stays automated");
    }
    assert!(improved(&s.g) > before, "the automated worker built something ({} -> {})", before, improved(&s.g));
    assert_eq!(s.phase(), Phase::Human(h));
}

/// AUTOMATE_NETWORK builds only routes (and resource connections); AUTOMATE_CITY works only on
/// its city's plots.
#[test]
fn network_and_city_automation_keep_to_their_jobs() {
    let mut s = session(47, true);
    let h = s.human;
    let d = s.g.d;
    let net = unit(&mut s, "UNIT_WORKER");
    let city = unit(&mut s, "UNIT_WORKER");
    automate(&mut s, net, AutomateType::AUTOMATE_NETWORK);
    automate(&mut s, city, AutomateType::AUTOMATE_CITY);
    let capital = s.g.players[h].cities[0];
    let mut builds = 0;
    for _ in 0..20 {
        for (u, network) in [(net, true), (city, false)] {
            if !s.g.units.contains(u) {
                continue;
            }
            let grp = group_of(&s.g, u);
            for m in &s.g.groups[grp].mission_queue {
                if m.mission_type != MissionType::MISSION_BUILD {
                    continue;
                }
                builds += 1;
                let b = &d.builds[BuildType::from_usize(m.data1 as usize)];
                let plot = s.g.unit_plot(u);
                if network {
                    let bonus = s.g.map.plots[plot].bonus_type;
                    assert!(b.route.is_some() && b.improvement.is_none() || bonus.is_some(), "network automation builds {}", b.base.type_);
                } else if m.data1 >= 0 && s.g.groups[grp].mission_queue.len() == 1 {
                    // Building where it stands: a plot of its city.
                    assert_eq!(s.g.map.plots[plot].working_city, Some(capital), "city automation works its city's plots");
                }
            }
        }
        next_turn(&mut s);
    }
    assert!(builds > 0, "the automated workers built");
}

/// AUTOMATE_EXPLORE: the explorer reveals the map on its own and never attacks (C++
/// `AI_exploreMove` attacks only for AI players).
#[test]
fn automated_explorers_reveal_the_map() {
    let mut s = session(43, true);
    let h = s.human;
    let team = s.g.players[h].team;
    let scout = unit(&mut s, "UNIT_SCOUT");
    automate(&mut s, scout, AutomateType::AUTOMATE_EXPLORE);
    let revealed = |g: &Game| g.map.plot_ids().filter(|&q| g.map.plots[q].revealed[team]).count();
    let before = revealed(&s.g);
    for _ in 0..8 {
        next_turn(&mut s);
    }
    assert!(revealed(&s.g) > before, "the explorer revealed plots ({} -> {})", before, revealed(&s.g));
    if s.g.units.contains(scout) {
        let grp = group_of(&s.g, scout);
        assert_eq!(s.g.groups[grp].automate_type, Some(AutomateType::AUTOMATE_EXPLORE));
        // Stop automation: the human's again.
        ok(&mut s, Command::DoCommand { units: vec![scout], command: CommandType::COMMAND_STOP_AUTOMATION, data1: -1, data2: -1, alt: false, confirmed: false });
        assert!(!s.g.group_is_automated(group_of(&s.g, scout)));
    }
}

/// FfH AI control (Sephi's block of `CvUnitAI::AI_update`): an enraged unit leaves its stack
/// and acts on its own; a defensive-only one gets its control back.
#[test]
fn ai_controlled_units_leave_the_stack_and_act() {
    let mut s = session(44, true);
    let h = s.human;
    let d = s.g.d;
    let enraged = d.t::<PromotionType>("PROMOTION_ENRAGED");
    let a = unit(&mut s, "UNIT_WARRIOR");
    let b = unit(&mut s, "UNIT_WARRIOR");
    s.g.unit_join_group(b, Some(group_of(&s.g, a)), false);
    assert_eq!(group_of(&s.g, a), group_of(&s.g, b));
    s.g.unit_set_has_promotion(b, enraged, true);
    assert!(s.g.unit_is_ai_control(b));
    next_turn(&mut s);
    assert!(s.g.units.contains(a));
    if s.g.units.contains(b) {
        assert_ne!(group_of(&s.g, a), group_of(&s.g, b), "the AI-controlled unit left the stack");
        assert!(!s.g.group_is_ai_control(group_of(&s.g, a)), "the rest is the human's");
    }
    // A defensive-only unit does not stay AI-controlled.
    let c = unit(&mut s, "UNIT_WARRIOR");
    s.g.units[c].only_defensive += 1;
    s.g.unit_set_has_promotion(c, enraged, true);
    let before = s.g.units[c].ai_control;
    next_turn(&mut s);
    if s.g.units.contains(c) {
        assert!(s.g.units[c].ai_control < before, "changeAIControl(-1)");
    }
    assert_eq!(s.phase(), Phase::Human(h));
}

/// Automation is a function of the game state: the journal of a game with automated units
/// (made through commands: a worker the capital trains, a starting unit exploring) replays to
/// the same game.
#[test]
fn a_game_with_automated_units_replays() {
    let mut s = session(45, false);
    let h = s.human;
    let d = s.g.d;
    let explorer = s.g.players[h].units.iter().copied().find(|&u| s.g.unit_can_automate(u, Some(AutomateType::AUTOMATE_EXPLORE))).expect("a starting unit that can explore");
    automate(&mut s, explorer, AutomateType::AUTOMATE_EXPLORE);
    let capital = s.g.players[h].cities[0];
    let worker = d.t::<UnitType>("UNIT_WORKER");
    ok(&mut s, Command::PushOrder { city: capital, order: OrderType::ORDER_TRAIN, data: worker.idx() as i32, alt: false, shift: false, ctrl: false });
    let mut automated = false;
    for _ in 0..30 {
        next_turn(&mut s);
        if !automated {
            if let Some(w) = s.g.players[h].units.iter().copied().find(|&u| s.g.units[u].unit_type == worker) {
                automate(&mut s, w, AutomateType::AUTOMATE_BUILD);
                automated = true;
            }
        }
    }
    assert!(automated, "the capital trained a worker");
    let lines = journal::parse_lines(&s.journal_lines().unwrap()).unwrap();
    let r = replay(lines, None, SessionOptions::default()).unwrap_or_else(|e| panic!("replay: {e}"));
    assert!(r.g.save() == s.g.save(), "the replay ends in a different game");
}
