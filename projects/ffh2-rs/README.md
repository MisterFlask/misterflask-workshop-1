# ffh2-rs

A Rust port of **Fall from Heaven 2** (0.41), the Civilization IV: Beyond the Sword mod:
the rules engine is ported function by function from the mod's C++ DLL and rules Python;
the AI is new. Base mod only; no UI (headless runner).

## Why

- **Speed.** FFH2's late game spends most of its turn time in the C++ AI (repeated path
  searches and per-unit plot scans) and in Python callbacks. Here the rules are compiled
  Rust, the Python is Rust, and the AI is designed for bounded cost per turn.
- **A stable base for changes.** One language, typed data tables, deterministic simulation
  (same seed = same game), and tests.

## Layout

| Path | What |
|---|---|
| `data/xml`, `data/text` | FFH2 0.41 gameplay XML (+ the stock BtS files it inherits), English text |
| `crates/ffh-data` | XML -> typed info tables (port of `CvInfos` / `CvXMLLoadUtility`) |
| `crates/ffh-core` | rules engine: CvGame, CvPlayer, CvTeam, CvCity, CvUnit, CvPlot, CvSelectionGroup, CvMap, map generation, deals, and the rules Python (`src/ffh/`: spells, events, random events, custom functions, CvGameUtils hooks) |
| `crates/ffh-ai` | the new AI (implements `ffh_core::Controller`) |
| `crates/ffh-cli` | `ffh` binary: data checks, AI-only autoplay, benchmarks |
| `crates/ffh-stress` | `ffh-stress` binary: AI games over a matrix of game settings (speed, handicap, era, world size, civs, climate, sea level, options, teams, victories), crashes caught per game |
| `crates/ffh-play` | `ffh-play` binary and library: human play (session core, browser and JSONL agent front-ends; in progress, see `docs/HUMAN.md`) |
| `docs/` | `PORTING.md` (status, deliberate deviations), `AI.md`, `ARCHITECTURE.md`, `DATA.md`, `porting/*.md` (C++ function maps) |
| `tools/` | reference fetch, XML vendoring, enum/define generators, C++ parity harness |

## Build and run

Rust stable, no external crates.

```
cargo build --release
cargo test --release
./target/release/ffh data                                  # load all XML, report warnings
./target/release/ffh autoplay --seed 1 --civs 4 --size WORLDSIZE_SMALL --turns 100
./target/release/ffh bench --seed 1 --civs 8 --size WORLDSIZE_STANDARD --turns 300
```

Stress runs (each game in a child process; panics, aborts and hangs are reported with the
settings to reproduce them; a checked build adds debug assertions and overflow checks):

```
./target/release/ffh-stress batch --matrix all --seeds 1,2 --turns 400 --out results.tsv
./target/release/ffh-stress run --seed 3 --civs 12 --size WORLDSIZE_TINY --speed GAMESPEED_QUICK --options GAMEOPTION_RAGING_BARBARIANS
CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true \
  cargo build --release --target-dir target/checked -p ffh-stress
```

`autoplay` prints one line per turn (time, units, cities, sync checksum) and a final
per-player summary; `bench` prints only the timing summary.

The human-only rules branches get the same treatment from `ffh-play soak`: the AI plays the
human slot through the human-play session (`docs/HUMAN.md`):

```
./target/release/ffh-play soak --jobs 4 --out soak.tsv          # 144 games: 2 seeds x 3 sizes x 6 civs x 4 option sets
./target/release/ffh-play soak run --seed 2 --size tiny --civ kuriotates --options challenges
```

## Faithfulness

Rules code follows the C++ line by line, including its quirks (marked `// C++ quirk:`), with
the same random-number calls in the same order (the C++ `CvRandom` and `CvFractal` are
verified bit-exact against the original source). The rules Python is ported the same way.
What is deliberately different is listed in `docs/PORTING.md`; the main items are the AI
(new), the exe's closed-source A* (re-implemented over the DLL's own callbacks) and the
Python map scripts (a Fractal generator ships instead).

## Status

The rules port is complete (no `todo!()` left in `ffh-core`) and audited line by line against
the C++/Python, and stress-tested with about 1,400 AI games over all game settings. AI-only games
play end to end; the AI itself is the main open work (`docs/AI.md`). Details, deliberate
deviations and next steps: `docs/PORTING.md`.
