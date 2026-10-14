//! Port of CvDefines.h constants used by game rules.

/// C++ MOVE_* flags (pathfinding / mission flags).
pub const MOVE_IGNORE_DANGER: i32 = 0x0000_0001;
pub const MOVE_SAFE_TERRITORY: i32 = 0x0000_0002;
pub const MOVE_NO_ENEMY_TERRITORY: i32 = 0x0000_0004;
pub const MOVE_DECLARE_WAR: i32 = 0x0000_0008;
pub const MOVE_DIRECT_ATTACK: i32 = 0x0000_0010;
pub const MOVE_THROUGH_ENEMY: i32 = 0x0000_0020;
pub const MOVE_MAX_MOVES: i32 = 0x0000_0040;

/// C++ RANDPLOT_* flags (`CvMap::syncRandPlot`).
pub const RANDPLOT_LAND: i32 = 0x0000_0001;
pub const RANDPLOT_UNOWNED: i32 = 0x0000_0002;
pub const RANDPLOT_ADJACENT_UNOWNED: i32 = 0x0000_0004;
pub const RANDPLOT_ADJACENT_LAND: i32 = 0x0000_0008;
pub const RANDPLOT_PASSIBLE: i32 = 0x0000_0010;
pub const RANDPLOT_NOT_VISIBLE_TO_CIV: i32 = 0x0000_0020;
pub const RANDPLOT_NOT_CITY: i32 = 0x0000_0040;

/// C++ `MAX_CIV_PLAYERS` in FFH2 0.41 is 35. This port sizes player arrays by the
/// number of players actually in the game (see `Game::num_players`); this
/// constant is only the upper bound for game setup.
pub const MAX_CIV_PLAYERS: usize = 35;

/// Score v2 (docs/SCORE.md): the scores of the alive civ players always sum to this many basis
/// points (10000 = the whole world; shown as a percentage with one decimal).
pub const SCORE_TOTAL_BP: i32 = 10_000;

/// C++ `INVALID_PLOT_COORD` (`-MAX_INT`).
pub const INVALID_PLOT_COORD: i32 = -i32::MAX;

pub const DIRECTION_RADIUS: i32 = 1;
pub const DIRECTION_DIAMETER: i32 = DIRECTION_RADIUS * 2 + 1;
pub const NUM_CITY_PLOTS: usize = 37;
pub const CITY_HOME_PLOT: usize = 0;
pub const CITY_PLOTS_RADIUS: i32 = 3;
pub const CITY_PLOTS_DIAMETER: i32 = CITY_PLOTS_RADIUS * 2 + 1;
pub const MAX_VOTE_CHOICES: usize = 8;

/// C++ `MAX_INT`.
pub const MAX_INT: i32 = i32::MAX;
/// C++ `MIN_INT`.
pub const MIN_INT: i32 = i32::MIN;

/// FFH2 group flags (Sephi's BETTER AI). Kept for AI bookkeeping parity.
pub const GROUPFLAG_NONE: i32 = 1;
