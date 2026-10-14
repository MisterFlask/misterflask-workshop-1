//! ffh-play: human play for ffh2-rs (design: `docs/PLAY_DESIGN.md`; how to play: `docs/HUMAN.md`).
//!
//! A human player acts only through public rules APIs, via ports of what the C++ exe does
//! for human input (`CvNet*::Execute` and the helpers that send those messages). One game
//! thread owns the `Game`; front-ends (a browser over HTTP, a JSONL agent over stdio) talk to
//! it with one JSON envelope.
//!
//! Status: phase P0 (skeleton).
//! * [`json`]: the JSON value, writer and strict parser used by the protocol and the journal.
//! * [`names`]: type names <-> indices for every info table and hard-coded C++ enum, so the
//!   protocol accepts `"UNIT_WARRIOR"` or `44` wherever a type is expected.

pub mod json;
pub mod names;

pub use json::{FieldError, FromJson, Json, ParseError};
pub use names::{Kind, Named, KINDS};
