#!/bin/sh
# build-rev.sh <git-rev> [--profile release|checked] [--test]: print the dir of that rev's binaries
# See docs/AGENTS.md; the logic is in ffh_agents.py (python3, std only).
exec python3 "$(dirname "$0")/ffh_agents.py" build-rev "$@"
