//! Game setup (port of the gameplay parts of `CvInitCore`).

use crate::ids::*;
use ffh_data::*;

/// C++ `SlotStatus` (only the states that matter without networking).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SlotStatus {
    Open,
    Computer,
    Closed,
    Taken,
}

/// Per-player slot setup (C++ `m_aeCiv[i]`, `m_aeLeader[i]`, ...).
#[derive(Clone, Debug)]
pub struct PlayerSetup {
    pub civ: Option<CivilizationType>,
    pub leader: Option<LeaderHeadType>,
    pub team: TeamId,
    pub handicap: Option<HandicapType>,
    pub art_style: Option<ArtStyleType>,
    pub slot_status: SlotStatus,
    /// C++ `isHuman()` source: a human-controlled slot.
    pub human: bool,
    pub leader_name: String,
    pub civ_description: String,
}

/// C++ `CvInitCore` (gameplay subset).
#[derive(Clone, Debug)]
pub struct GameSetup {
    pub world_size: WorldSizeType,
    pub climate: ClimateType,
    pub sea_level: SeaLevelType,
    pub era: EraType,
    pub game_speed: GameSpeedType,
    pub calendar: Option<CalendarType>,
    pub options: IdxVec<GameOptionType, bool>,
    pub victories: IdxVec<VictoryType, bool>,
    pub game_turn: i32,
    pub max_turns: i32,
    pub target_score: i32,
    pub max_city_elimination: i32,
    pub num_advanced_start_points: i32,
    pub sync_rand_seed: u32,
    pub map_rand_seed: u32,
    /// Map script name; this port ships its own generators (see `mapgen`).
    pub map_script: String,
    /// One entry per player slot, barbarian slot last.
    pub players: Vec<PlayerSetup>,
}

impl GameSetup {
    /// Open player slots reserved for players FFH2 adds mid-game (Infernals, Mercurians,
    /// `formEmpire`, ...; C++ uses any of its 35 civ slots via `addPlayerAdvanced`).
    pub const SPARE_SLOTS: usize = 4;

    /// A standard game: `n` AI civs (random civs/leaders are chosen by `game_init`, port of the
    /// FfH RANDOM_CIVILIZATION block in `CvGame::init`),
    /// [`Self::SPARE_SLOTS`] open slots, then the barbarian slot (barbarian civilization and
    /// leader, AI-controlled); given world size,
    /// normal speed/climate/sea level, first era, Noble difficulty.
    pub fn standard(d: &GameData, n_civs: usize, world: &str, seed: u32) -> GameSetup {
        let speed = d.t::<GameSpeedType>("GAMESPEED_NORMAL");
        let mut players = Vec::new();
        let n_slots = n_civs + Self::SPARE_SLOTS;
        for i in 0..=n_slots {
            let barbarian = i == n_slots;
            players.push(PlayerSetup {
                // The exe gives the barbarian slot BARBARIAN_CIVILIZATION / BARBARIAN_LEADER
                // and an AI slot status (CvPlayer::init only brings computer/taken slots alive).
                civ: if barbarian { opt_from_i32(d.defines.barbarian_civilization) } else { None },
                leader: if barbarian { opt_from_i32(d.defines.barbarian_leader) } else { None },
                team: TeamId(i as u8),
                handicap: d.ty::<HandicapType>("HANDICAP_NOBLE"),
                art_style: None,
                slot_status: if i < n_civs || barbarian {
                    SlotStatus::Computer
                } else {
                    SlotStatus::Open
                },
                human: false,
                leader_name: String::new(),
                civ_description: String::new(),
            });
        }
        GameSetup {
            world_size: d.t::<WorldSizeType>(world),
            climate: d.t::<ClimateType>("CLIMATE_TEMPERATE"),
            sea_level: d.t::<SeaLevelType>("SEALEVEL_MEDIUM"),
            era: EraType(0),
            game_speed: speed,
            calendar: None,
            options: IdxVec::from_elem(false, d.game_options.len()),
            victories: IdxVec::from_elem(true, d.victories.len()),
            game_turn: 0,
            max_turns: 0,
            target_score: 0,
            max_city_elimination: 0,
            num_advanced_start_points: 0,
            sync_rand_seed: seed,
            map_rand_seed: seed.wrapping_mul(2654435761).wrapping_add(1),
            map_script: "Fractal".into(),
            players,
        }
    }

    pub fn option(&self, o: GameOptionType) -> bool {
        self.options.get(o).copied().unwrap_or(false)
    }
}
