#!/bin/bash
# ab.sh [--base REV] [--new REV|DIR] [--out DIR] [--bins B] [ai_metrics.py args..]
#
# A/B AI metrics: builds the base revision and the new side (a worktree's current files, default
# this one; or a revision) with the shared build service, then runs crates/ffh-cli/scripts/ai_metrics.py
# on both with the same arguments and prints the base report followed by the new one. Games go
# through the shared run cache, so the base side is usually replayed from games other agents
# already played; all games share the machine-wide CPU slots. See docs/AGENTS.md.
#
#   --base REV   default: $FFH2_BASE, else the commit this worktree was created from (the
#                harness records it), else the merge-base of HEAD and master
#   --new X      a directory (worktree; build-tree.sh) or a revision (build-rev.sh); default: .
#   --out DIR    where base.json / new.json go (default: a new temp dir, printed)
#   --bins B     binaries to build (default ffh: ffh-play is not needed and slow to rebuild)
# Example: tools/agents/ab.sh --seeds 1-12 --turns 300 --at 100,200,300 --jobs 4
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
metrics="$here/../../crates/ffh-cli/scripts/ai_metrics.py"
base= new=. out= bins=ffh
while [ $# -gt 0 ]; do
    case "$1" in
    --base) base=$2; shift 2 ;;
    --new) new=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    --bins) bins=$2; shift 2 ;;
    -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
    *) break ;;
    esac
done
if [ -z "$base" ] && [ -n "${FFH2_BASE:-}" ]; then
    base=$FFH2_BASE
fi
if [ -z "$base" ]; then
    claude_base="$(git rev-parse --absolute-git-dir)/CLAUDE_BASE"    # written by the agent harness
    if [ -s "$claude_base" ] && git rev-parse -q --verify "$(cat "$claude_base")^{commit}" >/dev/null; then
        base=$(cat "$claude_base")
        why="the commit this worktree was created from"
    else
        base=$(git merge-base HEAD master)
        why="the merge-base of HEAD and master"
    fi
    echo "[ab] base: $why = $(git log -1 --format='%h %s' "$base") (--base REV to choose)" >&2
fi
base_dir=$("$here/build-rev.sh" "$base" --bins "$bins")
if [ -d "$new" ]; then
    new_dir=$("$here/build-tree.sh" "$new" --bins "$bins")
else
    new_dir=$("$here/build-rev.sh" "$new" --bins "$bins")
fi
[ -n "$out" ] || out=$(mktemp -d "${TMPDIR:-/tmp}/ab.XXXXXX")
mkdir -p "$out"
echo "[ab] base $base_dir" >&2
echo "[ab] new  $new_dir" >&2
python3 "$metrics" --bin "$base_dir/ffh" --label "base $base" --out "$out/base.json" "$@" > "$out/base.txt"
python3 "$metrics" --bin "$new_dir/ffh" --label "new $new" --out "$out/new.json" --compare "$out/base.json" "$@"
echo "[ab] results: $out/base.json $out/new.json" >&2
