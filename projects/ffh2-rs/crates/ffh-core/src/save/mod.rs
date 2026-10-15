//! Saved games: [`Game::save`] / [`Game::load`] (replaces C++ `CvGame::read`/`write` and the
//! per-object `read`/`write`, which this port does not port).
//!
//! A save holds the whole mutable game state, so a game saved at a turn boundary, loaded and
//! continued plays out exactly like the uninterrupted game (same `calculate_sync_checksum`
//! every turn, byte-identical later saves). Layout:
//!
//! ```text
//! magic "FFH2SAVE" | format version (u32 LE) | data fingerprint | Game
//! ```
//!
//! - **Data fingerprint**: static game data (`&'static GameData`) is not saved. The save records,
//!   for every info table, its type count and a hash of its type names (plus a hash of the
//!   enum-sized array lengths the encoding relies on); [`Game::load`] refuses a save made with
//!   different data ([`LoadError::DataMismatch`]).
//! - **Game**: every `Game` field in declaration order (`io.rs` describes the encoding), except
//!   the non-state members:
//!   * `d` (passed to `load`);
//!   * `controllers`: a loaded game has `NullController`s; the caller re-attaches its own
//!     (`g.controllers[p] = ..`), exactly as after `Game::new`;
//!   * `map_script`: only used while generating the map (`game_init`); a loaded game gets the
//!     default script.
//!
//!   Rules caches are saved as they are (`Player::ai_bonus_value`, the city rank caches, the
//!   path finders' kept search trees), so nothing has to be recomputed after loading.
//!
//! **Turn boundaries.** Save between two `update()` calls while no player's turn is active
//! (`num_game_turn_active == 0`, see [`Game::at_turn_boundary`]): the next `update()` then starts
//! the next game turn. Controllers may keep per-turn caches outside `Game` (see
//! `docs/ARCHITECTURE.md`); a save taken mid-turn restores the rules state exactly, but a
//! controller that remembers what it already did this turn will not.
//!
//! **Adding state.** Every saved struct is written by destructuring it exhaustively
//! (`persist_struct!` in `state.rs`, the `Game` impl below, `PathFinder` in `path_finder.rs`),
//! so a new field fails to compile until it is saved. Add it to the list in declaration order
//! and bump [`FORMAT_VERSION`].

pub(crate) mod io;
mod state;

pub use io::{Persist, Reader, Writer};

use std::fmt;

use crate::controller::null_controller;
use crate::defs::NUM_CITY_PLOTS;
use crate::game::Game;
use crate::mapgen::default_map_script;
use ffh_data::*;

/// First bytes of every save.
pub const MAGIC: &[u8; 8] = b"FFH2SAVE";

/// Version of the save layout; bump it whenever the encoding or a saved struct changes, or a
/// saved value changes meaning.
///
/// - 1: first layout.
/// - 2: score v2 (docs/SCORE.md): `player_score`, `team_score` and `Player::score_history` hold
///   shares of the world in basis points instead of FFH2's `calculateScore` (no new fields).
pub const FORMAT_VERSION: u32 = 2;

/// Why [`Game::load`] rejected a save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The input does not start with [`MAGIC`].
    NotASave,
    /// Saved with another [`FORMAT_VERSION`].
    UnsupportedVersion { found: u32, supported: u32 },
    /// Saved with different game data (XML): which table differs.
    DataMismatch(String),
    /// The input ends in the middle of a value.
    Truncated,
    /// A value that cannot have been written by `save`.
    Corrupt(String),
    /// Bytes left over after the game.
    TrailingData(usize),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotASave => write!(f, "not an ffh2-rs saved game"),
            LoadError::UnsupportedVersion { found, supported } => {
                write!(f, "saved game format version {found} is not supported (expected {supported})")
            }
            LoadError::DataMismatch(s) => write!(f, "saved game was made with different game data: {s}"),
            LoadError::Truncated => write!(f, "saved game is truncated"),
            LoadError::Corrupt(s) => write!(f, "saved game is corrupt: {s}"),
            LoadError::TrailingData(n) => write!(f, "saved game has {n} unexpected trailing bytes"),
        }
    }
}

impl std::error::Error for LoadError {}

/// One fingerprint entry: (registry class, number of types, hash of the type names).
type FingerprintEntry = (String, u32, u64);

/// FNV-1a over `parts`, each terminated by a 0 byte.
fn fnv1a<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for &b in p.iter().chain(std::iter::once(&0u8)) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// What a save must agree on with the loaded game data: the type list of every info table
/// (count + hash of the names, in index order) and the fixed array lengths.
fn data_fingerprint(d: &GameData) -> Vec<FingerprintEntry> {
    let global_lists = [
        FlavorType::CLASS,
        ArtStyleType::CLASS,
        CitySizeType::CLASS,
        ContactType::CLASS,
        DiplomacyPowerType::CLASS,
        DirectionType::CLASS,
    ];
    let mut v: Vec<FingerprintEntry> = INFO_FILES
        .iter()
        .map(|&(_, class)| class)
        .chain(global_lists)
        .map(|class| {
            let names = d.reg.names_of(class);
            (class.to_string(), names.len() as u32, fnv1a(names.iter().map(|s| s.as_bytes())))
        })
        .collect();
    let layout = [YieldType::NUM, CommerceType::NUM, DomainType::NUM, DirectionType::NUM, UnitAIType::NUM, NUM_CITY_PLOTS];
    let layout_bytes: Vec<[u8; 8]> = layout.iter().map(|&n| (n as u64).to_le_bytes()).collect();
    v.push(("layout".to_string(), layout.len() as u32, fnv1a(layout_bytes.iter().map(|b| &b[..]))));
    v
}

impl Game {
    /// Is this a turn boundary (no player's turn active: the next `update()` starts the next game
    /// turn)? Saves taken here resume exactly, controllers included (see the module docs).
    pub fn at_turn_boundary(&self) -> bool {
        self.num_game_turn_active == 0
    }

    /// The whole game state as bytes (see the module docs for what is not included).
    pub fn save(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.bytes(MAGIC);
        w.bytes(&FORMAT_VERSION.to_le_bytes());
        data_fingerprint(self.d).write(&mut w);
        write_game(self, &mut w);
        w.into_bytes()
    }

    /// A game from [`Game::save`] bytes, made with the same game data `d`. Controllers are
    /// `NullController`s and the map script is the default one: attach the real controllers
    /// before continuing.
    pub fn load(d: &'static GameData, bytes: &[u8]) -> Result<Game, LoadError> {
        let mut r = Reader::new(bytes);
        if r.remaining() < MAGIC.len() || r.bytes(MAGIC.len())? != MAGIC {
            return Err(LoadError::NotASave);
        }
        let version = u32::from_le_bytes(r.bytes(4)?.try_into().expect("4 bytes"));
        if version != FORMAT_VERSION {
            return Err(LoadError::UnsupportedVersion { found: version, supported: FORMAT_VERSION });
        }
        let saved = Vec::<FingerprintEntry>::read(&mut r)?;
        let current = data_fingerprint(d);
        if saved != current {
            let msg = match saved.iter().zip(&current).find(|(a, b)| a != b) {
                Some((a, b)) => format!("{}: {} types (hash {:016x}) in the save, {} {} types (hash {:016x}) in the data", a.0, a.1, a.2, b.0, b.1, b.2),
                None => format!("{} tables in the save, {} in the data", saved.len(), current.len()),
            };
            return Err(LoadError::DataMismatch(msg));
        }
        let g = read_game(d, &mut r)?;
        if r.remaining() != 0 {
            return Err(LoadError::TrailingData(r.remaining()));
        }
        Ok(g)
    }
}

/// Writes every `Game` field in declaration order (exhaustive destructuring: a new field is a
/// compile error here until it is saved or explicitly skipped).
fn write_game(g: &Game, w: &mut Writer) {
    let Game {
        d: _,
        setup,
        map,
        players,
        teams,
        units,
        cities,
        groups,
        plot_groups,
        deals,
        map_rand,
        soren_rand,
        log,
        controllers: _,
        elapsed_game_turns,
        start_turn,
        start_year,
        estimate_end_turn,
        turn_slice,
        cutoff_slice,
        num_game_turn_active,
        num_cities,
        total_population,
        trade_routes,
        free_trade_count,
        no_nukes_count,
        nukes_exploded,
        max_population,
        max_land,
        max_tech,
        max_wonders,
        init_population,
        init_land,
        init_tech,
        init_wonders,
        ai_auto_play,
        autorun,
        active_player,
        score_dirty,
        circumnavigated,
        final_initialized,
        nukes_valid,
        handicap,
        best_land_unit,
        winner,
        victory,
        game_state,
        script_data,
        crime,
        cut_losers_counter,
        flexible_difficulty_counter,
        global_counter,
        global_counter_limit,
        high_to_low_counter,
        increasing_difficulty_counter,
        max_global_counter,
        scenario_counter,
        event_triggered,
        gambling_ring,
        no_bonus,
        no_outside_tech_trades,
        slave_trade,
        smuggling_ring,
        rank_player,
        player_rank,
        player_score,
        rank_team,
        team_rank,
        team_score,
        unit_created_count,
        unit_class_created_count,
        building_class_created_count,
        project_created_count,
        force_civic_count,
        vote_outcome,
        religion_game_turn_founded,
        corporation_game_turn_founded,
        secretary_general_timer,
        vote_timer,
        diplo_vote,
        special_unit_valid,
        special_building_valid,
        religion_slot_taken,
        holy_city,
        headquarters,
        destroyed_cities,
        great_people_born,
        vote_selections,
        votes_triggered,
        next_vote_selection_id,
        next_vote_triggered_id,
        plot_extra_yields,
        plot_extra_costs,
        vote_source_religions,
        inactive_triggers,
        shrine_building_count,
        shrine_building,
        shrine_religion,
        num_culture_victory_cities,
        culture_victory_culture_level,
        trophies,
        map_script: _,
        path_finder,
        step_finder,
        route_finder,
        border_finder,
        area_finder,
        plot_group_finder,
    } = g;
    setup.write(w);
    map.write(w);
    players.write(w);
    teams.write(w);
    units.write(w);
    cities.write(w);
    groups.write(w);
    plot_groups.write(w);
    deals.write(w);
    map_rand.write(w);
    soren_rand.write(w);
    log.write(w);
    elapsed_game_turns.write(w);
    start_turn.write(w);
    start_year.write(w);
    estimate_end_turn.write(w);
    turn_slice.write(w);
    cutoff_slice.write(w);
    num_game_turn_active.write(w);
    num_cities.write(w);
    total_population.write(w);
    trade_routes.write(w);
    free_trade_count.write(w);
    no_nukes_count.write(w);
    nukes_exploded.write(w);
    max_population.write(w);
    max_land.write(w);
    max_tech.write(w);
    max_wonders.write(w);
    init_population.write(w);
    init_land.write(w);
    init_tech.write(w);
    init_wonders.write(w);
    ai_auto_play.write(w);
    autorun.write(w);
    active_player.write(w);
    score_dirty.write(w);
    circumnavigated.write(w);
    final_initialized.write(w);
    nukes_valid.write(w);
    handicap.write(w);
    best_land_unit.write(w);
    winner.write(w);
    victory.write(w);
    game_state.write(w);
    script_data.write(w);
    crime.write(w);
    cut_losers_counter.write(w);
    flexible_difficulty_counter.write(w);
    global_counter.write(w);
    global_counter_limit.write(w);
    high_to_low_counter.write(w);
    increasing_difficulty_counter.write(w);
    max_global_counter.write(w);
    scenario_counter.write(w);
    event_triggered.write(w);
    gambling_ring.write(w);
    no_bonus.write(w);
    no_outside_tech_trades.write(w);
    slave_trade.write(w);
    smuggling_ring.write(w);
    rank_player.write(w);
    player_rank.write(w);
    player_score.write(w);
    rank_team.write(w);
    team_rank.write(w);
    team_score.write(w);
    unit_created_count.write(w);
    unit_class_created_count.write(w);
    building_class_created_count.write(w);
    project_created_count.write(w);
    force_civic_count.write(w);
    vote_outcome.write(w);
    religion_game_turn_founded.write(w);
    corporation_game_turn_founded.write(w);
    secretary_general_timer.write(w);
    vote_timer.write(w);
    diplo_vote.write(w);
    special_unit_valid.write(w);
    special_building_valid.write(w);
    religion_slot_taken.write(w);
    holy_city.write(w);
    headquarters.write(w);
    destroyed_cities.write(w);
    great_people_born.write(w);
    vote_selections.write(w);
    votes_triggered.write(w);
    next_vote_selection_id.write(w);
    next_vote_triggered_id.write(w);
    plot_extra_yields.write(w);
    plot_extra_costs.write(w);
    vote_source_religions.write(w);
    inactive_triggers.write(w);
    shrine_building_count.write(w);
    shrine_building.write(w);
    shrine_religion.write(w);
    num_culture_victory_cities.write(w);
    culture_victory_culture_level.write(w);
    trophies.write(w);
    path_finder.write(w);
    step_finder.write(w);
    route_finder.write(w);
    border_finder.write(w);
    area_finder.write(w);
    plot_group_finder.write(w);
}

/// Reads the fields `write_game` wrote, in the same order (struct literal fields are evaluated
/// in the order written).
fn read_game(d: &'static GameData, r: &mut Reader<'_>) -> Result<Game, LoadError> {
    let mut g = Game {
        d,
        setup: Persist::read(r)?,
        map: Persist::read(r)?,
        players: Persist::read(r)?,
        teams: Persist::read(r)?,
        units: Persist::read(r)?,
        cities: Persist::read(r)?,
        groups: Persist::read(r)?,
        plot_groups: Persist::read(r)?,
        deals: Persist::read(r)?,
        map_rand: Persist::read(r)?,
        soren_rand: Persist::read(r)?,
        log: Persist::read(r)?,
        controllers: IdxVec::new(), // sized below
        elapsed_game_turns: Persist::read(r)?,
        start_turn: Persist::read(r)?,
        start_year: Persist::read(r)?,
        estimate_end_turn: Persist::read(r)?,
        turn_slice: Persist::read(r)?,
        cutoff_slice: Persist::read(r)?,
        num_game_turn_active: Persist::read(r)?,
        num_cities: Persist::read(r)?,
        total_population: Persist::read(r)?,
        trade_routes: Persist::read(r)?,
        free_trade_count: Persist::read(r)?,
        no_nukes_count: Persist::read(r)?,
        nukes_exploded: Persist::read(r)?,
        max_population: Persist::read(r)?,
        max_land: Persist::read(r)?,
        max_tech: Persist::read(r)?,
        max_wonders: Persist::read(r)?,
        init_population: Persist::read(r)?,
        init_land: Persist::read(r)?,
        init_tech: Persist::read(r)?,
        init_wonders: Persist::read(r)?,
        ai_auto_play: Persist::read(r)?,
        autorun: Persist::read(r)?,
        active_player: Persist::read(r)?,
        score_dirty: Persist::read(r)?,
        circumnavigated: Persist::read(r)?,
        final_initialized: Persist::read(r)?,
        nukes_valid: Persist::read(r)?,
        handicap: Persist::read(r)?,
        best_land_unit: Persist::read(r)?,
        winner: Persist::read(r)?,
        victory: Persist::read(r)?,
        game_state: Persist::read(r)?,
        script_data: Persist::read(r)?,
        crime: Persist::read(r)?,
        cut_losers_counter: Persist::read(r)?,
        flexible_difficulty_counter: Persist::read(r)?,
        global_counter: Persist::read(r)?,
        global_counter_limit: Persist::read(r)?,
        high_to_low_counter: Persist::read(r)?,
        increasing_difficulty_counter: Persist::read(r)?,
        max_global_counter: Persist::read(r)?,
        scenario_counter: Persist::read(r)?,
        event_triggered: Persist::read(r)?,
        gambling_ring: Persist::read(r)?,
        no_bonus: Persist::read(r)?,
        no_outside_tech_trades: Persist::read(r)?,
        slave_trade: Persist::read(r)?,
        smuggling_ring: Persist::read(r)?,
        rank_player: Persist::read(r)?,
        player_rank: Persist::read(r)?,
        player_score: Persist::read(r)?,
        rank_team: Persist::read(r)?,
        team_rank: Persist::read(r)?,
        team_score: Persist::read(r)?,
        unit_created_count: Persist::read(r)?,
        unit_class_created_count: Persist::read(r)?,
        building_class_created_count: Persist::read(r)?,
        project_created_count: Persist::read(r)?,
        force_civic_count: Persist::read(r)?,
        vote_outcome: Persist::read(r)?,
        religion_game_turn_founded: Persist::read(r)?,
        corporation_game_turn_founded: Persist::read(r)?,
        secretary_general_timer: Persist::read(r)?,
        vote_timer: Persist::read(r)?,
        diplo_vote: Persist::read(r)?,
        special_unit_valid: Persist::read(r)?,
        special_building_valid: Persist::read(r)?,
        religion_slot_taken: Persist::read(r)?,
        holy_city: Persist::read(r)?,
        headquarters: Persist::read(r)?,
        destroyed_cities: Persist::read(r)?,
        great_people_born: Persist::read(r)?,
        vote_selections: Persist::read(r)?,
        votes_triggered: Persist::read(r)?,
        next_vote_selection_id: Persist::read(r)?,
        next_vote_triggered_id: Persist::read(r)?,
        plot_extra_yields: Persist::read(r)?,
        plot_extra_costs: Persist::read(r)?,
        vote_source_religions: Persist::read(r)?,
        inactive_triggers: Persist::read(r)?,
        shrine_building_count: Persist::read(r)?,
        shrine_building: Persist::read(r)?,
        shrine_religion: Persist::read(r)?,
        num_culture_victory_cities: Persist::read(r)?,
        culture_victory_culture_level: Persist::read(r)?,
        trophies: Persist::read(r)?,
        map_script: default_map_script(),
        path_finder: Persist::read(r)?,
        step_finder: Persist::read(r)?,
        route_finder: Persist::read(r)?,
        border_finder: Persist::read(r)?,
        area_finder: Persist::read(r)?,
        plot_group_finder: Persist::read(r)?,
    };
    if g.players.is_empty() || g.teams.len() != g.players.len() {
        return Err(LoadError::Corrupt(format!("{} players and {} teams", g.players.len(), g.teams.len())));
    }
    g.controllers = IdxVec::from_fn(g.num_players(), |_| null_controller());
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::GameSetup;

    fn small_game(seed: u32) -> Game {
        let d = ffh_data::global();
        let setup = GameSetup::standard(d, 3, "WORLDSIZE_DUEL", seed);
        let mut g = Game::new(d, setup);
        g.autorun = true;
        g.game_init();
        g
    }

    #[test]
    fn fresh_game_round_trips() {
        let g = small_game(5);
        let bytes = g.save();
        assert!(bytes.starts_with(MAGIC));
        let h = Game::load(g.d, &bytes).expect("load");
        assert_eq!(h.save(), bytes, "save(load(save(g))) == save(g)");
        assert_eq!(h.calculate_sync_checksum(), g.calculate_sync_checksum());
        assert_eq!(h.controllers.len(), h.num_players());
        assert_eq!(h.units.next_id(), g.units.next_id());
    }

    #[test]
    fn bad_input_is_rejected() {
        let g = small_game(6);
        let d = g.d;
        let bytes = g.save();
        assert_eq!(Game::load(d, b"nope").err(), Some(LoadError::NotASave));
        let mut v = bytes.clone();
        v[8] = 99;
        assert!(matches!(Game::load(d, &v), Err(LoadError::UnsupportedVersion { found: 99, .. })));
        assert_eq!(Game::load(d, &bytes[..bytes.len() - 1]).err(), Some(LoadError::Truncated));
        let mut v = bytes.clone();
        v.push(0);
        assert_eq!(Game::load(d, &v).err(), Some(LoadError::TrailingData(1)));
        // Every truncation fails cleanly (no panic).
        for n in (0..bytes.len()).step_by(997) {
            assert!(Game::load(d, &bytes[..n]).is_err());
        }
    }

    #[test]
    fn data_mismatch_is_rejected() {
        let g = small_game(7);
        let mut w = Writer::new();
        w.bytes(MAGIC);
        w.bytes(&FORMAT_VERSION.to_le_bytes());
        let mut fp = data_fingerprint(g.d);
        fp[3].1 += 1;
        fp.write(&mut w);
        write_game(&g, &mut w);
        match Game::load(g.d, &w.into_bytes()) {
            Err(LoadError::DataMismatch(s)) => assert!(s.contains(&fp[3].0), "{s}"),
            other => panic!("expected a data mismatch, got {:?}", other.err()),
        }
    }
}
