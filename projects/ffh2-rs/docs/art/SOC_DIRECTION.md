# SOC_DIRECTION.md: proposed new art direction ("Songs-of-Conquest")

**Status: proposal, awaiting owner approval.** On approval this supersedes the "Cheerful
Readability" direction in `docs/ART.md`, which is then rewritten to match. Nothing here is wired
into the client yet. It records a deliberate deviation from the current art system; see
`docs/PORTING.md` conventions for how deviations are tracked.

## 1. The decision

Replace the current direction with the **rich, handcrafted 2.5D pixel-art look of *Songs of
Conquest***: detailed, painterly-dithered figures, dramatic directional light with a cool rim,
a warm-but-muted fantasy palette, bold readable silhouettes. This is the look the owner has been
developing (the dark blue-and-gold caster on the readability test sheet) and it is close to the
"Monastic Pixel Noir" mood the October 2026 revamp moved *away* from — so this reverses that
revamp for figures, intentionally.

Visual north-star (generated references, this folder):

- `soc/style-anchor-reference.png` — the owner's reference character, the finish to match.
- `soc/unit-fighter.png`, `soc/unit-knight.png`, `soc/unit-treant.png` — a cohesive sample
  faction generated against that anchor (OpenAI `gpt-image`, `edit_image` locked to the anchor).
- `soc/readability-128-96-64-32.png` — those units shrunk to 128/96/64/32 px (see §4).
- `soc/owner-banner-demo.png` — one unit with a per-team banner layer (see §5).

## 2. Why this is a pipeline change, not an asset swap

The current client (`crates/ffh-play/web/art`) draws **units as text `.spr` sprites**, rasterized
by JS at load, every pixel from the **closed 56-colour palette**, in a **32×32** frame (feet at
15,29), owner shown by a **team ring** + cloth-slot remap (`docs/ART.md` §2, §3.3, §7.4, §10).

The SoC look cannot live in that system: its fidelity needs more pixels and more colours than a
32×32 / 56-colour text sprite can hold, and nobody hand-authors SoC-detail figures as `.spr` text.
So adopting it forces four concrete changes, each a real decision:

| # | What | Current | Proposed | Recommended default |
|---|------|---------|----------|---------------------|
| D1 | **Unit resolution** | 32×32, fills the tile | larger, overflows the tile upward (SoC figures are ~1.5–2 tiles tall) | author at **128 px tall**, draw ~**2 tiles** tall at default (64 px/tile) zoom |
| D2 | **Palette** | closed 56-colour indexed; fog/light are index remaps | SoC needs open/near-true colour for figures | **open palette for units**; keep the indexed palette + remap passes for terrain/UI for now |
| D3 | **Authoring** | text `.spr`, JS-drawn | AI-generated + downscaled **PNG sprites** | add a **PNG unit-sprite** code path; generation workflow in §6 |
| D4 | **Mood** | cheerful daylight | dark/dramatic, cool rim light | confirm dark is the target (the references are dark) |

Terrain, cities, borders, fog and UI can **stay on the existing system** initially — this is a
units-first migration, so most of `ART.md` and the whole terrain/city pipeline keep working while
figures move to the new look. That keeps the blast radius small and the change reversible.

## 3. Owner identity (keep what already works)

The **team ring stays the primary owner mark** (`ART.md` §7.4): it reads under any art, on any
terrain, and is already implemented. A per-team **banner** layer (demonstrated in
`soc/owner-banner-demo.png`, one neutral asset tinted per team at load) is an *optional* second
mark, not required — it does not replace the ring. This means figures are authored **banner-less**
and allegiance is the engine's job, exactly as today.

## 4. The resolution reality (evidence, not assumption)

`soc/readability-128-96-64-32.png` shrinks the sample units to each zoom step. Result: the look is
clean to **64 px** (the default 2× tile), softens but holds at 96/128, and at **32 px (survey
zoom) collapses to a silhouette** — the owner's own reference figure does the same. Implication for
D1: the new unit frame should target the **64 px+ tile sizes**; at survey/chart zoom units should
fall back to a simplified token or the existing marker, not a blurred shrink of the PNG.

## 5. Proposed phased path (de-risk first, no big-bang)

1. **Bake-test in the real client (1 unit).** Add a minimal PNG-unit draw path to the web client,
   load one generated unit at 128 px, draw it ~2 tiles tall on its team ring over real terrain,
   screenshot it. Proves D1–D3 end-to-end before committing. *(Deliverable: a screenshot, ~1 day.)*
2. **Lock the generation + import recipe.** Fix prompt/anchor, size, background handling
   (transparent, soft contact shadow), and the downscale/trim/pack step into a repeatable tool
   under `web/art/tools/`. Produce 3–4 units as a style-locked set.
3. **Units migration.** Generate the roster, wire the PNG path for all units, keep the ring,
   handle zoom fallback (§4). Rewrite `ART.md` §10 (units) to the new direction.
4. **Decide on terrain/cities.** Either keep the cheerful-indexed board under the darker figures
   (contrast can work) or extend the SoC treatment outward in a later pass. Full `ART.md` rewrite
   only once the look is proven.

## 6. Generation workflow (as used for the references)

`edit_image` (OpenAI `gpt-image`) locked to `soc/style-anchor-reference.png` + a per-unit prompt,
1024², transparent background, then trim + LANCZOS downscale to the target height, with a light
edge/outline cleanup pass. The `gpt-image-2.5-sunburst` model was intermittently 502-ing; `flare`
is the reliable default. One neutral banner is tinted per team by a luminance→team-colour ramp over
its low-saturation cloth pixels.

## 7. Open sub-decisions for the owner

D1 target height, D2 how far the open palette extends (units only vs. everything), D4 dark vs. a
lighter SoC key, and whether terrain/cities migrate now or later. Defaults above are my
recommendation; correct them here and I proceed to §5 step 1.
