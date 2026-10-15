//! Save bundles (design §2.6): `<name>.ffhsave` holds plain `Game::save()` bytes (loadable by any
//! tool); `<name>.ffhsess` is a small JSON sidecar with the session state the rules need that is
//! not in `Game`: the deferred event popups (C++ keeps them in the player's popup queue, which
//! its saves include). Autosaves are a ring of the last N turn boundaries in `<dir>/auto/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ffh_core::ids::{PlayerId, TriggeredId};
use ffh_core::log::LogEntry;

use crate::human::DeferredEvent;

use crate::json::{FieldError, Json};

/// Sidecar format version.
pub const SIDECAR_VERSION: i64 = 1;

/// The session part of a save.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sidecar {
    pub human: PlayerId,
    /// Deferred event popups, in order (`[player, triggered id, launched]`; a version 1 pair
    /// without `launched` reads as not yet launched).
    pub pending_events: Vec<DeferredEvent>,
    /// The journal of the session that saved (for bug reports).
    pub journal: Option<String>,
    /// Lines that journal had when saving.
    pub journal_line: usize,
    /// The human's latest messages.
    pub messages_tail: Vec<LogEntry>,
    /// The session's AutoMoves rule already fired at this control point (absent: false).
    pub auto_moves_tried: bool,
    /// The turn report's snapshot, taken at the human's end of turn and not yet diffed (a save
    /// at a turn boundary; absent: none). Presentation only.
    pub report_snapshot: Option<Json>,
    /// The last turn report (absent: none). Presentation only.
    pub report: Option<Json>,
}

pub fn log_entry_json(e: &LogEntry) -> Json {
    crate::jobj! { "t" => e.turn, "player" => e.player, "text" => e.text.as_str() }
}

fn log_entry_from(j: &Json) -> Result<LogEntry, FieldError> {
    Ok(LogEntry { turn: j.req("t")?, player: j.opt("player")?, text: j.req("text")? })
}

impl Sidecar {
    pub fn to_json(&self) -> Json {
        crate::jobj! {
            "v" => SIDECAR_VERSION,
            "human" => self.human,
            "pending_events" => Json::Arr(self.pending_events.iter().map(|e| crate::jarr![e.player, e.triggered, e.launched]).collect()),
            "journal" => self.journal.as_deref(),
            "journal_line" => self.journal_line,
            "messages_tail" => Json::Arr(self.messages_tail.iter().map(log_entry_json).collect()),
            "auto_moves_tried" => self.auto_moves_tried,
            "report_snapshot" => self.report_snapshot.clone(),
            "report" => self.report.clone(),
        }
    }

    pub fn from_json(j: &Json) -> Result<Sidecar, String> {
        let e = |e: FieldError| e.to_string();
        match j.req::<i64>("v").map_err(e)? {
            SIDECAR_VERSION => {}
            v => return Err(format!("sidecar version {v} is not supported (expected {SIDECAR_VERSION})")),
        }
        let mut pending_events = Vec::new();
        for (i, pair) in j.req_arr("pending_events").map_err(e)?.iter().enumerate() {
            let p = pair.at(0).and_then(|v| v.as_i64()).and_then(|v| u8::try_from(v).ok());
            let id = pair.at(1).and_then(|v| v.as_i64()).and_then(|v| u32::try_from(v).ok());
            let launched = match pair.as_arr().map(<[Json]>::len) {
                Some(2) => Some(false),
                Some(3) => pair.at(2).and_then(Json::as_bool),
                _ => None,
            };
            match (p, id, launched) {
                (Some(p), Some(id), Some(launched)) => pending_events.push(DeferredEvent { player: PlayerId(p), triggered: TriggeredId(id), launched }),
                _ => return Err(format!("field `pending_events`: element {i}: expected [player, triggered id, launched]")),
            }
        }
        let messages_tail = j.req_arr("messages_tail").map_err(e)?.iter().map(log_entry_from).collect::<Result<_, _>>().map_err(e)?;
        Ok(Sidecar {
            human: j.req("human").map_err(e)?,
            pending_events,
            journal: j.opt("journal").map_err(e)?,
            journal_line: j.req("journal_line").map_err(e)?,
            messages_tail,
            auto_moves_tried: j.get_or("auto_moves_tried", false).map_err(e)?,
            report_snapshot: j.opt_value("report_snapshot").map_err(e)?.filter(|v| !v.is_null()).cloned(),
            report: j.opt_value("report").map_err(e)?.filter(|v| !v.is_null()).cloned(),
        })
    }
}

/// Where a save bundle was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavePaths {
    pub save: PathBuf,
    pub sidecar: PathBuf,
}

/// A save name as given by a client: 1-64 characters of `[A-Za-z0-9_.-]`, not starting with a
/// dot (no path separators, so no way out of the saves directory).
pub fn sanitize_name(name: &str) -> Result<&str, String> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if ok {
        Ok(name)
    } else {
        Err(format!("invalid save name {name:?}: use 1-64 of A-Z a-z 0-9 _ . - (not starting with '.')"))
    }
}

/// Writes `path` atomically (temporary file + rename).
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", path.extension().and_then(|e| e.to_str()).unwrap_or("")));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

/// Writes `<dir>/<name>.ffhsave` and `<dir>/<name>.ffhsess`.
pub fn write_bundle(dir: &Path, name: &str, save: &[u8], sidecar: &Sidecar) -> io::Result<SavePaths> {
    let name = sanitize_name(name).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    fs::create_dir_all(dir)?;
    let paths = SavePaths { save: dir.join(format!("{name}.ffhsave")), sidecar: dir.join(format!("{name}.ffhsess")) };
    write_atomic(&paths.save, save)?;
    write_atomic(&paths.sidecar, sidecar.to_json().to_pretty().as_bytes())?;
    Ok(paths)
}

/// Reads a save and its sidecar (if there is one next to it).
pub fn read_bundle(save: &Path) -> Result<(Vec<u8>, Option<Sidecar>), String> {
    let bytes = fs::read(save).map_err(|e| format!("{}: {e}", save.display()))?;
    let sidecar_path = save.with_extension("ffhsess");
    let sidecar = match fs::read(&sidecar_path) {
        Ok(b) => {
            let j = Json::parse_bytes(&b).map_err(|e| format!("{}: {e}", sidecar_path.display()))?;
            Some(Sidecar::from_json(&j).map_err(|e| format!("{}: {e}", sidecar_path.display()))?)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", sidecar_path.display())),
    };
    Ok((bytes, sidecar))
}

/// The autosave of `turn`: `<dir>/auto/T{turn:04}`; then keeps only the newest `keep` autosaves.
pub fn autosave(dir: &Path, turn: i32, save: &[u8], sidecar: &Sidecar, keep: usize) -> io::Result<SavePaths> {
    let auto = dir.join("auto");
    let paths = write_bundle(&auto, &format!("T{turn:04}"), save, sidecar)?;
    let mut saves: Vec<PathBuf> = fs::read_dir(&auto)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "ffhsave") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('T')))
        .collect();
    // T0001 < T0002 < ... < T9999 < T10000 by (length, name).
    saves.sort_by_key(|p| {
        let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        (n.len(), n)
    });
    while saves.len() > keep.max(1) {
        let old = saves.remove(0);
        let _ = fs::remove_file(old.with_extension("ffhsess"));
        let _ = fs::remove_file(&old);
    }
    Ok(paths)
}

/// The `saves` query: the manual saves of the game's directory and its autosaves, by name
/// (`{dir, saves: [{name, path, auto, sidecar}]}`; no directory: an empty list).
pub fn list_json(dir: Option<&Path>) -> Json {
    let Some(dir) = dir else { return crate::jobj! { "dir" => Json::Null, "saves" => Json::arr() } };
    let mut saves = Vec::new();
    for (sub, auto) in [(dir.to_path_buf(), false), (dir.join("auto"), true)] {
        let Ok(entries) = fs::read_dir(&sub) else { continue };
        let mut found: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "ffhsave")).collect();
        found.sort();
        for p in found {
            let name = p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            saves.push(crate::jobj! {
                "name" => if auto { format!("auto/{name}") } else { name },
                "path" => p.display().to_string(),
                "auto" => auto,
                "sidecar" => p.with_extension("ffhsess").exists(),
            });
        }
    }
    crate::jobj! { "dir" => dir.display().to_string(), "saves" => Json::Arr(saves) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitized() {
        assert!(sanitize_name("before-war_2.v1").is_ok());
        for bad in ["", ".hidden", "../x", "a/b", "a\\b", "C:x", "x y", &"a".repeat(65)] {
            assert!(sanitize_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn sidecar_round_trip_and_autosave_ring() {
        let s = Sidecar {
            human: PlayerId(0),
            pending_events: vec![DeferredEvent { player: PlayerId(0), triggered: TriggeredId(7), launched: true }],
            journal: Some("saves/journal.jsonl".into()),
            journal_line: 12,
            messages_tail: vec![LogEntry { turn: 3, player: Some(0), text: "hi \"there\"".into() }],
            auto_moves_tried: true,
            report_snapshot: Some(crate::jobj! { "turn" => 3 }),
            report: None,
        };
        assert_eq!(Sidecar::from_json(&Json::parse(&s.to_json().to_string()).unwrap()).unwrap(), s);
        // Optional members of version 1 read as their defaults.
        let old = Json::parse(r#"{"v":1,"human":0,"pending_events":[[0,4]],"journal":null,"journal_line":3,"messages_tail":[]}"#).unwrap();
        let old = Sidecar::from_json(&old).unwrap();
        assert!(!old.auto_moves_tried);
        assert!(old.report_snapshot.is_none() && old.report.is_none());
        assert_eq!(old.pending_events, [DeferredEvent { player: PlayerId(0), triggered: TriggeredId(4), launched: false }]);

        let dir = std::env::temp_dir().join(format!("ffh-play-save-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for t in [1, 2, 3, 10] {
            autosave(&dir, t, b"FFH2SAVE", &s, 2).unwrap();
        }
        let mut left: Vec<String> = fs::read_dir(dir.join("auto")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["T0003.ffhsave", "T0003.ffhsess", "T0010.ffhsave", "T0010.ffhsess"]);
        let (bytes, side) = read_bundle(&dir.join("auto/T0010.ffhsave")).unwrap();
        assert_eq!(bytes, b"FFH2SAVE");
        assert_eq!(side, Some(s));
        assert!(write_bundle(&dir, "../escape", b"x", &Sidecar::default()).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
