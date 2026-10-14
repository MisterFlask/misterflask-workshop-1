//! Game message log (replaces C++ `addMessage` / `addReplayMessage` / debug logs).
//!
//! Disabled by default (benchmarks); enable for debugging and UIs. Use the
//! closure form so formatting costs nothing when disabled:
//! `g.log.msg(turn, Some(p), || format!("..."))`.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    pub turn: i32,
    pub player: Option<u8>,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct GameLog {
    pub enabled: bool,
    pub entries: Vec<LogEntry>,
    /// Keep at most this many entries (0 = unbounded).
    pub cap: usize,
}

impl GameLog {
    pub fn msg(&mut self, turn: i32, player: Option<crate::ids::PlayerId>, f: impl FnOnce() -> String) {
        if !self.enabled {
            return;
        }
        if self.cap > 0 && self.entries.len() >= self.cap {
            self.entries.remove(0);
        }
        self.entries.push(LogEntry { turn, player: player.map(|p| p.0), text: f() });
    }
}
