//! ffh-play: human play for ffh2-rs (design: `docs/PLAY_DESIGN.md`; how to play: `docs/HUMAN.md`).
//!
//! A human player acts only through public rules APIs, via ports of what the C++ exe does
//! for human input (`CvNet*::Execute` and the helpers that send those messages). One game
//! thread owns the `Game`; front-ends (a browser over HTTP, a JSONL agent over stdio) talk to
//! it with one JSON envelope.
//!
//! Status: phases P0 (skeleton) and P1 (session core, in-process API).
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
//! * [`setup`], [`save`], [`journal`].

pub mod command;
pub mod decision;
pub mod guard;
pub mod human;
pub mod journal;
pub mod json;
pub mod names;
pub mod save;
pub mod selection;
pub mod session;
pub mod setup;

pub use command::{CmdErr, CmdOk, Command, Confirm};
pub use decision::{Decision, DecisionKind, Pending};
pub use guard::QueryGuard;
pub use human::{Answer, Answerer, HumanController, Policy, POLICIES};
pub use json::{FieldError, FromJson, Json, ParseError};
pub use names::{Kind, Named, KINDS};
pub use session::{Phase, PlayError, Session, SessionOptions, Stop};
pub use setup::NewGame;
