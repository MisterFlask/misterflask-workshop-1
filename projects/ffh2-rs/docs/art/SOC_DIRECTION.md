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
| D3 | **Authoring** | text `.spr`, JS-drawn | AI-generated + downscaled **PNG sprites** | add a PNG sprite path; workflow in §6 |
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

## 6. Generation workflow (as used for the references)

`edit_image` (OpenAI `gpt-image`) locked to `soc/style-anchor-reference.png` + a per-unit prompt,
1024², transparent background, then trim + LANCZOS downscale to target height with a light
edge/outline cleanup. `gpt-image-2.5-sunburst` was intermittently 502-ing; `flare` is the reliable
default. The banner is one neutral asset tinted per team by a luminance→team-colour ramp over its
low-saturation cloth.

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
