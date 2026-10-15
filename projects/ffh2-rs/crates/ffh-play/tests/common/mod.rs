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
