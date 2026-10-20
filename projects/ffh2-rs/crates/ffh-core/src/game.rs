//! Port of `CvGame` (CvGame.h / CvGame.cpp) and the root `Game` object that owns all state.
//!
//! The struct and the CvInitCore-backed accessors live here; CvGame's methods are split by
//! topic into `impl Game` blocks (C++ line ranges in docs/porting/game.md):
//! - `game/setup_init.rs`: new game creation (`init`, `reset`, `initDiplomacy`, free state and
//!   units, starting plots, start normalization, `addPlayer`, score/event initialization);
//! - `game/turn.rs`: `update`, `doTurn`, turn bookkeeping, global queries/counters, score
//!   ranking, victory tests, deals, global warming;
//! - `game/votes.rs`: diplomatic votes, resolutions, secretary general, vote sources;
//! - `game/world.rs`: religions/holy cities, corporations/HQs, unit/building/project limits,
//!   forced civics, special units/buildings, shrine and culture-victory caches, plot extra
//!   yields/costs, barbarian cities/units and animals, FfH additions (Armageddon counter,
//!   trophies, disabled religions).
//!
//! Naming: trivial C++ getters/setters are field accesses (`getNumCities()` ->
//! `self.num_cities`, `changeNoNukesCount(i)` -> `self.no_nukes_count += i`). Methods drop a
//! leading `get` (`getSorenRandNum` -> `soren_rand_num`, `getGameTurn` -> `game_turn`) unless
//! that would clash with a field (`getGlobalCounter` -> `get_global_counter`, the field
//! `global_counter` is C++ `m_iGlobalCounter`).
//!
//! Not ported (UI/network/replay/serialization/debug): selection and action handling
//! (`selectUnit`, `handleAction`, `doControl`, `cycle*`, ...), `updateColoredPlots`,
//! `updateBlockadedPlots`, `updateSelectionList`, `updateTestEndTurn`, replay messages,
//! `read`/`write`, hall of fame, pause/active player, PBEM/pitboss/hot-seat and MP turn
//! timers, debug mode, checksums of options, soundtracks, flyout menus.

#![allow(unused_imports)]
use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::area::Area;
use crate::city::City;
use crate::controller::{null_controller, Controller, ControllerRef};
use crate::deal::Deal;
use crate::group::SelectionGroup;
use crate::ids::*;
use crate::log::GameLog;
use crate::map::Map;
use crate::mapgen::{default_map_script, MapScriptRef};
use crate::path::{FinderKind, PathFinder};
use crate::player::Player;
use crate::plot_group::PlotGroup;
use crate::rng::Rng;
use crate::setup::{GameSetup, PlayerSetup, SlotStatus};
use crate::slab::Slab;
use crate::structs::*;
use crate::team::Team;
use crate::unit::Unit;
use ffh_data::*;

mod setup_init;
mod turn;
mod votes;
mod world;

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
    /// C++ `CvGame::m_deals` (`getNumDeals()` = `deals.len()`, `addDeal`/`deleteDeal` =
    /// `insert`/`remove`, `firstDeal`/`nextDeal` = `iter`).
    pub deals: Slab<DealId, Deal>,
    /// C++ `m_mapRand` (map generation).
    pub map_rand: Rng,
    /// C++ `m_sorenRand` (synchronized game randomness).
    pub soren_rand: Rng,
    pub log: GameLog,
    /// One controller per player slot.
    pub controllers: IdxVec<PlayerId, ControllerRef>,
    // ---- C++ CvGame members (port of CvGame.h) go below ----
    /// C++ `m_iElapsedGameTurns`.
    pub elapsed_game_turns: i32,
    /// C++ `m_iStartTurn`.
    pub start_turn: i32,
    /// C++ `m_iStartYear`.
    pub start_year: i32,
    /// C++ `m_iEstimateEndTurn`.
    pub estimate_end_turn: i32,
    /// C++ `m_iTurnSlice` (one `update()` per slice).
    pub turn_slice: i32,
    /// C++ `m_iCutoffSlice` (MP turn timer; kept for completeness).
    pub cutoff_slice: i32,
    /// C++ `m_iNumGameTurnActive`: players whose turn is active.
    pub num_game_turn_active: i32,
    /// C++ `m_iNumCities`.
    pub num_cities: i32,
    /// C++ `m_iTotalPopulation`.
    pub total_population: i32,
    /// C++ `m_iTradeRoutes` (change with `change_trade_routes`).
    pub trade_routes: i32,
    /// C++ `m_iFreeTradeCount` (change with `change_free_trade_count`).
    pub free_trade_count: i32,
    /// C++ `m_iNoNukesCount`.
    pub no_nukes_count: i32,
    /// C++ `m_iNukesExploded`.
    pub nukes_exploded: i32,
    /// C++ `m_iMaxPopulation` (score normalization, `initScoreCalculation`). The `max_*` and
    /// `init_*` counters only feed the legacy FFH2 score (`player_legacy_score`) and tools.
    pub max_population: i32,
    /// C++ `m_iMaxLand`.
    pub max_land: i32,
    /// C++ `m_iMaxTech`.
    pub max_tech: i32,
    /// C++ `m_iMaxWonders`.
    pub max_wonders: i32,
    /// C++ `m_iInitPopulation`.
    pub init_population: i32,
    /// C++ `m_iInitLand`.
    pub init_land: i32,
    /// C++ `m_iInitTech`.
    pub init_tech: i32,
    /// C++ `m_iInitWonders`.
    pub init_wonders: i32,
    /// C++ `m_iAIAutoPlay`: turns left of AI autoplay. While > 0 the game does not end when no
    /// human is alive, and victories put the game in `GAMESTATE_EXTENDED`.
    pub ai_auto_play: i32,
    /// C++ `gDLL->GetAutorun()` (exe flag): the game keeps running with no human player alive.
    /// The headless runner sets it; it has no other effect.
    pub autorun: bool,
    /// C++ `GC.getGameINLINE().getActivePlayer()` (CvInitCore): the player whose screen is
    /// shown. Always `None` when headless, so active-player UI branches are skipped.
    pub active_player: Option<PlayerId>,
    /// C++ `m_bScoreDirty`: `update_score` recomputes ranks only when set (rank timing feeds
    /// the FfH challenge options in `do_turn`, so it is kept although C++ treats it as UI).
    pub score_dirty: bool,
    /// C++ `m_bCircumnavigated`.
    pub circumnavigated: bool,
    /// C++ `m_bFinalInitialized` (change with `set_final_initialized`).
    pub final_initialized: bool,
    /// C++ `m_bNukesValid`.
    pub nukes_valid: bool,
    /// C++ `m_eHandicap`: game difficulty (C++ NO_HANDICAP only between construction and
    /// `init`; here set by `Game::new`).
    pub handicap: HandicapType,
    /// C++ `m_eBestLandUnit`.
    pub best_land_unit: Option<UnitType>,
    /// C++ `m_eWinner`.
    pub winner: Option<TeamId>,
    /// C++ `m_eVictory`.
    pub victory: Option<VictoryType>,
    /// C++ `m_eGameState` (change with `set_game_state`).
    pub game_state: GameStateType,
    /// C++ `m_szScriptData` (Python script data).
    pub script_data: String,
    // FfH: Added by Kael 08/07/2007
    /// C++ `m_iCrime` (FfH vote effect; starts at 10).
    pub crime: i32,
    /// C++ `m_iCutLosersCounter` (GAMEOPTION_CHALLENGE_CUT_LOSERS; starts at 100).
    pub cut_losers_counter: i32,
    /// C++ `m_iFlexibleDifficultyCounter` (GAMEOPTION_FLEXIBLE_DIFFICULTY).
    pub flexible_difficulty_counter: i32,
    /// C++ `m_iGlobalCounter`: the Armageddon counter, raw (`getTrueGlobalCounter`); the
    /// percentage `getGlobalCounter` is `get_global_counter()`. Change with `change_global_counter`.
    pub global_counter: i32,
    /// C++ `m_iGlobalCounterLimit` (starts at `GLOBAL_COUNTER_LIMIT_DEFAULT`).
    pub global_counter_limit: i32,
    /// C++ `m_iHighToLowCounter` (GAMEOPTION_CHALLENGE_HIGH_TO_LOW).
    pub high_to_low_counter: i32,
    /// C++ `m_iIncreasingDifficultyCounter` (GAMEOPTION_CHALLENGE_INCREASING_DIFFICULTY).
    pub increasing_difficulty_counter: i32,
    /// C++ `m_iMaxGlobalCounter`: highest raw Armageddon counter reached.
    pub max_global_counter: i32,
    /// C++ `m_iScenarioCounter`.
    pub scenario_counter: i32,
    /// C++ `m_pabEventTriggered`.
    pub event_triggered: IdxVec<EventTriggerType, bool>,
    /// C++ `m_pabGamblingRing` (vote effect, per vote source).
    pub gambling_ring: IdxVec<VoteSourceType, bool>,
    /// C++ `m_pabNoBonus` (vote effect).
    pub no_bonus: IdxVec<BonusType, bool>,
    /// C++ `m_pabNoOutsideTechTrades` (vote effect).
    pub no_outside_tech_trades: IdxVec<VoteSourceType, bool>,
    /// C++ `m_pabSlaveTrade` (vote effect).
    pub slave_trade: IdxVec<VoteSourceType, bool>,
    /// C++ `m_pabSmugglingRing` (vote effect).
    pub smuggling_ring: IdxVec<VoteSourceType, bool>,
    // FfH: End Add
    /// C++ `m_aiRankPlayer`: player by rank (index = rank; C++ initializes to player 0).
    pub rank_player: Vec<PlayerId>,
    /// C++ `m_aiPlayerRank`.
    pub player_rank: IdxVec<PlayerId, i32>,
    /// C++ `m_aiPlayerScore`: score v2 in basis points (share of the world, docs/SCORE.md),
    /// cached by `update_score` from `world_scores()`.
    pub player_score: IdxVec<PlayerId, i32>,
    /// C++ `m_aiRankTeam`: team by rank.
    pub rank_team: Vec<TeamId>,
    /// C++ `m_aiTeamRank`.
    pub team_rank: IdxVec<TeamId, i32>,
    /// C++ `m_aiTeamScore`: sum of the members' `player_score` (basis points; alive teams sum
    /// to 10000).
    pub team_score: IdxVec<TeamId, i32>,
    /// C++ `m_paiUnitCreatedCount`.
    pub unit_created_count: IdxVec<UnitType, i32>,
    /// C++ `m_paiUnitClassCreatedCount`.
    pub unit_class_created_count: IdxVec<UnitClassType, i32>,
    /// C++ `m_paiBuildingClassCreatedCount`.
    pub building_class_created_count: IdxVec<BuildingClassType, i32>,
    /// C++ `m_paiProjectCreatedCount`.
    pub project_created_count: IdxVec<ProjectType, i32>,
    /// C++ `m_paiForceCivicCount` (change with `change_force_civic_count`).
    pub force_civic_count: IdxVec<CivicType, i32>,
    /// C++ `m_paiVoteOutcome` (set with `set_vote_outcome`).
    pub vote_outcome: IdxVec<VoteType, PlayerVote>,
    /// C++ `m_paiReligionGameTurnFounded` (-1 = not founded).
    pub religion_game_turn_founded: IdxVec<ReligionType, i32>,
    /// C++ `m_paiCorporationGameTurnFounded` (-1 = not founded).
    pub corporation_game_turn_founded: IdxVec<CorporationType, i32>,
    /// C++ `m_aiSecretaryGeneralTimer`.
    pub secretary_general_timer: IdxVec<VoteSourceType, i32>,
    /// C++ `m_aiVoteTimer`.
    pub vote_timer: IdxVec<VoteSourceType, i32>,
    /// C++ `m_aiDiploVote` (`getDiploVoteCount`; change with `change_diplo_vote`).
    pub diplo_vote: IdxVec<VoteSourceType, i32>,
    /// C++ `m_pabSpecialUnitValid`.
    pub special_unit_valid: IdxVec<SpecialUnitType, bool>,
    /// C++ `m_pabSpecialBuildingValid` (set with `make_special_building_valid`).
    pub special_building_valid: IdxVec<SpecialBuildingType, bool>,
    /// C++ `m_abReligionSlotTaken`.
    pub religion_slot_taken: IdxVec<ReligionType, bool>,
    /// C++ `m_paHolyCity` (read with `get_holy_city`, which ignores destroyed cities).
    pub holy_city: IdxVec<ReligionType, Option<CityId>>,
    /// C++ `m_paHeadquarters` (read with `get_headquarters`).
    pub headquarters: IdxVec<CorporationType, Option<CityId>>,
    /// C++ `m_aszDestroyedCities`.
    pub destroyed_cities: Vec<String>,
    /// C++ `m_aszGreatPeopleBorn`.
    pub great_people_born: Vec<String>,
    /// C++ `m_voteSelections` (FFreeListTrashArray; see docs/porting/game.md on iteration order).
    pub vote_selections: Vec<VoteSelectionData>,
    /// C++ `m_votesTriggered`.
    pub votes_triggered: Vec<VoteTriggeredData>,
    /// Next `VoteSelectionData::id` (C++ FFreeListTrashArray ID allocation).
    pub next_vote_selection_id: u32,
    /// Next `VoteTriggeredData::id`.
    pub next_vote_triggered_id: u32,
    /// C++ `m_aPlotExtraYields`.
    pub plot_extra_yields: Vec<PlotExtraYield>,
    /// C++ `m_aPlotExtraCosts`.
    pub plot_extra_costs: Vec<PlotExtraCost>,
    /// C++ `m_mapVoteSourceReligions` (hash_map; absent = NO_RELIGION). `getVoteSourceReligion`
    /// is a field read; set with `set_vote_source_religion`.
    pub vote_source_religions: IdxVec<VoteSourceType, Option<ReligionType>>,
    /// C++ `m_aeInactiveTriggers` (`initEvents`).
    pub inactive_triggers: Vec<EventTriggerType>,
    /// C++ `m_iShrineBuildingCount` (cache, see `change_shrine_building`).
    pub shrine_building_count: i32,
    /// C++ `m_aiShrineBuilding` (sized `GC.getNumBuildingInfos()`, first
    /// `shrine_building_count` entries used).
    pub shrine_building: Vec<Option<BuildingType>>,
    /// C++ `m_aiShrineReligion`.
    pub shrine_religion: Vec<Option<ReligionType>>,
    /// C++ `m_iNumCultureVictoryCities`.
    pub num_culture_victory_cities: i32,
    /// C++ `m_eCultureVictoryCultureLevel`.
    pub culture_victory_culture_level: Option<CultureLevelType>,
    // ---- replacements for state C++ keeps outside CvGame ----
    /// Trophies (C++ Python `CvDataStorage`, persistent across games in C++; per game here).
    pub trophies: BTreeMap<String, i32>,
    /// The map script (C++ Python map script module).
    pub map_script: MapScriptRef,
    /// Not in C++: the units whose `unit_kill` is running their `unitKilled` event; a kill of one
    /// of them meanwhile does nothing (see `unit_kill`). Empty between rules calls; not saved.
    pub dying: Vec<UnitId>,
    /// C++ `GC.getPathFinder()`.
    pub path_finder: RefCell<PathFinder>,
    /// C++ `GC.getStepFinder()`.
    pub step_finder: RefCell<PathFinder>,
    /// C++ `GC.getRouteFinder()`.
    pub route_finder: RefCell<PathFinder>,
    /// C++ `GC.getBorderFinder()`.
    pub border_finder: RefCell<PathFinder>,
    /// C++ `GC.getAreaFinder()`.
    pub area_finder: RefCell<PathFinder>,
    /// C++ `GC.getPlotGroupFinder()`.
    pub plot_group_finder: RefCell<PathFinder>,
}

impl Game {
    /// C++ `CvGame::CvGame()` + `reset(eHandicap)`: a game in its pre-`init` state, with players
    /// and teams sized to `setup` (one team slot per player slot, as C++
    /// `MAX_TEAMS == MAX_PLAYERS`), an empty map (`Map::default()`, built by `map_init`),
    /// placeholder players, teams at their `CvTeam::reset` defaults, and every CvGame member at
    /// its `reset` default. The new-game sequence is `game_init` (game/setup_init.rs).
    pub fn new(d: &'static GameData, setup: GameSetup) -> Game {
        let num_players = setup.players.len();
        let num_teams = num_players;
        let handicap = Self::setup_handicap(d, &setup);
        let mut g = Game {
            d,
            setup,
            map: Map::default(),
            players: IdxVec::from_fn(num_players, |_| Player::default()),
            teams: IdxVec::from_fn(num_teams, |t| Team::new(t, num_teams, d)),
            units: Slab::new(),
            cities: Slab::new(),
            groups: Slab::new(),
            plot_groups: Slab::new(),
            deals: Slab::new(),
            map_rand: Rng::new(0),
            soren_rand: Rng::new(0),
            log: GameLog::default(),
            controllers: IdxVec::from_fn(num_players, |_| null_controller()),
            // CvGame members: placeholders, set by `reset` below.
            elapsed_game_turns: 0,
            start_turn: 0,
            start_year: 0,
            estimate_end_turn: 0,
            turn_slice: 0,
            cutoff_slice: 0,
            num_game_turn_active: 0,
            num_cities: 0,
            total_population: 0,
            trade_routes: 0,
            free_trade_count: 0,
            no_nukes_count: 0,
            nukes_exploded: 0,
            max_population: 0,
            max_land: 0,
            max_tech: 0,
            max_wonders: 0,
            init_population: 0,
            init_land: 0,
            init_tech: 0,
            init_wonders: 0,
            ai_auto_play: 0,
            autorun: false,
            active_player: None,
            score_dirty: false,
            circumnavigated: false,
            final_initialized: false,
            nukes_valid: false,
            handicap,
            best_land_unit: None,
            winner: None,
            victory: None,
            game_state: GameStateType::GAMESTATE_ON,
            script_data: String::new(),
            crime: 0,
            cut_losers_counter: 0,
            flexible_difficulty_counter: 0,
            global_counter: 0,
            global_counter_limit: 0,
            high_to_low_counter: 0,
            increasing_difficulty_counter: 0,
            max_global_counter: 0,
            scenario_counter: 0,
            event_triggered: IdxVec::new(),
            gambling_ring: IdxVec::new(),
            no_bonus: IdxVec::new(),
            no_outside_tech_trades: IdxVec::new(),
            slave_trade: IdxVec::new(),
            smuggling_ring: IdxVec::new(),
            rank_player: Vec::new(),
            player_rank: IdxVec::new(),
            player_score: IdxVec::new(),
            rank_team: Vec::new(),
            team_rank: IdxVec::new(),
            team_score: IdxVec::new(),
            unit_created_count: IdxVec::new(),
            unit_class_created_count: IdxVec::new(),
            building_class_created_count: IdxVec::new(),
            project_created_count: IdxVec::new(),
            force_civic_count: IdxVec::new(),
            vote_outcome: IdxVec::new(),
            religion_game_turn_founded: IdxVec::new(),
            corporation_game_turn_founded: IdxVec::new(),
            secretary_general_timer: IdxVec::new(),
            vote_timer: IdxVec::new(),
            diplo_vote: IdxVec::new(),
            special_unit_valid: IdxVec::new(),
            special_building_valid: IdxVec::new(),
            religion_slot_taken: IdxVec::new(),
            holy_city: IdxVec::new(),
            headquarters: IdxVec::new(),
            destroyed_cities: Vec::new(),
            great_people_born: Vec::new(),
            vote_selections: Vec::new(),
            votes_triggered: Vec::new(),
            next_vote_selection_id: 0,
            next_vote_triggered_id: 0,
            plot_extra_yields: Vec::new(),
            plot_extra_costs: Vec::new(),
            vote_source_religions: IdxVec::new(),
            inactive_triggers: Vec::new(),
            shrine_building_count: 0,
            shrine_building: Vec::new(),
            shrine_religion: Vec::new(),
            num_culture_victory_cities: 0,
            culture_victory_culture_level: None,
            trophies: BTreeMap::new(),
            map_script: default_map_script(),
            dying: Vec::new(),
            path_finder: RefCell::new(PathFinder::new(FinderKind::Path)),
            step_finder: RefCell::new(PathFinder::new(FinderKind::Step)),
            route_finder: RefCell::new(PathFinder::new(FinderKind::Route)),
            border_finder: RefCell::new(PathFinder::new(FinderKind::Border)),
            area_finder: RefCell::new(PathFinder::new(FinderKind::Area)),
            plot_group_finder: RefCell::new(PathFinder::new(FinderKind::PlotGroup)),
        };
        g.reset(handicap);
        g
    }

    /// The handicap the exe passes to `CvGame::init`: the first human slot's handicap, else
    /// `STANDARD_HANDICAP` (AI-only games), else the first handicap.
    pub fn setup_handicap(d: &GameData, setup: &GameSetup) -> HandicapType {
        setup
            .players
            .iter()
            .find(|p| p.human)
            .and_then(|p| p.handicap)
            .or_else(|| opt_from_i32(d.defines.standard_handicap))
            .unwrap_or(HandicapType(0))
    }

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

    /// Civ player ids (C++ `for (iI = 0; iI < MAX_CIV_PLAYERS; iI++)`).
    pub fn civ_player_ids(&self) -> impl Iterator<Item = PlayerId> + 'static {
        (0..self.players.len() - 1).map(|i| PlayerId(i as u8))
    }

    /// Civ team ids (C++ `for (iI = 0; iI < MAX_CIV_TEAMS; iI++)`).
    pub fn civ_team_ids(&self) -> impl Iterator<Item = TeamId> + 'static {
        (0..self.teams.len() - 1).map(|i| TeamId(i as u8))
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

    // ---- CvGame getters backed by CvInitCore (`self.setup`) ----

    /// C++ `getGameTurn()` (stored in CvInitCore; change with `set_game_turn`).
    #[inline]
    pub fn game_turn(&self) -> i32 {
        self.setup.game_turn
    }

    /// C++ `getMaxTurns()` (`setMaxTurns` = `self.setup.max_turns = v`).
    #[inline]
    pub fn max_turns(&self) -> i32 {
        self.setup.max_turns
    }

    /// C++ `getMaxCityElimination()`.
    #[inline]
    pub fn max_city_elimination(&self) -> i32 {
        self.setup.max_city_elimination
    }

    /// C++ `getNumAdvancedStartPoints()`.
    #[inline]
    pub fn num_advanced_start_points(&self) -> i32 {
        self.setup.num_advanced_start_points
    }

    /// C++ `getTargetScore()`: in basis points of the world (score v2, docs/SCORE.md; 5000 = half
    /// the world, 0 = no score victory).
    #[inline]
    pub fn target_score(&self) -> i32 {
        self.setup.target_score
    }

    /// C++ `getStartEra()`.
    #[inline]
    pub fn start_era(&self) -> EraType {
        self.setup.era
    }

    /// C++ `getGameSpeedType()`.
    #[inline]
    pub fn game_speed_type(&self) -> GameSpeedType {
        self.setup.game_speed
    }

    /// C++ `getCalendar()` (CvInitCore defaults it to `STANDARD_CALENDAR`).
    pub fn calendar(&self) -> CalendarType {
        self.setup
            .calendar
            .or_else(|| opt_from_i32(self.d.defines.standard_calendar))
            .unwrap_or(CalendarType::CALENDAR_DEFAULT)
    }

    /// C++ `isVictoryValid(eIndex)` (`setVictoryValid` = `self.setup.victories[v] = b`).
    #[inline]
    pub fn is_victory_valid(&self, victory: VictoryType) -> bool {
        self.setup.victories[victory]
    }

    /// C++ `isOption(eIndex)`. FfH locks three options: NO_ESPIONAGE is always on,
    /// RANDOM_PERSONALITIES and PICK_RELIGION always off. (`setOption` = `self.setup.options[o] = b`.)
    pub fn is_option(&self, option: GameOptionType) -> bool {
        // FfH: Added by Kael 07/12/2008 (locked options)
        if option == GameOptionType::GAMEOPTION_NO_ESPIONAGE {
            return true;
        }
        if option == GameOptionType::GAMEOPTION_RANDOM_PERSONALITIES {
            return false;
        }
        if option == GameOptionType::GAMEOPTION_PICK_RELIGION {
            return false;
        }
        self.setup.option(option)
    }

    /// C++ `getNumHumanPlayers()` (CvInitCore::getNumHumans: human civ slots).
    pub fn num_human_players(&self) -> i32 {
        let num_civ = self.setup.players.len().saturating_sub(1);
        self.setup.players.iter().take(num_civ).filter(|p| p.human).count() as i32
    }

    // ---- network / multiplayer queries used by rules code (single process: always false) ----

    /// C++ `isNetworkMultiPlayer()`.
    #[inline]
    pub fn is_network_multi_player(&self) -> bool {
        false
    }

    /// C++ `isGameMultiPlayer()` (network, PBEM or hot seat).
    #[inline]
    pub fn is_game_multi_player(&self) -> bool {
        false
    }

    /// C++ `isPbem()`.
    #[inline]
    pub fn is_pbem(&self) -> bool {
        false
    }

    /// C++ `isHotSeat()`.
    #[inline]
    pub fn is_hot_seat(&self) -> bool {
        false
    }

    /// C++ `isSimultaneousTeamTurns()` (only in network games).
    #[inline]
    pub fn is_simultaneous_team_turns(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_has_reset_defaults() {
        let d = ffh_data::global();
        let setup = GameSetup::standard(d, 4, "WORLDSIZE_SMALL", 1234);
        let n = 4 + GameSetup::SPARE_SLOTS + 1; // civs, open slots, barbarian
        let g = Game::new(d, setup);
        assert_eq!(g.num_players(), n);
        assert_eq!(g.num_teams(), n);
        assert_eq!(g.barbarian_player(), PlayerId((n - 1) as u8));
        assert_eq!(g.barbarian_team(), TeamId((n - 1) as u8));
        assert_eq!(g.civ_player_ids().count(), n - 1);
        assert_eq!(g.num_human_players(), 0);
        assert!(g.team_ids().all(|t| g.teams[t].id == t && g.teams[t].at_war.len() == n));
        assert_eq!(g.handicap, d.t::<HandicapType>("HANDICAP_NOBLE"));
        assert_eq!(g.game_state, GameStateType::GAMESTATE_ON);
        assert_eq!(g.crime, 10);
        assert_eq!(g.cut_losers_counter, 100);
        assert_eq!(g.global_counter_limit, d.defines.global_counter_limit_default);
        assert_eq!(g.unit_created_count.len(), d.units.len());
        assert_eq!(g.vote_outcome.len(), d.votes.len());
        assert!(g.vote_outcome.iter().all(|&v| v == PlayerVote::NO_PLAYER_VOTE));
        assert!(g.religion_game_turn_founded.iter().all(|&t| t == -1));
        assert!(g.holy_city.iter().all(|c| c.is_none()));
        assert_eq!(g.shrine_building.len(), d.buildings.len());
        assert_eq!(g.rank_player.len(), n);
        assert_eq!(g.player_score.len(), n);
        assert_eq!(g.vote_source_religions.len(), d.vote_sources.len());
        assert_eq!(g.map_rand.seed(), 0);
        assert_eq!(g.soren_rand.seed(), 0);
        assert!(!g.is_game_multi_player());
    }

    #[test]
    fn locked_options() {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 1);
        setup.options[GameOptionType::GAMEOPTION_PICK_RELIGION] = true;
        setup.options[GameOptionType::GAMEOPTION_RAGING_BARBARIANS] = true;
        let g = Game::new(d, setup);
        assert!(g.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE));
        assert!(!g.is_option(GameOptionType::GAMEOPTION_PICK_RELIGION));
        assert!(!g.is_option(GameOptionType::GAMEOPTION_RANDOM_PERSONALITIES));
        assert!(g.is_option(GameOptionType::GAMEOPTION_RAGING_BARBARIANS));
        assert!(!g.is_option(GameOptionType::GAMEOPTION_NO_BARBARIANS));
    }
}
