# R3D: the experimental 3D renderer of the map

**Status: experimental, off by default.** A fourth map renderer of the browser client
(`crates/ffh-play`), beside the flat debug renderer, the 2D pixel art (docs/ART.md) and the
isometric 2.5D renderer (docs/art/SOC_DIRECTION.md). It draws the same game state as a fully
three-dimensional, painterly dark-fantasy world: real terrain relief, forests of procedurally modelled
trees, settlements with architectural families, 3D figures for units and monsters, water, magic and
corruption, through a stylised lighting and post-processing pipeline. It is a consumer of the game
state only: no rule, save, protocol or simulation code changed for it.

Contents: 1 running it · 2 the existing renderers (what was there) · 3 architecture · 4 the world ·
5 the pipeline · 6 asset recipes · 7 placement · 8 readability and interaction · 9 fog of war ·
10 the lab, tests and tools · 11 performance · 12 configuration · 13 removing it · 14 limitations
and next steps · 15 provenance.

---

## 1. Running and toggling it

```
cargo build --release
./target/release/ffh-play serve            # open http://127.0.0.1:8642/?renderer=3d
```

* `?renderer=3d` for one page, or **Tools → Options → "3D painterly (experimental)"** (remembered by
  the browser, like the other renderer choices). Choosing another renderer there switches back.
* The **3D** button at the map's top-left opens the tuning panel: the *painterly* and *plain* presets,
  the light (afternoon or day: the map is always in daylight) and a switch or slider for every stage
  of the pipeline (§12), with
  the frame time and per-pass GPU times.
* `?r3d=plain`, `?r3d=painterly` or `?r3d={"kuwahara":0,"outline":0}` override the look for one page.
* It needs WebGL2. Without it the client says so and keeps the 2D map.
* The feature flag is `EXPERIMENTAL_3D` in `assets/app/look.js`: false hides the option and ignores
  `?renderer=3d`; the module is then never loaded (it is loaded on demand even when the flag is on).

## 2. The existing rendering architecture (as found)

* **Engine and transport.** `ffh-play serve` (Rust, std only) runs the game thread and an HTTP
  server; the browser client is vanilla ES modules (`assets/app/*.js`, no build step, no
  dependencies), embedded in the binary (`src/transport/http.rs` `ASSETS`) or served from disk with
  `--assets DIR`.
* **Game state to the client.** `/api/map` gives the map as flat per-plot arrays from the human
  team's side (`vis`, `plot_type`, `terrain`, `feature`, `bonus`, `improvement`, `route`, `owner`,
  `river` bits, `city`, `yield`; unrevealed plots carry -1 everywhere, fogged plots their remembered
  state), kept current with tile deltas; `/api/view` gives the units in sight, the revealed cities and
  the players; `/api/data` the type tables. `state.js` holds them (`S.map`, `S.units`, `S.unitsAt`,
  `S.cities`, `S.players`, ...).
* **The map view.** `map.js` `MapView` owns the canvas, the camera (`cam.cx`, `cam.cy` the map point
  looked at, `cam.ts` pixels per plot), input (click, right-click, drag-pan, wheel and key zoom,
  hover tooltip), the overlays (`overlay.hover/sel/targets/path/waypoints/units`), the layers
  (terrain, political, yield), the minimap and combat flashes. It draws with one of its renderers:
  the flat one (in `map.js`), `artmap.js` (2D pixel art, chunks baked by a worker), or `iso.js`
  (2:1 isometric sprites), which owns its own projection: MapView delegates `draw()`,
  `screenToMap()`, `screenOf()`, `panDelta()` and `zooms()` to it.
* **Selection and orders** (`units.js`) work in plot coordinates through MapView's handlers, so a
  renderer only has to map screen points to plots and back.

The 3D renderer plugs in exactly where the isometric one does.

## 3. Architecture

```
assets/app/
  map.js          MapView: + this.r3d, useR3d(on) (dynamic import), delegation (≈60 lines)
  main.js, look.js, tools.js   the renderer choice (≈30 lines)
  r3d/
    map3d.js      Map3D: the adapter MapView talks to; camera, picking, sync, frame loop, panel
    world.js      plot kinds and the continuous heightfield (coasts, hills, mountains, rivers)
    terrain.js    terrain chunk meshes; per-plot data textures
    decals.js     CPU-painted world layers: roads, fields, wet banks, settlement ground, light pools
    scene.js      GPU scene: terrain chunks, sea, rivers, instanced props by asset and chunk,
                  rings and path, particles; the draw callbacks of the pipeline
    pipeline.js   the passes (shadow, scene, SSAO, glow, compose, Kuwahara, finish, overlays)
    shaders.js    all GLSL
    dress.js      what stands where: trees, rocks, structures, settlements, unit figures
    hud.js        city plates, stack badges, far-zoom counters, yields, flashes (2D canvas)
    config.js     tunables, presets, lighting
    gl.js math.js noise.js mesh.js   WebGL2 plumbing, matrices, deterministic noise, mesh builder
    assets/       registry.js catalog.js palette.js basic.js fallbacks.js index.js
                  vegetation.js units.js architecture.js props.js fx.js  (the recipe modules)
r3d-lab/          the lab: synthetic scenes for focused work (not shipped)
tools/r3d-lab.mjs r3d-perf.mjs r3d-compare.mjs   lab screenshots, performance, comparisons
```

Data flow per change: `S.map`/`S.view` change → `Map3D.sync()` rebuilds only what changed (the world
model and the dirty terrain chunks when the map changed; the static dressing of the plots around
what changed — a plot's dressing depends only on it and its neighbours, so a unit's step redresses
the nine plots around each end and repacks only the instance buckets of the chunks they touch;
the paint layers when the map, the cities or the light changed; the unit figures when the view or
the selection changed; the overlay marks when the overlay changed) → the frame loop renders. Per frame no geometry is generated: instance buffers and meshes
are reused; only uniforms change (a walking unit's instances are re-uploaded while it walks).

The boundary: the 3D code reads `S`, `MapView`'s camera/overlay/layer and the type tables, and
calls nothing that changes the game. MapView's own behaviour is unchanged when `r3d` is null.

## 4. The world

**Coordinates.** Map point (mx, my) — plot (x, y) covers [x, x+1) × [y, y+1), north +y — is world
(X, Y, Z) = (mx, height, −my); one world unit is one plot. Plot centres are where units, cities and
markers stand, so the logical grid survives however the ground is shaped (picking is checked against
it, §10).

**Height** (`world.js`). The plot kinds (ocean, coast, land, hills, peak, ice, and *void* for
unrevealed plots) are blended with smoothed bilinear weights whose lookup is warped by two octaves of
noise (bays and headlands, then a ragged edge; never half a plot, so a plot's centre keeps its kind).
Lowlands undulate; hills add rolling ridged crests; peaks add sharp ridged massifs (narrow crests,
deep gullies, finer spurs) and a lopsided summit near each peak plot's centre (its flanks reach
out unevenly, spurs with gullies between them, and a lower second summit on one shoulder; on the
rock, creases bump the shading normal, finer than the mesh can carry). Rivers (the plot-edge river
bits, chained through shared corners, smoothed with Chaikin and given a slight meander) carve a
channel with sloping banks. Everything is a pure function of the map and the plot index.

Height noise is periodic over the map's width when the map wraps (the ground meets itself at the
seam; every copy of the map is the same ground). Hills are rounded domes on each hill plot over a
low shoulder (overlapping into ridges where hills neighbour); desert lowlands get wind-combed dunes.

**Ground painting** (terrain shader). Each fragment blends the materials of the four plots around it
with weights sharpened by brush-dab noise, so borders between terrains are bands of interleaved
strokes; the lookup is warped up to most of a plot between plot centres and not at all at them, so a
plot's centre always shows its own ground while grass, plains and desert meet in organic shapes. Each material is a three-tone painted mass (broad soft shapes, a lighter tone where they
rise) with impressionist brush dabs at two sizes (oriented elliptical marks on a jittered lattice,
one painter's hand: orientations within a 60° fan). Over that: features' floors (forest litter, ice,
flood silt, ash), beaches on the lowest rim of the land, rock with tilted strata on steep ground and
high slopes (warm lit faces, cool shaded ones), snow on summits and gentle high ledges, wet banks,
fields, trodden settlement ground and roads (from the decal layer), marsh pools. Hell terrains are
ash and scorched earth split by a few meandering smouldering fissures (iso-lines of broad noise,
emissive, pulsing slowly). All patterns are anchored to the map, periodic over its width when it
wraps (no seam), so nothing swims when the camera moves.

**Water.** The sea is one plane at sea level; its shader reads the sea floor's height (a texture at
4 samples a plot) through a warped, averaged lookup, so the shelf's colour bands follow soft
contours: teal over the shelf to indigo over the deep. Waves are two drifting noise layers in the
normal; restrained sun glints; foam on the surf line and a slower line offshore. Rivers are ribbons
wider than their carved beds: the banks cut the water's edge; flecks drift downstream.

**Decal and light layers** (`decals.js`), painted with a 2D canvas when the map changes: roads (each
routed plot to its routed neighbours, bowed so a network reads as tracks), fields (farms, plantations,
the gardens of villages), wet banks, settlement ground; and pools of local light (towns, lava, mana
nodes, ancient forests) that light the terrain and the props.

## 5. The pipeline (`pipeline.js`, `shaders.js`)

| pass | what | optional |
|---|---|---|
| shadow | sun depth maps over the ground in view, texel-snapped (no shimmer when panning): the props in a 2048² map (crisp, 12-tap rotated Poisson PCF), the terrain in a 1024² one (6 wide taps, soft) applied at part strength, so a hill's cast shadow darkens the plain behind it a little instead of punching a hole in it | `shadows`, `terrainShadow` |
| scene | depth pre-pass (each pixel shaded once), then terrain, sea, rivers, props into three targets: lit colour (linear HDR; alpha = keep sharp), view normal + linear depth, emission + object id | — |
| ssao | half-resolution scalable ambient obscurance, depth-aware blur | `ssao` |
| glow | emission blurred at half resolution | `glow` |
| compose | occlusion, glow, exposure, ACES-fitted filmic curve → display colour | — |
| paint | structure tensor → smoothed → anisotropic Kuwahara (Kyprianidis et al., polynomial sector weights); radius grows with the zoom (the brush belongs to the world) and so does the strength (0.35 at the gameplay zooms, full close up: the filter erases what is narrower than its radius, and at 96 px a plot that is roads and pool rims); units (keep-sharp mask) mostly spared | `kuwahara`, `kuwaharaRadius` |
| finish | ink outlines from depth (far side of a jump only), creases and object-id boundaries on props, faded with distance, coloured from the paint below (sepia ink, not black); split-tone grade, saturation/contrast, paper grain anchored to the world (reconstructed from depth), vignette | `outline`, `grade`, `grain`, `vignette` |
| aa | FXAA (Timothy Lottes' console variant) on the finished colour before the overlays: takes the stair-step off silhouettes and the ink outlines and stops them crawling when panning (the HUD is a separate 2D canvas, so it stays crisp) | `fxaa` |
| overlay | borders (crisp ink line + soft owner band), political wash, grid, target/path/hover/selection frames on the ground mesh (depth-equal), team rings and the path ribbon draped on the ground; each also drawn ghosted where something stands in front; units hidden behind scenery as hatched team-coloured x-ray silhouettes (depth test GREATER); particles (additive) | `borders`, `grid`, `particles` |

The light model (scene pass): a wrapped, softly stepped sun term (broad light masses), coloured
shadows (a cool sky ambient and a shadow tint), broad cloud shadows drifting slowly, anchored to the world (two octaves: no plot-sized blots), local
light pools, rim light on silhouettes (stronger on units), soft metal highlights, light through
leaves, an aerial haze with distance and valley mist, and the fog of war (§9). HUD text and the
interface marks are never filtered: the strategic overlays draw after the post passes and the
labels on the 2D canvas.

## 6. Asset recipes

A recipe models one kind of thing with the mesh builder and is registered by name:

```js
import { recipe } from "./registry.js";
recipe("tree.oak", { variants: 8, build(mb, r, { variant, param }) { /* model with mb */ } });
```

* `r` is a seeded generator (the variant number seeds it): variant 3 of `tree.oak` is the same tree
  everywhere. `param` is a recipe-specific string (a race, an architectural family, a mana school).
* **Mesh builder** (`mesh.js`): a transform stack (`push/pop/translate/rotateX|Y|Z/scale/part(fn)`),
  primitives (`box` with taper and skew, `lathe` with wobble, `cylinder`, `cone`, `tube` along a
  path with parallel-transport frames, `sphere`, `blob` (noise-displaced), `prism`, `gable`, `hip`,
  `card`, `tri/quad`) and per-part edits from a `mark()` (`sphereNormals` for soft foliage masses,
  `heightAO`, `heightSway`, `tint`, `displace`). The state (`color`, `mat`, `sway`, `bone`, `ao`)
  sticks to every vertex added.
* **Winding matters.** Props are drawn without face culling and lit on both sides: the shader
  flips the normal of a back face (`gl_FrontFacing`), so thin cards and leaves light correctly
  from behind. A triangle must therefore be wound counter-clockwise as seen from the side its
  normal points to; one wound against its explicit normals (`triN`, `quadN`) is shaded
  inside out. The builder's primitives do this; hand-made faces must too.
* **Vertex format**: position, normal, linear albedo, and `aMat` = (material, sway, bone, baked
  occlusion). Materials: matte, foliage (light through leaves), metal, team (the owner's colour times
  the vertex colour's luminance), emissive, skin, team glow. Bones drive a small idle animation in
  the vertex shader (breathing, arms, cape, wings, tail); sway drives wind.
* **Palette** (`assets/palette.js`): every recipe takes its colours from one designed palette
  (warm daylight key; saturation spent on owner colours, magic and fire), plus the mana schools'
  colours.
* **Scale contract**: one unit = one plot; a humanoid ~0.48 tall, broadleaf trees 0.30-0.42,
  conifers 0.34-0.50, great ancient trees 0.8-1.1, houses ~0.15, keeps 0.3-0.4, towers 0.4-0.55,
  resource markers 0.10-0.20, a city within ~0.48 of its plot centre. Foundations, roots and trunk
  flares reach below y = 0 so nothing floats on sloping ground.

### 6.1 The recipe modules

| module | what it models | entries it fills |
|---|---|---|
| `basic.js` | the fallbacks: a tree, a pine, a figure, a house, a rock, a bridge | — |
| `vegetation.js` | oak, birch, pine (and snowy), jungle, palm, ancient (mallorn-like giants with lantern fruit), willow, dead and blighted trees, saplings, bushes, reeds, dry shrubs and cacti, ferns, mushrooms (and glowing), grass tufts | `forest` (a wooded plot's species mix by feature, ground and relief, with undergrowth), `flora` |
| `units.js` | humanoid figures assembled from a race table (human, elf, dwarf, orc, goblin, lizard, undead, demon; `-dark` garb) with body types, headgear, hair, capes, wings, 18 weapons and shields, posed by archetype (warrior, soldier, spearman, archer, mage, priest, worker, hero, scout, settler, cavalry on horse, wolf, boar or nightmare); creatures (wolf, bear, spider, dragon, demon, golem, elemental, skeleton, serpent, angel, bird and fifteen beasts); galleys, ships, catapults | `unit` (every unit type of the game: ~110 name rules, then race by name or civilization, then a fallback by combat class, domain and tier) |
| `architecture.js` | seven architectural families mapped from the civilizations (human, elven, dwarven, infernal, tribal, desert, coastal, with arcane, dark, gothic and northern variants): houses, halls, towers, keeps, walls, gates, wall towers, wells, stalls, banners | `family`, `city` (a layout from size, capital, walls and family: a ring street, a square before the landmark, walls), `settlement` (cottage to town, forts, castles, citadels, towers, pirate ports, goblin forts) |
| `props.js` | rocks by ground (boulders, strata outcrops, crags, obsidian, sandstone), terrain features (oasis, volcano, flames, ice floes, flood silt, scrub, souls, fallout), every improvement (farms, pastures with livestock, camps, mines, quarries, mills, vineyards, boats, lairs and ruins, mana nodes per school), distinct landmarks (Yggdrasil, the Maelstrom, the Dragon Bones, the Pool of Tears, standing stones, ...) and a marker for every resource | `rocks`, `feature`, `improvement`, `bonus` |
| `fx.js` | particle emitters: hell embers, fire and volcano sparks and ash, soul wisps, ancient-forest motes, city sparks by family | `emitters` |

Budgets (triangles, full detail): trees ≤ ~700 (great trees ~2100), units ≤ ~2000, houses ≤ ~250,
keeps ≤ ~1800, a walled size-12 capital ~3–11k, rocks ≤ 200, improvements ~600, resource markers
≤ ~340, landmarks ≤ ~2500. A recipe also gets the generic simplified copy (`buildAssetLOD`) for
distance and shadows whenever simplifying saves at least a third of its triangles.

### 6.2 Extending it

* **A new model.** Add a `recipe(...)` to the module of its family (or a new module imported by
  `assets/index.js`; each module is imported on its own, so a broken one costs only its own
  models). Look at it in the lab grid:
  `node tools/r3d-lab.mjs --out a.png --query "grid=my.thing&variants=6&ts=168"` (`&params=a,b` gives
  each row a param). `node --test assets/test/r3d.test.mjs` checks every recipe builds the same
  twice, has whole, finite triangles and simplifies for distance.
* **A game type to a model.** Catalog entries (`assets/catalog.js` lists the contract of each) map
  the game's types to recipes; a module fills them with `extendCatalog({ unit(name, ctx) {...} })`
  (later modules override earlier ones per entry). Whatever the catalog leaves unanswered, or names
  without a recipe, falls back to `basic.js`, so a new unit or improvement in the game's tables
  never breaks the map.
* **Particles.** `emitters("plot" | "city", ctx)` returns emitters
  (`{dx, dy, dz, color, size, rise, count, spread}`); structures carry theirs as an `.emit` list;
  `scene.js` turns them into GPU point sprites.
* **A light or a preset.** Add an entry to `LIGHTING` or `PRESETS` in `config.js`; the panel lists
  them. A new pipeline switch: a key in `DEFAULTS`/`PRESETS`, a row in `PANEL`, and its use in
  `pipeline.js` or as a uniform in `map3d.js` `frameState()`.
* **A pass.** Passes are methods of `Pipeline` drawing full-screen programs between the targets it
  owns (`this.t`); `render()` lists them in order and times each one (shown in the panel).

## 7. Placement (`dress.js`, `assets/catalog.js`)

The catalog maps the game's types to recipes: `forest` (a wooded plot's species mix by feature,
ground and relief), `flora`, `rocks`, `feature`, `settlement` (cottage to town, forts, towers, in the
plot owner's architectural family), `improvement`, `bonus`, `family` (civilization → architecture),
`city` (a settlement's layout from its size, capital, walls, family), `unit` (unit type, combat
class, domain and the owner's civilization → figure and race), `emitters` (particles). Recipe
modules fill entries; anything unfilled falls back to `basic.js`. Placement is a pure function of
the state and the plot index: trees on a jittered grid thinned toward open neighbours, with a glade
where a unit, city or structure holds the plot; a structure moves to the back of its plot when a
unit stands there; resources at the front-left; bridges where a road crosses a river edge; a unit
stack shows its top unit with up to two of its company behind it (at a city, at its gate).

## 8. Readability and interaction

* **Camera**: perspective with a narrow (24°) field of view, looking north; pitched ~62° from the
  horizon far out and ~38° close up (~48° at the normal zoom, 96 px a plot); zoom steps 28-220 px a
  plot, eased; drag, arrows, minimap and wheel work as in the other renderers.
* **Picking**: the ray under the pointer is marched over the heightfield and refined by bisection; a
  click selects what you see in front (a mountain hides what is behind it).
* **Marks**: team rings under every stack (gold-rimmed when selected), hover / selection / target
  frames and the path on the ground, borders, all crisp over the painted frame and ghosted through
  trees and buildings; city plates (size, name, capital star), stack counts, moves and health; below
  60 px a plot a team-coloured counter pin over each stack.
* Units walk between plots (a short glide with a step bob, then keep facing their heading, turned at
  most 100° from the camera); nothing moves under `prefers-reduced-motion`.

## 9. Fog of war and hidden information

The 3D renderer sees only what the other renderers see: the server's map arrays (unrevealed plots
are -1 everywhere; fogged plots carry what was seen) and the units in sight. Unrevealed plots are
modelled as flat *void* (no hidden plot shapes the height of its neighbours, no river or road is
traced through them, nothing is placed on them) and painted as dark parchment behind a brushy,
burnt-paper frontier. Remembered plots keep their remembered terrain, structures and cities, dimmed
and cooled; no unit is drawn there (the view has none). Particles fade out of sight.

## 10. The lab, tests and tools

* **Lab** (`crates/ffh-play/r3d-lab`, `tools/r3d-lab.mjs`): the real renderer on synthetic scenes
  without a game server: the vertical-slice `showcase`, recipe grids
  (`grid=tree.oak,tree.pine&variants=6`, `&params=a,b` a param per row, `&float=1` on water),
  settlements side by side (`scene=cities&civs=CIVILIZATION_BANNOR,CIVILIZATION_KHAZAD&sizes=2,6,12&imps=IMPROVEMENT_TOWN`),
  any map written as legend rows and dressed by the real placement (`rows=.www.kkk.,.zzz.qqq.`;
  `scenes.js` `LEGEND`),
  overlays (`&sel=x,y&path=...`), any pipeline override (`&r3d={...}`).
  `node tools/r3d-lab.mjs --out a.png --query "scene=showcase&ts=96"`; `--shots FILE` for many;
  `--serve` to open it in a browser; `--stability` for the panning test (§11).
* **Browser smoke test** (`tools/web-smoke.mjs`, run by `tools/web-check.sh --browser`): a
  `?renderer=3d` step checks the frame is drawn and not blank, that picking through the relief finds
  every in-view plot under its own screen point, click-select, hover, the zoom steps, pan, the
  political layer and switching back to the art.
* **Web test** (`tests/web.rs`): every module is embedded and reached (the on-demand import counts).
* **Node tests** (`assets/test/r3d.test.mjs`, run by `tools/web-check.sh`): the ground is seamless
  across a wrapping map's seam and a pure function of the map; unrevealed plots shape nothing
  (pretending the server had sent a hidden mountain and river changes no height); river edges chain
  into continuous, carved lines; every recipe is deterministic, finite and simplifies for distance;
  every unit type of the game gets a figure that builds; a scene's dressing is deterministic, uses
  only registered recipes and places nothing on an unrevealed plot.
* `tools/r3d-perf.mjs` (§11) and `tools/r3d-compare.mjs` (the same scene in every renderer).
* **Software or GPU.** The lab, compare and perf tools render with SwiftShader by default (software:
  works anywhere, including GPU-less containers, but a 3D frame takes seconds). On a machine with a
  GPU, `--gpu` draws with it (Chromium's full headless mode); each tool prints the WebGL renderer
  it got first, so a silent fallback to software is visible. Real frame rates come only from a
  GPU run (`node tools/r3d-perf.mjs --gpu --save ...`) or from the panel in a browser.
* `ffh-play stage corrupt DIR`: the vertical slice's real game (below).

### 10.1 The vertical slice

Two scenes, the same renderer:

* **A real game** (`ffh-play stage corrupt DIR`, `src/stage.rs`): seed 11, a standard map at low sea
  level, the human as the Ljosalfar against six AI civilizations, 200 turns of AI autoplay. Then
  the heartland of the evil civilization with the most cities is corrupted by FfH2's own hell rule:
  every land plot it owns within two plots of its capital takes the plot counter an Infernal owner
  gives its plots each turn (`plot_change_plot_counter(p, 100)`, the Infernal branch of
  `CvGameUtils.doHellTerrain`), so its terrains turn to burning sands, broken lands and fields of
  perdition and its resources to their hell kinds, as in play. Nothing about the scene is staged
  for the renderer beyond that: what stands within six plots of the corrupted capital is what the AI
  game produced — sea, coast, a river with flood plains, tundra, grassland, plains, desert, hills
  and a peak, 32 forest plots, roads, a pasture, a plantation, a winery, a mine, the Seven Pines
  (a unique landmark), raw mana and seven other resources, three Svartalfar cities, ten
  warriors, archers, workers, a hunter and a wild elephant. `node tools/r3d-compare.mjs --save DIR/corrupt.ffhsave --x 48 --y 15` renders it
  in every renderer. The shots below (centred on the corrupted heartland between Gereth Minar and
  Thariss, 96 px a plot except where noted) are from the review-fixes tree:

  | renderer | |
  |---|---|
  | 2D pixel art | [slice-art.jpg](r3d/slice-art.jpg) |
  | isometric 2.5D | [slice-iso.jpg](r3d/slice-iso.jpg) |
  | 3D, every stylisation off | [slice-3d-plain.jpg](r3d/slice-3d-plain.jpg) |
  | 3D painterly | [slice-3d-painterly.jpg](r3d/slice-3d-painterly.jpg) |
  | 3D painterly, close (168 px) | [slice-3d-close.jpg](r3d/slice-3d-close.jpg) |
  | 3D painterly, far (40 px) | [slice-3d-far.jpg](r3d/slice-3d-far.jpg) |

  Against this same scene the review found its findings; the 3D painterly shot now reads cleanly
  (short daylight shadows, slate-violet Svartalfar cities, distinct hell terrains), see
  `docs/r3d/REVIEW.md`.
* **The lab showcase** (`r3d-lab/scenes.js` `showcase`): a 24 × 14 synthetic map that puts every
  kind of thing the renderer draws in one view — sea, coast, a river with a bridge, roads, plains,
  grassland, marsh, a mountain range, hills, forest, an ancient elven forest with a city, a volcano
  in hell terrain with an infernal city, farms, mines, villages, a mana node, units of several
  races and creatures (a dragon among them) — built from the real type tables (the lab's
  `data.json`), so the same catalog code runs on it.

## 11. Performance

Measured in headless Chromium with SwiftShader (a CPU rasteriser; WebGL renderer: ANGLE / Vulkan
1.3 SwiftShader), on the real-game slice (`stage corrupt`, centred on the corrupted heartland).
**These are a CPU rasteriser's times -- several seconds a frame -- and say nothing about a real GPU;
only the _relative_ costs of the passes carry over.** `--gpu` measures on the machine's GPU; the
owner can run `node tools/r3d-perf.mjs --gpu --save DIR/corrupt.ffhsave --x 48 --y 15`.

Per-pass GPU cost (SwiftShader, painterly preset, one run), with each pass's share of the frame:

| pass | ts 40 (far) | ts 96 (gameplay) |
|---|---|---|
| scene (geometry + shading) | 2903 ms (~67%) | 5309 ms (~66%) |
| kuwahara (paint filter) | 857 ms (~20%) | 2164 ms (~27%) |
| shadow | 224 ms | 211 ms |
| overlay | 118 ms | 161 ms |
| glow | 63 ms | 55 ms |
| finish (ink + grade) | 57 ms | 49 ms |
| ssao | 41 ms | 59 ms |
| fxaa | 42 ms | 24 ms |
| compose | 26 ms | 19 ms |
| **whole frame** | **4364 ms** | **8083 ms** |

The scene pass dominates and the Kuwahara paint filter is the clear second; FXAA (§5, F17) and the
other post passes are cheap. The quality ladder (§12 `autoQuality`) therefore drops the paint filter
first and the render scale (which scales the scene pass) second.

Initial world sync (CPU, this slice): textures 195 ms + dressing 425 ms + instance upload 436 ms +
paint layers 236 ms + world 41 ms, about 1.3 s for the full redress of ~10.8k props and 123 unit
figures; per-move redressing is incremental. On a more crowded map this is the ~2 s of start-up
JavaScript noted in §14 -- spreading it over frames or onto a worker is a next step.

Panning stability (`tools/r3d-lab.mjs --stability`, showcase at 96 px; pan, then re-align by the best
whole-pixel shift and compare): mean |diff| about 1.5 of 255 with 1.6% of pixels off by more than 24
for the painterly preset (1.2% plain, 1.7% painterly with the paint filter off) -- low crawl, from
the texel-snapped shadow frustum and the world-anchored paint and grain. FXAA is on in the painterly
figure.

GPU numbers: none yet (the owner runs `r3d-perf --gpu`).

## 12. Configuration

`r3d/config.js`: `PRESETS.painterly` (default) and `PRESETS.plain` (every stylisation off: an
ordinary real-time render of the same geometry, for comparison), `LIGHTING` (one `daylight` look:
always daylight by design, a warm high sun at about 42 degrees; a saved light that is no longer
offered -- a "dusk", "afternoon" or "day" -- falls back to it), and the switches of the panel
(`PANEL`): shadows, ssao, glow, kuwahara (+ radius), outline (+ width), paint (pigment variation),
bands (stepped light), stylize (coloured shadows), rim, clouds, haze, mist, grade, grain, vignette,
borders, grid, fogWar, particles, animate, renderScale, fxaa (anti-aliasing), autoQuality (the
measured-frame-time quality ladder).

## 13. Removing it

Delete `assets/app/r3d/`, `crates/ffh-play/r3d-lab/`, `tools/r3d-*.mjs`, this file; remove the r3d
lines of `ASSETS` in `src/transport/http.rs`, `useR3d` and the `r3d` delegation lines in `map.js`,
`use3d`/the `"3d"` branch in `main.js`, `EXPERIMENTAL_3D` in `look.js` and `tools.js`, the
`renderer3d` step of `tools/web-smoke.mjs`, `stage corrupt` in `src/stage.rs`/`src/main.rs`, and the
`await import(` line of `tests/web.rs`'s scanner. Nothing else depends on it.

## 14. Limitations and next steps

What is not there, or not good enough yet, as of this prototype:

* **No GPU numbers.** Everything here was developed and measured in headless Chromium with
  SwiftShader (a CPU rasteriser): frames there take seconds, and §11's absolute times say nothing
  about a real GPU. The design is GPU-conventional (instancing, a depth pre-pass, half-resolution
  occlusion and glow, full-screen passes), but it has not been profiled on one.
* **Motion.** Units glide between plots and idle (breathing, capes, wings, tails, wind); there are
  no attack, death or work animations, and combat shows only the 2D flashes.
* **Always daylight, by choice.** There is one `daylight` light (a warm, high sun); no dusk,
  night or day/night cycle.
* **Faces and hands** are blank at the closest zoom; crossbowmen and slingers use the bow and
  spear poses.
* **Map edges.** Where the map does not wrap, north and south run into the dark-parchment border
  (continuous, no gap); east and west simply end at the last plot.
* **Fog of war** follows the plot grid on purpose (it says which plots are in sight); the
  unexplored is dark parchment, not modelled.
* **Particles** draw after the paint passes and cannot be occluded behind tall props per pixel
  (a point sprite is depth-tested at its centre).
* **Water** has no reflections and no waterfalls where rivers drop.

Next steps, roughly by value: profile on real GPUs (the quality ladder -- paint filter off, then
render scale -- is in, but its thresholds are tuned only against SwiftShader, and shadow-map size
is not yet a ladder step); spread the initial world sync over frames or a worker (about 2 s of
JavaScript on a crowded map); combat and work animations; a reflection or sky term for water; the
sub-pixel ship geometry (rigging, floats) that FXAA cannot recover.

## 15. Provenance

Every mesh, texture and pattern is generated by code in this repository at run time; no external
art asset, model, texture, font or library was added.
