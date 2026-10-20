#!/usr/bin/env bash
# Builds the static site (docs/WEB.md): the browser client with the game engine compiled to
# WebAssembly, playable from any static host (GitHub Pages) without `ffh-play serve`.
#
#   tools/web-site.sh OUT [--bin PATH] [--wasm FILE]
#
#   OUT           the site's directory (created; a directory this script made before is emptied
#                 first, any other non-empty directory is refused)
#   --bin PATH    use this ffh-play instead of `cargo build --release -p ffh-play` (for example
#                 `$(tools/agents/build-tree.sh --bins ffh-play)/ffh-play`)
#   --wasm FILE   use this ffh_web.wasm instead of building it
#
# Steps: builds ffh-play (native) and the engine (`cargo build --release --target wasm32-wasip1
# -p ffh-web`, without debug info, with LTO; both honour CARGO_TARGET_DIR), runs `ffh-play
# export-site OUT --data data` (index.html, app/, art/, web/data.pak), adds the page's scripts
# (crates/ffh-web/site: web/boot.js, web/engine.js, web/wasi.js and coi-sw.js at the root),
# gzips the engine and the data pack (web/ffh.wasm.gz, web/data.pak.gz; the plain pack is
# removed), and writes web/version.txt (the commit and its date) and .nojekyll.
#
# Serve OUT with any static server (`node tools/web-site-smoke.mjs --site OUT` tests it in
# headless Chromium). Works from any directory. Needs cargo, the Rust target wasm32-wasip1
# (`rustup target add wasm32-wasip1`), gzip; git for version.txt.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
site_js="$root/crates/ffh-web/site"
cargo="${CARGO:-cargo}"

usage() {
  sed -n '5,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
  exit 2
}
die() {
  echo "web-site: $*" >&2
  exit 1
}
say() {
  echo "web-site: $*" >&2
}

out=""
bin=""
wasm=""
while [ $# -gt 0 ]; do
  case "$1" in
    -h | --help) usage ;;
    --bin | --wasm)
      [ $# -ge 2 ] || die "$1 needs a value"
      if [ "$1" = --bin ]; then bin="$2"; else wasm="$2"; fi
      shift 2
      ;;
    -*) echo "web-site: unknown option $1" >&2; usage ;;
    *)
      [ -z "$out" ] || { echo "web-site: more than one OUT ($out, $1)" >&2; usage; }
      out="$1"
      shift
      ;;
  esac
done
[ -n "$out" ] || usage

# Paths given relative to the caller's directory.
abs() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s\n' "$PWD/$1" ;;
  esac
}
out="$(abs "$out")"
[ -z "$bin" ] || { bin="$(abs "$bin")"; [ -x "$bin" ] || die "--bin $bin: not an executable"; }
[ -z "$wasm" ] || { wasm="$(abs "$wasm")"; [ -f "$wasm" ] || die "--wasm $wasm: no such file"; }

command -v gzip > /dev/null || die "gzip not found"
for f in boot.js engine.js wasi.js coi-sw.js; do
  [ -f "$site_js/$f" ] || die "missing $site_js/$f (the page's scripts)"
done

# The site's directory: new, empty, or one this script made (it has .nojekyll, written first).
if [ -e "$out" ] && [ ! -d "$out" ]; then die "$out exists and is not a directory"; fi
if [ -d "$out" ] && [ -n "$(ls -A "$out")" ] && [ ! -f "$out/.nojekyll" ]; then
  die "$out is not empty and is not a site this script built; remove it or choose another OUT"
fi
is_wasm() { [ "$(head -c 4 "$1" | od -An -tx1 | tr -d ' \n')" = 0061736d ]; } # "\0asm"
[ -z "$wasm" ] || is_wasm "$wasm" || die "--wasm $wasm is not a WebAssembly module"
if [ -z "$bin" ] || [ -z "$wasm" ]; then
  command -v "$cargo" > /dev/null || die "cargo not found (or give --bin and --wasm)"
fi
if [ -z "$wasm" ]; then
  sysroot="$(cd "$root" && rustc --print sysroot 2> /dev/null || true)"
  if [ -z "$sysroot" ] || [ ! -d "$sysroot/lib/rustlib/wasm32-wasip1" ]; then
    die "the Rust target wasm32-wasip1 is not installed${sysroot:+ in $sysroot}: run \`rustup target add wasm32-wasip1\` (or give --wasm FILE)"
  fi
fi

# cargo's target directory: $CARGO_TARGET_DIR (a relative one from the caller's directory, as
# cargo started there would read it; the builds below run from $root), else $root/target.
if [ -n "${CARGO_TARGET_DIR:-}" ]; then
  CARGO_TARGET_DIR="$(abs "$CARGO_TARGET_DIR")"
  export CARGO_TARGET_DIR
  target="$CARGO_TARGET_DIR"
else
  target="$root/target"
fi

if [ -z "$bin" ]; then
  say "building ffh-play (native, release)"
  (cd "$root" && "$cargo" build --release -p ffh-play)
  bin="$target/release/ffh-play"
  [ -x "$bin" ] || die "the build did not produce $bin"
fi

if [ -z "$wasm" ]; then
  say "building the engine (wasm32-wasip1, release, LTO, no debug info)"
  (cd "$root" && CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_LTO=true \
    "$cargo" build --release --target wasm32-wasip1 -p ffh-web)
  wasm="$target/wasm32-wasip1/release/ffh_web.wasm"
  [ -f "$wasm" ] && is_wasm "$wasm" || die "the build did not produce a WebAssembly module at $wasm"
fi

if [ -d "$out" ] && [ -n "$(ls -A "$out")" ]; then
  say "emptying $out (a previous site)"
  find "$out" -mindepth 1 -delete
fi
mkdir -p "$out"
: > "$out/.nojekyll"

say "exporting the client and the data ($bin export-site)"
"$bin" export-site "$out" --data "$root/data" >&2
for f in index.html app/main.js art/art.bundle.txt web/data.pak; do
  [ -f "$out/$f" ] || die "ffh-play export-site did not write $f"
done
grep -q 'src="web/boot.js"' "$out/index.html" || die "$out/index.html does not load web/boot.js"

mkdir -p "$out/web"
cp "$site_js/boot.js" "$site_js/engine.js" "$site_js/wasi.js" "$out/web/"
cp "$site_js/coi-sw.js" "$out/coi-sw.js"

# -n: no name or time stamp in the header, so the same input gives the same bytes.
gzip -9 -n -c "$wasm" > "$out/web/ffh.wasm.gz"
gzip -9 -n -f "$out/web/data.pak"
[ ! -e "$out/web/data.pak" ] || die "gzip left web/data.pak"

# The commit the site was built from and its date (UTC); "-dirty" when the project has
# uncommitted changes to tracked files.
version="unknown $(date -u '+%Y-%m-%d %H:%M UTC') (not a git checkout)"
if command -v git > /dev/null && sha="$(git -C "$root" rev-parse --short HEAD 2> /dev/null)"; then
  when="$(TZ=UTC git -C "$root" log -1 --format=%cd --date=format-local:'%Y-%m-%d %H:%M UTC' HEAD)"
  if [ -n "$(git -C "$root" status --porcelain --untracked-files=no -- . 2> /dev/null)" ]; then sha="$sha-dirty"; fi
  version="$sha $when"
fi
printf '%s\n' "$version" > "$out/web/version.txt"

size() { du -h "$1" | cut -f1; }
say "built $out ($version): $(find "$out" -type f | wc -l) files, $(du -sh "$out" | cut -f1); engine $(size "$out/web/ffh.wasm.gz"), data $(size "$out/web/data.pak.gz") (gzipped)"
