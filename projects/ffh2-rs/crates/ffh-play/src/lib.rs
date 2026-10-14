//! ffh-play: human play for ffh2-rs (design: `docs/PLAY_DESIGN.md`).
//!
//! A human player acts only through public rules APIs, via ports of what the C++ exe does
//! for human input (`CvNet*::Execute` and the helpers that send those messages). One game
//! thread owns the `Game`; front-ends (a browser over HTTP, a JSONL agent over stdio) talk to
//! it with one JSON envelope.
//!
//! Status: phase P0 (skeleton).
//! * [`json`]: the JSON value, writer and strict parser used by the protocol and the journal.

pub mod json;

pub use json::{FieldError, FromJson, Json, ParseError};
