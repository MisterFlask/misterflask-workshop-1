//! Game setup (port of the gameplay parts of `CvInitCore`).
//!
//! [`GameSetup::standard`] is the default game the CLI plays; [`SetupOptions`] +
//! [`GameSetup::from_options`] / [`Game::new_with_options`] cover the custom-game screen:
//! world size, number of civs (up to `MAX_CIV_PLAYERS`), team grouping, game speed, handicap,
//! start era, climate, sea level, game options, victory conditions and turn limit.

use crate::defs::MAX_CIV_PLAYERS;
use crate::game::Game;
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
    /// Score victory target in basis points of the world (score v2, docs/SCORE.md; 0 = none).
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

/// How the civ slots are grouped into teams (the custom-game team column).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamSetup {
    /// Every civ on its own team (slot `i` on team `i`).
    Solo,
    /// Consecutive civ slots share a team: slots `0..n` on team 0, `n..2n` on team `n`, ...
    /// (the team index is that of the group's first slot; the other team slots stay empty, as
    /// in a Civ4 custom game).
    TeamsOf(usize),
    /// Explicit team slot per civ slot (each entry `< MAX_CIV_PLAYERS`).
    Explicit(Vec<u8>),
}

/// Custom-game settings. [`SetupOptions::new`] gives the [`GameSetup::standard`] game.
#[derive(Clone, Debug)]
pub struct SetupOptions {
    /// Number of AI civs (1..=`MAX_CIV_PLAYERS`).
    pub civs: usize,
    pub world_size: WorldSizeType,
    pub game_speed: GameSpeedType,
    /// The game handicap (C++ `CvGame::init(eHandicap)`, the human's difficulty in a normal
    /// game): AI bonuses (`iAIUnitCostPercent`, `iAIFreeXP`, ...), barbarians and animals read it.
    pub handicap: HandicapType,
    /// Handicap of each AI slot (`CvInitCore::getHandicap(ePlayer)`); the exe gives AI slots
    /// `STANDARD_HANDICAP` (Noble) whatever the game difficulty.
    pub player_handicap: HandicapType,
    pub era: EraType,
    pub climate: ClimateType,
    pub sea_level: SeaLevelType,
    /// Game options turned on.
    pub options: Vec<GameOptionType>,
    /// Victory conditions turned off (all are on by default).
    pub disabled_victories: Vec<VictoryType>,
    /// `CvInitCore::m_iMaxTurns` (0 = from the game speed, as the exe does).
    pub max_turns: i32,
    /// `CvInitCore::m_iTargetScore` (0 = none). In basis points of the world (score v2,
    /// docs/SCORE.md): a team wins by score at this share or more (5000 = half the world).
    pub target_score: i32,
    /// `CvInitCore::m_iMaxCityElimination` (0 = none).
    pub max_city_elimination: i32,
    pub teams: TeamSetup,
    /// Open slots after the civs for players FFH2 adds mid-game; capped so that civs + spare
    /// slots fit in `MAX_CIV_PLAYERS`.
    pub spare_slots: usize,
    pub seed: u32,
}

impl SetupOptions {
    /// The standard game: `civs` AI civs, world `world` (e.g. `"WORLDSIZE_SMALL"`), normal
    /// speed, Noble, first era, temperate climate, medium sea level, no options.
    pub fn new(d: &GameData, civs: usize, world: &str, seed: u32) -> SetupOptions {
        let noble = d.ty::<HandicapType>("HANDICAP_NOBLE").unwrap_or(HandicapType(0));
        SetupOptions {
            civs,
            world_size: d.t::<WorldSizeType>(world),
            game_speed: d.t::<GameSpeedType>("GAMESPEED_NORMAL"),
            handicap: opt_from_i32(d.defines.standard_handicap).unwrap_or(noble),
            player_handicap: noble,
            era: EraType(0),
            climate: d.t::<ClimateType>("CLIMATE_TEMPERATE"),
            sea_level: d.t::<SeaLevelType>("SEALEVEL_MEDIUM"),
            options: Vec::new(),
            disabled_victories: Vec::new(),
            max_turns: 0,
            target_score: 0,
            max_city_elimination: 0,
            teams: TeamSetup::Solo,
            spare_slots: GameSetup::SPARE_SLOTS,
            seed,
        }
    }

    /// Team slot of civ slot `i`.
    fn team_of(&self, i: usize) -> TeamId {
        let t = match &self.teams {
            TeamSetup::Solo => i,
            TeamSetup::TeamsOf(n) => {
                let n = (*n).max(1);
                i / n * n
            }
            TeamSetup::Explicit(v) => v.get(i).map_or(i, |&t| t as usize),
        };
        TeamId(t as u8)
    }
}

impl GameSetup {
    /// Open player slots reserved for players FFH2 adds mid-game (Infernals, Mercurians,
    /// `formEmpire`, ...; C++ uses any of its 35 civ slots via `addPlayerAdvanced`).
    pub const SPARE_SLOTS: usize = 4;

    /// Most civs a game can have (C++ `MAX_CIV_PLAYERS`); with that many there is no open slot
    /// for the players FFH2 adds mid-game.
    pub const MAX_CIVS: usize = MAX_CIV_PLAYERS;

    /// A standard game: `n` AI civs (random civs/leaders are chosen by `game_init`, port of the
    /// FfH RANDOM_CIVILIZATION block in `CvGame::init`),
    /// [`Self::SPARE_SLOTS`] open slots, then the barbarian slot (barbarian civilization and
    /// leader, AI-controlled); given world size,
    /// normal speed/climate/sea level, first era, Noble difficulty.
    pub fn standard(d: &GameData, n_civs: usize, world: &str, seed: u32) -> GameSetup {
        GameSetup::from_options(d, &SetupOptions::new(d, n_civs, world, seed))
    }

    /// The game described by `o`: `o.civs` AI civ slots (teams per `o.teams`), up to
    /// `o.spare_slots` open slots, then the barbarian slot.
    pub fn from_options(d: &GameData, o: &SetupOptions) -> GameSetup {
        assert!(o.civs >= 1 && o.civs <= MAX_CIV_PLAYERS, "civs must be in 1..={MAX_CIV_PLAYERS}");
        let mut players = Vec::new();
        let n_slots = o.civs + o.spare_slots.min(MAX_CIV_PLAYERS - o.civs);
        for i in 0..=n_slots {
            let barbarian = i == n_slots;
            let team = if i < o.civs { o.team_of(i) } else { TeamId(i as u8) };
            assert!(barbarian || team.idx() < n_slots, "team slot {} of civ slot {i} is out of range", team.idx());
            players.push(PlayerSetup {
                // The exe gives the barbarian slot BARBARIAN_CIVILIZATION / BARBARIAN_LEADER
                // and an AI slot status (CvPlayer::init only brings computer/taken slots alive).
                civ: if barbarian { opt_from_i32(d.defines.barbarian_civilization) } else { None },
                leader: if barbarian { opt_from_i32(d.defines.barbarian_leader) } else { None },
                team,
                handicap: Some(o.player_handicap),
                art_style: None,
                slot_status: if i < o.civs || barbarian { SlotStatus::Computer } else { SlotStatus::Open },
                human: false,
                leader_name: String::new(),
                civ_description: String::new(),
            });
        }
        let mut options = IdxVec::from_elem(false, d.game_options.len());
        for &opt in &o.options {
            options[opt] = true;
        }
        let mut victories = IdxVec::from_elem(true, d.victories.len());
        for &v in &o.disabled_victories {
            victories[v] = false;
        }
        GameSetup {
            world_size: o.world_size,
            climate: o.climate,
            sea_level: o.sea_level,
            era: o.era,
            game_speed: o.game_speed,
            calendar: None,
            options,
            victories,
            game_turn: 0,
            max_turns: o.max_turns,
            target_score: o.target_score,
            max_city_elimination: o.max_city_elimination,
            num_advanced_start_points: Self::advanced_start_points(d, o.era, o.world_size, o.game_speed),
            sync_rand_seed: o.seed,
            map_rand_seed: o.seed.wrapping_mul(2654435761).wrapping_add(1),
            map_script: "Fractal".into(),
            players,
        }
    }

    /// C++ `CvInitCore::resetAdvancedStartPoints` (run by the era, world size and game speed
    /// setters): era points, scaled by the world size and the game speed growth percent.
    pub fn advanced_start_points(d: &GameData, era: EraType, world: WorldSizeType, speed: GameSpeedType) -> i32 {
        let mut points = d.eras[era].advanced_start_points;
        points *= d.worlds[world].advanced_start_points_mod;
        points /= 100;
        points *= d.game_speeds[speed].growth_percent;
        points /= 100;
        points
    }

    pub fn option(&self, o: GameOptionType) -> bool {
        self.options.get(o).copied().unwrap_or(false)
    }
}

impl Game {
    /// `Game::new` for the custom game `o`, with the game handicap the exe passes to
    /// `CvGame::init(eHandicap)` (with no human player `Game::new` falls back to
    /// `STANDARD_HANDICAP`). Call `game_init` after attaching controllers.
    pub fn new_with_options(d: &'static GameData, o: &SetupOptions) -> Game {
        let mut g = Game::new(d, GameSetup::from_options(d, o));
        g.handicap = o.handicap;
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_matches_default_options() {
        let d = ffh_data::global();
        let s = GameSetup::standard(d, 3, "WORLDSIZE_TINY", 5);
        assert_eq!(s.players.len(), 3 + GameSetup::SPARE_SLOTS + 1);
        assert!(s.players.iter().enumerate().all(|(i, p)| p.team == TeamId(i as u8)));
        assert!(s.options.iter().all(|&b| !b));
        assert!(s.victories.iter().all(|&b| b));
    }

    #[test]
    fn max_civs_leave_no_spare_slot_and_teams_group_slots() {
        let d = ffh_data::global();
        let mut o = SetupOptions::new(d, GameSetup::MAX_CIVS, "WORLDSIZE_HUGE", 1);
        o.teams = TeamSetup::TeamsOf(2);
        let s = GameSetup::from_options(d, &o);
        assert_eq!(s.players.len(), MAX_CIV_PLAYERS + 1);
        assert_eq!(s.players[1].team, TeamId(0));
        assert_eq!(s.players[2].team, TeamId(2));
        assert_eq!(s.players[MAX_CIV_PLAYERS].team, TeamId(MAX_CIV_PLAYERS as u8));
    }

    #[test]
    fn advanced_start_points_scale_with_world_and_speed() {
        let d = ffh_data::global();
        let era = EraType(0);
        let world = d.t::<WorldSizeType>("WORLDSIZE_STANDARD");
        let speed = d.t::<GameSpeedType>("GAMESPEED_NORMAL");
        let expected = d.eras[era].advanced_start_points * d.worlds[world].advanced_start_points_mod / 100
            * d.game_speeds[speed].growth_percent
            / 100;
        assert_eq!(GameSetup::advanced_start_points(d, era, world, speed), expected);
    }
}
