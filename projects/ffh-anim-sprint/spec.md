# Animation spec (shared by every arm)

All sizes are display pixels. Nothing here changes the M7 static-unit contract: the static
sprite `m7/roster/units/unit_swordsman.png` (96×120) stays the unit's still image, and every
animation frame is built so that idle frame 0 *is* that sprite.

## Canvas and anchor

| | |
|---|---|
| Frame canvas | **160×128** RGBA |
| Anchor | canvas point **(80, 126)** → the tile centre (`tile_x+96, tile_y+48+6`), exactly like the M7 unit rule (feet 2 px above the canvas bottom) |
| Static sprite inside the canvas | pasted unchanged at **(32, 8)** (its bottom-centre (48,118) lands on the anchor) |
| Safe region | opaque pixels must stay inside **x 16–143** (the region the 3× templates show); y 0–127 |
| Scale | the figure is 86 px tall standing. **One** scale per animation; never normalise each frame's height (a lying body would be blown up) |

Frames are drawn at 1×, soft-edged like the M7 sprites (premultiplied LANCZOS down-scales, no
re-gridding). Banner, contact shadow and tile highlight are drawn by the engine, not baked in.
The engine places the banner from the **idle frame-0 bounding box**, fixed per unit, so it
does not jump while the unit animates. A hit flash or spark may be an engine overlay, not part
of the frame (round 1 frames carry no FX at all).

## Facing

The sprite faces **lower-left** (3/4 view). Its sword is in the right hand, the **far** arm,
raised at screen-left; the round shield is on the **near** left arm at screen-right. The strike
goes toward the lower-left. Hits come from the lower-left, so flinch recoils and death falls
toward the upper-right (head upper-right, feet toward the attacker). Other facings are the
engine's job (mirror for lower-right; upper facings need back views, out of scope here).

## Animations

| Anim | Frames | Loop | Frame 0 | Notes |
|---|---|---|---|---|
| `idle` | 4 | yes | the static sprite, unchanged | subtle breathing; must loop without a pop |
| `strike` | 8 | no (returns to idle) | the static sprite, unchanged | `hit` event on frame 4 |
| `flinch` | 4 | no (returns to idle) | hit pose | the frame where damage registers is 0 |
| `death` | 8 | no; **last frame held** | same pose as flinch frame 0 | ends on the corpse |

Per-frame durations are in `beats.md` and in each manifest; arms may change them only with a
reason written in their `notes.md`.

## Files an arm writes

```
arms/<ARM>/<anim>/frame_00.png … frame_NN.png   160×128 RGBA
arms/<ARM>/<anim>/manifest.json
arms/<ARM>/sheet_<anim>.png                      frames side by side, 160 px apart
arms/<ARM>/notes.md  timings.jsonl  prompts/
```

`manifest.json`:
```json
{"unit": "swordsman", "arm": "G1", "anim": "strike", "canvas": [160, 128], "anchor": [80, 126],
 "loop": false, "hold_last": false,
 "frames": [{"file": "frame_00.png", "ms": 100, "events": []}, ...]}
```
`lib/animkit.py` writes all of this (`write_anim`); use it rather than hand-writing files.

## Gates (automatic fail for that animation)

- facing flip (the figure mirrored), or sword and shield swapping hands (outside death frames
  4–7 where the sword may leave the hand);
- a missing prop (no sword or no shield) in a frame where the beat sheet has it;
- frame 0 not pixel-identical to the static sprite (idle, strike).
