//! Staged situations for testing the clients (`ffh-play stage
//! capture|attack|automoves|load|trade|automate|election|settlement|corrupt DIR`,
//! and the transports' tests): a saved game at a human control point arranged with rules calls,
//! so that a client can be tried on a situation a fresh game reaches only by chance (a captured
//! city's keep/raze decision, an attack, the AutoMoves rule between commands, the transport
//! picker, a trade with a civilization met). Each writes only `DIR/KIND.ffhsave` and its sidecar: the staged game is played with
//! no journal and no autosaves, so a `DIR` that is also a game's directory keeps its journal and
//! autosaves (P5 review). Nothing here is used in play.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ffh_core::ids::{PlayerId, UnitId, VoteId};
use ffh_core::path::{FinderKind, PathFinder};
use ffh_core::structs::MissionData;
use ffh_data::*;

use crate::command::{CmdOk, Command};
use crate::human::AdvisorAnswerer;
use crate::json::Json;
use crate::session::{Session, SessionOptions, Stop};
use crate::setup::NewGame;

/// A saved game at a human control point (turn 4 of a duel, seed 11) where the human's warrior
/// `unit` stands next to an undefended size-3 AI city at (`x`, `y`) of a team at war with it: a
/// `move` onto the city captures it and raises the keep/raze decision inside the command.
pub struct StagedCapture {
    /// `<dir>/capture.ffhsave` (with its sidecar).
    pub save: PathBuf,
    pub unit: UnitId,
    pub x: i32,
    pub y: i32,
    /// Game turn of the save.
    pub turn: i32,
}

/// A saved game at a human control point (turn 3 of a duel, seed 20) where one group (`ready`,
/// a new warrior in the capital) is ready and another (`mover`, a land unit) has a MOVE_TO order
/// two or more turns long queued with its moves left (an "auto unit"); every other group sleeps
/// and no barbarian or animal is left (a human group in danger drops its orders). Skipping
/// `ready` leaves nothing ready while `mover` has orders: the exe's next frame sends AutoMoves
/// (`updateTestEndTurn`), which moves `mover` before the end of the turn.
pub struct StagedAutoMoves {
    pub save: PathBuf,
    pub ready: UnitId,
    pub mover: UnitId,
    /// Where `mover` stands in the save.
    pub from: (i32, i32),
    pub turn: i32,
}

impl StagedCapture {
    pub fn to_json(&self) -> Json {
        crate::jobj! { "stage" => "capture", "save" => self.save.display().to_string(), "unit" => self.unit, "x" => self.x, "y" => self.y, "turn" => self.turn }
    }
}

impl StagedAutoMoves {
    pub fn to_json(&self) -> Json {
        crate::jobj! {
            "stage" => "automoves", "save" => self.save.display().to_string(), "ready" => self.ready, "mover" => self.mover,
            "from" => crate::jarr![self.from.0, self.from.1], "turn" => self.turn,
        }
    }
}

/// A duel with the human's settler settled, played (forced end turns) to `turn`.
fn duel_at(dir: &Path, seed: u32, turn: i32) -> Result<Session, String> {
    duel_as(dir, seed, turn, None)
}

/// [`duel_at`] with the human's civilization (`None`: random).
fn duel_as(dir: &Path, seed: u32, turn: i32, civ: Option<&str>) -> Result<Session, String> {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, seed);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
    ng.civ = civ.map(|c| d.t::<CivilizationType>(c));
    // Only the staged save goes to `dir`: no journal, no autosave ring (which would overwrite and
    // prune the autosaves of a game kept there).
    let opts = SessionOptions { saves_dir: Some(dir.to_path_buf()), autosave_keep: 0, journal: false, ..Default::default() };
    let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), opts).map_err(|e| format!("new game: {e:?}"))?;
    let advance = |s: &mut Session| match s.advance(&mut |_| {}) {
        Stop::Human => Ok(()),
        other => Err(format!("the game stopped: {other:?}")),
    };
    advance(&mut s)?;
    let h = s.human;
    let ok = |s: &mut Session, c: Command| match s.execute(&c) {
        Ok(CmdOk::Applied { .. }) => Ok(()),
        other => Err(format!("{c:?} -> {other:?}")),
    };
    let settler = s.g.players[h].units.iter().copied().find(|&u| d.units[s.g.units[u].unit_type].found).ok_or("no settler")?;
    ok(&mut s, Command::Mission { units: vec![settler], mission: MissionType::MISSION_FOUND, data1: -1, data2: -1, flags: 0, append: false })?;
    while s.g.game_turn() < turn {
        ok(&mut s, Command::EndTurn { force: true })?;
        advance(&mut s)?;
    }
    Ok(s)
}

/// Writes a [`StagedCapture`] into `dir` (a game directory).
pub fn capture(dir: &Path) -> Result<StagedCapture, String> {
    let d = ffh_data::global();
    let mut s = duel_at(dir, 11, 4)?;
    let h = s.human;
    let (c, owner) = s.g.cities.iter().find(|(_, c)| c.owner != h).map(|(id, c)| (id, c.owner)).ok_or("no AI city")?;
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
    let unit = attacker.ok_or("no plot to attack from")?;
    let paths = s.save("capture").map_err(|e| format!("save: {e:?}"))?;
    Ok(StagedCapture { save: paths.save, unit, x: cx, y: cy, turn: s.g.game_turn() })
}

/// A saved game at a human control point (turn 4 of a duel, seed 11) where the human's warrior
/// `unit` stands next to a barbarian warrior at (`x`, `y`) in the open: hovering it shows the
/// attack odds, a `move` there attacks it.
pub struct StagedAttack {
    pub save: PathBuf,
    pub unit: UnitId,
    pub enemy: UnitId,
    pub x: i32,
    pub y: i32,
    pub turn: i32,
}

impl StagedAttack {
    pub fn to_json(&self) -> Json {
        crate::jobj! { "stage" => "attack", "save" => self.save.display().to_string(), "unit" => self.unit, "enemy" => self.enemy, "x" => self.x, "y" => self.y, "turn" => self.turn }
    }
}

/// Writes a [`StagedAttack`] into `dir` (a game directory).
pub fn attack(dir: &Path) -> Result<StagedAttack, String> {
    let d = ffh_data::global();
    let mut s = duel_at(dir, 11, 4)?;
    let h = s.human;
    let g = &mut s.g;
    let barbarian = g.barbarian_player();
    let warrior = d.t::<UnitType>("UNIT_WARRIOR");
    let capital = *g.players[h].cities.first().ok_or("no city")?;
    let (cx, cy) = (g.cities[capital].x, g.cities[capital].y);
    // An empty land plot near the capital for our warrior, and one next to it for theirs.
    let free = |g: &ffh_core::game::Game, p: ffh_core::ids::PlotIdx| {
        !g.plot_is_water(p) && !g.plot_is_impassable(p) && g.map.plots[p].units.is_empty() && g.map.plots[p].plot_city.is_none()
    };
    for r in 1..4 {
        for (dx, dy) in [(r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, -r), (r, -r), (-r, r)] {
            let Some(p) = g.plot_xy(cx, cy, dx, dy) else { continue };
            if !free(g, p) {
                continue;
            }
            for (ex, ey) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                let (px, py) = (g.map.plots[p].x, g.map.plots[p].y);
                let Some(q) = g.plot_xy(px, py, ex, ey) else { continue };
                if !free(g, q) || g.map.plots[q].owner.is_some_and(|o| o != h) {
                    continue;
                }
                let (qx, qy) = (g.map.plots[q].x, g.map.plots[q].y);
                let unit = g.player_init_unit(h, warrior, px, py, None, None);
                let enemy = g.player_init_unit(barbarian, warrior, qx, qy, None, None);
                if g.unit_can_move_or_attack_into(unit, q, false) {
                    let paths = s.save("attack").map_err(|e| format!("save: {e:?}"))?;
                    return Ok(StagedAttack { save: paths.save, unit, enemy, x: qx, y: qy, turn: s.g.game_turn() });
                }
                g.unit_kill(enemy, false, None);
                g.unit_kill(unit, false, None);
            }
        }
    }
    Err("no two free plots near the capital".into())
}

/// Writes a [`StagedAutoMoves`] into `dir` (a game directory).
pub fn auto_moves(dir: &Path) -> Result<StagedAutoMoves, String> {
    let d = ffh_data::global();
    let mut s = duel_at(dir, 20, 3)?;
    let h = s.human;
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
    let (grp, tx, ty) = target.ok_or("no land group with a destination two turns away")?;
    let mover = g.groups[grp].units[0];
    for other in g.players[h].groups.clone() {
        if other != grp && g.group_ready_to_move(other, true) {
            g.group_set_activity_type(other, ActivityType::ACTIVITY_SLEEP);
        }
    }
    let capital = *g.players[h].cities.first().ok_or("no city")?;
    let (cx, cy) = (g.cities[capital].x, g.cities[capital].y);
    let ready = g.player_init_unit(h, d.t::<UnitType>("UNIT_WARRIOR"), cx, cy, None, None);
    let turn = g.game_turn();
    g.group_insert_at_end_mission_queue(grp, MissionData { mission_type: MissionType::MISSION_MOVE_TO, data1: tx, data2: ty, flags: 0, push_turn: turn }, false);
    let ready_group = g.units[ready].group.ok_or("the new warrior has no group")?;
    if crate::decision::ready_groups(g, h) != [ready_group] {
        return Err("more than the new warrior is ready".into());
    }
    if !g.player_has_auto_unit(h) {
        return Err("the mover has no orders to continue".into());
    }
    let from = (g.units[mover].x, g.units[mover].y);
    let paths = s.save("automoves").map_err(|e| format!("save: {e:?}"))?;
    Ok(StagedAutoMoves { save: paths.save, ready, mover, from, turn })
}

/// A saved game at a human control point (a duel at turn 1 whose capital is on the coast)
/// where a new warrior `unit` stands in the capital at (`x`, `y`) with two new triremes
/// `transports` it can load into: the `load` action asks which one (the LOADUNIT picker,
/// `confirm load_unit`), answered with `do_command COMMAND_LOAD_UNIT`, `data1` = the
/// transport's owner and `data2` = its id (CvDLLButtonPopup LOADUNIT `OnOkClicked`).
pub struct StagedLoad {
    pub save: PathBuf,
    pub unit: UnitId,
    pub transports: Vec<UnitId>,
    pub x: i32,
    pub y: i32,
    pub turn: i32,
}

impl StagedLoad {
    pub fn to_json(&self) -> Json {
        crate::jobj! {
            "stage" => "load", "save" => self.save.display().to_string(), "unit" => self.unit, "transports" => self.transports.clone(),
            "x" => self.x, "y" => self.y, "turn" => self.turn,
        }
    }
}

/// Writes a [`StagedLoad`] into `dir` (a game directory): the first of a few fixed seeds whose
/// capital is on the coast.
pub fn load(dir: &Path) -> Result<StagedLoad, String> {
    let d = ffh_data::global();
    let trireme = d.t::<UnitType>("UNIT_TRIREME");
    let warrior = d.t::<UnitType>("UNIT_WARRIOR");
    for seed in [4, 1, 2, 3, 5, 6, 7, 8, 9, 10] {
        let mut s = duel_at(dir, seed, 1)?;
        let h = s.human;
        let g = &mut s.g;
        let capital = *g.players[h].cities.first().ok_or("no city")?;
        if !g.city_is_coastal(capital, d.defines.min_water_size_for_ocean) {
            continue;
        }
        let (x, y) = (g.cities[capital].x, g.cities[capital].y);
        let transports = vec![g.player_init_unit(h, trireme, x, y, None, None), g.player_init_unit(h, trireme, x, y, None, None)];
        let unit = g.player_init_unit(h, warrior, x, y, None, None);
        let offered = crate::selection::load_targets(g, &[unit]);
        if offered != transports {
            return Err(format!("seed {seed}: the warrior can load into {offered:?}, not the two triremes {transports:?}"));
        }
        let paths = s.save("load").map_err(|e| format!("save: {e:?}"))?;
        return Ok(StagedLoad { save: paths.save, unit, transports, x, y, turn: s.g.game_turn() });
    }
    Err("no seed with a coastal capital".into())
}

/// A saved game at a human control point (turn 4 of a duel, seed 11) where the human has met the
/// AI player `with` and is at peace with it, knows Currency (gold trading) and both have 100
/// gold: the diplomacy screen offers a trade whose tables hold gold both ways (a gift of gold is
/// accepted).
pub struct StagedTrade {
    pub save: PathBuf,
    pub with: PlayerId,
    pub turn: i32,
}

impl StagedTrade {
    pub fn to_json(&self) -> Json {
        crate::jobj! { "stage" => "trade", "save" => self.save.display().to_string(), "with" => self.with, "turn" => self.turn }
    }
}

/// Writes a [`StagedTrade`] into `dir` (a game directory).
pub fn trade(dir: &Path) -> Result<StagedTrade, String> {
    let mut s = duel_at(dir, 11, 4)?;
    let h = s.human;
    let with = s.g.civ_player_ids().find(|&p| p != h && s.g.players[p].alive).ok_or("no AI player")?;
    let g = &mut s.g;
    let (my, their) = (g.players[h].team, g.players[with].team);
    g.team_meet(my, their, false);
    // Gold trading (`CvTeam::isGoldTrading`: either side's tech with bGoldTrading, Currency).
    let currency = g.d.techs.keys().find(|&t| g.d.techs[t].gold_trading).ok_or("no gold-trading tech")?;
    g.team_set_has_tech(my, currency, true, Some(h), false, false);
    // Gold to trade (C++ `CvPlayer::setGold` is a plain setter).
    g.players[h].gold = 100;
    g.players[with].gold = 100;
    let paths = s.save("trade").map_err(|e| format!("save: {e:?}"))?;
    Ok(StagedTrade { save: paths.save, with, turn: s.g.game_turn() })
}

/// Free land plots around the human's capital (nearest first), for staged units.
fn free_plots_near_capital(g: &ffh_core::game::Game, h: PlayerId) -> Result<Vec<(i32, i32)>, String> {
    let capital = *g.players[h].cities.first().ok_or("no city")?;
    let (cx, cy) = (g.cities[capital].x, g.cities[capital].y);
    let mut out = Vec::new();
    for r in 1..4 {
        for (dx, dy) in [(r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, -r), (r, -r), (-r, r)] {
            let Some(p) = g.plot_xy(cx, cy, dx, dy) else { continue };
            if !g.plot_is_water(p) && !g.plot_is_impassable(p) && g.map.plots[p].units.is_empty() && g.map.plots[p].plot_city.is_none() {
                out.push((g.map.plots[p].x, g.map.plots[p].y));
            }
        }
    }
    Ok(out)
}

/// A saved game at a human control point (turn 4 of a duel, seed 11, the Bannor) where a new
/// worker `worker` and a new scout `scout` of the human wait for orders on free plots near the
/// capital, and the human knows the techs for roads, farms and mines: the unit panel offers
/// their automate buttons (P7).
pub struct StagedAutomate {
    pub save: PathBuf,
    pub worker: UnitId,
    pub scout: UnitId,
    pub turn: i32,
}

impl StagedAutomate {
    pub fn to_json(&self) -> Json {
        crate::jobj! { "stage" => "automate", "save" => self.save.display().to_string(), "worker" => self.worker, "scout" => self.scout, "turn" => self.turn }
    }
}

/// Writes a [`StagedAutomate`] into `dir` (a game directory).
pub fn automate(dir: &Path) -> Result<StagedAutomate, String> {
    let d = ffh_data::global();
    let mut s = duel_as(dir, 11, 4, Some("CIVILIZATION_BANNOR"))?;
    let h = s.human;
    let g = &mut s.g;
    let team = g.players[h].team;
    for t in ["TECH_EXPLORATION", "TECH_AGRICULTURE", "TECH_MINING"] {
        g.team_set_has_tech(team, d.t::<TechType>(t), true, Some(h), false, false);
    }
    let plots = free_plots_near_capital(g, h)?;
    let (&(wx, wy), &(sx, sy)) = (plots.first().ok_or("no free plot")?, plots.get(1).ok_or("no second free plot")?);
    let worker = g.player_init_unit(h, d.t::<UnitType>("UNIT_WORKER"), wx, wy, None, None);
    let scout = g.player_init_unit(h, d.t::<UnitType>("UNIT_SCOUT"), sx, sy, None, None);
    let paths = s.save("automate").map_err(|e| format!("save: {e:?}"))?;
    Ok(StagedAutomate { save: paths.save, worker, scout, turn: s.g.game_turn() })
}

/// A saved game at a human control point (turn 3 of a duel, seed 15) where both players run a
/// council's civic, the human's team is its secretary general, and the secretary's election
/// (BUTTONPOPUP_CHOOSEELECTION) waits for the human: `pending.elections` holds `selection` (P7).
pub struct StagedElection {
    pub save: PathBuf,
    pub selection: VoteId,
    pub turn: i32,
}

impl StagedElection {
    pub fn to_json(&self) -> Json {
        crate::jobj! { "stage" => "election", "save" => self.save.display().to_string(), "selection" => self.selection, "turn" => self.turn }
    }
}

/// Writes a [`StagedElection`] into `dir` (a game directory).
pub fn election(dir: &Path) -> Result<StagedElection, String> {
    let d = ffh_data::global();
    let mut s = duel_at(dir, 15, 3)?;
    let h = s.human;
    let g = &mut s.g;
    let team = g.players[h].team;
    let mut council = None;
    for name in ["DIPLOVOTE_OVERCOUNCIL", "DIPLOVOTE_UNDERCOUNCIL"] {
        let vs = d.t::<VoteSourceType>(name);
        let Some(civic) = d.vote_sources[vs].civic else { continue };
        if let Some(t) = d.civics[civic].tech_prereq {
            g.team_set_has_tech(team, t, true, Some(h), false, false);
        }
        if g.player_can_do_civics(h, Some(civic)) {
            council = Some((vs, civic));
            break;
        }
    }
    let (vs, civic) = council.ok_or("no council the human can join")?;
    let option = d.civics[civic].civic_option_type.ok_or("a civic without an option")?;
    for p in g.civ_player_ids().collect::<Vec<_>>() {
        if g.players[p].alive {
            g.player_set_civics(p, option, Some(civic));
        }
    }
    let sg = d.votes.keys().find(|&v| d.votes[v].secretary_general && d.votes[v].vote_source_type[vs]).ok_or("no secretary general vote")?;
    g.vote_outcome[sg] = ffh_core::structs::PlayerVote(team.0 as i32);
    g.vote_timer[vs] = 0;
    g.secretary_general_timer[vs] = 5;
    let before: Vec<VoteId> = g.vote_selections.iter().map(|v| v.id).collect();
    g.do_vote_selection();
    let selection = g.vote_selections.iter().map(|v| v.id).find(|id| !before.contains(id)).ok_or("no election to hold")?;
    let paths = s.save("election").map_err(|e| format!("save: {e:?}"))?;
    Ok(StagedElection { save: paths.save, selection, turn: s.g.game_turn() })
}

/// A saved game at a human control point (turn 2 of a duel, seed 12, the Kuriotates) where a
/// new settler `settler` of the human stands on a city site at (`x`, `y`): founding there asks
/// city or settlement (BUTTONPOPUP_CONFIRMSETTLEMENT, P7).
pub struct StagedSettlement {
    pub save: PathBuf,
    pub settler: UnitId,
    pub x: i32,
    pub y: i32,
    pub turn: i32,
}

impl StagedSettlement {
    pub fn to_json(&self) -> Json {
        crate::jobj! { "stage" => "settlement", "save" => self.save.display().to_string(), "settler" => self.settler, "x" => self.x, "y" => self.y, "turn" => self.turn }
    }
}

/// Writes a [`StagedSettlement`] into `dir` (a game directory).
pub fn settlement(dir: &Path) -> Result<StagedSettlement, String> {
    let d = ffh_data::global();
    let mut s = duel_as(dir, 12, 2, Some("CIVILIZATION_KURIOTATES"))?;
    let h = s.human;
    let g = &mut s.g;
    let capital = *g.players[h].cities.first().ok_or("no city")?;
    let (cx, cy) = (g.cities[capital].x, g.cities[capital].y);
    let mut site = None;
    for q in g.map.plot_ids() {
        let (x, y) = g.map.xy(q);
        let dist = g.plot_distance(x, y, cx, cy);
        if (4..=6).contains(&dist) && g.player_can_found(h, x, y, false) && g.map.plots[q].units.is_empty() && site.map_or(true, |(b, _, _)| dist < b) {
            site = Some((dist, x, y));
        }
    }
    let (_, x, y) = site.ok_or("no city site near the capital")?;
    let settler = g.player_init_unit(h, d.t::<UnitType>("UNIT_SETTLER"), x, y, None, None);
    let paths = s.save("settlement").map_err(|e| format!("save: {e:?}"))?;
    Ok(StagedSettlement { save: paths.save, settler, x, y, turn: s.g.game_turn() })
}

/// A saved game at a human control point in a real mid-game world, for the 3D renderer's vertical
/// slice (docs/R3D.md): seed 11, a standard map at low sea level, the human playing the Ljosalfar
/// against six AI civilizations, 200 turns of AI autoplay; then the heartland of the evil
/// civilization with the most cities is corrupted by FfH2's own hell rule -- every land plot it
/// owns within two plots of its capital takes the plot counter an Infernal owner gives its plots
/// each turn (`plot_change_plot_counter(p, 100)`, CvGameUtils.doHellTerrain's Infernal branch):
/// the terrains turn to their hell forms and the resources to their hell kinds, as in play.
pub struct StagedCorrupt {
    pub save: PathBuf,
    /// The corrupted civilization and its capital.
    pub player: PlayerId,
    pub x: i32,
    pub y: i32,
    /// Plots whose counter was raised.
    pub plots: usize,
    pub turn: i32,
}

impl StagedCorrupt {
    pub fn to_json(&self) -> Json {
        crate::jobj! { "stage" => "corrupt", "save" => self.save.display().to_string(), "player" => self.player.0 as i64, "x" => self.x, "y" => self.y, "plots" => self.plots as i64, "turn" => self.turn }
    }
}

pub fn corrupt(dir: &Path) -> Result<StagedCorrupt, String> {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, 11);
    ng.opponents = 6;
    ng.world = d.t("WORLDSIZE_STANDARD");
    ng.sea_level = d.t("SEALEVEL_LOW");
    ng.civ = Some(d.t::<CivilizationType>("CIVILIZATION_LJOSALFAR"));
    let opts = SessionOptions { saves_dir: Some(dir.to_path_buf()), autosave_keep: 0, journal: false, ..Default::default() };
    let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), opts).map_err(|e| format!("new game: {e:?}"))?;
    let advance = |s: &mut Session| match s.advance(&mut |_| {}) {
        Stop::Human => Ok(()),
        other => Err(format!("the game stopped: {other:?}")),
    };
    advance(&mut s)?;
    match s.execute(&Command::AutoPlay { turns: 200 }) {
        Ok(CmdOk::Applied { .. }) => {}
        other => return Err(format!("autoplay -> {other:?}")),
    }
    advance(&mut s)?;
    let evil = d.t::<AlignmentType>("ALIGNMENT_EVIL");
    let g = &mut s.g;
    let player = g
        .player_ids()
        .filter(|&p| g.players[p].alive && g.players[p].alignment == Some(evil) && g.player_get_capital_city(p).is_some())
        .max_by_key(|&p| (g.players[p].cities.len(), std::cmp::Reverse(p.0)))
        .ok_or("no evil civilization with a capital")?;
    let capital = g.player_get_capital_city(player).ok_or("no capital")?;
    let (cx, cy) = (g.cities[capital].x, g.cities[capital].y);
    let mut plots = 0;
    for p in g.map.plot_ids() {
        let (x, y) = g.map.xy(p);
        if g.plot_distance(x, y, cx, cy) <= 2 && g.map.plots[p].owner == Some(player) && !g.plot_is_water(p) {
            g.plot_change_plot_counter(p, 100);
            plots += 1;
        }
    }
    let turn = s.g.game_turn();
    let paths = s.save("corrupt").map_err(|e| format!("save: {e:?}"))?;
    Ok(StagedCorrupt { save: paths.save, player, x: cx, y: cy, plots, turn })
}
