//! The journal (design §2.6): one JSON line per state-changing input, so a game can be replayed
//! exactly (`ffh-play replay`) and bugs reported with a deterministic repro.
//!
//! Lines (`k` = kind; inputs are stamped with `(t, s)` = `(game_turn, turn_slice)` when they
//! were applied, for early divergence detection):
//!
//! ```text
//! {"k":"new","v":1,"setup":{..}}                       header: a new game (NewGame, indices)
//! {"k":"load","v":1,"fnv":"<save hash>","human":0}      header: a loaded save
//! {"k":"take_over","v":1,"fnv":"<save hash>","human":3} header: an AI save taken over
//! {"k":"cmd","t":57,"s":1234,"c":"move",..}             a command, in its canonical request form
//! {"k":"answer","t":57,"s":1236,"kind":"keep_city","option":1}   a blocking decision's answer
//! {"k":"checkpoint","turn":57,"checksum":-12345}        at each turn boundary and a new game's first control point (the autosaves)
//! ```

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ffh_core::game::Game;
use ffh_core::ids::PlayerId;
use ffh_data::GameData;

use crate::decision::Decision;
use crate::human::{Answer, Answerer};
use crate::json::Json;

/// Journal format version (the `v` of header lines).
pub const JOURNAL_VERSION: i64 = 1;

/// `(game_turn, turn_slice)` when an input was applied.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub turn: i32,
    pub slice: i32,
}

impl Stamp {
    pub fn of(g: &Game) -> Stamp {
        Stamp { turn: g.game_turn(), slice: g.turn_slice }
    }
}

/// One journal line.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    New { setup: Json },
    Load { fnv: String, human: PlayerId },
    TakeOver { fnv: String, human: PlayerId },
    /// `cmd` is the canonical command object (with its `c` member).
    Cmd { stamp: Stamp, cmd: Json },
    Answer { stamp: Stamp, kind: String, option: usize },
    Checkpoint { turn: i32, checksum: i32 },
}

impl Line {
    pub fn kind(&self) -> &'static str {
        match self {
            Line::New { .. } => "new",
            Line::Load { .. } => "load",
            Line::TakeOver { .. } => "take_over",
            Line::Cmd { .. } => "cmd",
            Line::Answer { .. } => "answer",
            Line::Checkpoint { .. } => "checkpoint",
        }
    }

    pub fn to_json(&self) -> Json {
        match self {
            Line::New { setup } => crate::jobj! { "k" => "new", "v" => JOURNAL_VERSION, "setup" => setup.clone() },
            Line::Load { fnv, human } => crate::jobj! { "k" => "load", "v" => JOURNAL_VERSION, "fnv" => fnv.as_str(), "human" => *human },
            Line::TakeOver { fnv, human } => crate::jobj! { "k" => "take_over", "v" => JOURNAL_VERSION, "fnv" => fnv.as_str(), "human" => *human },
            Line::Cmd { stamp, cmd } => {
                let mut j = crate::jobj! { "k" => "cmd", "t" => stamp.turn, "s" => stamp.slice };
                if let Json::Obj(m) = cmd {
                    for (k, v) in m {
                        j.set(k, v.clone());
                    }
                }
                j
            }
            Line::Answer { stamp, kind, option } => {
                crate::jobj! { "k" => "answer", "t" => stamp.turn, "s" => stamp.slice, "kind" => kind.as_str(), "option" => *option }
            }
            Line::Checkpoint { turn, checksum } => crate::jobj! { "k" => "checkpoint", "turn" => *turn, "checksum" => *checksum },
        }
    }

    pub fn parse(s: &str) -> Result<Line, String> {
        let j = Json::parse(s).map_err(|e| e.to_string())?;
        Line::from_json(&j)
    }

    pub fn from_json(j: &Json) -> Result<Line, String> {
        let e = |e: crate::json::FieldError| e.to_string();
        let stamp = || -> Result<Stamp, String> { Ok(Stamp { turn: j.req("t").map_err(e)?, slice: j.req("s").map_err(e)? }) };
        let header_version = || -> Result<(), String> {
            match j.req::<i64>("v").map_err(e)? {
                JOURNAL_VERSION => Ok(()),
                v => Err(format!("journal version {v} is not supported (expected {JOURNAL_VERSION})")),
            }
        };
        Ok(match j.req_str("k").map_err(e)? {
            "new" => {
                header_version()?;
                Line::New { setup: j.req_obj("setup").map_err(e)?.clone() }
            }
            "load" => {
                header_version()?;
                Line::Load { fnv: j.req("fnv").map_err(e)?, human: j.req("human").map_err(e)? }
            }
            "take_over" => {
                header_version()?;
                Line::TakeOver { fnv: j.req("fnv").map_err(e)?, human: j.req("human").map_err(e)? }
            }
            "cmd" => {
                let Json::Obj(m) = j else { unreachable!("req_str succeeded on an object") };
                let cmd = Json::Obj(m.iter().filter(|(k, _)| !matches!(k.as_str(), "k" | "t" | "s")).cloned().collect());
                Line::Cmd { stamp: stamp()?, cmd }
            }
            "answer" => Line::Answer { stamp: stamp()?, kind: j.req("kind").map_err(e)?, option: j.req("option").map_err(e)? },
            "checkpoint" => Line::Checkpoint { turn: j.req("turn").map_err(e)?, checksum: j.req("checksum").map_err(e)? },
            k => return Err(format!("unknown journal line kind {k:?}")),
        })
    }
}

/// FNV-1a (64 bit) of a save, as hex: names the save a `load` header refers to.
pub fn fnv_hex(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Replay state shared by the replaying session's journal and its `JournalAnswerer`: the
/// recorded lines and how far the replay got.
#[derive(Debug, Default)]
pub struct Cursor {
    pub lines: Vec<Line>,
    pub pos: usize,
    /// First divergence found (the replay stops there).
    pub desync: Option<String>,
}

impl Cursor {
    pub fn new(lines: Vec<Line>, pos: usize) -> Cursor {
        Cursor { lines, pos, desync: None }
    }

    pub fn peek(&self) -> Option<&Line> {
        self.lines.get(self.pos)
    }

    fn fail(&mut self, msg: String) {
        if self.desync.is_none() {
            self.desync = Some(format!("journal line {}: {msg}", self.pos + 1));
        }
    }

    /// The next line must be `checkpoint {turn, checksum}`.
    pub fn expect_checkpoint(&mut self, turn: i32, checksum: i32) {
        match self.peek().cloned() {
            Some(Line::Checkpoint { turn: t, checksum: c }) if (t, c) == (turn, checksum) => self.pos += 1,
            Some(Line::Checkpoint { turn: t, checksum: c }) => {
                self.fail(format!("checkpoint differs: journal turn {t} checksum {c}, replay turn {turn} checksum {checksum}"))
            }
            other => self.fail(format!("replay reached checkpoint turn {turn}, journal has {}", describe(other.as_ref()))),
        }
    }

    /// The next line must be an answer to a decision of `kind` asked at `stamp`.
    pub fn take_answer(&mut self, kind: &str, stamp: Stamp) -> Option<usize> {
        match self.peek().cloned() {
            Some(Line::Answer { stamp: s, kind: k, option }) if k == kind && s == stamp => {
                self.pos += 1;
                Some(option)
            }
            other => {
                self.fail(format!("replay asked a {kind} decision at turn {} slice {}, journal has {}", stamp.turn, stamp.slice, describe(other.as_ref())));
                None
            }
        }
    }
}

fn describe(l: Option<&Line>) -> String {
    match l {
        None => "nothing more".into(),
        Some(Line::Cmd { stamp, cmd }) => format!("cmd {} at turn {} slice {}", cmd.get("c").map_or("?".into(), |c| c.to_string()), stamp.turn, stamp.slice),
        Some(Line::Answer { stamp, kind, .. }) => format!("a {kind} answer at turn {} slice {}", stamp.turn, stamp.slice),
        Some(l) => format!("a {} line", l.kind()),
    }
}

enum Sink {
    Off,
    Memory(Vec<String>),
    File { w: BufWriter<File>, path: PathBuf },
    /// Replaying: inputs are not written; checkpoints are checked against the cursor.
    Replay(Arc<Mutex<Cursor>>),
}

/// Where the session writes its journal.
pub struct Journal {
    sink: Sink,
    /// Lines written (the sidecar's `journal_line`).
    count: usize,
}

impl Journal {
    pub fn off() -> Journal {
        Journal { sink: Sink::Off, count: 0 }
    }

    /// Lines kept in memory (tests, in-process tools).
    pub fn memory() -> Journal {
        Journal { sink: Sink::Memory(Vec::new()), count: 0 }
    }

    /// A new journal file in `dir`: `journal.jsonl`, or `journal-N.jsonl` if taken.
    pub fn create_in(dir: &Path) -> std::io::Result<Journal> {
        std::fs::create_dir_all(dir)?;
        let mut path = dir.join("journal.jsonl");
        let mut n = 1;
        while path.exists() {
            path = dir.join(format!("journal-{n}.jsonl"));
            n += 1;
        }
        let w = BufWriter::new(File::create(&path)?);
        Ok(Journal { sink: Sink::File { w, path }, count: 0 })
    }

    pub fn replay(cursor: Arc<Mutex<Cursor>>) -> Journal {
        Journal { sink: Sink::Replay(cursor), count: 0 }
    }

    pub fn write(&mut self, line: &Line) {
        self.count += 1;
        match &mut self.sink {
            Sink::Off => {}
            Sink::Memory(v) => v.push(line.to_json().to_string()),
            Sink::File { w, path } => {
                let r = writeln!(w, "{}", line.to_json()).and_then(|_| w.flush());
                if let Err(e) = r {
                    // The journal is a debugging aid: losing it must not stop the game.
                    eprintln!("ffh-play: cannot write journal {}: {e}", path.display());
                }
            }
            Sink::Replay(cursor) => {
                if let Line::Checkpoint { turn, checksum } = line {
                    cursor.lock().unwrap().expect_checkpoint(*turn, *checksum);
                }
            }
        }
    }

    /// The lines of a memory journal.
    pub fn lines(&self) -> Option<&[String]> {
        match &self.sink {
            Sink::Memory(v) => Some(v),
            _ => None,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match &self.sink {
            Sink::File { path, .. } => Some(path),
            _ => None,
        }
    }

    /// Number of lines written so far.
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn is_replay(&self) -> bool {
        matches!(self.sink, Sink::Replay(_))
    }

    /// Replaying: the command the journal has at `stamp` while the game runs (between turn
    /// slices), taken; only the AI autoplay's stop (`autoplay {turns: 0}`, the one command a
    /// transport sends while the game runs) is such a command. None otherwise, and when not
    /// replaying (a journal line at another stamp is then reported at the next control point).
    pub fn take_running_cmd(&mut self, stamp: Stamp) -> Option<Json> {
        let Sink::Replay(cursor) = &self.sink else { return None };
        let mut c = cursor.lock().unwrap();
        match c.peek() {
            Some(Line::Cmd { stamp: s, cmd }) if *s == stamp && is_running_cmd(cmd) => {
                let cmd = cmd.clone();
                c.pos += 1;
                Some(cmd)
            }
            _ => None,
        }
    }
}

/// Whether a journaled command is one applied while the game runs: the AI autoplay's stop.
pub fn is_running_cmd(cmd: &Json) -> bool {
    cmd.get("c").and_then(Json::as_str) == Some("autoplay") && cmd.get("turns").and_then(Json::as_i64) == Some(0)
}

/// Reads a journal file.
pub fn read_file(path: &Path) -> Result<Vec<Line>, String> {
    let f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, l) in BufReader::new(f).lines().enumerate() {
        let l = l.map_err(|e| format!("{}: {e}", path.display()))?;
        if l.trim().is_empty() {
            continue;
        }
        out.push(Line::parse(&l).map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?);
    }
    Ok(out)
}

/// Parses journal lines.
pub fn parse_lines<S: AsRef<str>>(lines: &[S]) -> Result<Vec<Line>, String> {
    lines.iter().enumerate().map(|(i, l)| Line::parse(l.as_ref()).map_err(|e| format!("line {}: {e}", i + 1))).collect()
}

/// Answers decisions from the journal being replayed (design §2.4): the next line must be an
/// answer of the same kind with the same stamp, else the replay has diverged.
pub struct JournalAnswerer {
    pub cursor: Arc<Mutex<Cursor>>,
}

impl Answerer for JournalAnswerer {
    fn ask(&self, g: &Game, d: &Decision) -> Answer {
        match self.cursor.lock().unwrap().take_answer(d.kind.name(), Stamp::of(g)) {
            Some(i) => Answer::Option(i),
            None => Answer::Advisor,
        }
    }
}

/// One journal line for people (`ffh-play journal print`): commands with type names.
pub fn pretty(d: &GameData, line: &Line) -> String {
    match line {
        Line::New { setup } => format!("new game {}", crate::command::names_in(d, setup)),
        Line::Load { fnv, human } => format!("load save {fnv} as player {}", human.0),
        Line::TakeOver { fnv, human } => format!("take over player {} of save {fnv}", human.0),
        Line::Cmd { stamp, cmd } => format!("T{} s{}: {}", stamp.turn, stamp.slice, crate::command::describe(d, cmd)),
        Line::Answer { stamp, kind, option } => format!("T{} s{}: answer {kind}: option {option}", stamp.turn, stamp.slice),
        Line::Checkpoint { turn, checksum } => format!("-- checkpoint (turn {turn}), checksum {:08x}", *checksum as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_round_trip() {
        let lines = [
            Line::New { setup: crate::jobj! { "seed" => 3 } },
            Line::Load { fnv: fnv_hex(b"abc"), human: PlayerId(0) },
            Line::TakeOver { fnv: "00".into(), human: PlayerId(2) },
            Line::Cmd { stamp: Stamp { turn: 5, slice: 77 }, cmd: crate::jobj! { "c" => "end_turn", "force" => true } },
            Line::Answer { stamp: Stamp { turn: 6, slice: 80 }, kind: "keep_city".into(), option: 1 },
            Line::Checkpoint { turn: 6, checksum: -5 },
        ];
        for l in &lines {
            let s = l.to_json().to_string();
            assert_eq!(&Line::parse(&s).unwrap(), l, "{s}");
        }
        assert_eq!(fnv_hex(b""), "cbf29ce484222325");
        assert!(Line::parse(r#"{"k":"new","v":2,"setup":{}}"#).unwrap_err().contains("version 2"));
        assert!(Line::parse(r#"{"k":"zzz"}"#).is_err());
    }

    #[test]
    fn cursor_detects_divergence() {
        let mut c = Cursor::new(vec![Line::Checkpoint { turn: 1, checksum: 9 }, Line::Answer { stamp: Stamp { turn: 2, slice: 3 }, kind: "free_tech".into(), option: 4 }], 0);
        c.expect_checkpoint(1, 9);
        assert!(c.desync.is_none());
        assert_eq!(c.take_answer("free_tech", Stamp { turn: 2, slice: 4 }), None);
        assert!(c.desync.as_ref().unwrap().contains("journal line 2"));
    }
}
