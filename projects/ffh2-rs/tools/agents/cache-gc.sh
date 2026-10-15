#!/bin/sh
# cache-gc.sh [--dry-run]: enforce the cache's disk limits (also runs after every build)
# See docs/AGENTS.md; the logic is in ffh_agents.py (python3, std only).
exec python3 "$(dirname "$0")/ffh_agents.py" gc "$@"
