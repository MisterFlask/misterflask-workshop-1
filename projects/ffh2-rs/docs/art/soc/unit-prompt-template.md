# Unit sprite prompt template (canonical)

The recipe for SoC-direction unit sprites. Fill the `[BRACKETS]`, generate, then run the
post-process below. Reference result: `knight_native64.png` (native sprite),
`_native_readability.png` (reads at 64/48/32), `_iso_native.png` (on the board).

## The prompt

> Create a single 64×64 pixel-art character sprite for a dark fantasy 4X strategy game inspired by
> the visual aesthetic of Songs of Conquest and classic isometric fantasy strategy games.
>
> **Character**
> - Character: `[CHARACTER DESCRIPTION / CLASS / RACE]`
> - Silhouette: `[MOST RECOGNIZABLE BODY SHAPE]`
> - Equipment: `[ONE DISTINCTIVE WEAPON OR ITEM]`
> - Primary colors: `[TWO DOMINANT COLORS]`
> - Accent color: `[ONE BRIGHT ACCENT]`
> - Pose: `[SIMPLE, RECOGNIZABLE POSE]`
>
> **Art direction** — Handcrafted, old-school fantasy pixel art with a slightly gothic,
> dark-fantasy atmosphere. Chunky, deliberately placed pixel clusters, bold silhouettes,
> exaggerated fantasy proportions, selective high-contrast shading. Distinctive, immediately
> recognizable outline; exaggerate head, shoulders, weapon and defining features for readability at
> very small sizes. Restrained palette of ~16–24 colors, 3–4 shades per major material, large clean
> color regions over intricate textures. Deep shadows, warm highlights, rich but controlled colors;
> atmospheric, never muddy or noisy.
>
> **Technical** — Exactly 64×64 native pixels; full body within canvas; three-quarter perspective
> for a 2.5D strategy map; one character, centered; clear silhouette on light AND dark terrain;
> sharp hard-edged pixels, no antialiasing; no subpixel detail, gradients or smooth brushwork; at
> most one identifying magical glow; transparent background; small simple contact shadow under feet.
>
> **Readability** — Must be recognizable at native resolution without zooming. Prioritize class,
> weapon, posture, faction colors. Drop decorations that would be 1–2 indistinguishable pixels. No
> armor engravings, accessory clutter, hair strands, particle spam, or fine facial detail.
>
> **Output** — A single isolated, game-ready sprite. No UI, frames, labels, text, scenery,
> sprite-sheet layout, or extra characters.
>
> Critical: design for 64×64 from the outset; do NOT make a detailed high-res illustration and
> shrink it.

## Generate + post-process

The OpenAI `gpt-image` API cannot emit a literal 64×64 native sprite, so:

1. `generate_image`, model `gpt-image-2.5-flare`, `size 1024x1024`, **`quality low`**
   (canonical default — the chunky style gains nothing from `high`, which only adds finer detail
   that fights the 64px target and costs ~9× more; use `medium` when a unit needs more shading
   finesse), `background transparent`, `n 2`. The prompt's chunky/low-color constraints keep the
   1024 render blocky so it downsamples cleanly (NOT a smooth illustration being shrunk).
2. Trim to the alpha bounding box; fit to a **64×64** canvas (feet near the bottom row).
3. Crisp the alpha (threshold ~80 → 1-bit-ish edge). Palette quantization is **optional** — skip it
   unless a specific target needs it.
4. Verify at native: it must still read at **64, 48 and 32 px** (see `_native_readability.png`).
   On the map, draw it pixelated (`imageSmoothingEnabled = false`) at ~62 px (Civ4 scale).

`tools/` script for steps 2–4 lives beside the pipeline when the migration lands; for now it is
`scratchpad/native64.py` in the authoring session.

## Example fill (the reference Knight)

Character: a human knight, a heavily armored man-at-arms · Silhouette: broad-shouldered figure with
a tall plumed helm and a large kite shield · Equipment: an upright longsword · Primary colors: steel
grey and deep royal blue · Accent: bright gold · Pose: standing battle-ready, three-quarter facing
right, sword upright, shield forward.
