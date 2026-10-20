# Art mode M7: HD pixel isometric

M7 is a bright, saturated "HD pixel art" look for the isometric map, close to the reference screenshot in `ref/reference_full.png`. An image model (OpenAI `gpt-image-2.5-sunburst`) draws the art. Code handles everything around it: anchors, background removal, scaling, seamless tiles, footprint geometry and placement.

This folder is self-contained. It holds the tooling, the exact prompts, the style anchors, and the 20 sprites accepted so far (the starting roster). A future session can use it to make **new** units, buildings, terrain tiles and features that match. The package was tested that way: a fresh session given only this README made a Hill Giant, a Sheaim mage, a Mage Guild, a jungle and a pasture. All five are now in the roster.

| | Measured |
|---|---|
| New unit from a text description | 1–2 min, about $0.10 (Lich 74 s, Hill Giant 119 s) |
| New building | about 2 min, $0.10–0.40 (Mage Guild 105 s, $0.13) |
| New feature or improvement tile | 1–1.5 min, $0.07–0.10 (jungle 87 s, pasture 70 s) |
| New base terrain (swatch) | 1.5–5 min, $0.10–0.25 (tundra needed a second round) |
| Variants per attempt | 3, generated in parallel; you pick one |
| Second round needed | about 25–35% of assets |

Costs are estimates from a deliberately pessimistic price table (`lib/imggen.py`); real cost is likely lower.

## Contents

```
README.md            this document
m7.py                the one command you run (unit / building / feature / tile / recut / retile / accept / roster-sheet / scene / spend)
budget.json          spend caps; every API call is checked against these first
lib/                 imggen.py (API + spend ledger), cutout.py (keying/scaling/anchoring),
                     tiles.py (seamless tiles, inset, lift+shadow), cleanup.py (magenta fringe),
                     compose.py (map compositor = the placement contract in code)
prompts/             style_unit.txt, style_building.txt, style_terrain.txt (verbatim; change only deliberately)
prompts/assets/      the full prompt that produced each accepted asset (+ any revision anchor image)
ref/                 the reference screenshot, crops of it, palette.json
anchors/             style anchor sheets fed to the model (see the sections below)
roster/              accepted sprites (units/, buildings/, terrain/) + manifest.json
layouts/             test and reference maps for `m7.py scene`
work/<name>/         scratch, created per asset: raw/, cut/, review_<round>.png, calls.jsonl, prompt_<round>.txt, job_<round>.json
```
`work/` is scratch: `accept` copies everything needed to reproduce an asset into `roster/` and `prompts/assets/`. Don't commit `work/` to the repo.

## In the ffh2-rs repository

This folder lives at `projects/ffh2-rs/tools/m7`; the game's isometric renderer uses its roster
(`docs/ISO.md` has the renderer's side: what game object maps to which key, what the engine
draws). Changes made here since the package arrived:

- **`python3 m7.py export`** copies the roster into the client (`crates/ffh-play/assets/app/iso`)
  and writes its index `iso/sprites.js`. Run it after every `accept`, then build.
- **Overlays:** an inset tile also writes its object alone, and `accept` stores it as
  `roster/terrain/ovl_<name>.png` (192 x 128, the tile's anchor). The game draws overlays over
  any base tile, so farms, hills and quarries need no per-terrain copies; inset tiles themselves
  are not exported.
- **Resources** (`res_*`) are made on the feature path at width 70-110; the game stands them on
  the plot's front-right quarter (anchor 40 px right of and 30 px below the tile centre).
- The 1-tile **geometry sentence** now ends with the "CAMERA CHECK" sentence (`CAMERA_CHECK` in
  `m7.py`): without it the model drew walled footprints at about 1.4:1 (base cover 0.60-0.71).
- `accept` keeps the sprite's own `size` in the manifest (the image-generation size is
  `gen_size`; it used to overwrite `size`).
- `budget.json` caps this folder's spend; `spend.jsonl` is committed, so the caps count every call
  made for the game so far.

## Setup

```bash
pip install pillow numpy scipy requests
export OPENAI_API_KEY=sk-...                 # PowerShell: $env:OPENAI_API_KEY="sk-..."
cd ffh-art-mode-m7
python m7.py unit --name unit_test --desc "a test" --dry-run   # prints prompt + image list, no API call
```

**Budget.** `budget.json` caps spend for the tags `m7/units`, `m7/buildings` and `m7/terrain` (root `m7`), plus a global `total`.
- A call that would exceed a cap is refused before it is sent.
- `python m7.py spend` shows totals per root and per sub-tag next to the caps.
- Raise the caps for bulk work.

**Platform and concurrency notes**
- On Windows, pass paths as `C:/...` or relative paths. Git Bash `/c/...` paths fail inside Python.
- Avoid very long working directories.
- Use **one package folder per concurrent session**. `spend.jsonl`, `work/` and `roster/manifest.json` are shared files. Merge rosters afterwards.

## The asset contract (what the game's art-mode loader must do)

All sizes are display pixels, with no further upscaling. The projection is 2:1 isometric, lit from the upper-left. `lib/compose.py` implements all of it and is the reference implementation.

| Asset | Key / file | Canvas | Anchor (canvas point → map point) |
|---|---|---|---|
| Terrain tile | `tile_<name>.png` | 192×128 | Canvas top-left = tile top-left. The top face is the diamond (96,0) (192,48) (96,96) (0,48). Rows below it are a 30 px skirt (an earth or water side face), visible only at map edges and banks. |
| Feature | `feat_<name>.png` | ≥192 wide, any height | Canvas bottom-centre → the tile's bottom vertex (tile_x+96, tile_y+96). Mountains are 228 wide so neighbours merge into a range. |
| Building, 1 tile | `bld_<name>.png` | 192 wide, ≤256 tall | Canvas bottom-centre → the tile's bottom vertex. |
| Building, 2×2 | `bld_<name>.png` | 384 wide, ≤320 tall | Canvas bottom-centre → the bottom vertex of the **front-most** footprint tile (r+1, c+1). |
| Unit | `unit_<name>.png` | 96×120; grows for big units (Hill Giant 107×134) | Canvas bottom-centre → the tile centre (tile_x+96, tile_y+48+6). Feet sit 2 px above the canvas bottom. |

**Map placement**
- Tile (r, c) goes at screen `x = X0 + (c − r)·96`, `y = Y0 + (c + r)·48`.
- Draw tiles back to front by (r + c, r).
- Draw water tiles 8 px lower. The land tiles' earth skirts then show as brown banks.
- Then draw features, buildings and units, sorted by the depth (r + c) of their front-most tile. On the same tile, the order is feature, building, unit.

**Drawn by the engine, not baked into sprites:**
- the faction banner beside each unit (`draw_banner`);
- the unit's soft contact shadow (`draw_shadow`);
- the tile selection highlight (`draw_highlight`).

Faction colour therefore stays a runtime choice. Place the banner just left of the sprite's **opaque bounding box**, not at a fixed offset, so big units don't hide it. compose.py does this: `x = anchor_x + min(−34, bbox_left − w/2 − 10)`.

Buildings carry their own flags. Features carry their own lift and drop shadow.

**`roster/manifest.json`** has one entry per asset. Each records:
- kind, file, size and anchor rule;
- the processing settings: height, width, footprint, mode, k, p, skirt and so on;
- the ordered anchor images and the prompt file.

An art-mode loader can read sizes and anchors from it instead of hard-coding them.

## How every asset is made

1. **Describe it visually.**
   - The model doesn't know what "Hyborem" or "Priest of Leaves" looks like. Give the Fall from Heaven name and faction first, then say what to draw: body, colours, props, silhouette.
   - Templates are below. Write long descriptions to a file and pass `--desc-file`.
2. **Generate.**
   - `python m7.py <kind> --name <key> --desc "..."` fires 3 concurrent edit calls at medium quality, 1024×1024 (1536×1024 for 2×2 buildings and inset objects). About 20 s per batch.
   - Each call gets the fixed style prompt for its kind plus your description, the anchor images in order, and a request for a flat #FF00FF magenta background.
3. **Code normalises every variant.**
   - Key out the background. Enclosed pockets of background (inside a bow, between legs) are cleared too, but only when they really are near-flat key colour; magenta-ish *art* survives.
   - Despill the edges, drop specks, trim, and scale with premultiplied LANCZOS (units to a figure height, everything else to a width).
   - Optionally re-grid to 2 px blocks (buildings and terrain).
   - Place on the spec canvas with the spec anchor.
   - Remove leftover magenta fringe with edge-only rules.
   - Tiles also go through the seamless-tile maths.
   - Each variant prints its stats: pockets cleared or kept, fringe pixels removed, and the geometry checks for buildings.
4. **Look at `work/<name>/review_<round>.png`.**
   - Top row: the variants enlarged next to roster references (swordsman and Hyborem for units, grass and farm for tiles, forest for features).
   - Bottom row: each variant on a small test map beside accepted assets.
   - Pick by holistic look at 1×. Then prefer dark **coloured** outlines over black, a clear silhouette, the strongest faction cues, and effects that stay readable at 1×.
5. **Revise only if no variant is good enough.**
   - `--round r2 --revise r1:<k>` appends that raw output as the **last** image, at full size.
   - Description: "<Name>, redrawn as a cleaner, more readable version of the <thing> in the last image" plus concrete fixes.
   - Revisions are good for colour, outline, contrast and simplification, and bad for structural or footprint changes; start a fresh round for those.
   - A revision may **mirror or re-pose** the figure, so check facing again.
   - Free re-processing, no API cost: `recut` (`--height`, `--width`, `--pixel`, `--lift`, `--shadow`, `--holes`) and `retile` (`--k`, `--p`, `--seed`, `--cx`, `--cy`, `--skirt`, `--obj-width`, `--bottom`). Both overwrite that round's cuts and review sheet.
6. **Accept.** `python m7.py accept --name <key> --variant <k> [--round r2] [--mirror] [--as other_key]`. This:
   - copies the sprite into `roster/`;
   - saves the exact prompt (and any revision anchor image) to `prompts/assets/`;
   - writes a manifest entry with all settings.
7. **Keep the roster anchor current (units).** After accepting units for a faction, rebuild that faction's roster sheet (see Units).

Every call is logged with its full prompt and ordered images, in `work/<name>/calls.jsonl` and `spend.jsonl`.

## Previewing in a scene

```bash
cp layouts/layout_ref.json work/my_map.json      # edit it, then:
python m7.py scene --layout work/my_map.json --out work/my_map.png
```

**Layout format**
- `terrain`: rows of tile codes. Defaults: `G` grass, `F` farm, `W` water, `H` hills.
- `features`: rows of feature codes. Defaults: `f` forest, `m` mountain, `.` none.
- `buildings`: `{"rc": [r, c], "type": "<key without bld_>", "footprint": 1|2}`.
- `units`: `{"rc": [r, c], "type": "<key without unit_>", "banner": "blue|red|green|purple|black|white|yellow|orange", "highlight": true}`.

**New assets** are mapped with `"terrain_codes": {"T": "tile_tundra"}` and `"feature_codes": {"j": "feat_jungle"}`. Any extra water-like tiles that should drop 8 px go in `"water_keys": ["tile_water", "tile_ocean"]`.

A key that isn't in `roster/` is drawn as a placeholder, and `scene` prints a warning naming it. Tall features hide the tiles behind them; that's expected.

## Units

```bash
python m7.py unit --name unit_hill_giant --height 132 --desc-file work/hill_giant.txt
```

**Anchors, in order.** Prompts can refer to "the second image", "the last image", and so on.
1. `anchors/anchor_units.png`: the master style sheet, four reference figures at 3×. Added automatically.
2. A **roster sheet** of accepted sprites in this game. The default is `anchors/anchor_roster_r1.png` (swordsman, worker, priest, Hyborem). This was the single strongest consistency lever: every round-2 variant matched the roster's outline colour, pixel size and proportions on the first try. Pass `--roster anchors/anchor_roster_<faction>.png` to use a faction sheet.
3. Optionally, one reference crop with `--ref` for a close archetype:
   - `ref/crop_archer.png` for bow units
   - `ref/crop_horseman.png` for mounted units
   - `ref/crop_worker.png` for civilians
   - `ref/crop_swordsman.png` for melee

   A crop also imposes its facing.

With a roster sheet attached, this sentence is appended automatically (override with `--extra`): "Match the outline colour, shading and pixel size of the attached roster sheet exactly (it shows the other units of this same game)."

**Description template** (it follows `prompts/style_unit.txt`):
```
a <UNIT NAME IN CAPS> from Fall from Heaven (<civ / faction / race>): <body type and build>, <skin / hair / face cues>,
<clothing and armour with 2-3 named colours>, <primary weapon or prop and how it is held>, <secondary prop>.
<FX sentence written as SOLID opaque pixels only, e.g. "drawn as a few SOLID opaque pixel highlights only (...),
not translucent haze or smoke"  -  or "No glow, no magic effects." for mundane units>.
<size sentence relative to the roster, e.g. "Same size and proportions as the other units in the attached roster
sheet, slightly taller and thinner"  or  "much bulkier than the human swordsman, as big as the demon in the roster">.
Compact, chunky, readable silhouette.
```
Put the one or two identity features that must survive in CAPS (TUSKS, WHITE fletching, RED war paint).

**Heights** (`--height`, the figure in px after trimming):
- 82–86 for ordinary humans (worker 82, swordsman 86).
- 88–90 for slim or tall figures and tall staves (elf archer 88, robed casters 90).
- About 108 for monsters (ogre, Hyborem). The judges felt 108 is only "slightly bigger".
- 125–135 for **towering** units. Tested: the Hill Giant at 132 reads as towering and still fits the tile; its canvas grows automatically.

The style prompt asks for chibi proportions. For giants and monsters, say "much bigger and more muscular than the chibi human units" in the description.

**Worked examples:** every unit in `roster/units/` has its exact prompt in `prompts/assets/`. Good starting points:
- `unit_lich.txt`: a caster, accepted first time in 74 s.
- `unit_hill_giant.txt`: a giant at height 132.
- `unit_hyborem.txt`: a revision round.

**Faction roster sheets.** For a full game, keep one sheet of 4–6 accepted units per civilization so colours stay on-faction. Include one human-scale unit, plus a big unit when you're about to make a big one:
```bash
python m7.py roster-sheet --out anchors/anchor_roster_dark.png roster/units/unit_swordsman.png roster/units/unit_hyborem.png roster/units/unit_lich.png roster/units/unit_sheaim_mage.png
```

**Magenta, pink or purple effects (Sheaim, infernals, fey).** The background key is magenta, so these effects need care.
- The keyer only clears enclosed pockets that are near-flat key colour, so painted magenta orbs and portals survive. Check the stat `pocket_px_kept_as_art`.
- Prompt for effects as **filled** discs or swirls of deep violet plus hot rose-pink with pale cores, outlined, and add: "Do NOT use pure bright magenta (#FF00FF) anywhere on the figure; that colour is reserved for the background."
- If the design really needs pure magenta, generate on another key with `--key '#00ffff'`. The prompt and cutout switch to cyan automatically. Don't use a cyan key for water or ice art.

**Other unit pitfalls**
- **Effects turn to noise at 1×.** Describe them as a few SOLID opaque pixels, and prefer one bold flame element over flames on every edge. Hyborem's first round failed this way.
- **Black outlines.** The model sometimes draws near-black outlines instead of dark brown or navy. Handle it by picking variants.
- **Facing isn't controlled.** Fix it with `accept --mirror`.
- **Tiny detached sparks.** Bits under about 40 raw px (roughly 7×7 of 1024) are dropped as specks. Ask for bigger sparks, or keep them touching the body.
- **Don't re-grid units** (`--pixel 2/3`). It breaks faces. Units stay soft-edged on purpose.
- **Untested:** mounted, multi-figure and naval units. Expect a second round. Use `ref/crop_horseman.png` for riders, and check the canvas width.

## Buildings

```bash
python m7.py building --name bld_mage_guild --footprint 1 --ref ref/crop_city.png --ref anchors/buildings/crop_keep.png --desc-file work/mage_guild.txt
python m7.py building --name bld_elven_city --footprint 2 --ref ref/crop_forest.png --ref anchors/buildings/crop_houses.png --desc "..."
```

**Anchors: always three images.**
1. The closest reference crop by subject:
   - `ref/crop_city.png` for fortified or urban buildings
   - `ref/crop_forest.png` for nature or elven ones
   - `anchors/buildings/crop_houses.png` for small civic or rural ones
   - `ref/crop_mountains.png` for dwarven or mountain ones
   - `ref/crop_river_bridge.png` for stonework
2. A second crop for materials or detail (`crop_houses` or `crop_keep`).
3. `anchors/buildings/house_style_city_raw.png`, the accepted city. Added automatically; it keeps outline weight, saturation and stone colour consistent.

Pass `--ref` twice for images 1 and 2. If no crop fits, cut a new one from `ref/reference_full.png` (1448×1086) with `PIL.Image.crop((x0,y0,x1,y1))`, keeping the UI out. Upscale crops under about 120 px by 2× NEAREST.

**Description template** (it follows `prompts/style_building.txt`; aim for about 80–130 words):
```
the <NAME>, a <role> for a fantasy strategy game, on a single isometric diamond footprint tile:
<concrete materials and colours>, <one strong focal element>, an entrance/door facing the viewer (lower-left),
<2-3 supporting details>. <mood>, same chunky pixel style, outlines and lighting as the attached sprites.
```
A **geometry sentence** is appended automatically (turn it off with `--no-geometry`). It pins the base diamond's corners to the image edges, and that carries the scale, because the sprite is scaled by its **widest** element.
- The city covered 91% of its footprint and the Mage Guild 88%.
- The Temple of Leaves, made before this sentence existed, has a canopy wider than its base, so its base covers only about 70%.

**Checks** (each warns when it fails):
- the bottom corner is within 3 px of the anchor;
- the sprite doesn't float above the anchor;
- the base spans at least 85% of the footprint width at the left/right vertex row;
- height is at most 256 for 1 tile, or 320 for 2×2.

**Revisions** (`--round r2 --revise r1:<k> --quality high`) work for colour, outline and contrast fixes. For example: "a refined version of the last attached image (my draft of X): keep its exact composition and footprint (...), but make the stone cool light GREY (not cream), bolder dark outlines, more contrast between sunlit left faces and shaded right faces". They fail at structural fixes: "fill the footprint with a platform" turned the temple into a busy slab. Re-prompt from scratch instead.

**Worked examples:**
- `prompts/assets/bld_city.txt`: 2×2, high-quality refine round.
- `bld_temple_leaves.txt`: 1 tile.
- `bld_mage_guild.txt`: 1 tile, first round, made from this README.

**Pitfalls**
- **Large raws as anchors.** The automatic house-style anchor and any `--revise` raw are sent full-size, about 1000+ input tokens each. That is affordable, but it is why revision calls cost about 30% more.
- **Untested:** footprints of 3×3 or more.

## Terrain

There are three paths. Pick by what the asset is.

### a) Base terrain that covers the whole tile: the swatch path
Use this for grassland, plains, desert, tundra, snow, marsh, ocean, lava or hell ground.
```bash
python m7.py tile --name tile_desert --mode swatch --skirt 214,176,108 \
  --ref ref/crop_coast_boat.png --ref ref/crop_mountains.png --ref anchors/terrain/anchor_mapmid.png \
  --desc "Paint ONE NEW image: a seamless, full-frame, edge-to-edge ground texture of open DESERT in exactly this art style, seen from the same high isometric camera. Only warm golden sand: pale and mid ochre tones with gentle variation, many small wind ripples and darker sand tufts lit from the top-left, a few tiny pebbles scattered sparsely. Chunky pixel-art elements about as big as the grass tufts in the attached images. Even density everywhere, no large features, no strong gradients. No dunes, no plants, no rocks, no buildings, no units, no tile grid lines, no border, no text. The sand fills the entire canvas uniformly."
```
**How it becomes seamless.** The model is never trusted to tile. `lib/tiles.py` resamples a window of the swatch in **lattice coordinates**: a runs along (96, −48) and b along (96, 48), exactly the screen offsets between neighbouring iso tiles. Any texture periodic in (a, b) therefore tiles with itself in every direction. Near the seams, a dithered cross-fade picks per art-pixel block whether to sample at a or at a+1, so nothing ghosts.

**Parameters**
- `--k`: source px per output px. Match the size of features to the **grass tile's tufts** (view the review at 2×).
  - Shipped values are 1.6–1.8 (grass, water). The tundra needed 3.0 because its revised swatch was drawn with bigger elements. The valid range is about 1.2–4.2.
  - If the swatch is drawn with very fine detail, it looks mushy at any k. Re-prompt for bigger, chunkier elements rather than lowering k.
- `--p`: pixel block size. Use 3 for grass, and 2 when there are small bright glints (water). It must divide 48.
- `--skirt`: the side face. `earth` has a **green grass lip** on top, which is wrong under non-grass terrain, so use an `r,g,b` body colour there: tundra 140,112,84; desert 214,176,108; snow 200,210,225; dark rock 96,92,104. The other options are `water` and `none`.
- Re-run free: `python m7.py retile --name tile_desert --k 2.2 --p 2`.

**Repetition.** A single swatch repeats visibly over a large block of tiles; the tundra showed this most. Make 2–3 variants per terrain and let the engine pick one per tile by hash. Cut extra variants from the same raw at no cost:
```bash
python m7.py retile --name tile_desert --seed 2 --cx 0.4 --cy 0.6
python m7.py accept --name tile_desert --variant 1 --as tile_desert_b
```
Alternatively, accept another generated variant `--as tile_desert_c`.

### b) Tile = base terrain + an object: the inset path
Use this for farms, hills, pasture, quarry, oasis, cottage plots, flood plains and other improvements.
```bash
python m7.py tile --name tile_pasture --mode inset --base roster/terrain/tile_grass.png --obj-width 164 --bottom 94 \
  --ref ref/crop_farm_fields.png --ref ref/crop_sheep.png --ref anchors/terrain/anchor_mapmid.png \
  --desc "Paint ONE NEW sprite in exactly this art style: an isometric PASTURE seen from the same high isometric camera - a flat diamond-shaped (2:1, twice as wide as tall) fenced meadow with three white fluffy sheep and a small wooden trough, enclosed by a low brown wooden post-and-rail fence. Centered, filling most of the width. No grass around it, no trees, no buildings, no people, no cliff/side faces."
```
The object is cut out and pasted inside the finished base tile's diamond, then re-masked and re-skirted. The tile edges stay pure base-tile pixels, so it joins its neighbours without seams. Inset mode defaults to `--p 2`, matching the shipped farm and hills.
- **`--obj-width`:** use 160–170. Fenced or tall objects come out about 96 px tall at 168 and get their top cropped, so use 160–164 for them. Anything over about 170 is clipped at the side vertices.
- **`--bottom`:** use 92–94; the diamond's bottom vertex is at y = 96.
- **`"top_cropped": true`** in the output means the object's top was cut off. Lower the width, or raise `--bottom`, with a free `retile`.
- **Base:** any finished tile, e.g. `--base roster/terrain/tile_desert.png` for an oasis.

Rejected in the sprint: asking the model to repaint a magenta diamond template. The geometry held, but at 192 px the content came out too small and faint.

### c) A feature standing on a tile: the feature path
Use this for forests, jungles, mountains, volcanoes, ruins, crystal spires, mana nodes and resources.
```bash
python m7.py feature --name feat_jungle --width 186 \
  --desc "Paint ONE NEW sprite in exactly this art style: a JUNGLE map feature - a dense clump of 7 to 9 broad-leafed tropical trees and palms of varied heights with hanging vines, deep emerald and lime greens, dark outlines, lighter highlights on the top-left and deep shadow on the lower-right. The clump's footprint is a wide flat 2:1 isometric diamond (twice as wide as tall): trees at the back are higher on screen, trees at the front lower. Centered with margin. No ground plate, no grass base, no shadow on the ground, no other objects, no text."
```
**Pipeline.** Cut out, scale to `--width`, and re-grid to 2 px blocks. Then lift the sprite 6 px above the anchor (`--lift`) with a dark silhouette drop shadow (`--shadow`), and finish with an edge-only fringe recolour (`--edge-colour`: dark green by default; pick one that suits the asset).

**Width**
- 186 fills the footprint.
- About 228 makes neighbours merge (mountain ranges, dense jungle).
- 150–170 suits sparse resources and single ruins.
- 60–120 suits small props.

**Default anchors:** crop_forest, crop_mountains and anchor_mapmid. For rock, snow or volcanoes, use `ref/crop_mountains.png` and `anchors/terrain/anchor_mountains.png`.

**Terrain style prompt.** `prompts/style_terrain.txt` describes the map's palette: yellow-green grass, dark conical pines, royal-blue water, wheat-gold fields. For other biomes, name the new palette explicitly in the description ("Only warm golden sand: ..."), or the model drifts toward grass.

**Worked examples** (settings are in `roster/manifest.json`):
- swatch: `prompts/assets/tile_grass.txt`, `tile_water.txt`
- inset: `tile_farm.txt`, `tile_hills.txt`, `tile_pasture.txt`
- feature: `feat_forest.txt`, `feat_mountain.txt`, `feat_jungle.txt`

## Covering the rest of Fall from Heaven

| You need | Path | Notes |
|---|---|---|
| A civ's units (warriors, archers, adepts, priests, heroes) | unit | Build that civ's roster sheet after the first 2–3 units. Heroes get a bigger `--height`. |
| Demons, undead, animals, giants | unit | Height 108–135. Effects as solid pixels. Hyborem's prompt shows the revision pattern; the Hill Giant shows towering size. |
| Sheaim, infernal and fey magic | unit | See "Magenta, pink or purple effects". |
| Mounted units, ships, siege | unit (untested) | `--ref ref/crop_horseman.png` for riders, `ref/crop_coast_boat.png` for ships. Expect a second round. |
| Cities by culture (elven, dwarven, infernal) | building, `--footprint 2` | Reuse the bld_city prompt shape and swap the materials. |
| Buildings and wonders | building, `--footprint 1` (or 2) | Three anchors. Check that base coverage is at least 85%. The Mage Guild is the model example. |
| Base terrains (desert, tundra, snow, marsh, hell, ocean) | tile swatch | `r,g,b` skirt, 2–3 variants each, palette named in the description. |
| Improvements (mine, quarry, pasture, cottage, farm) | tile inset | Inset on the right base tile. |
| Forests, jungles, mountains, volcanoes, ruins, mana nodes, resources | feature | Width 60–228 by density. |
| Roads, rivers between tiles, coasts, props | not yet built | This is the main gap to the reference look. River and coast transitions need neighbour-aware overlays; roads run along tile edges. |

## Quality checklist before `accept`

- At 1× on the test map, does it look good and read instantly as the named FfH thing?
- Do its outline colour, pixel size and saturation match the roster references in the review?
- **Holes:** does background show through *inside* the figure (orbs, portals, windows)? Compare with the raw in `work/<name>/raw/`. `pocket_px_cleared` that is large on an asset with enclosed art is suspicious.
- Is there any pink or magenta fringe? Check at 2×.
- Do effects stay readable at 1×?
- **Units:** is the size right relative to the swordsman, and the facing?
- **Buildings:** are there no geometry warnings?
- **Tiles:** any seams on the test map? Any `top_cropped`?

## Troubleshooting

- **"unknown budget tag" / "BUDGET" errors:** add or raise the tag in `budget.json`.
- **Holes in magenta or purple art:**
  - The pocket keyer clears only near-flat key-coloured pockets, and `lib/cleanup.py` touches only semi-transparent pixels, near-pure #FF00FF, and opaque magenta-ish pixels within 3 px of the edge.
  - If art still gets damaged, the art itself is near-#FF00FF. Re-prompt with violet and rose-pink, or use `--key '#00ffff'`.
- **Sprite floats above its anchor:** `m7.py` re-trims and re-places after clean-up; the geometry check reports `bottom_row`.
- **Fake checkerboard background:** M7 never uses `--background transparent`. Always flat magenta (or `--key`).
- **A model call fails:** `lib/imggen.py` retries on 429 and 5xx errors and drops unsupported parameters on a 400. Check `work/<name>/calls.jsonl` and `spend.jsonl`.

## Provenance

M7 won a timed bake-off on 19 Oct 2026 against eight other methods, scored by three judges with holistic look weighted 50%.
- It placed #2 overall, a near-tie behind text-only prompting.
- It was **#1 for closeness to the reference style, for the city and for units**.
- In the round-2 speed test it was the fastest method on unseen units (47–74 s each).

Full report: https://claude.ai/artifact/W4HyFKYa8sXtTssRksW4yb

This package follows the sprint's M7 run in model, prompts, anchors, sizes, cutout parameters and seamless-tile maths. Re-processing the sprint's raw outputs through `m7.py` reproduces the shipped city, temple, forest and grass tile pixel-for-pixel; the Lich and the farm differ by 3 and 12 visible pixels (the stricter pocket keyer).

Improvements since the sprint (most found by the replication test):
- prompts, image order and processing settings are logged and saved on accept;
- the pocket keyer no longer eats magenta art, and the fringe clean-up is edge-only everywhere;
- buildings re-fit to the anchor after clean-up and warn when off-centre;
- the round-2 roster sheet is the default unit anchor, and the building geometry sentence is appended by default;
- inset tiles default to 2 px blocks;
- terrain variants can be cut with `--seed/--cx/--cy` and accepted `--as` another key;
- banners avoid big units;
- `--key` allows another background colour.
