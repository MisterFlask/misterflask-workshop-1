#!/bin/sh
# slot.sh [-n K | --exclusive] <cmd..>: run a command holding machine-wide CPU slots
# See docs/AGENTS.md; the logic is in ffh_agents.py (python3, std only).
exec python3 "$(dirname "$0")/ffh_agents.py" slot "$@"
