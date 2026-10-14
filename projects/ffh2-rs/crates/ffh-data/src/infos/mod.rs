//! Info class structs, one module per XML area. See lib.rs `game_data!` for the table list.

pub mod basic;
pub mod terrain;
pub mod civics;
pub mod game;
pub mod building;
pub mod civ;
pub mod unit;
pub mod event;

pub use basic::*;
pub use terrain::*;
pub use civics::*;
pub use game::*;
pub use building::*;
pub use civ::*;
pub use unit::*;
pub use event::*;

use crate::registry::Rd;

/// C++ `CvInfoBase`: common fields of every info class.
#[derive(Debug, Clone, Default)]
pub struct InfoBase {
    pub graphical_only: bool,
    pub type_: String,
    /// Text key of the name (C++ `m_szTextKey`, read from `<Description>`).
    pub description: String,
    pub civilopedia: String,
    pub help: String,
    pub strategy: String,
    pub button: String,
}

impl InfoBase {
    /// C++ `CvInfoBase::read`.
    pub fn read(rd: &Rd) -> InfoBase {
        InfoBase {
            graphical_only: rd.bool("bGraphicalOnly"),
            type_: rd.string("Type"),
            description: rd.string("Description"),
            civilopedia: rd.string("Civilopedia"),
            help: rd.string("Help"),
            strategy: rd.string("Strategy"),
            button: rd.string("Button"),
        }
    }
}
