//! Entity identifiers.
//!
//! C++ uses `PlayerTypes`/`TeamTypes` enums and `IDInfo {owner, id}` handles into
//! per-player free lists. Here every unit/city/group/... lives in a global
//! [`crate::slab::Slab`] keyed by a monotonically increasing id that is never
//! reused, so a stale id can never alias a newer object.

use ffh_data::Idx;
use std::fmt;

macro_rules! entity_id {
    ($(#[$m:meta])* $name:ident($t:ty)) => {
        $(#[$m])*
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(pub $t);
        impl Idx for $name {
            #[inline]
            fn from_usize(i: usize) -> Self {
                $name(i as $t)
            }
            #[inline]
            fn idx(self) -> usize {
                self.0 as usize
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

entity_id!(
    /// C++ `PlayerTypes`. The barbarian player is the last slot (`Game::barbarian_player()`).
    PlayerId(u8)
);
entity_id!(
    /// C++ `TeamTypes`. The barbarian team is the last slot (`Game::barbarian_team()`).
    TeamId(u8)
);
entity_id!(
    /// C++ `CvUnit` handle (`IDInfo`).
    UnitId(u32)
);
entity_id!(
    /// C++ `CvCity` handle (`IDInfo`).
    CityId(u32)
);
entity_id!(
    /// C++ `CvSelectionGroup` handle.
    GroupId(u32)
);
entity_id!(
    /// C++ `CvArea` id.
    AreaId(u32)
);
entity_id!(
    /// C++ `CvPlotGroup` handle.
    PlotGroupId(u32)
);
entity_id!(
    /// C++ `CvDeal` id.
    DealId(u32)
);
entity_id!(
    /// Index of a plot in `Map::plots` (`y * width + x`).
    PlotIdx(u32)
);
entity_id!(
    /// C++ `EventTriggeredData::m_iId`.
    TriggeredId(u32)
);
entity_id!(
    /// C++ `VoteSelectionData::iId` / `VoteTriggeredData::iId`.
    VoteId(u32)
);
