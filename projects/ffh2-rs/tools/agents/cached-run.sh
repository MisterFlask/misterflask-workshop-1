#!/bin/sh
# cached-run.sh [--no-cache] [-v] <bin> <args..>: run a deterministic command through the run cache
# See docs/AGENTS.md; the logic is in ffh_agents.py (python3, std only).
exec python3 "$(dirname "$0")/ffh_agents.py" run "$@"
