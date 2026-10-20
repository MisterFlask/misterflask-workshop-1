# ffh-play in the browser: the static site

**Play: https://misterflask.github.io/misterflask-workshop-1/**

The browser client of `ffh-play` (docs/HUMAN.md) with the whole game engine compiled to
WebAssembly, served as plain files from GitHub Pages: nothing to download or install, no server.
The page is the same client `ffh-play serve` serves; the game runs in a Web Worker in the page.
CI rebuilds and redeploys the site on every push to `master` that touches `projects/ffh2-rs`
(below), so the site follows master.

## How it works

| Piece | Where | What it does |
|---|---|---|
| The client | `crates/ffh-play/assets`, the art from `crates/ffh-play/web/art` | unchanged; its own files are loaded by relative URLs (`app/main.js`, `../art/art.bundle.txt`), so it works below a path such as `/misterflask-workshop-1/`; it still talks HTTP (`fetch("/api/...")`) |
| The engine | `crates/ffh-web` (wasm glue) around `crates/ffh-play/src/transport/web.rs` (the web host) | what `ffh-play serve`'s game thread does, synchronously: `ffh_request` takes one HTTP request and answers it exactly as `http::route` answers `/api` (status, view, map deltas, data, setup, the `POST /api` envelope), running the AI turns inside it. Built for `wasm32-wasip1` (a WASI reactor) with a 16 MiB stack, like the server's game thread |
| The page side | `crates/ffh-web/site/boot.js` (loaded before `app/main.js`) | replaces `fetch`: requests whose path is `/api` or `/api/...` go to the engine and come back as real `Response`s; it answers `GET /api/status` and the long-poll `GET /api/wait` itself from the last status the engine published; shows the loading progress; restarts a crashed engine |
| The worker | `crates/ffh-web/site/engine.js`, `wasi.js` | downloads `web/ffh.wasm.gz` and `web/data.pak.gz` (decompressed in the browser), runs the engine on a WASI shim with an in-memory file system: the game data at `/data`, the saves at `/saves` |
| Isolation | `coi-sw.js` (a service worker at the site's root) | see below |
| The data | `web/data.pak.gz` | `data/xml/**` and `data/text/**` in one file: `FFHPAK1\n`, then per file `<path>\n<length>\n<bytes>` (`ffh_play::site::data_pack`) |

**Cross-origin isolation.** While the engine runs (an end of turn while the AI players move, or a
move of yours that conquers a city), the page still needs answers: the status, queries, and the
player's answer to a blocking decision raised inside that rules call (keep or raze a conquered
city, a free tech, a religion, an offer, city or settlement: docs/HUMAN.md). With
`SharedArrayBuffer` and `Atomics.wait` the worker can take those requests while the engine
runs, and that needs a cross-origin-isolated page (`Cross-Origin-Opener-Policy: same-origin`,
`Cross-Origin-Embedder-Policy: require-corp`). GitHub Pages cannot send headers, so `coi-sw.js`
re-serves the site's own responses with them: on the first visit the page registers it and
reloads once (a session flag stops a loop); later visits are isolated from the start.

**The fallback.** Where service workers are unavailable (some private windows) or isolation does
not take, the game still runs: requests reach the worker as messages, between requests only, and
every blocking decision gets the advisor's pick (also one your own move raises, such as keeping
or razing a city you conquer). The page says so once.

**Saves** live in the browser: the engine writes them under `/saves` as the native server does
(your saves, plus the autosaves; 20 kept), and the worker copies every changed file to IndexedDB
(database `ffh-play`, store `saves`) after each request. The Load screen lists them after a
reload. A crashed engine shows the crash screen as the native client does; a fresh engine starts
and the last autosave can be loaded.

**Version.** `web/version.txt` holds the commit the site was built from and its date; the page logs
it (`ffh-web build ...` in the console).

## The site's files

`tools/web-site.sh OUT` writes:

```
OUT/index.html        the client's page, relative URLs, web/boot.js loaded before app/main.js
OUT/app/**            the client (every file `ffh-play serve` embeds)
OUT/art/**            the art toolkit's modules and art/art.bundle.txt
OUT/web/boot.js engine.js wasi.js
OUT/web/ffh.wasm.gz   the engine (gzip -9)
OUT/web/data.pak.gz   the game data (gzip -9)
OUT/web/version.txt   commit and date
OUT/coi-sw.js         the isolation service worker (at the root: its scope is the site)
OUT/.nojekyll
```

About 7 MB on disk; a first visit downloads the engine (1.5 MB) and the data (1.1 MB), compressed,
plus the client and the art.

## Build and test locally

Needs cargo, the Rust target `wasm32-wasip1` (`rustup target add wasm32-wasip1`) and gzip; the
smoke test needs node and the `playwright` package with its Chromium.

```
tools/web-site.sh /tmp/ffh-site                          # builds ffh-play and the engine, writes the site
node tools/web-site-smoke.mjs --site /tmp/ffh-site        # headless Chromium plays it (below)
node tools/web-site-smoke.mjs --site /tmp/ffh-site --serve --port 8000
                                                          # just serve it: http://127.0.0.1:8000/misterflask-workshop-1/
```

- `web-site.sh` works from any directory and honours `CARGO_TARGET_DIR`. `--bin PATH` uses a
  built `ffh-play` instead of building one (agents on the shared machine:
  `--bin "$(tools/agents/build-tree.sh --bins ffh-play)/ffh-play"`), `--wasm FILE` a built
  engine. OUT must be new, empty or a site the script built (it is emptied).
- Any static server works (`python3 -m http.server -d /tmp/ffh-site 8000`, then
  `http://localhost:8000/`): open it as `localhost`/`127.0.0.1` or over HTTPS, since service
  workers (isolation) need a secure context.
- `web-site-smoke.mjs` serves the site below `/misterflask-workshop-1/` as Pages does, with no
  isolation headers, so the service worker has to make the page isolated; then it waits for the
  engine, starts a new game on a tiny map (seed 5), checks the art draws the map, ends 3 turns
  (popups answered), saves, reloads with `?renderer=iso` (a fresh engine: the save must come
  back from IndexedDB), loads the save, checks the isometric renderer draws, and ends a turn. Any
  page error, console error, failed request or HTTP error fails it; screenshots go to `--out`
  (default `target/web-site-smoke`). `--isolated`: the server sends the isolation headers itself.
  `--no-sw`: a browser without service workers (the fallback). `--turns N`, `--seed N`,
  `--base /path/`, `--headed`.
- The page side's own tests: `node --test crates/ffh-web/site/test/` (a stand-in engine with the
  real interface; they build small wasm programs with rustc). The client's checks are unchanged
  (`tools/web-check.sh`, `tools/web-smoke.mjs` against `ffh-play serve`), as is
  `tests/web.rs` (the page references only embedded files, by relative URLs).

## Deployment (CI)

`.github/workflows/ffh-pages.yml` (repository root), on a push to `master` that touches
`projects/ffh2-rs/**` or the workflow, and by hand (Actions -> "ffh2-rs web" -> Run workflow):

1. **build**: checkout, Rust stable with `wasm32-wasip1`, `tools/web-site.sh _site`, node 22 and
   Playwright's Chromium, the smoke test twice (service-worker isolation; `--no-sw`, the
   fallback), then the Pages artifact. Screenshots of a failed smoke test are kept as the run's
   artifact `web-site-smoke`. A failure stops the deployment: the site keeps the last good build.
2. **deploy**: `actions/deploy-pages` to the `github-pages` environment; only for `master` (a run
   by hand on another branch builds and tests the site without deploying it).

Runs are serialized (concurrency group `pages`, a newer push waits). A full run takes a few
minutes (no cargo cache: with no external crates, a fresh checkout rebuilds every crate anyway).

**One-time setup** (the repository owner): Settings -> Pages -> Build and deployment -> Source:
**GitHub Actions**. Until then the workflow fails at "configure-pages". Pages on a private
repository needs a paid plan; on a public one it is free.

## Measured

In headless Chromium (software rendering, this repository's 4-core container), the engine is ready
about a second after the page; a new game starts in under a second; 200 turns of AI autoplay on a
large map with 8 AI civilizations take 9 s and 400 turns on a huge map with 12 take 62 s (about
1.5-2x native), with no stack overflow; a keep/raze decision raised by a capture is answered on
the isolated page while the status, the view and queries keep being served. The engine plays the
same games as the native build: journals written in the browser replay natively with the same
checksums.

## Limits

- **One tab at a time.** Each tab runs its own engine with the saves as they were when it loaded;
  two tabs saving into the same game can overwrite each other's files.
- **Saves stay in this browser**, for this site: clearing the site's data deletes them, and they
  do not follow you to another browser or device (no export yet).
- **Speed.** The engine is single-threaded WebAssembly: late-game AI turns on large maps take
  longer than with the native `ffh-play serve`. Its memory is 32-bit (at most 4 GiB).
- **Without isolation** (the fallback above), the advisor answers every blocking decision, and the
  page's requests wait while a turn runs.
- Needs a current browser: WebAssembly, module workers, `DecompressionStream` (Chrome/Edge,
  Firefox 113+, Safari 16.4+). The 3D renderer (`?renderer=3d`) needs WebGL2, as on the native
  server.
- Only the browser client: `ffh-play agent`, `soak`, `bench` and the other commands are native.
