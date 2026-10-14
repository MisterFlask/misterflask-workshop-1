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
