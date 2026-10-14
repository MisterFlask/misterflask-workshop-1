//! Port of CvSpellInterface.py (part b). Each Python function becomes a Rust `fn`
//! registered by its Python name in the `find_*` lookups below (see dispatch.rs).

#![allow(unused_imports)]
use crate::ffh::dispatch::*;
use crate::ffh::pycall::PyCall;
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

/// PyResult functions (`spellX`).
pub fn find_spell(name: &str) -> Option<SpellFn> {
    match name {
        _ => None,
    }
}

/// PyRequirement functions (`reqX`).
pub fn find_req(name: &str) -> Option<ReqFn> {
    match name {
        _ => None,
    }
}

/// Unit-only effects: promotion PyPerTurn (`effectX`), spell PyMiscast (`miscastX`).
pub fn find_unit_fn(name: &str) -> Option<UnitFn> {
    match name {
        _ => None,
    }
}

/// Plot callbacks: improvement/feature PythonOnMove (`onMoveX`), improvement PythonAtRange (`atRangeX`).
pub fn find_plot_fn(name: &str) -> Option<PlotFn> {
    match name {
        _ => None,
    }
}

/// Post-combat callbacks (`postCombatX`).
pub fn find_combat_fn(name: &str) -> Option<CombatFn> {
    match name {
        _ => None,
    }
}

/// Vote PyResult functions (`voteX`).
pub fn find_vote_fn(name: &str) -> Option<VoteFn> {
    match name {
        _ => None,
    }
}
