#!/bin/bash
# fanout-worker.sh --job J --worker K --result-branch BR --seeds 201-212 -- <ffh stats game args>
#
# Run by a sibling cloud session (docs/AGENTS.md "Fan-out"). Builds the checked-out commit, plays
# the given seed slice with `ffh stats <game args> --seed S`, and pushes the raw stdout of each
# game (one file per seed) plus a manifest to the result branch BR. The orchestrator
# (tools/agents/fanout.py collect) fetches BR and re-keys each game into its local run cache.
#
# Deterministic games mean the stdout a worker stores equals what the orchestrator's own binary
# would print (but for timing fields), so the games need not be rebuilt at home. Only successful
# (exit 0, one JSON line) games are kept. Idempotent: rerun to fill in seeds that failed.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
proj=$(cd "$here/../.." && pwd)                 # projects/ffh2-rs
cd "$proj"

job= worker= branch= seeds=
while [ $# -gt 0 ]; do
    case "$1" in
    --job) job=$2; shift 2 ;;
    --worker) worker=$2; shift 2 ;;
    --result-branch) branch=$2; shift 2 ;;
    --seeds) seeds=$2; shift 2 ;;
    --) shift; break ;;
    *) echo "fanout-worker: unexpected argument $1" >&2; exit 2 ;;
    esac
done
game_args=("$@")
[ -n "$job" ] && [ -n "$worker" ] && [ -n "$branch" ] && [ -n "$seeds" ] || {
    echo "usage: fanout-worker.sh --job J --worker K --result-branch BR --seeds A-B -- <ffh stats args>" >&2
    exit 2
}

expand_seeds() {                                # "201-204,207" -> lines 201 202 203 204 207
    local p a b
    IFS=',' read -ra parts <<<"$1"
    for p in "${parts[@]}"; do
        if [[ $p == *-* ]]; then a=${p%-*}; b=${p#*-}; seq "$a" "$b"; else echo "$p"; fi
    done
}
mapfile -t SEEDS < <(expand_seeds "$seeds")
echo "[worker $worker] job $job: ${#SEEDS[@]} seeds (${SEEDS[0]}..${SEEDS[-1]}), args: ${game_args[*]}" >&2

# Build the checked-out commit with the shared build service (cold here; ~1 min).
B=$(tools/agents/build-rev.sh HEAD --bins ffh)
echo "[worker $worker] binary dir: $B" >&2

res="fanout-res/$job/w$worker"
rm -rf "$res"; mkdir -p "$res"

play() {                                        # $1 = seed; writes $res/<seed>.out on success
    local s=$1 tmp="$res/.$1.tmp"
    if "$B/ffh" stats --seed "$s" "${game_args[@]}" >"$tmp" 2>/dev/null \
       && grep -q '^{' "$tmp"; then
        mv "$tmp" "$res/$s.out"
    else
        rm -f "$tmp"
        echo "[worker $worker] seed $s FAILED" >&2
    fi
}
export -f play
export res B
# Run games holding machine-wide CPU slots (nproc in this container), bounded parallelism.
printf '%s\n' "${SEEDS[@]}" | xargs -P "$(nproc)" -I{} bash -c 'tools/agents/slot.sh bash -c "play {}"' 2>&1 \
    | grep -v '^\[agents\] ' || true

ok=(); for s in "${SEEDS[@]}"; do [ -f "$res/$s.out" ] && ok+=("$s"); done
printf '{"job":"%s","worker":%s,"commit":"%s","seeds_requested":%d,"seeds_ok":%d,"ok":[%s]}\n' \
    "$job" "$worker" "$(git rev-parse HEAD)" "${#SEEDS[@]}" "${#ok[@]}" \
    "$(IFS=,; echo "${ok[*]}")" > "$res/manifest.json"
echo "[worker $worker] ${#ok[@]}/${#SEEDS[@]} games ok" >&2

# Commit the results and push them to the result branch (a fresh commit, this worker's files only).
git add -f "$res"
git -c user.email=agent@ffh2-rs -c user.name="fanout worker $worker" \
    commit -q -m "fan-out $job worker $worker: ${#ok[@]}/${#SEEDS[@]} games" >&2
for attempt in 1 2 3 4; do
    if git push -f origin "HEAD:refs/heads/$branch" >&2; then
        echo "[worker $worker] pushed $branch" >&2
        exit 0
    fi
    echo "[worker $worker] push failed (attempt $attempt), retrying" >&2
    sleep $((attempt * attempt))
done
echo "[worker $worker] could not push $branch" >&2
exit 1
