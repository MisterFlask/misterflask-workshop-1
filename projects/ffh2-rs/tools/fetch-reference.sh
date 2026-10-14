#!/usr/bin/env bash
# Fetch the pinned reference sources this port is derived from.
#
#   FFH2 0.41 (DLL C++ source, gameplay XML, Python):
#     github.com/jlippitt/FFH2-Better-AI @ 39c753c ("Mod in initial state")
#     -- the unmodified FFH2 0.41 base, before that repo's AI changes.
#   Stock BTS XML that FFH2 inherits (GlobalDefines, yields, hurry, ...):
#     github.com/rigiechang/Broken-BTS @ 7bb2280 / 9a5358c
#     -- NOTE: that repo's BTS copy carries mod edits; tools/vendor_xml.py
#        reconstructs stock values (see docs/DATA.md).
#
# Usage: tools/fetch-reference.sh [REF_DIR]   (default: ../../ref relative to repo root, i.e. /home/user/ref)
set -euo pipefail
REF="${1:-/home/user/ref}"
mkdir -p "$REF/src"

clone() { # url dir commit
  local url=$1 dir=$2 commit=$3
  if [ ! -d "$dir/.git" ]; then
    GIT_LFS_SKIP_SMUDGE=1 git clone --filter=blob:none --no-checkout "$url" "$dir"
  fi
  git -C "$dir" checkout -q "$commit"
}

clone https://github.com/jlippitt/ffh2-better-ai "$REF/src/ffh2-better-ai" 39c753c
ln -sfn "$REF/src/ffh2-better-ai" "$REF/ffh2"

BTS="$REF/src/broken-bts"
if [ ! -d "$BTS/.git" ]; then
  GIT_LFS_SKIP_SMUDGE=1 git clone --filter=blob:none --no-checkout https://github.com/rigiechang/broken-bts "$BTS"
fi
( cd "$BTS" && git checkout -q 7bb2280 -- \
    sid-meiers-civilization-iv-beyond-the-sword/Beyond-the-Sword/Assets/XML \
    sid-meiers-civilization-iv-beyond-the-sword/Assets/XML/Terrain/CIV4YieldInfos.xml \
    sid-meiers-civilization-iv-beyond-the-sword/Assets/XML/GameInfo/CIV4UpKeepInfo.xml \
    sid-meiers-civilization-iv-beyond-the-sword/Assets/XML/GameInfo/CIV4ClimateInfo.xml \
    sid-meiers-civilization-iv-beyond-the-sword/Assets/XML/GameInfo/CIV4SeaLevelInfo.xml \
    sid-meiers-civilization-iv-beyond-the-sword/Warlords/Assets/XML/GlobalDefines.xml
  git checkout -q 9a5358c -- sid-meiers-civilization-iv/Assets/XML/GameInfo/CIV4HurryInfo.xml )
ln -sfn "$BTS" "$REF/bts"
echo "reference sources ready in $REF"
