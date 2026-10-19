//! Malakim: nothing kept. Tried (docs/AI_LOG.md): Religious Fervor held for a war (win 30% ->
//! 22.5%, share at the end -100), the Citadel of Light valued in frontier cities at war (never
//! buildable: the AI does not research Sorcery). Religious Fervor stays with the generic play,
//! cast the first turn the AI-only gate (5 cities with the state religion) opens.

use super::CivPlay;

pub struct Malakim;

impl CivPlay for Malakim {}
