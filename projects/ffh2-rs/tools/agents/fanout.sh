#!/bin/sh
# fanout.sh prepare|status|collect|cleanup ...: fan a game batch out to sibling cloud sessions.
# See docs/AGENTS.md "Fan-out"; the logic is in fanout.py (python3, std only). The agent drives the
# sessions (create_session/archive_session); this script does the git and run-cache work.
exec python3 "$(dirname "$0")/fanout.py" "$@"
