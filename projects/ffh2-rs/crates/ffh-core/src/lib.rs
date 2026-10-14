//! FFH2 rules engine: a deterministic, rules-faithful port of the FFH2 0.41 DLL
//! (CvGame, CvPlayer, CvTeam, CvCity, CvUnit, CvPlot, CvSelectionGroup, CvMap, ...)
//! and the rules parts of its Python (spells, events, custom functions).
//!
//! See `projects/ffh2-rs/CLAUDE.md` for porting conventions.

pub mod area;
pub mod city;
pub mod controller;
pub mod deal;
pub mod defs;
pub mod ffh;
pub mod found_value;
pub mod game;
pub mod governor;
pub mod group;
pub mod ids;
pub mod log;
pub mod map;
pub mod mapgen;
pub mod mapscript;
pub mod path;
pub mod player;
pub mod plot;
pub mod plot_group;
pub mod rng;
pub mod save;
pub mod setup;
pub mod slab;
pub mod structs;
pub mod team;
pub mod unit;
pub mod utils;

pub use controller::{Controller, ControllerRef, NullController};
pub use game::Game;
pub use ids::*;
pub use setup::{GameSetup, PlayerSetup, SlotStatus};
pub use ffh_data::{self as data, GameData, Idx, IdxVec};
