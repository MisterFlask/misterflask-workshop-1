//! Queries (design §3, §4.6): what a client reads. Every query is pure: it runs on the game under
//! a [`QueryGuard`](crate::guard::QueryGuard), keeps to the allow-list of §2.5 (no game path
//! finder, no AI valuations), and sees the game from the human's side only (the fog rules of
//! [`crate::view::fog`]), unless debug reveal is on.
//!
//! A query is decoded from the protocol ([`Query::from_json`]) and answered by [`run`] with a
//! [`Ctx`]: the session's state a query needs besides the game (the human, the open event
//! popups, the message store, the turn report, the interface path finder in [`UiState`]).
//! `Session::query` builds it at a control point; `DecisionReader` (human.rs) builds a smaller
//! one inside a blocking decision.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::CivicType;
use ffh_core::path::{FinderKind, PathFinder};

use crate::decision::Decision;
use crate::guard::QueryGuard;
use crate::json::{FieldError, Json};
use crate::report::{MessageStore, Report};
use crate::view::map::MapTracker;
use crate::view::{self, Viewer};

/// A query (design §3). Type-valued fields are typed; plots are `x, y`.
#[derive(Clone, Debug, PartialEq)]
pub enum Query {
    Status,
    Pending,
    View,
    /// The map arrays; with `since` (a revision the client holds) and its `epoch`, only the
    /// tiles changed since ([`crate::view::map::MapTracker`]), else the full map.
    Map { since: Option<i64>, epoch: Option<String> },
    Plot { x: i32, y: i32 },
    /// Unit details and the actions the selection can take (C++ `canHandleAction`).
    Group { units: Vec<UnitId> },
    /// A path preview through the interface finder. `flags`: the `MOVE_*` flags (the exe's
    /// previews use 0).
    Path { units: Vec<UnitId>, x: i32, y: i32, flags: i32 },
    /// An attack preview: attacker, defender, odds and the strength breakdown.
    Combat { units: Vec<UnitId>, x: i32, y: i32 },
    City { city: CityId },
    Techs,
    /// The civics screen; `with`: civics to try (merged over the current ones), whose anarchy,
    /// upkeep and validity the reply adds as `proposed`.
    Civics { with: Vec<CivicType> },
    Religions,
    Players,
    /// What can be traded with `with`, both ways (C++ `buildTradeTable`), with denials.
    Trade { with: PlayerId },
    /// Messages from index `since` (`view.log_next` is the next index).
    Log { since: usize },
    /// The turn report of the last turn.
    Report,
    /// `{:#?}` of a game object (`unit`, `group`, `city`, `plot`, `player`, `team`, `deal`) or
    /// an info entry (a kind name such as `UnitType`, or a table name such as `units`).
    /// `oid` is the object's id (or the info entry's index); `id` is the envelope's request id.
    Inspect { kind: String, oid: i64 },
    /// A compact text summary (LLM agents).
    Text,
    /// An ASCII map of `w` x `h` plots from `(x, y)`.
    Ascii { x: i32, y: i32, w: i32, h: i32, layer: Layer },
    /// The static data tables (§4.7).
    Data,
    /// The new-game lists.
    Setup,
    /// The saves of the game's directory.
    Saves,
}

/// The layers of the ASCII map.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Layer {
    Terrain,
    Political,
    Yield,
    Units,
}

impl Layer {
    pub const ALL: [Layer; 4] = [Layer::Terrain, Layer::Political, Layer::Yield, Layer::Units];

    pub fn name(self) -> &'static str {
        match self {
            Layer::Terrain => "terrain",
            Layer::Political => "political",
            Layer::Yield => "yield",
            Layer::Units => "units",
        }
    }

    pub fn parse(s: &str) -> Option<Layer> {
        Layer::ALL.into_iter().find(|l| l.name() == s)
    }
}

/// Every query name (`q`).
pub const QUERY_NAMES: &[&str] = &[
    "status", "pending", "view", "map", "plot", "group", "path", "combat", "city", "techs", "civics", "religions", "players", "trade", "log",
    "report", "inspect", "text", "ascii", "data", "setup", "saves",
];

/// Why a query was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryErr {
    /// Malformed: unknown query, missing field, out of range.
    Invalid(String),
    /// Someone else's unit or city (or a city screen of a foreign city).
    NotOwner,
    /// An id that does not exist (any more).
    Gone,
    /// Not now: inside a decision (messages, report, saves), or a rules call holds the game's
    /// path finders.
    Busy(String),
}

impl QueryErr {
    /// The protocol error code.
    pub fn code(&self) -> &'static str {
        match self {
            QueryErr::Invalid(_) => "invalid",
            QueryErr::NotOwner => "not_owner",
            QueryErr::Gone => "gone",
            QueryErr::Busy(_) => "busy",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            QueryErr::Invalid(s) | QueryErr::Busy(s) => s.clone(),
            QueryErr::NotOwner => "not yours".into(),
            QueryErr::Gone => "no such object".into(),
        }
    }
}

impl std::fmt::Display for QueryErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.detail())
    }
}

impl From<FieldError> for QueryErr {
    fn from(e: FieldError) -> QueryErr {
        QueryErr::Invalid(e.to_string())
    }
}

pub(crate) fn invalid(s: impl Into<String>) -> QueryErr {
    QueryErr::Invalid(s.into())
}

impl Query {
    /// The protocol name (`q`).
    pub fn name(&self) -> &'static str {
        match self {
            Query::Status => "status",
            Query::Pending => "pending",
            Query::View => "view",
            Query::Map { .. } => "map",
            Query::Plot { .. } => "plot",
            Query::Group { .. } => "group",
            Query::Path { .. } => "path",
            Query::Combat { .. } => "combat",
            Query::City { .. } => "city",
            Query::Techs => "techs",
            Query::Civics { .. } => "civics",
            Query::Religions => "religions",
            Query::Players => "players",
            Query::Trade { .. } => "trade",
            Query::Log { .. } => "log",
            Query::Report => "report",
            Query::Inspect { .. } => "inspect",
            Query::Text => "text",
            Query::Ascii { .. } => "ascii",
            Query::Data => "data",
            Query::Setup => "setup",
            Query::Saves => "saves",
        }
    }

    /// Decodes a request object (`{"q": "path", ...}`; other members are ignored).
    pub fn from_json(j: &Json) -> Result<Query, QueryErr> {
        let q = j.req_str("q")?;
        Ok(match q {
            "status" => Query::Status,
            "pending" => Query::Pending,
            "view" => Query::View,
            "map" => Query::Map { since: j.opt("since")?, epoch: j.opt("epoch")? },
            "plot" => Query::Plot { x: j.req("x")?, y: j.req("y")? },
            "group" => Query::Group { units: j.req("units")? },
            "path" => Query::Path { units: j.req("units")?, x: j.req("x")?, y: j.req("y")?, flags: j.get_or("flags", 0)? },
            "combat" => Query::Combat { units: j.req("units")?, x: j.req("x")?, y: j.req("y")? },
            "city" => Query::City { city: j.req("city")? },
            "techs" => Query::Techs,
            "civics" => Query::Civics {
                with: if j.opt_value("with")?.is_some_and(|w| !w.is_null()) { crate::names::list_field(ffh_data::global(), j, "with")? } else { Vec::new() },
            },
            "religions" => Query::Religions,
            "players" => Query::Players,
            "trade" => Query::Trade { with: j.req("with")? },
            "log" => Query::Log { since: j.get_or("since", 0)? },
            "report" => Query::Report,
            "inspect" => Query::Inspect { kind: j.req("kind")?, oid: j.req("oid")? },
            "text" => Query::Text,
            "ascii" => {
                let layer = match j.opt_str("layer")? {
                    None => Layer::Terrain,
                    Some(s) => Layer::parse(s).ok_or_else(|| invalid(format!("field `layer`: one of terrain, political, yield, units, got {s:?}")))?,
                };
                Query::Ascii { x: j.get_or("x", 0)?, y: j.get_or("y", 0)?, w: j.get_or("w", 0)?, h: j.get_or("h", 0)?, layer }
            }
            "data" => Query::Data,
            "setup" => Query::Setup,
            "saves" => Query::Saves,
            other => return Err(invalid(format!("unknown query {other:?}"))),
        })
    }

    /// The canonical request form (`from_json` of it gives the same query).
    pub fn to_json(&self) -> Json {
        let ids = |v: &[UnitId]| Json::Arr(v.iter().map(|&u| Json::from(u)).collect());
        let j = crate::jobj! { "q" => self.name() };
        match self {
            Query::Map { since, epoch } => j.with("since", *since).with("epoch", epoch.as_deref()),
            Query::Civics { with } => j.with("with", Json::Arr(with.iter().map(|&c| Json::idx(c)).collect())),
            Query::Plot { x, y } => j.with("x", *x).with("y", *y),
            Query::Group { units } => j.with("units", ids(units)),
            Query::Path { units, x, y, flags } => j.with("units", ids(units)).with("x", *x).with("y", *y).with("flags", *flags),
            Query::Combat { units, x, y } => j.with("units", ids(units)).with("x", *x).with("y", *y),
            Query::City { city } => j.with("city", *city),
            Query::Trade { with } => j.with("with", *with),
            Query::Log { since } => j.with("since", *since),
            Query::Inspect { kind, oid } => j.with("kind", kind.as_str()).with("oid", *oid),
            Query::Ascii { x, y, w, h, layer } => j.with("x", *x).with("y", *y).with("w", *w).with("h", *h).with("layer", layer.name()),
            _ => j,
        }
    }
}

/// Session state of the client side that queries use and transports share (design §3).
pub struct UiState {
    /// Debug reveal: queries answer with the true state (replies carry `"reveal": true`).
    pub reveal: bool,
    /// The interface path finder (C++ `GC.getInterfacePathFinder()`): path previews never touch
    /// the game's saved finders (F12).
    pub finder: PathFinder,
    /// Bumped whenever the game changed at a control point (a command, `advance`).
    pub view_version: u64,
    /// The map as the `map` query last answered it, for the tile deltas (`map {since}`).
    pub map: MapTracker,
}

impl Default for UiState {
    fn default() -> UiState {
        UiState { reveal: false, finder: PathFinder::new(FinderKind::Path), view_version: 0, map: MapTracker::default() }
    }
}

impl UiState {
    /// Locks the shared state, also after a panic while it was held (a query that failed inside
    /// a decision is answered `busy`, design §2.4): its fields are valid at any time (the finder
    /// is a cache), so a poisoned lock must not take the session down later.
    pub fn lock(ui: &Mutex<UiState>) -> MutexGuard<'_, UiState> {
        ui.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What a query needs besides the game.
pub struct Ctx<'a> {
    /// The human player (the view's side).
    pub h: PlayerId,
    /// The open event popups of the human (`SessionShared::open_events_of`).
    pub events: &'a [TriggeredId],
    /// The blocking decision being asked, if any (status phase `decision`).
    pub decision: Option<&'a Decision>,
    /// The human's messages (`None` inside a decision).
    pub messages: Option<&'a MessageStore>,
    /// The last turn report (`None` inside a decision or before the first).
    pub report: Option<&'a Report>,
    /// Inside a decision: messages, report and saves are not available.
    pub in_decision: bool,
    /// The game's saves directory.
    pub saves_dir: Option<&'a Path>,
    pub ui: &'a Mutex<UiState>,
}

/// Answers `q` from `g` for `ctx.h` (under a `QueryGuard`).
pub fn run(g: &Game, ctx: &Ctx, q: &Query) -> Result<Json, QueryErr> {
    let guard = QueryGuard::new(g);
    let reveal = UiState::lock(ctx.ui).reveal;
    let v = Viewer::new(&guard, ctx.h, reveal);
    let busy = |what: &str| QueryErr::Busy(format!("{what} is not available during a decision"));
    let finders = || if guard.finders_busy() { Err(QueryErr::Busy("a rules call holds the path finders".into())) } else { Ok(()) };
    let out = match q {
        Query::Status => view::status(&v, ctx),
        Query::Pending => crate::decision::pending(v.g, ctx.h, ctx.events).to_json(),
        Query::View => view::view(&v, ctx),
        Query::Map { since, epoch } => {
            // The tracker remembers what it answered, so that a later `since` gets the tiles
            // changed after it.
            let rows = view::map::rows(&v);
            let mut ui = UiState::lock(ctx.ui);
            ui.map.update(rows);
            ui.map.reply(*since, epoch.as_deref())
        }
        Query::Plot { x, y } => view::map::plot(&v, *x, *y)?,
        Query::Group { units } => {
            finders()?;
            view::units::group(&v, units)?
        }
        Query::Path { units, x, y, flags } => {
            finders()?;
            // The search runs without the lock: the finder is taken out and put back (a fresh
            // one stays if the search panics, which is harmless for a cache).
            let mut finder = std::mem::replace(&mut UiState::lock(ctx.ui).finder, PathFinder::new(FinderKind::Path));
            let r = view::units::path(&v, &mut finder, units, *x, *y, *flags);
            UiState::lock(ctx.ui).finder = finder;
            r?
        }
        Query::Combat { units, x, y } => {
            finders()?;
            view::units::combat(&v, units, *x, *y)?
        }
        Query::City { city } => view::city::city(&v, *city)?,
        Query::Techs => view::player::techs(&v),
        Query::Civics { with } => view::player::civics(&v, with),
        Query::Religions => view::player::religions(&v),
        Query::Players => view::player::players(&v),
        Query::Trade { with } => view::diplo::trade(&v, *with)?,
        Query::Log { since } => match ctx.messages {
            Some(m) => m.since_json(*since),
            None => return Err(busy("the message log")),
        },
        Query::Report => {
            if ctx.in_decision {
                return Err(busy("the turn report"));
            }
            ctx.report.map_or(Json::Null, Report::to_json)
        }
        Query::Inspect { kind, oid } => view::text::inspect(&v, kind, *oid)?,
        Query::Text => view::text::text(&v, ctx),
        Query::Ascii { x, y, w, h, layer } => view::text::ascii(&v, *x, *y, *w, *h, *layer)?,
        Query::Data => view::data::data(g.d).clone(),
        Query::Setup => crate::setup::setup_lists(g.d),
        Query::Saves => {
            if ctx.in_decision {
                return Err(busy("the saves list"));
            }
            crate::save::list_json(ctx.saves_dir)
        }
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ffh_data::Idx;

    #[test]
    fn json_round_trip() {
        let qs = vec![
            Query::Status,
            Query::Pending,
            Query::View,
            Query::Map { since: None, epoch: None },
            Query::Map { since: Some(7), epoch: Some("e-1".into()) },
            Query::Plot { x: 3, y: 4 },
            Query::Group { units: vec![UnitId(1), UnitId(2)] },
            Query::Path { units: vec![UnitId(1)], x: 5, y: 6, flags: 0 },
            Query::Combat { units: vec![UnitId(1)], x: 5, y: 6 },
            Query::City { city: CityId(3) },
            Query::Techs,
            Query::Civics { with: Vec::new() },
            Query::Civics { with: vec![CivicType::from_usize(2)] },
            Query::Religions,
            Query::Players,
            Query::Trade { with: PlayerId(2) },
            Query::Log { since: 10 },
            Query::Report,
            Query::Inspect { kind: "unit".into(), oid: 4 },
            Query::Text,
            Query::Ascii { x: 1, y: 2, w: 10, h: 5, layer: Layer::Units },
            Query::Data,
            Query::Setup,
            Query::Saves,
        ];
        let mut seen: Vec<&str> = Vec::new();
        for q in &qs {
            let j = q.to_json();
            assert_eq!(&Query::from_json(&j).unwrap(), q, "{j}");
            assert_eq!(Query::from_json(&Json::parse(&j.to_string()).unwrap()).unwrap(), *q);
            if !seen.contains(&q.name()) {
                seen.push(q.name());
            }
        }
        assert_eq!(seen, QUERY_NAMES);
    }

    #[test]
    fn decoding_errors_name_the_field() {
        let e = Query::from_json(&Json::parse(r#"{"q":"path","units":[1],"x":"a","y":1}"#).unwrap()).unwrap_err();
        assert!(matches!(&e, QueryErr::Invalid(s) if s.contains("field `x`")), "{e}");
        let e = Query::from_json(&Json::parse(r#"{"q":"ascii","layer":"sky"}"#).unwrap()).unwrap_err();
        assert!(matches!(&e, QueryErr::Invalid(s) if s.contains("layer")), "{e}");
        let e = Query::from_json(&Json::parse(r#"{"q":"fly"}"#).unwrap()).unwrap_err();
        assert_eq!(e.code(), "invalid");
        assert_eq!(Query::from_json(&Json::parse(r#"{"q":"ascii"}"#).unwrap()).unwrap(), Query::Ascii { x: 0, y: 0, w: 0, h: 0, layer: Layer::Terrain });
        // `inspect` names its object `oid`: `id` is the envelope's request id (P4 review).
        let j = Json::parse(r#"{"id":5,"k":"q","q":"inspect","kind":"unit","oid":0}"#).unwrap();
        assert_eq!(Query::from_json(&j).unwrap(), Query::Inspect { kind: "unit".into(), oid: 0 });
        let e = Query::from_json(&Json::parse(r#"{"id":0,"q":"inspect","kind":"unit"}"#).unwrap()).unwrap_err();
        assert!(matches!(&e, QueryErr::Invalid(s) if s.contains("field `oid`")), "{e}");
    }
}
