//! Port of `CvGame` (CvGame.h / CvGame.cpp) and the root `Game` object that owns all state.

#![allow(unused_imports)]
use crate::area::Area;
use crate::city::City;
use crate::controller::{null_controller, Controller, ControllerRef};
use crate::deal::Deal;
use crate::group::SelectionGroup;
use crate::ids::*;
use crate::log::GameLog;
use crate::map::Map;
use crate::player::Player;
use crate::plot_group::PlotGroup;
use crate::rng::Rng;
use crate::setup::GameSetup;
use crate::slab::Slab;
use crate::team::Team;
use crate::unit::Unit;
use ffh_data::*;

/// The whole game state. C++ `CvGame` + the global objects it reaches through `GC`
/// (map, players, teams) + the per-player object lists (units, cities, groups, plot groups).
pub struct Game {
    /// Static game data (C++ `GC.getXInfo(..)`, `GC.getDefineINT(..)`).
    pub d: &'static GameData,
    /// C++ `CvInitCore` (gameplay subset).
    pub setup: GameSetup,
    /// C++ `CvMap` (plots, areas).
    pub map: Map,
    /// C++ `GET_PLAYER(i)`; length `num_players()`, barbarian last.
    pub players: IdxVec<PlayerId, Player>,
    /// C++ `GET_TEAM(i)`; length `num_teams()`, barbarian last.
    pub teams: IdxVec<TeamId, Team>,
    /// All units (C++ per-player `m_units` free lists). Owner lists: `Player::units`.
    pub units: Slab<UnitId, Unit>,
    /// All cities. Owner lists: `Player::cities`.
    pub cities: Slab<CityId, City>,
    /// All selection groups. Owner lists: `Player::groups`.
    pub groups: Slab<GroupId, SelectionGroup>,
    /// All plot groups. Owner lists: `Player::plot_groups`.
    pub plot_groups: Slab<PlotGroupId, PlotGroup>,
    /// C++ `CvGame::m_deals`.
    pub deals: Slab<DealId, Deal>,
    /// C++ `m_mapRand` (map generation).
    pub map_rand: Rng,
    /// C++ `m_sorenRand` (synchronized game randomness).
    pub soren_rand: Rng,
    pub log: GameLog,
    /// One controller per player slot.
    pub controllers: IdxVec<PlayerId, ControllerRef>,
    // ---- C++ CvGame members (port of CvGame.h) go below ----
}

impl Game {
    /// Number of player slots (C++ `MAX_PLAYERS`, sized to this game).
    #[inline]
    pub fn num_players(&self) -> usize {
        self.players.len()
    }

    /// Number of team slots (C++ `MAX_TEAMS`, sized to this game).
    #[inline]
    pub fn num_teams(&self) -> usize {
        self.teams.len()
    }

    /// C++ `BARBARIAN_PLAYER`.
    #[inline]
    pub fn barbarian_player(&self) -> PlayerId {
        PlayerId((self.players.len() - 1) as u8)
    }

    /// C++ `BARBARIAN_TEAM`.
    #[inline]
    pub fn barbarian_team(&self) -> TeamId {
        TeamId((self.teams.len() - 1) as u8)
    }

    /// C++ `MAX_CIV_PLAYERS` sized to this game: civ players are `0..num_civ_players()`.
    #[inline]
    pub fn num_civ_players(&self) -> usize {
        self.players.len() - 1
    }

    /// C++ `MAX_CIV_TEAMS` sized to this game.
    #[inline]
    pub fn num_civ_teams(&self) -> usize {
        self.teams.len() - 1
    }

    /// All player ids (C++ `for (iI = 0; iI < MAX_PLAYERS; iI++)`).
    pub fn player_ids(&self) -> impl Iterator<Item = PlayerId> + 'static {
        (0..self.players.len()).map(|i| PlayerId(i as u8))
    }

    /// All team ids (C++ `for (iI = 0; iI < MAX_TEAMS; iI++)`).
    pub fn team_ids(&self) -> impl Iterator<Item = TeamId> + 'static {
        (0..self.teams.len()).map(|i| TeamId(i as u8))
    }

    /// Controller for player `p` (cheap Arc clone; call methods on the result).
    #[inline]
    pub fn controller(&self, p: PlayerId) -> ControllerRef {
        self.controllers[p].clone()
    }

    /// C++ `CvGame::getSorenRandNum`.
    #[inline]
    pub fn soren_rand_num(&mut self, num: i32, log: &str) -> i32 {
        self.soren_rand.get_i(num, log)
    }

    /// C++ `CvGame::getMapRandNum`.
    #[inline]
    pub fn map_rand_num(&mut self, num: i32, log: &str) -> i32 {
        self.map_rand.get_i(num, log)
    }
}
