//! Shared helpers of the session tests (design §8).

#![allow(dead_code)]

use std::path::PathBuf;

use ffh_data::*;
use ffh_play::bot::Bot;
use ffh_play::{NewGame, Session, SessionOptions};

/// ffh-stress's invariant checks, reused without editing ffh-stress (design F20).
#[path = "../../../ffh-stress/src/check.rs"]
pub mod check;

/// The T1 game: small map, 3 AI opponents.
pub fn new_game(seed: u32) -> NewGame {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, seed);
    ng.opponents = 3;
    ng.world = d.t::<WorldSizeType>("WORLDSIZE_SMALL");
    ng
}

/// A session played by a fresh bot (memory journal unless `saves` is given).
pub fn bot_session(ng: &NewGame, saves: Option<PathBuf>) -> (Session, Bot) {
    let bot = Bot::new(ng.seed);
    let opts = SessionOptions { saves_dir: saves, ..Default::default() };
    let s = Session::new_game(ng, bot.answerer(), opts).expect("new game");
    (s, bot)
}

/// A fresh directory under the system temp dir, removed by `Drop`.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        let p = std::env::temp_dir().join(format!("ffh-play-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `(game turn, sync checksum)` at a control point.
pub fn checkpoint(s: &Session) -> (i32, i32) {
    (s.g.game_turn(), s.g.calculate_sync_checksum())
}

/// A saved game at a human control point (turn 4 of a duel, seed 11) where the human's warrior
/// stands next to an undefended size-3 AI city of a team at war with it: a `move` onto the city
/// captures it and raises the keep/raze decision inside the command (staged with rules calls, as
/// in `tests/decisions.rs`). The transports' tests load it.
pub struct StagedCapture {
    /// `<dir>/capture.ffhsave` (with its sidecar).
    pub save: PathBuf,
    pub unit: ffh_core::ids::UnitId,
    /// The city's plot.
    pub x: i32,
    pub y: i32,
    /// Game turn of the save.
    pub turn: i32,
}

/// Writes a [`StagedCapture`] into `dir` (the session's game directory).
pub fn staged_capture(dir: &std::path::Path) -> StagedCapture {
    use ffh_play::command::{CmdOk, Command};
    use ffh_play::Stop;
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, 11);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
    let opts = SessionOptions { saves_dir: Some(dir.to_path_buf()), ..Default::default() };
    let mut s = Session::new_game(&ng, std::sync::Arc::new(ffh_play::human::AdvisorAnswerer), opts).expect("new game");
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let h = s.human;
    let ok = |s: &mut Session, c: Command| match s.execute(&c) {
        Ok(CmdOk::Applied { .. }) => {}
        other => panic!("{c:?} -> {other:?}"),
    };
    let settler = s.g.players[h].units.iter().copied().find(|&u| d.units[s.g.units[u].unit_type].found).expect("a settler");
    ok(&mut s, Command::Mission { units: vec![settler], mission: MissionType::MISSION_FOUND, data1: -1, data2: -1, flags: 0, append: false });
    while s.g.game_turn() < 4 {
        ok(&mut s, Command::EndTurn { force: true });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    }
    let (c, owner) = s.g.cities.iter().find(|(_, c)| c.owner != h).map(|(id, c)| (id, c.owner)).expect("an AI city");
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
    let unit = attacker.expect("a plot to attack from");
    let paths = s.save("capture").expect("save at the control point");
    StagedCapture { save: paths.save, unit, x: cx, y: cy, turn: s.g.game_turn() }
}

/// A saved game at a human control point (turn 3 of a duel, seed 20) where one group (`ready`,
/// a new warrior in the capital) is ready and another (`mover`, a land unit) has a MOVE_TO order
/// two or more turns long queued with its moves left (an "auto unit"); every other group sleeps
/// and no barbarian or animal is left (a human group in danger drops its orders). Skipping
/// `ready` leaves nothing ready while `mover` has orders: the exe's next frame sends AutoMoves
/// (`updateTestEndTurn`), which moves `mover` before the end of the turn.
pub struct StagedAutoMoves {
    pub save: PathBuf,
    pub ready: ffh_core::ids::UnitId,
    pub mover: ffh_core::ids::UnitId,
    /// Where `mover` stands in the save.
    pub from: (i32, i32),
    pub turn: i32,
}

/// Writes a [`StagedAutoMoves`] into `dir` (the session's game directory).
pub fn staged_auto_moves(dir: &std::path::Path) -> StagedAutoMoves {
    use ffh_core::path::{FinderKind, PathFinder};
    use ffh_core::structs::MissionData;
    use ffh_play::command::{CmdOk, Command};
    use ffh_play::Stop;
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, 20);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
    let opts = SessionOptions { saves_dir: Some(dir.to_path_buf()), ..Default::default() };
    let mut s = Session::new_game(&ng, std::sync::Arc::new(ffh_play::human::AdvisorAnswerer), opts).expect("new game");
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let h = s.human;
    let ok = |s: &mut Session, c: Command| match s.execute(&c) {
        Ok(CmdOk::Applied { .. }) => {}
        other => panic!("{c:?} -> {other:?}"),
    };
    let settler = s.g.players[h].units.iter().copied().find(|&u| d.units[s.g.units[u].unit_type].found).expect("a settler");
    ok(&mut s, Command::Mission { units: vec![settler], mission: MissionType::MISSION_FOUND, data1: -1, data2: -1, flags: 0, append: false });
    while s.g.game_turn() < 3 {
        ok(&mut s, Command::EndTurn { force: true });
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    }
    let g = &mut s.g;
    let barbarian = g.barbarian_player();
    for u in g.players[barbarian].units.clone() {
        g.unit_kill(u, false, None);
    }
    // A land group of ours and a destination two or more turns away (found with a finder of
    // our own, as the interface does).
    let mut target = None;
    'search: for &grp in &g.players[h].groups {
        let head = g.groups[grp].units[0];
        if g.unit_get_domain_type(head) != DomainType::DOMAIN_LAND || !g.group_ready_to_move(grp, true) {
            continue;
        }
        let (x, y) = (g.units[head].x, g.units[head].y);
        let mut finder = PathFinder::new(FinderKind::Path);
        finder.set_group(Some(grp));
        for r in 3..8 {
            for (dx, dy) in [(r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, -r), (r, -r), (-r, r)] {
                let Some(p) = g.plot_xy(x, y, dx, dy) else { continue };
                let (tx, ty) = (g.map.plots[p].x, g.map.plots[p].y);
                if finder.generate_path(g, x, y, tx, ty, false, 0, false) && finder.path_turns().is_some_and(|t| t >= 2) {
                    target = Some((grp, tx, ty));
                    break 'search;
                }
            }
        }
    }
    let (grp, tx, ty) = target.expect("a land group with a destination two turns away");
    let mover = g.groups[grp].units[0];
    for other in g.players[h].groups.clone() {
        if other != grp && g.group_ready_to_move(other, true) {
            g.group_set_activity_type(other, ActivityType::ACTIVITY_SLEEP);
        }
    }
    let capital = g.players[h].cities[0];
    let (cx, cy) = (g.cities[capital].x, g.cities[capital].y);
    let ready = g.player_init_unit(h, d.t::<UnitType>("UNIT_WARRIOR"), cx, cy, None, None);
    let turn = g.game_turn();
    g.group_insert_at_end_mission_queue(grp, MissionData { mission_type: MissionType::MISSION_MOVE_TO, data1: tx, data2: ty, flags: 0, push_turn: turn }, false);
    assert_eq!(ffh_play::decision::ready_groups(g, h), [g.units[ready].group.unwrap()], "only the new warrior is ready");
    assert!(g.player_has_auto_unit(h), "the mover has orders to continue");
    let from = (g.units[mover].x, g.units[mover].y);
    let paths = s.save("automoves").expect("save at the control point");
    StagedAutoMoves { save: paths.save, ready, mover, from, turn }
}
