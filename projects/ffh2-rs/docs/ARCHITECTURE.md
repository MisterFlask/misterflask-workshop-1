# Architecture

## Goals

1. A known, working starting point: FFH2 0.41 (base mod), ported from the C++ DLL and the
   rules Python. Equivalence to FFH2 is temporary, not an invariant: once the base is stable
   and attractive, the rules will change sweepingly (goal 3).
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
ffh-play   Human play: a session that drives one human slot through ports of the C++ input paths.
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

## Human play (ffh-play)

`crates/ffh-play` (design: `docs/PLAY_DESIGN.md`, use: `docs/HUMAN.md`) adds a human player
without changing the rules engine:

- **Game thread.** One `Session` owns the `Game` and drives it as the exe does: `advance` calls
  `update()` until the human's control point; `execute` applies a command between `update()`
  calls (ports of `CvNet*::Execute` and the exe helpers that send them), then lets the game
  settle. Front-ends talk to that thread; the game is never shared.
- **Decision policy.** The human slot's controller (`HumanController`) defers what C++ asks with
  a popup whose "no answer" the rules already handle (production, events, votes, research) and
  asks an `Answerer` synchronously where no answer would lose information (free tech, keep/raze,
  election, offer, religion). AI work C++ also runs for humans goes to a private advisor `Ai`.
- **Purity contract.** Every read of `&Game` outside a rules call goes through a `QueryGuard`,
  which restores the interior-mutable caches (path finders, `ai_bonus_value`, city rank caches)
  that `&self` rules functions fill, so reads never change later results or save bytes.
- **Saves.** A save is `Game::save()` bytes plus a JSON sidecar with the session state the rules
  need and `Game` lacks (the deferred event popups). Saves taken at a human control point resume
  exactly (the human plays slot 0, before any AI acts that turn; ffh-ai caches are per turn).
- **Journal.** One JSON line per input (command, decision answer) stamped with `(turn, slice)`,
  plus a checksum checkpoint per turn; `ffh-play replay` re-plays it and stops at the first
  difference.
- **The human's slot can move.** FfH's `reassignPlayerAdvanced` (Impersonate Leader, the
  "switch civilizations" event, the High to Low challenge) moves the human flag and controller to
  another slot, and the active player with it (ffh-core, as C++); the session follows after every
  rules call.
- **Soak.** `ffh-play soak` lets `ffh_ai::Ai` play the human slot through the session over a
  settings matrix (each game in a child process, ffh-stress's invariant checks every 5 turns), so
  the rules' human-only branches run before people play; failures print a repro command.
- **Views and queries.** Clients read the game only through protocol queries (`query.rs`,
  `view/`), answered under a `QueryGuard` from the human team's side: `view/fog.rs` holds the
  only visibility predicates (revealed and visible plots, invisible units, revealed cities, met
  players). Path previews use a session-owned interface path finder, never the game's. A turn
  report diffs a fog-filtered snapshot taken at the end of the human's turn against its next
  control point. Queries can also be served inside a blocking decision (`Answerer::ask_with`).
- **Transports.** One JSON envelope (`cmd`, `q`, `answer`, `session`) over two transports:
  `ffh-play agent` reads JSON lines from stdin on the game thread itself; `ffh-play serve` runs a
  std-only HTTP/1.1 server (127.0.0.1, a thread per connection) whose GET requests are served
  from a published status and cached view and map, and whose `POST /api` envelopes go over a
  channel to the game thread. A blocking decision raised inside a rules call reads the same
  input (the next lines, the same channel), so it is answered while the command that raised it
  waits. A panic on the game thread is caught and reported with the replay command.
- **Browser client.** `crates/ffh-play/assets/` (embedded with `include_str!`, served by
  `ffh-play serve`): vanilla ES modules and a `<canvas>` map, no build step and no external
  code. It only renders what the server computes (fog, legal actions, odds, paths, turns) and
  sends the same commands as the agent; the terrain is cached in offscreen chunks redrawn where
  the map changed. Checked by `tests/web.rs` (files and imports), and with node by
  `tools/web-check.sh`, `tools/web-api-smoke.mjs` and the headless-Chromium `tools/web-smoke.mjs`.

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
