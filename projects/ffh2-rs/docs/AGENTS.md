# Parallel agents: building, comparing, running games, cleaning up

Several agents work on this repository at once, each in its own git worktree, on one machine
(4 cores, 15 GB RAM, ~40 GB disk). `tools/agents/` (std-only Python and sh) lets them share
work instead of repeating it. The shared state lives outside the repository in
`/home/user/.cache/ffh2-rs` (`$FFH2_CACHE`): it survives container restarts and worktree removal,
and limits its own disk use.

| Without | With `tools/agents/` |
|---|---|
| every worktree builds the workspace from scratch into its own `target/` (cargo keys builds by absolute path, so nothing is shared) | `build-rev.sh` / `build-tree.sh` build in a few shared workspaces, incrementally, and keep the outputs by source content |
| an A/B test replays the base side's games that other agents already played | `ai_metrics.py` goes through a run cache: same binary + data + arguments = stored result |
| each agent runs as many games as it likes; with 3-6 agents the load reaches 10-16 on 4 cores | every game holds one of `nproc` machine-wide CPU slots |
| a worktree is a full 200 MB checkout (170 MB of it `projects/ffh-simplified`) | new agent worktrees are sparse: ~30 MB |

Run the commands from your worktree (paths below are relative to `projects/ffh2-rs`).

## Build

```
B=$(tools/agents/build-rev.sh <git-rev>)              # e.g. HEAD, master, a sha; prints a directory
W=$(tools/agents/build-tree.sh)                       # your worktree's files as they are now
$W/ffh stats --seed 1 ...                             # $B/ffh, $B/ffh-stress, $B/ffh-play
tools/agents/build-tree.sh --bins ffh                 # only `ffh` (3x faster after an ffh-ai change)
tools/agents/build-tree.sh --test                     # also `cargo test --release` on those files
tools/agents/build-rev.sh <rev> --profile checked     # release + debug assertions + overflow checks
```

- `build-tree.sh` takes the worktree's files exactly as git sees them (tracked files as on disk,
  untracked files that are not ignored), so it builds uncommitted work. `build-tree.sh --test`
  runs the same `cargo build --release && cargo test --release` the commit rule asks for, on
  those files; once they passed for an exact set of files they are not rerun (`--force-test`).
  Arguments after `--` go to `cargo test` (e.g. `-- -p ffh-ai`).
- An output is keyed by the content of `projects/ffh2-rs` (+ profile, toolchain): asking again for
  the same content, from any rev or worktree, returns at once. Outputs are immutable, so a batch
  can keep using one while you rebuild. The 12 most recently used per profile are kept (one used
  in the last 30 minutes or currently running is never deleted).
- Builds run one at a time, machine-wide (a waiting build says who holds the lock), in one of 3
  workspaces, each with persistent cargo target dirs: a build goes to the workspace where it
  recompiles least -- an ffh-ai change against the integration tip recompiles ffh-ai and the
  binaries only -- and a build that recompiles ffh-core anyway goes to the least recently used
  workspace, keeping the warm ones warm. Every build runs at the same path (the workspace is
  renamed there), so identical sources give byte-identical binaries.
- `ffh`, `ffh-stress`, `ffh-play` in the output are small wrappers: they run `bin/<name>` with
  `FFH_DATA_DIR` pointing at the output's own copy of `data/`. A cargo-built binary otherwise
  loads `data/` from the source directory it was built in (compiled in), which for the shared
  workspaces holds whatever was built last. So use the wrappers (copying or symlinking a wrapper
  is fine); if you run `bin/ffh` directly, set `FFH_DATA_DIR=$B/data`.
- Do not build in your worktree unless you need a debug build or a tool cargo provides (clippy,
  `cargo run` of a test helper); a `target/` there is a private cold build (~6 min, ~1.4 GB with
  tests). If you do, `rm -rf target` when done.

## Compare against a base

```
tools/agents/ab.sh --seeds 1-12 --turns 300 --at 100,200,300 --jobs 4
tools/agents/ab.sh --base <rev> --new <rev-or-worktree-dir> --out $DIR <ai_metrics.py args..>
```

`ab.sh` builds the base (default: where your branch forked from master) and the new side (default:
this worktree, uncommitted files included) with `--bins ffh`, runs `ai_metrics.py` on both with the
same arguments and prints the base report, then the new one. The base side's games are usually
already in the run cache. Manually:

```
BASE=$(tools/agents/build-rev.sh master --bins ffh)
NEW=$(tools/agents/build-tree.sh --bins ffh)
python3 crates/ffh-cli/scripts/ai_metrics.py --bin $BASE/ffh --seeds 1-12 --out base.json
python3 crates/ffh-cli/scripts/ai_metrics.py --bin $NEW/ffh --seeds 1-12 --compare base.json
```

## Run games

- `ai_metrics.py` (when the cache dir exists) looks each game up in the run cache and takes a CPU
  slot for each game it plays; `--jobs` only bounds the threads. `--no-cache` replays nothing;
  `--no-slots` ignores the slots (do not, unless you need it).
- One deterministic command: `tools/agents/cached-run.sh [-v] $B/ffh stats --seed 3 ...`.
- The cache key is: the binary's sha256 (for a wrapper: its binary's) + the sha256 of the data
  files it loads + every argument + every `FFH_*` environment variable (`FFH_AI_TRACE`, ...) + the
  content of arguments naming files. Only runs that exit 0 are stored; stdin is /dev/null.
  Identical runs started at the same time are played once (the others wait for it).
- A cached result is byte-identical to the run that produced it. Fresh runs of the same game
  differ only in their timing fields (`setup_ms`, `ms_per_turn`, `slowest_ms` of `ffh stats`):
  cached games keep their original timings (the report says how many games came from the cache),
  so judge speed with `ffh bench`, not with cached metrics.
- Not cached: `bench`, `serve`, `agent`, anything with `--out` (writes a file). Copied binaries
  are fine (keyed by content); a binary whose data dir cannot be found runs uncached.

## CPU slots

The machine has `nproc` slots (`FFH2_SLOTS` or `config.json` to change). Anything that runs games
outside `ai_metrics.py` should hold slots:

```
tools/agents/slot.sh $B/ffh autoplay --seed 3 ...            # one slot
tools/agents/slot.sh -n 4 $B/ffh-stress batch --jobs 4 ...   # a batch with its own parallelism
tools/agents/slot.sh -n 4 $B/ffh-play soak --jobs 4 ...
tools/agents/slot.sh --exclusive $B/ffh bench --seed 1 ...   # all slots: timing benchmarks
```

Waiting requests queue in order, so `--exclusive` waits for the running games to finish and new
ones wait for it. Commands under `slot.sh` do not take more slots (nested `slot.sh`, or
`ai_metrics.py` run under it). Builds do not take slots, and agents that bypass the slots still
load the machine: `--exclusive` makes a benchmark quieter, not quiet.

## Worktrees

- `tools/agents/install-hooks.sh` (done once per repository; `--check`, `--uninstall`) installs a
  post-checkout hook: a worktree created under `.claude/worktrees/` becomes a sparse checkout of
  the root files, `.claude/` and `projects/ffh2-rs/` (~30 MB instead of ~200 MB). The main
  checkout and worktrees elsewhere are never touched; status, commits, merges and cherry-picks
  work as usual (changes outside the sparse set are applied without being checked out). To get
  the other projects: `git sparse-checkout disable`. To create a full worktree:
  `FFH2_FULL_CHECKOUT=1 git worktree add ...`.
- When you finish: `rm -rf projects/ffh2-rs/target` if you built there.
- `tools/agents/prune-worktrees.sh [--dry-run] [--idle-minutes 90]` removes worktrees under
  `.claude/worktrees/` that are clean (`git status` empty), idle (no file, HEAD or reflog change
  for 90 minutes; `target/` ignored), whose HEAD is on a branch (the branch stays), not locked
  (the harness locks the worktrees of running agents) and not used by any process (cwd, open
  files, arguments). It uses `git worktree remove`, never `--force`, and also deletes the cargo
  `target/` dirs of every idle worktree. `--unlock-stale` also takes worktrees whose lock names a
  process that no longer exists (left by a previous container). It is for whoever manages the
  session; from an agent task run it with `--dry-run` only.

## The cache

```
tools/agents/cache-status.sh     # disk use, workspaces, outputs, run cache hits, slots, build lock
tools/agents/cache-gc.sh         # enforce the limits now (also runs after every build)
```

Limits (defaults; override in `/home/user/.cache/ffh2-rs/config.json`, e.g.
`{"workspaces": 2, "max_total_gb": 8}`): `workspaces` 3, `keep_outputs` 12 per profile,
`runs_max_mb` 512 and `runs_max_days` 30 for the run cache, `max_total_gb` 12 for everything
(above it the target dirs of the least recently used workspaces go first), `slots` 0 (= nproc).
Locks are `flock`s, released when their process dies; a workspace left mid-build by a killed
build is put back by the next one. Deleting the whole directory is always safe when nothing runs.

## Fan-out to sibling cloud sessions

For a large or long batch (many seeds, or `--until-supremacy` full games), split the games across
up to 4 sibling cloud sessions, each a fresh 4-core container. `tools/agents/fanout.sh` does the
deterministic git and run-cache work; the agent drives the sessions (the `create_session` /
`archive_session` tools are the agent's, not callable from a script). The run cache is the join
point: every game a worker plays is re-keyed into the orchestrator's **local** run cache, so once
the batch is collected a normal `ai_metrics.py --seeds ...` (or `ab.sh`) with the same binary and
arguments is all cache hits.

This relies only on the simulation being deterministic (same seed + same rules → same game,
verified below), not on builds being byte-identical across containers: `collect` stores each
worker's stdout under the key the local binary produces.

```
# 1. push the commit under test and slice the seeds (prints one command per worker)
tools/agents/fanout.sh prepare --rev HEAD --seeds 601-648 --workers 4 \
    --game-args --civs 6 --size WORLDSIZE_STANDARD --turns 300 --at 100,200,300
# 2. start one sibling session per worker (create_session, source_revision=fanout/<job>),
#    each running the printed fanout-worker.sh command (build its slice, push its results)
# 3. watch them, then import every game into the local run cache:
tools/agents/fanout.sh status  --job <job>
B=$(tools/agents/build-rev.sh <rev> --bins ffh)
tools/agents/fanout.sh collect --job <job> --bin $B/ffh      # missing seeds are reported; run live
# 4. the metrics are now instant (all hits), and clean up
python3 crates/ffh-cli/scripts/ai_metrics.py --bin $B/ffh --seeds 601-648 --civs 6 --turns 300 --compare base.json
tools/agents/fanout.sh cleanup --job <job>                   # deletes the branches; archive the sessions
```

(In this cloud environment the git proxy forbids branch deletion over https: `cleanup` reports
that it could not delete the `fanout/<job>` and `fanout/res/<job>/*` branches and clears only the
local job file. They are small and harmless under `fanout/`; remove them later from the GitHub UI
or with a token that has delete rights.)

A worker (`fanout-worker.sh`, run by each session) builds the checked-out commit with the shared
build service, plays its slice holding this container's CPU slots, and pushes each game's raw
stdout plus a manifest to `fanout/res/<job>/w<k>`. It is idempotent (rerun to fill in seeds that
failed). The orchestrator fetches those branches, re-keys each game locally, and reports which
seeds are still missing (run those live). Worth it when the games dominate: a worker costs a fresh
container (~1 min cold build) before it plays, so for a handful of short games, run them locally.

## Measured (this container: 4 cores, 15 GB RAM, single agent)

- **Build.** Cold full build of all three binaries (~100% of the Rust recompiled): ~56 s. The
  common AI-iteration case -- an `ffh-ai` change, `--bins ffh`, against a warm workspace holding
  the integration tip (rebuilds `ffh-ai` + `ffh-cli`, ~23% of the source): ~8.5 s. A change to
  only non-compiled files (docs, `tools/`, scripts): an incremental relink, ~0.1 s; identical
  content asked for again: instant (`up to date`). `ffh-play` is the slowest crate to rebuild, so
  `--bins ffh` is the default for AI work.
- **Byte-identical builds.** `master` and this branch differ only in non-compiled files, and the
  build service produced the same `ffh` (same exe-sha256), so the run cache treated their games as
  one: an A/B over the two replayed every game from the cache (`N of N games from the run cache`).
- **Run cache.** A 40-turn game stores in ~0.5 s and replays in ~0.06 s. A full 300-turn 6-civ
  standard game plays in ~8 s (~26 ms/turn) and replays in well under 0.1 s. So the base side of
  an A/B, already played by any agent, is effectively free.
- **CPU slots.** With `nproc` = 4, launching 8 games at once (each under `slot.sh`) runs 4 at a
  time; the rest queue, so the machine is not oversubscribed.
- **Determinism across containers.** Same seed → same game checksum in a sibling container: seeds
  201-208 (200 turns, 6 civs) played by two fan-out workers matched this container's checksums
  exactly (see `docs/AI.md` / commit log). This is what makes fan-out and the shared run cache
  sound.
