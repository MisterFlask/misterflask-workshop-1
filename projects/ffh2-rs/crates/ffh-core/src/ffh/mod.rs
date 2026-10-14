//! Ports of the FFH2 rules Python: CvSpellInterface.py (spells), CvEventManager.py
//! (event handlers), CustomFunctions.py, CvGameUtils.py (rules callbacks),
//! CvRandomEventInterface.py (random event callbacks).
//!
//! The hook signatures in events.rs / spells.rs / game_utils.rs / random_events.rs are
//! the stable API that ported DLL code calls.

pub mod custom;
pub mod dispatch;
pub mod events;
pub mod game_utils;
pub mod pycall;
pub mod random_events;
pub mod random_events_a;
pub mod random_events_b;
pub mod spells;
pub mod spells_a;
pub mod spells_b;
