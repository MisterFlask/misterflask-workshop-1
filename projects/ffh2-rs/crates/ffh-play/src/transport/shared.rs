//! What the HTTP threads read without the game thread (design §2.1, §4.2): the status with its
//! version and the long-poll, and the cached view and map of the last control point (or
//! decision). The game thread publishes; GET requests never wait for it.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::json::Json;

/// The longest long-poll (`/api/wait?timeout_ms=`).
pub const MAX_WAIT: Duration = Duration::from_secs(60);

struct State {
    /// Bumped by every publish.
    version: u64,
    /// The status without the members added here (version, ms_in_phase, map_rev, game, crash).
    base: Json,
    phase: String,
    phase_since: Instant,
    /// The serialized view and map (served as they are).
    view: Option<Arc<String>>,
    map: Option<Arc<String>>,
    map_rev: Option<String>,
    /// The game's directory, journal and last autosave (`Host::game_info`).
    game: Json,
    /// The crash report while the status is `crashed`.
    crash: Option<Json>,
    quit: bool,
}

/// The status and caches the game thread publishes for the HTTP threads.
pub struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

impl Default for Shared {
    fn default() -> Shared {
        Shared::new()
    }
}

impl Shared {
    pub fn new() -> Shared {
        Shared {
            state: Mutex::new(State {
                version: 0,
                base: crate::jobj! { "phase" => "no_game" },
                phase: "no_game".into(),
                phase_since: Instant::now(),
                view: None,
                map: None,
                map_rev: None,
                game: Json::Null,
                crash: None,
                quit: false,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Every field is valid at any time: a panic elsewhere must not stop the server.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Publishes a new status (`base`: the game's status, `phase` first) and wakes the
    /// long-polls.
    pub fn publish(&self, base: Json) {
        let mut st = self.lock();
        let phase = base.get("phase").and_then(Json::as_str).unwrap_or("running").to_string();
        if phase != st.phase {
            st.phase = phase;
            st.phase_since = Instant::now();
        }
        st.base = base;
        st.version += 1;
        drop(st);
        self.changed.notify_all();
    }

    /// The game's directory, journal and autosave (shown in the status).
    pub fn set_game(&self, game: Json) {
        self.lock().game = game;
    }

    /// The crash report (the status's `crash` while the phase is `crashed`).
    pub fn set_crash(&self, crash: Option<Json>) {
        self.lock().crash = crash;
    }

    /// Replaces the cached view and map; `None` keeps the previous one (a view that failed to
    /// build, design §2.4).
    pub fn set_caches(&self, view: Option<Json>, map: Option<Json>) {
        let view = view.map(|v| Arc::new(v.to_string()));
        let map = map.map(|m| (m.get("map_rev").and_then(Json::as_str).map(str::to_string), Arc::new(m.to_string())));
        let mut st = self.lock();
        if let Some(v) = view {
            st.view = Some(v);
        }
        if let Some((rev, m)) = map {
            st.map_rev = rev;
            st.map = Some(m);
        }
    }

    /// No game: no view, no map.
    pub fn clear_caches(&self) {
        let mut st = self.lock();
        st.view = None;
        st.map = None;
        st.map_rev = None;
        st.game = Json::Null;
    }

    pub fn view(&self) -> Option<Arc<String>> {
        self.lock().view.clone()
    }

    pub fn map(&self) -> Option<Arc<String>> {
        self.lock().map.clone()
    }

    fn status_of(st: &State) -> Json {
        let mut j = crate::jobj! { "version" => st.version as i64 };
        for (k, v) in st.base.as_obj().unwrap_or_default() {
            j.set(k, v.clone());
        }
        j.set("ms_in_phase", st.phase_since.elapsed().as_millis().min(i64::MAX as u128) as i64);
        j.set("map_rev", st.map_rev.as_deref());
        if !st.game.is_null() {
            j.set("game", st.game.clone());
        }
        if let Some(c) = &st.crash {
            j.set("crash", c.clone());
        }
        j
    }

    /// The status (design §4.3).
    pub fn status(&self) -> Json {
        Shared::status_of(&self.lock())
    }

    pub fn version(&self) -> u64 {
        self.lock().version
    }

    /// The long-poll: the status once its version is above `since`, or after `timeout` (at most
    /// [`MAX_WAIT`]), or when the server quits.
    pub fn wait(&self, since: u64, timeout: Duration) -> Json {
        let deadline = Instant::now() + timeout.min(MAX_WAIT);
        let mut st = self.lock();
        while st.version <= since && !st.quit {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            st = self.changed.wait_timeout(st, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
        }
        Shared::status_of(&st)
    }

    /// The server is shutting down: long-polls return.
    pub fn quit(&self) {
        self.lock().quit = true;
        self.changed.notify_all();
    }

    pub fn quitting(&self) -> bool {
        self.lock().quit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_poll_wakes_on_publish_and_times_out() {
        let s = Arc::new(Shared::new());
        let st = s.status();
        assert_eq!((st.req::<i64>("version"), st.req_str("phase")), (Ok(0), Ok("no_game")));
        // Times out with the same version.
        let t0 = Instant::now();
        assert_eq!(s.wait(0, Duration::from_millis(30)).req::<i64>("version"), Ok(0));
        assert!(t0.elapsed() >= Duration::from_millis(30));
        // Wakes when published.
        let s2 = s.clone();
        let h = std::thread::spawn(move || s2.wait(0, Duration::from_secs(20)));
        std::thread::sleep(Duration::from_millis(20));
        s.publish(crate::jobj! { "phase" => "human", "turn" => 3 });
        let got = h.join().unwrap();
        assert_eq!((got.req::<i64>("version"), got.req_str("phase"), got.req::<i64>("turn")), (Ok(1), Ok("human"), Ok(3)));
        // An older version answers at once.
        assert_eq!(s.wait(0, Duration::from_secs(20)).req::<i64>("version"), Ok(1));
        // Caches: a failed view (None) keeps the previous one.
        s.set_caches(Some(crate::jobj! { "turn" => 3 }), Some(crate::jobj! { "map_rev" => "ab12" }));
        s.set_caches(None, None);
        assert_eq!(s.view().as_deref().map(String::as_str), Some(r#"{"turn":3}"#));
        assert_eq!(s.status().req_str("map_rev"), Ok("ab12"));
        s.set_crash(Some(crate::jobj! { "message" => "boom" }));
        assert_eq!(s.status().get("crash").and_then(|c| c.get("message")).and_then(Json::as_str), Some("boom"));
        // Quitting releases the waiters.
        let s3 = s.clone();
        let h = std::thread::spawn(move || s3.wait(5, Duration::from_secs(20)));
        std::thread::sleep(Duration::from_millis(20));
        s.quit();
        let t0 = Instant::now();
        h.join().unwrap();
        assert!(t0.elapsed() < Duration::from_secs(5));
    }
}
