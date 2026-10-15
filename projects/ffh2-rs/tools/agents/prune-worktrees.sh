#!/bin/sh
# prune-worktrees.sh [--dry-run] [--idle-minutes N] [--unlock-stale]: remove idle clean agent worktrees
# See docs/AGENTS.md; the logic is in ffh_agents.py (python3, std only).
exec python3 "$(dirname "$0")/ffh_agents.py" prune-worktrees "$@"
