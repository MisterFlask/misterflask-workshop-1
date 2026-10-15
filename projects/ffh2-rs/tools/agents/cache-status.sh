#!/bin/sh
# cache-status.sh: disk use, workspaces, build outputs, run cache, slots, build lock
# See docs/AGENTS.md; the logic is in ffh_agents.py (python3, std only).
exec python3 "$(dirname "$0")/ffh_agents.py" status "$@"
