//! ffh-play: human play for ffh2-rs (design: `docs/PLAY_DESIGN.md`; how to play: `docs/HUMAN.md`).
//!
//! A human player acts only through public rules APIs, via ports of what the C++ exe does
//! for human input (`CvNet*::Execute` and the helpers that send those messages). One game
//! thread owns the `Game`; front-ends (a browser over HTTP, a JSONL agent over stdio) talk to
//! it with one JSON envelope.
//!
//! Status: phases P0 (skeleton), P1 (session core, in-process API), P2 (the soak), P3 (views and
//! queries), P4 (transports: the JSONL agent and the HTTP server) and P5 (the browser client).
//! * [`json`]: the JSON value, writer and strict parser used by the protocol and the journal.
//! * [`names`]: type names <-> indices for every info table and hard-coded C++ enum, so the
//!   protocol accepts `"UNIT_WARRIOR"` or its index wherever a type is expected.
//! * [`session`]: [`Session`] owns the game: `advance` runs it to the human's next control
//!   point, `execute` applies a [`Command`], saves, autosaves, the journal and its replay.
//! * [`human`]: the human slot's `Controller` (which rules decisions are deferred, blocking or
//!   the advisor's) and the [`Answerer`] that answers blocking [`Decision`]s.
//! * [`command`], [`selection`]: commands as ports of the C++ human input paths.
//! * [`decision`]: blocking decisions and the pending items of a control point.
//! * [`guard`]: [`QueryGuard`], which keeps reads of `&Game` free of side effects.
//! * [`view`]: what the human may see (P1: the fog rules of the game log).
//! * [`setup`], [`save`], [`journal`], [`bot`] (a scripted human for tests).
//! * [`soak`]: the AI plays the human slot through the session over a settings matrix
//!   (`ffh-play soak`), with ffh-stress's invariant checks.
//! * [`query`], [`report`]: the protocol's queries and the turn report.
//! * [`transport`]: the JSON envelope over stdio (`ffh-play agent`) and HTTP (`ffh-play serve`),
//!   which also serves the browser client (`assets/`, P5).
//! * [`stage`]: saves at staged situations for testing the clients (`ffh-play stage`).

pub mod bench;
pub mod bot;
pub mod command;
pub mod decision;
pub mod guard;
pub mod human;
pub mod journal;
pub mod json;
pub mod names;
pub mod query;
pub mod report;
pub mod save;
pub mod selection;
pub mod session;
pub mod setup;
pub mod soak;
pub mod stage;
pub mod transport;
pub mod view;

pub use command::{CmdErr, CmdOk, Command, Confirm};
pub use decision::{Decision, DecisionKind, Pending};
pub use guard::QueryGuard;
pub use human::{Answer, Answerer, DecisionReader, HumanController, Policy, POLICIES};
pub use json::{FieldError, FromJson, Json, ParseError};
pub use names::{Kind, Named, KINDS};
pub use query::{Query, QueryErr};
pub use session::{Phase, PlayError, Session, SessionOptions, Stop};
pub use setup::NewGame;
