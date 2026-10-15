#!/usr/bin/env bash
# Checks the browser client of ffh-play (docs/PLAY_DESIGN.md §8 item 14): `node --check` of every
# module and tool script, then the pure-module tests (`node --test crates/ffh-play/assets/test/*.test.mjs`).
# Skipped (exit 0) when node is not installed. Not part of `cargo test`.
#
#   tools/web-check.sh              syntax and unit tests
#   tools/web-check.sh --smoke      also the node smoke client: 20 turns against `ffh-play serve`
#                                   (tools/web-api-smoke.mjs; builds nothing, needs
#                                   target/release/ffh-play)
#   tools/web-check.sh --browser    also the headless-Chromium smoke test (tools/web-smoke.mjs;
#                                   skipped when no browser is found)
#
# NODE=/path/to/node overrides the node binary.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
assets="$root/crates/ffh-play/assets"

node="${NODE:-}"
if [ -z "$node" ]; then node="$(command -v node || true)"; fi
if [ -z "$node" ] && [ -x /opt/node22/bin/node ]; then node=/opt/node22/bin/node; fi
if [ -z "$node" ]; then
  echo "web-check: node not found; skipped"
  exit 0
fi

smoke=0
browser=0
for a in "$@"; do
  case "$a" in
    --smoke) smoke=1 ;;
    --browser) browser=1 ;;
    *) echo "usage: tools/web-check.sh [--smoke] [--browser]" >&2; exit 2 ;;
  esac
done

n=0
for f in "$assets"/app/*.js "$assets"/test/*.mjs "$root"/tools/*.mjs; do
  "$node" --check "$f"
  n=$((n + 1))
done
echo "web-check: $n files parse"

"$node" --test "$assets"/test/*.test.mjs

if [ "$smoke" = 1 ]; then
  "$node" "$root/tools/web-api-smoke.mjs"
fi
if [ "$browser" = 1 ]; then
  "$node" "$root/tools/web-smoke.mjs"
fi
