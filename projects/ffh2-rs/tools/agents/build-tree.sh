#!/bin/sh
# build-tree.sh [<worktree>] [--profile release|checked] [--test]: same for a worktree's current files
# See docs/AGENTS.md; the logic is in ffh_agents.py (python3, std only).
exec python3 "$(dirname "$0")/ffh_agents.py" build-tree "$@"
