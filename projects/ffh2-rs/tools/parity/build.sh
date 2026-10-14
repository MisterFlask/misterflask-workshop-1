#!/usr/bin/env bash
# Builds the C++ parity harness from the pinned reference DLL source and
# regenerates the Rust test fixtures. Requires tools/fetch-reference.sh first.
# Note: MSVC's `unsigned long` is 32-bit; we compile with -m32 semantics emulated by
# forcing a 32-bit `unsigned long` through a typedef-free trick: on LP64 Linux
# `unsigned long` is 64-bit, so we patch it to `unsigned int` in a temp copy.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REF=${1:-/home/user/ref}
SRC="$REF/ffh2/CvGameCoreDLL"
TMP=$(mktemp -d)
for f in CvRandom.h CvRandom.cpp CvFractal.h CvFractal.cpp; do
  sed -e 's/unsigned long/unsigned int/g' "$SRC/$f" > "$TMP/$f"
done
sed -e 's/unsigned long/unsigned int/g; s/%lu/%u/g; s/4294967295UL/4294967295U/; s/2654435761UL/2654435761U/' "$HERE/parity_main.cpp" > "$TMP/parity_main.cpp"
g++ -O1 -w -I "$HERE/stubs" -I "$TMP" -o "$TMP/parity" "$TMP/parity_main.cpp" "$TMP/CvRandom.cpp" "$TMP/CvFractal.cpp"
OUT="$HERE/../../crates/ffh-core/tests/fixtures"
mkdir -p "$OUT"
"$TMP/parity" random > "$OUT/cvrandom.txt"
"$TMP/parity" fractal > "$OUT/cvfractal.txt"
echo "wrote $OUT/cvrandom.txt $OUT/cvfractal.txt"
