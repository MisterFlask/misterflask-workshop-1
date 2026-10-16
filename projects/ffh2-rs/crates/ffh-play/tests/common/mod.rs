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

/// [`new_game`] with the game unable to end early: `GAMEOPTION_ALWAYS_PEACE` (no AI can declare
/// war on or eliminate the human) and `GAMEOPTION_NO_BARBARIANS` (no barbarians to kill it), so
/// no victory can fire and no human can be eliminated inside the usual run lengths. A bot game
/// then runs the full requested length and `bot::run` reaches the until-turn with `Stop::Human`
/// for ANY seed and under any AI/content change -- the basis of the breadth tests (fog
/// invariants, read-only queries, turn reports), which must assert coverage, never a seed
/// producing a particular emergent outcome.
pub fn peaceful_game(seed: u32) -> NewGame {
    let mut ng = new_game(seed);
    ng.options.push(GameOptionType::GAMEOPTION_ALWAYS_PEACE);
    ng.options.push(GameOptionType::GAMEOPTION_NO_BARBARIANS);
    ng
}

/// A session played by a fresh bot (memory journal unless `saves` is given).
pub fn bot_session(ng: &NewGame, saves: Option<PathBuf>) -> (Session, Bot) {
    let bot = Bot::new(ng.seed);
    let opts = SessionOptions { saves_dir: saves, ..Default::default() };
    let s = Session::new_game(ng, bot.answerer(), opts).expect("new game");
    (s, bot)
}

/// Sets up a capture directly on an existing session (modeled on [`ffh_play::stage::capture`], a
/// construction primitive for the decision tests): finds an AI city, makes it an undefended
/// size-3 city, puts the human's team at war with its owner, and places a human `UNIT_WARRIOR` on
/// a free land plot adjacent to it. Returns `(warrior, city_x, city_y)`; the caller then runs
/// `Command::Move { units: vec![warrior], x: city_x, y: city_y, append: false, declare_war: None }`
/// to capture the city and raise the `keep_city` decision inside the command -- a deterministic
/// capture with no combat RNG (the seed only shaped a believable board). Panics with a clear
/// message if the map has no AI city or no free adjacent plot to attack from.
pub fn construct_capture(s: &mut Session) -> (ffh_core::ids::UnitId, i32, i32) {
    let h = s.human;
    let d = s.g.d;
    let (c, owner) = s
        .g
        .cities
        .iter()
        .find(|(_, c)| c.owner != h)
        .map(|(id, c)| (id, c.owner))
        .expect("construct_capture: the map has no AI city to capture");
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
    let unit = attacker.expect("construct_capture: no free land plot adjacent to the AI city to attack from");
    (unit, cx, cy)
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

/// A `map` reply applied to the map a client holds (the client's `applyMapDelta`, tiles.js): a
/// full reply replaces it, a delta (`full: false`) sets each listed tile's values.
pub fn apply_map(client: &ffh_play::Json, reply: &ffh_play::Json) -> ffh_play::Json {
    use ffh_play::Json;
    if reply.get("full").and_then(Json::as_bool) != Some(false) {
        return reply.clone();
    }
    let tiles: Vec<usize> = reply.req_arr("tiles").unwrap().iter().map(|t| t.as_i64().unwrap() as usize).collect();
    let mut out = reply.clone();
    for name in ffh_play::view::map::LAYERS {
        let mut a = client.req_arr(name).unwrap().to_vec();
        let values = reply.req_arr(name).unwrap();
        assert_eq!(values.len(), tiles.len(), "{name}");
        for (k, &i) in tiles.iter().enumerate() {
            a[i] = values[k].clone();
        }
        out.set(name, Json::Arr(a));
    }
    out.set("full", true);
    out
}

/// Two maps have the same arrays, revision and hash.
pub fn assert_same_map(a: &ffh_play::Json, b: &ffh_play::Json, what: &str) {
    for k in ffh_play::view::map::LAYERS.iter().chain(&["w", "h", "map_rev", "epoch", "rev", "reveal"]) {
        assert_eq!(a.get(k), b.get(k), "{what}: `{k}` differs");
    }
}

/// `(game turn, sync checksum)` at a control point.
pub fn checkpoint(s: &Session) -> (i32, i32) {
    (s.g.game_turn(), s.g.calculate_sync_checksum())
}

/// The staged saves (`ffh_play::stage`, also `ffh-play stage`): a capture's keep/raze
/// decision, and the AutoMoves rule between commands. The transports' tests load them.
pub use ffh_play::stage::{StagedAutoMoves, StagedCapture};

/// Writes a [`StagedCapture`] into `dir` (the session's game directory).
pub fn staged_capture(dir: &std::path::Path) -> StagedCapture {
    ffh_play::stage::capture(dir).expect("staged capture")
}

/// Writes a [`StagedAutoMoves`] into `dir` (the session's game directory).
pub fn staged_auto_moves(dir: &std::path::Path) -> StagedAutoMoves {
    ffh_play::stage::auto_moves(dir).expect("staged auto moves")
}
