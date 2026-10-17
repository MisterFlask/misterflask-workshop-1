# Terrain tile prompt template (canonical)

The recipe for SoC-direction base terrain tiles (one 2:1 isometric diamond each). Fill the
`[BRACKETS]`, generate, then mask + downscale. Reference set: `terrain-tiles.png` (the six base
tiles), `terrain-board.png` (tiled into a map with units).

## The prompt

> Create a single isometric base terrain tile for a dark fantasy 4X strategy game inspired by the
> visual aesthetic of Songs of Conquest and classic fantasy strategy games.
>
> **Terrain**
> - Terrain type: `[GRASSLAND / PLAINS / TUNDRA / DESERT / HILL / MARSH / SNOW / HELL / …]`
> - Biome mood: `[FERTILE / WINDSWEPT / ANCIENT / BLIGHTED / FROZEN / HAUNTED / …]`
> - Primary landform: `[FLAT GROUND / GENTLE RISE / ROCKY PATCH / SHALLOW WATER EDGE / …]`
> - Surface features: `[1–3 LARGE FEATURES ONLY]`
> - Primary colors: `[TWO DOMINANT COLORS]`
> - Accent color: `[ONE SMALL ACCENT]`
>
> **Art direction** — Handcrafted old-school fantasy pixel art, slightly gothic/dark-fantasy.
> Chunky deliberate pixel clusters, simplified forms, broad value grouping, selective high-contrast
> shading. Large readable shapes over texture; atmospheric and game-like, not painterly. ~12–20
> colors; forms read by shape and value first, color second. Rich controlled colors, deep shadows,
> subtle highlights; no muddy midtones or noise.
>
> **Isometric** — Fixed 2:1 projection from above (~26.565°); fits a standard iso grid; light from
> the upper-left; part of a seamless map, not a floating diorama; clean coherent ground plane,
> tileable next to other terrains; minimal vertical variation.
>
> **Technical** — a 2:1 isometric diamond of ground filling the canvas; one tile; no frame/UI/
> labels/text; no characters, buildings, roads or improvements; no exaggerated base/plinth/floating
> platform; crisp hard-edged pixels, no AA, no gradients/blur/soft rendering.
>
> **Readability** — identifiable at in-game size via 2–3 cues: ground color, one dominant surface
> pattern, one large secondary shape. No dense texture, scattered props, or excessive vegetation.
> Leave calm space for units, improvements, roads, borders, icons on top.
>
> **Tiling** — connects naturally with adjacent copies; edge treatment flexible, not over-specific.
>
> Critical: design for native low-res readability; do NOT make a detailed illustration and shrink it.

## Generate + post-process

1. `generate_image`, `gpt-image-2.5-flare`, **`quality low`**, `background opaque`,
   **`size 1536x768`** (2:1; `1024x512` is under the model's minimum pixel budget). The model draws
   one iso diamond on a flat backdrop.
2. Mask to a 2:1 diamond centred in the canvas (half-width ≈ 0.405·W, half-height ≈ 0.405·H),
   crop to the diamond bbox, downscale to **64×32** (LANCZOS). `scratchpad/process_terrain.py`.
3. On the map, draw each tile pixelated at 64×32; tiles sit edge-to-edge on the 64×32 grid.

## Known limits (next pass)

- **Repetition:** each tile is self-contained, so copies repeat identically (visible on desert
  dunes, plains stripes). Fix: generate **2–3 variants per terrain** and pick per-plot by a hash.
- **No edge blending:** terrains meet as hard diamond seams, not the transition bands a real terrain
  system uses. Acceptable for the SoC distinct-tile look; revisit if it reads too grid-like.
- Faint dark rim from the diamond mask — tighten the mask inset if needed.

## Example fill (grassland)

Terrain: GRASSLAND · mood: FERTILE · landform: FLAT GROUND · features: a few lit grass tufts + one
small wildflower cluster · colors: mossy green + lit yellow-green · accent: warm yellow.
