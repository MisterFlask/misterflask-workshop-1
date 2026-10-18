# Handoff (state at the end of session `session_015m21G1a13FKUWE1YyaXREp`, 2026-10-15)

Start here, then read `CLAUDE.md` (conventions), `README.md`, and the doc for the area you
touch (`docs/PORTING.md`, `docs/AI.md`, `docs/PLAY_DESIGN.md` + `docs/HUMAN.md`, `docs/ART.md`,
`docs/SCORE.md`).

**Direction:** FFH2 0.41 is the starting point, not an invariant. Equivalence to FFH2 is
temporary: once the base is stable and attractive, the game will change sweepingly. Rules
changes are made deliberately and listed in `docs/PORTING.md` ("Changes from FFH2").

## Update, session `session_01RVFohb7FefbibSXFuVfcRN` (2026-10-18): AI round 4

On branch `ccr-875bd6ad-shu1bx` (not yet merged to master; it also carries the r3d renderer work
and the diplomacy/trade AI that master lacks). A diagnosis of the whole AI by dimension, each
candidate re-checked by a skeptic and verified on reserved seeds before merging:
- merged: random events chosen by value (`events.rs`), Conquest without its flat AIWeight, a first
  settler that cannot reach its site founds in place, Luchuirp Golden Hammers become Engineers,
  threatened workers actually flee, research seeks a buildable transport for settlers stuck in
  port; plus a behaviour-identical speed-up (`unit_update_bonus_affinity`, -15% time per game).
  Together on unused seeds: legacy score +8% at turn 300 (t 4.6), population +15%, power +5.5%.
- the cost and the next strategic gap: 6 of 32 full games now reach the time limit between two or
  three near-equal powers (0 before); no single change causes it (docs/AI.md, "Round 4").
- measured and not kept (docs/AI.md "Looked promising on development seeds, failed on fresh
  ones", "Workers", the Conquest section): promoting every stack member, valuing Lighthouse/
  Harbor/free-resource buildings, reachable worker build targets, no early idle workers, two
  Conquest variants.
- tooling: `crates/ffh-cli/scripts/ab_compare.py` (paired per-seed t over two `ai_metrics.py`
  results; handles full games stopped early). The shared build cache keeps 12 outputs: long A/B
  queues lose their binaries to garbage collection unless rebuilt (`build-tree.sh` is cheap).
- not touched, worth a look: council votes and elections (every AI abstains; an AI secretary never
  picks a resolution, so the Overcouncil/Undercouncil are inert); pathfinding is ~30% of late-game
  time.

## What is on master (merged, tested)

`master` == `ccr-17ac9b54-lbgjpn` (the integration branch used so far). Everything below passes
`cargo build --release && cargo test --release` (0 warnings) and `bash tools/web-check.sh --smoke --browser`.

- **Rules engine (`ffh-core`, `ffh-data`)**: complete port of the FFH2 0.41 DLL + rules Python, audited
  against the C++/Python; no `todo!()` left. Changes from FFH2 are listed in `docs/PORTING.md`.
  Stress-tested over ~1,400 games across all settings (`crates/ffh-stress`). Save/load is exact
  (`Game::save`/`Game::load`; resumed games are byte-identical).
- **Score v2** (user decision; a change from FFH2): a civ's score is its share of the world's people,
  land and knowledge in basis points (sums to 10000), see `docs/SCORE.md`. FFH2's formula survives as
  `legacy_score` for tooling. **For AI A/B tests use `legacy_score`**, techs, cities, research, etc.:
  the share score averages 10000/N whatever the AI does.
- **AI (`ffh-ai`)**: new design (not a port of the C++ AI; the user agreed porting the C++ AI is not
  worth it). Rounds 1-3 merged: finance/bankruptcy guard, expansion with escorts, naval settling,
  siege campaigns, civ-specific play (Kuriotates settlements, Lanun coves), advanced start, etc.
  `docs/AI.md` records every measured change and tried-and-not-kept variants.
- **Human play (`ffh-play`)**: session core faithful to the C++ human-input paths, JSONL agent
  transport, HTTP server and a vanilla-JS browser client (map, units, city screen, tech tree, civics,
  religion, diplomacy/trade, popups, save/load, spectator, debug tools). Run:
  `cargo build --release && ./target/release/ffh-play serve`, open http://127.0.0.1:8642/.
  `docs/HUMAN.md` is the tester guide. The client on master still draws **flat placeholder colours**
  (the art client is on a handoff branch, below).
- **Art**: original pixel art, "cheerful readability" (the user's style reference
  `docs/art/style-reference-map.png` and unit atlases `docs/art/unit-atlas-*.png`; it replaced the
  first "Monastic Pixel Noir" pass): style guide `docs/ART.md`, 1013 sprites + toolkit under
  `crates/ffh-play/web/art` (`tools/check.sh`; the Python authoring pipeline in `tools/atlas/`).
- **Runner tooling**: `ffh autoplay|bench|stats|inspect` (`--until-supremacy` stops a game once it is
  decided: victory, last civ standing, or 1.5x the runner-up's score held 30 turns from turn 200),
  `crates/ffh-cli/scripts/ai_metrics.py` (multi-seed metrics, `--compare`).

## Unfinished work: branches pushed as `origin/handoff/*`

All are based on master (or one commit behind it) and **none is merged**. "Built" = implemented and
measured by its author; "unverified" = the skeptical re-measurement on fresh seeds / the two-lens
review did not run yet. Process used so far for every change: implement on a branch → independent
re-measurement (AI) or two-lens review (everything else) → fix → integrate → full tests → merge to
master (the user asked to merge finished work into master as it lands).

### AI round 3b (`handoff/ai3b-*`) — built, unverified
| branch | claim by its author (6 civs, standard map, 300 turns unless noted) |
|---|---|
| `ai3b-beelines-fixed` | goal-directed research with round 3's religion-order bug fixed: on 72 seeds techs 21.5→28.7, research 85→144, legacy score 874→1090, religions founded/game 1.2→3.7, fewer cities lost. Biggest gain measured so far. |
| `ai3b-worker-safety` | workers flee danger: barbarian worker captures ~125→~5.5 per game, economy neutral-to-up, CPU −5%. Also fixes `ffh stats` (adds `captured_or_razed_by_barb`, `workers_captured_by_barb`). |
| `ai3b-siege-estimate` | exact strike odds over defender health states (`odds.rs`) replace the over-optimistic expected-value sim: strike success 82→88%, units lost in failed strikes 116→51 (96 seeds); economy flat. |
| `ai3b-kuriotates-tuning` | Kuriotates settlements without the research collapse: on 80 Kuriotates games legacy score +54 vs current (t 2.3), research +30 (t 6.0). |
| `ai3b-victory-pursuit` | victory planner (`victory.rs`): full 690-turn games won before the time limit 5/24→19/24 (mostly cultural), victory ~50 turns earlier; research −20% late (culture slider). Tower of Mastery found unreachable — see its docs. Includes planner tests. |
Next: verify each on fresh seeds (e.g. 601-648; compare against master), then integrate (expect
conflicts in `docs/AI.md`, `lib.rs` mod list, `production.rs`), measure the combination with
leave-one-out, merge.

### Play polish (`handoff/polish-*`)
| branch | state |
|---|---|
| `polish-client-art` | built, unreviewed: the browser client draws with the pixel art (art embedded via build.rs; flat renderer kept as fallback); node tests (45) and Chromium smoke (72 steps) pass per its author. |
| `polish-art-pass2` | built, unreviewed: light and ground pass (warp octave + Bayer transition bands, moonlit clearings, grey remembered snow, lit towns; ART.md budget revised deliberately). |
| `polish-p7-hooks` | 3 real commits (controller hooks for human-only popups; automation of human groups; AI offers to humans) + a **WIP snapshot** (client/stage/smoke edits; may not build). See `docs/PLAY_DESIGN.md` §7 P7. |
| `polish-rules-fixes` | 4 real commits (Lyre FAssert is reachable in C++ too; `reassign_player_advanced` moves the active player; PlayerVote values are teams; `Text` reads grammatical forms like CvGameText) + a WIP doc edit. |
| (not started) text | vendor stock BtS/Civ4 English text FfH2 inherits (e.g. TXT_KEY_UNIT_WARRIOR is missing; names fall back to humanized type names) from `/home/user/rigiechang/broken-bts` (fetch via `tools/fetch-reference.sh`) and implement Civ4 message formatting (`%s1`, `%s2_city`, `[COLOR_*]`...): the message log shows raw placeholders. |
Next: review each (lens A: faithfulness/look vs mood board; lens B: tests, browser smoke, robustness),
fix, integrate in the order client-art → art-pass2 → rules-fixes → p7-hooks → text, run all tests +
`bash tools/web-check.sh --smoke --browser` + a ~100-turn browser game, merge.

### Parallel-agent infra (`handoff/agent-infra`) — 1 real commit + WIP snapshot
Shared build service for any git rev (one stable checkout + persistent target, so builds are incremental
across revs), deterministic run cache keyed by binary hash + args + env, machine-wide CPU slots,
sparse agent worktrees (a post-checkout hook) and a worktree prune script; docs `docs/AGENTS.md`.
Unreviewed; the WIP touches the repo-root `CLAUDE.md` (check that edit). Approved by the user, plus:
**fan-out of game batches to sibling cloud sessions** (user approved; not built yet): an agent pushes
the commit to a temporary `fanout/` branch, starts ≤4 worker sessions (`create_session`, each a
fresh container) that build and run a seed slice and push results to `fanout/res/<job>/<slice>`,
collects them into the run cache, deletes the temporary branches and archives the workers. Verify
cross-container determinism (same seed → same checksum) first.

## Facts worth knowing
- Time in AI rounds: ~60% running deterministic games, ~18% builds/tests, ~21% LLM work. Measurement
  needs fresh seeds: two round-3 items looked good on their authors' seeds and failed on fresh ones.
- Tests that rely on a seed's story (`crates/ffh-play/tests/*`: bot survives, captures a city) break
  when the AI gets stronger; their comments explain how to re-pick a seed (`ffh-play bot --seed S`).
- 4 cores: run at most ~2-3 heavy agents at once; prune finished agent worktrees (each ~200 MB, mostly
  `projects/ffh-simplified`'s committed node_modules) or the disk fills.
- Reference sources (read-only, not in the repo): `tools/fetch-reference.sh` → `/home/user/ref/ffh2`
  (DLL C++, Python, XML) and `/home/user/ref/bts`.
- Out of scope for now (user): scenarios (world-builder saves + ScenarioFunctions.py); porting the C++
  AI. In scope: the sandbox game, AI quality (incl. pursuing victory conditions), human play, art.
