# SOC_DIRECTION.md: new art direction ("Songs-of-Conquest", 2.5D isometric)

**Status: proposal, awaiting owner approval of the open decisions in §7.** The direction itself is
settled: this **replaces** the "Cheerful Readability" direction in `docs/ART.md`. Treat the current
`ART.md` as **void for direction** — its daylight mood, closed 56-colour palette and 32×32 plan-view
frame are discarded. (Its terrain/city/fog *mechanics* may be reused where convenient, but nothing
in it constrains the new look.) On approval, `ART.md` is rewritten to this; nothing is wired into
the client yet.

## 1. The decision

Two parts:

1. **The look:** the rich, handcrafted **2.5D pixel-art of *Songs of Conquest*** — detailed,
   painterly-dithered figures, dramatic directional light with a cool rim, a warm-but-muted
   fantasy palette, bold readable silhouettes. This is the owner's reference (the dark blue-and-gold
   caster on the readability sheet).
2. **The board:** a **2.5D isometric** map (diamond tiles seen at an angle), as in the UX mock and
   SoC itself — replacing the current north-up square-plan board.

Visual north-star (generated references, this folder): `soc/style-anchor-reference.png` (the finish
to match); `soc/unit-fighter.png`, `soc/unit-knight.png`, `soc/unit-treant.png` (a cohesive sample
faction); `soc/readability-128-96-64-32.png` (zoom sweep, §4); `soc/owner-banner-demo.png` (per-team
banner, §3).

## 2. The isometric board is a VIEW change only

This is the load-bearing constraint the owner set: **isometric is a rendering change, not a model
or gameplay change.**

- The simulation stays a **square plot grid** (`ffh-core` `CvMap`: plots at `x,y`, `plot_xy`,
  wrapping, areas, pathing — all unchanged). No gameplay, AI, RNG, or rules change.
- **Nothing in `ffh-core` / `ffh-data` changes. No save-format bump.** The change lives entirely in
  the web client's render/projection layer (`crates/ffh-play/web/art` + the view code that places
  tiles on screen).
- The client gains one **projection**: plot `(x,y)` → screen diamond, a fixed 2:1 iso transform.
  Everything the renderer already knows per tile (terrain, owner, feature, unit) is unchanged; only
  *where and in what shape* it is drawn changes. Map wrap still works — it is just the projection
  applied to the same grid.
- **Draw order** becomes back-to-front by grid row (painter's order for iso), which the current
  north-to-south row order already approximates.

So: model-preserving, but a real chunk of **client** rendering work (tiles become diamonds, the
camera/hit-testing map to iso, units/rings anchor to diamond centres).

## 3. What changes vs. the old system

| # | What | Old (discarded) | New | Recommended default |
|---|------|-----------------|-----|---------------------|
| D0 | **Projection** | 32×32 square, plan view | 2.5D isometric diamonds | 2:1 diamond, **64×32 px** tile footprint at 1× |
| D1 | **Unit size** | 32×32, fills tile | **Civ4-like: roughly tile-scaled, only slight overhang** | drawn ~1 tile tall (~52–64 px on screen at default zoom), the figure barely rises into the tile above; author ~96 px and downscale |
| D2 | **Palette** | closed 56-colour indexed | open / near-true colour for figures | open for units; terrain can reuse indexed passes at first |
| D3 | **Authoring** | text `.spr`, JS-drawn | AI-generated **native-64 PNG sprites** | 64×64 template (`soc/unit-prompt-template.md`) → quantize; §6 |
| D4 | **Mood** | cheerful daylight | **bright sunny daylight (CHOSEN)** | sunny base key; golden-hour kept as an optional later lighting pass |

Owner identity keeps the **team ring**, now drawn as an **isometric ellipse** under the figure's
feet on its diamond; a per-team **banner** (`soc/owner-banner-demo.png`, one neutral asset tinted at
load) is optional. Figures are authored **banner-less**; allegiance is the engine's.

## 4. The resolution reality (evidence)

`soc/readability-128-96-64-32.png` shrinks the sample units to each zoom step: clean to **64 px**,
holds at 96/128, and at **32 px it collapses to a silhouette**. So the new unit frame targets the
**64 px+ tile sizes**; at the smallest survey/chart zoom, units fall back to a simplified token, not
a blurred shrink.

## 5. Phased path (de-risk first)

**Implemented:** the isometric renderer is in the browser client as a remembered **Tools →
Options** choice (or `?renderer=iso`) — `crates/ffh-play/assets/app/iso.js`, a third renderer
beside flat and art. It draws the real `/api/map` + `/api/view` with PNG tiles and sprites on team
rings, owns its own projection/hit-testing, and embeds its PNGs via `http::ASSET_BYTES`. Run
`ffh-play serve --new` and pick "Isometric 2.5D" in Options. Every overworld entity is ported,
entity by entity:
- **Terrain tiles:** the full set — grass/plains/desert/snow/tundra/marsh, coast/ocean water, and
  the broken-lands/burning-sands/fields-of-perdition hell terrains (shallows reuses coast).
- **Relief:** peaks as snow-capped mountain sprites; hills as a convex dome shaded over any
  terrain (the plot centre stays flat so units/overlays anchor where hit-testing expects).
- **Features:** oasis, ice, scrub, flood plains, volcano and flames as one sprite per plot, sized
  per sprite. The wooded features (forest, young forest, jungle, ancient and burnt forest) are
  individual trees (pine, palm, great tree, dead tree; two variants each) scattered over the plot
  and jittered per plot, so a forest reads as one continuous stand.
- **Depth:** everything that stands up (trees, mountains, structures, resources, cities, units) is
  drawn in one pass sorted by where it touches the ground. A unit in a forest therefore stands
  among the trees: the ones behind it are drawn first, the ones in front over its legs. Trees stop
  below a unit's shoulders, and the front arc of its team ring is redrawn over them so ownership
  still shows. A tree that would hide much of a structure or resource icon is left out, opening a
  glade around it.
- **Rivers & roads:** rivers along the south/east plot edges (the diamond's lower-left/-right
  edges), roads/railroads connecting plot centres to routed neighbours.
- **Resources:** one icon per visual class (mana, crop, animal, fish, ore, goods) covering all
  ~58 bonuses, drawn at the tile front.
- **Improvements:** one structure per class (lair, fort, mana node, cottage, farm, mine) covering
  all ~81, incl. the lairs/forts that spawn on the map; sized per class, drawn under the resource
  icon so a worked resource still shows.
- **Cities:** settlement sprites on the team ring, with a capital star, size badge, name chip and a
  per-team owner banner (the neutral pennant tinted to the owner's colour).
- **Units:** eleven archetypes by keyword — knight (foot melee), cavalry (mounted), archer, lich
  (casters), spider (beasts), siege, ship (naval), demon, settler, scout, worker; the rest fall
  back to a labelled token.
- **View modes:** Terrain, Political (owner wash + territory borders) and Yields (per-plot
  food/production/commerce) all work in iso, matching the flat renderer's three layers.

Remaining (polish, not blocking): per-resource and per-improvement art beyond the class icons;
more unit archetypes (angels, dragons, named heroes); and, per §5 step 4, rewriting `ART.md` to
this direction now that it is proven on screen.

1. **Isometric bake test in the real client.** Add a minimal iso projection + a PNG-unit draw path:
   render a small patch of SoC diamond terrain, place one generated unit (Knight, 128 px) on its
   iso team ring, screenshot it. Proves projection + resolution + palette + the look together,
   model untouched. *(Deliverable: a screenshot.)*
2. **Lock the recipes.** The generation+import recipe for units (prompt/anchor, size, transparent
   bg + soft shadow, downscale/trim) and the first iso terrain diamonds, as repeatable tools under
   `web/art/tools/`.
3. **Board + units migration.** Iso projection for the whole map (terrain, features, cities, rings,
   borders, fog, hit-testing, camera), the unit roster as PNG sprites, zoom fallback (§4).
4. **Rewrite `ART.md`** to this direction once the look is proven on screen.

## 6. Generation workflow

Units use the **canonical 64×64 unit prompt template** (`soc/unit-prompt-template.md`): fill the
brackets, `generate_image` with `gpt-image-2.5-flare` at 1024² transparent, **quality `low`** (the
chunky style gains nothing from `high` — `low` is the chunkiest *and* ~9× cheaper; use `medium` when
a unit needs more shading), then downscale to a true **64×64** and crisp the alpha (palette
quantization optional), and confirm it reads at 64/48/32 px. Draw it on the map
pixelated at ~62 px. This replaced an earlier style-anchored 1024-illustration approach that drifted
smooth and lost the pixel craft. (`gpt-image-2.5-sunburst` was intermittently 502-ing; `flare` is
the reliable default.) The optional banner is one neutral asset tinted per team by a
luminance→team-colour ramp over its low-saturation cloth.

Reference outputs: `soc/knight-native-64.png` (native sprite), `soc/readability-native-64.png`
(reads at 64/48/32), `soc/board-native-64.png` (on the isometric board at Civ4 scale).

## 7. Open decisions for the owner

- **D4 — mood: RESOLVED → bright sunny daylight.** Neutral daylight keeps terrain types distinct
  and units readable; golden-hour is kept as an optional mood/lighting pass (dusk, screenshots, a
  possible time-of-day toggle), not the base board. Figures must be **daylight-keyed** (lit from the
  upper-left, bright) to sit on a sunny board, so the earlier dark reference units are re-generated
  in this key.
- **D0 — iso tile footprint** (64×32 default) and **D1 — on-screen unit size** (Civ4 proportions:
  ~1 tile tall, slight overhang — default ~56 px at 64-px tiles, not the towering SoC height).
- **D2 — how far the open palette extends** (units only, or terrain too from the start).

D4 is set (sunny). The remaining defaults stand unless changed; proceeding to §5 step 1 (the
isometric bake test).
