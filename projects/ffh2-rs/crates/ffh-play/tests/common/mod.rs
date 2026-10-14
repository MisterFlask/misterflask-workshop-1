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
