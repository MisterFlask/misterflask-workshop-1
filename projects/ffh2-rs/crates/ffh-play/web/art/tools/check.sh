#!/bin/sh
# check.sh: the art toolkit's checks (docs/ART.md, "Toolkit").
#   1. node --check on every module;
#   2. tools/validate.mjs: sprites parse, palette keys only, sizes, rules, types.json vs data/xml,
#      every game type resolves to a sprite.
# Skipped (exit 0) when node is not installed. Extra arguments go to validate.mjs (-v lists warnings).
set -e
cd "$(dirname "$0")/.."
NODE="${NODE:-node}"
if ! command -v "$NODE" >/dev/null 2>&1; then
  if [ -x /opt/node22/bin/node ]; then NODE=/opt/node22/bin/node; else
    echo "art check: node not found, skipped"; exit 0
  fi
fi
for f in *.js preview/*.js tools/*.mjs; do "$NODE" --check "$f"; done
"$NODE" tools/validate.mjs "$@"
