# Architecture

## Goals

1. Rules-faithful port of FFH2 0.41 (base mod): same formulas, same data, same
   order of effects as the C++ DLL and the rules Python.
2. Much faster than the original: native code, no Python callbacks, 64-bit
   (no memory-allocation crashes), cache-friendly state, parallel AI analysis.
3. A stable base for further changes: plain-data state, explicit module
   boundaries, deterministic simulation, tests.

## Crates

```
ffh-data   XML -> typed info tables (GameData). No game state.
ffh-core   Rules engine. Owns all mutable game state (Game). Deterministic.
ffh-ai     Controllers (the new AI). Reads state, issues the same calls a human UI would.
ffh-cli    `ffh` binary: autoplay, benchmarks, inspection.
ffh-stress `ffh-stress` binary: AI games over a matrix of game settings, crashes caught per game.
```

`ffh-core` must stay free of OS dependencies (no filesystem/threads/clock in the
simulation) so it compiles to WebAssembly unchanged; `ffh-data` reads files only in
its loader entry point.

## State model

C++ is an object graph reached through globals (`GC`, `GET_PLAYER`, `GET_TEAM`).
Rust uses one owning root, `Game`, with id-keyed storage:

- `players`, `teams`: `IdxVec` sized to the players in this game (FFH2's DLL reserves
  35 civ slots; arrays here are sized to the actual game).
- `units`, `cities`, `groups`, `plot_groups`, `deals`: `Slab`s keyed by
  never-reused ids. Owners keep ordered id lists (`Player::units`, ...).
- `map`: plots in a flat `Vec` (index `y * width + x`), areas in a `Slab`.
- `d: &'static GameData`: immutable data, loaded once per process.

C++ methods become `impl Game` methods prefixed by class (`unit_can_move_into(u, ..)`),
so any rules function can touch any object through `&mut self` without aliasing
problems, and the C++ call graph maps one-to-one onto Rust.

## Randomness and determinism

All game randomness goes through `Game::soren_rand` (C++ `m_sorenRand`), map
generation through `Game::map_rand` -- both bit-exact ports of `CvRandom`
(verified against the compiled original, `tools/parity`). Simulation code never
iterates hash maps, reads clocks, or depends on thread scheduling, so a seed fully
determines a game.

## Rules / AI boundary

The C++ rules code calls `AI_*` functions synchronously. In this port:

- decisions go through the `Controller` trait (`controller.rs`): `raze_conquered_city`,
  `choose_event`, `choose_research`, `diplo_vote`, ... with deterministic defaults;
- AI bookkeeping that rules code reads (memory, war plans, contact counters, unit AI
  types) is plain state on the owning object;
- citizen assignment is a new governor (`governor.rs`) used by every player;
- the AI proper (`ffh-ai`) plays turns via `Controller::play_turn`, using only public
  rules APIs (the same moves a player could make).

## Saved games

`ffh-core/src/save/` (replaces C++ `read`/`write`): `Game::save(&self) -> Vec<u8>` and
`Game::load(d, &bytes) -> Result<Game, LoadError>`. std-only binary format: magic `FFH2SAVE`,
format version, a fingerprint of the static data (type count + hash of the type names of every
info table, plus the fixed array lengths; a save made with other XML is refused), then every
`Game` field in declaration order (varint integers, length-prefixed strings/vectors, tagged
options; slabs keep their ids and next id). Encoding details: `save/io.rs`.

- Everything mutable is saved, rules caches included (`Player::ai_bonus_value`, city rank
  caches, the path finders' kept search trees), so a loaded game needs no recomputation.
- Not saved: `d`, `controllers` (a loaded game has `NullController`s: the caller re-attaches
  its controllers, as after `Game::new`) and `map_script` (only used by `game_init`).
- Resume guarantee: a game saved at a turn boundary (`Game::at_turn_boundary()`: no player's
  turn active, so the next `update()` starts the next game turn) or right after `game_init`,
  loaded with fresh controllers and continued, plays out exactly like the uninterrupted game
  (`crates/ffh-ai/tests/save_resume.rs` checks sync checksums and byte-identical saves every
  turn). This relies on controllers keeping no state that matters across turns: any cache a
  controller keeps outside `Game` must be keyed by game turn (or otherwise recomputed) so that
  a fresh controller decides the same way. Mid-turn saves restore the rules state exactly but
  not the controllers' memory of what they already did this turn.
- Every saved struct is written by exhaustive destructuring, so adding a field fails to compile
  until the save code lists it (bump `save::FORMAT_VERSION` when the layout changes).

## Python

FFH2 implements much of its content in Python (spells, events, per-turn effects,
lairs, rituals). These are ported to Rust under `ffh-core/src/ffh/`, dispatched by
the callback names stored in XML (`PyResult`, `PyRequirement`, `PythonOnMove`, ...),
resolved once at startup instead of `eval`-ing strings per call.

## Verification

- Data: integration tests spot-check loaded values against the XML; loading must
  produce zero warnings (unknown types, enum-order mismatches).
- Parity: `tools/parity` compiles self-contained DLL sources (CvRandom, CvFractal)
  unmodified against stubs and generates fixtures the Rust tests compare against.
- Simulation: autoplay soak tests (no panics, invariants hold), determinism tests
  (same seed -> same final state hash), and benchmarks (turn time at late game).
- Stress: `ffh-stress` plays AI games over the settings matrix (also on a build with debug
  assertions and overflow checks) and cross-checks city/unit/group/area/team bookkeeping.
