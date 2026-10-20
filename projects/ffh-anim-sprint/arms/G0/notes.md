# G0: one-call sprite sheet, no pose guides

## Method

One image call per round draws the whole strike as a 4x2 sprite sheet. Cell 1 holds the true sprite at 3x: it is
the character reference and it calibrates the slicer's single scale. Cells 2-8 are empty magenta. The model poses the
character from the beat text alone (`gen.sheet_prompt(..., guide="none")`), with the roster sheet as the second
image. Up to 3 rounds. A later round may only reword the per-cell texts or add one sentence via `extra=`. The best
single variant is sliced with `sheet.slice_sheet` and written with `ak.write_anim`.

Code: `arms/G0/make_g0.py` (`template` / `gen` / `slice` / `write` / `seq`). It rebuilds everything from the raws in
`work/G0/raw/`. Prompts are in `arms/G0/prompts/strike_r{1,2,3}.txt`.

## What I did (strike)

- **Round 1** (`strike_r1`, n=2): plain beat texts. Both variants were usable and followed the beats well
  without guides. Variant 1 kept every frame the same size as the reference (it also redrew cell 1 7 % larger, so
  calibration scaled everything back). Variant 2 drew cells 2-8 about 6 % larger (linear) than cell 1. Both drew the
  whole bottom row 27-35 px too high in canvas terms. Defects in variant 1: frame 2 has a pale, half-keyed patch on
  the shorts; frame 3's blade crosses its cell and leaves the safe region (x 10); frame 6 already has the sword fully
  back at the ready angle, so it repeats frame 7.
- **Round 2** (`strike_r2`): I reworded cell 3 (blade explicitly behind the head and shoulders), cell 4 (blade level
  at shoulder height, tip inside the cell) and cell 7 (sword only halfway back), and added one sentence: every figure
  the same size as cell 1, boots on cell 1's ground line, every sword inside its cell. Results: cell 4 and cell 7 got
  better in variant 2. The bottom row still floated 28-40 px high in both variants. Variant 1 grew about 7 %
  again. In both variants the sword in cell 2 went so high that it ran off the top of the canvas region.
- **Round 3** (`strike_r3`): round 2's wording, plus cell 2 reworded to keep the sword hand just above the
  forehead. Variant 1 fixed cell 2, but its cell 4 blade left the safe region again (x 11), cell 7 went back to
  a fully raised sword (repeating frame 7), and cell 5's shield stayed in front of the body. Variant 2 clipped the
  cell 2 blade again, held the cell 3 sword in front instead of behind the head, and grew about 7 %.
- **Picked `r2_2`** (round 2, variant 2) as a single variant, with no mixed frames. The three finalists were r1_1,
  r2_2 and r3_1. All three keep the character consistent at the same scale. r2_2 matches the most beats: frame 2
  has the blade behind the head, frame 3 has the blade level and inside the safe region, frame 4 has the shield swung
  back, and frame 6 is a real halfway recover. Its one clear defect is frame 1 (below). r3_1 frame 1 would fix that
  defect, but swapping it in would mix variants, which this method's "pick the best variant" rule does not do.
- **Slicer setting:** I called `slice_sheet(..., ground_snap=48)` instead of the default 20. In all six variants
  the model ignored the ground line in the empty bottom-row cells, even when the prompt asked for it. It drew those
  figures roughly centred in the 512 px cell, 27-42 canvas px above the ground row. Every strike beat has a foot on
  the ground, so snapping the lowest opaque row onto the ground row is the same correction the slicer already makes,
  just with a larger reach. For r2_2 the snaps were frames 4-7: +29, +28, +29, +29 px. No shared file was edited.

Gates by eye on `preview/strike_strip3x.png` and `strike_map1x.png`: the metrics flagged nothing (0 gate flags),
and I agree.
- No facing flip: he faces the lower-left in every frame.
- No hand swap: the sword stays in the screen-left (right) hand and the shield on the screen-right (left) arm
  throughout, including the wind-up.
- No missing prop: sword and shield are present in every frame.
- Frame 0 is the static sprite (enforced by `write_anim`). Every frame stays inside x 16-143.

## What worked

- Even without guides, the model reads the beat text well. It gives a clear anticipation (sword overhead, tip up
  and back), a wind-up with the blade behind the head, a level swing, a low lunge for contact with the blade
  slashing down-left, a follow-through with the tip near the ground, and a recover and settle. Facing and handedness
  were right in all 42 generated cells across 6 variants.
- The character stays consistent within a sheet: face, outfit, sword, shield and outline style. Colour drift is low
  (drift mean 0.97). Non-rigidity is high (mean 0.345; frames 1-5 at 0.40-0.44), so these are real redrawn poses,
  not transforms of frame 0.
- Rewording a cell's text reliably changed that cell's pose: the level blade in cell 4 and the halfway sword in
  cell 7 both came through in round 2.

## What failed

- **Frame 1:** the raised blade's tip is cut off flat at the canvas top. About 7 px of blade is lost (14 opaque
  pixels on row 0). Without a guide, the model has no idea where the top of the canvas region lies inside the cell.
  The round 3 wording fixed it in one variant and not the other.
- **No ground line in empty cells:** the bottom row floated 27-42 px high in every variant. The prompt sentence about
  the ground line did nothing. Only the slicer snap fixes it.
- **Scale is a coin flip:** in each round, one of the two variants drew cells 2-8 about 6-7 % larger (linear)
  than cell 1. The slicer calibrates scale from cell 1 alone, so that growth survives as a size pop at frame 0 to
  frame 1. The extra "same size as cell 1" sentence did not stop it. r2_2 is one of the good ones: frame 7 is +5 %
  in area (about 2.5 % linear) against the sprite.
- **Pose fidelity is loose where the text is vague:**
  - Frame 1's torso hardly leans back.
  - The front-foot step in frame 3 is small.
  - Frame 5 shows no hair trailing back.
  - Hair is a little fluffier than in the sprite in every redrawn frame, so frames 0 to 1 and 7 to 0 pop slightly in
    the hair.
- **Small keying leftovers:** 8-24 magenta-tinted fringe pixels per frame along the outlines, most around the sword
  and the hand in frames 3-4. They are visible at 3x and hard to see at 1x.
- The sword-tip arc is jumpy rather than smooth (onion: tip at (68,0), then (106,77), then (21,78)). At 50 ms, the
  frame 2 to 3 jump from behind the head to level in front reads as a snap. That is fine for a strike, but nothing
  in this method controls spacing.

## Spend and time

- 3 calls (n=2 each, 6 images), all tagged `anim/G0`. Ledger total **$0.2345**: $0.0775, $0.0784 and $0.0786.
- About 20 s per call.
- 8 minutes (481 s) of wall-clock time for the strike, including slicing, previews and these notes (`timings.jsonl`).

Extra deliverable: `arms/G0/sequence_strike_3x.{gif,mp4}` (one tile at 3x) and `sequence_strike_1x.{gif,mp4}`
(3x3 map at 1x; the MP4 is stored at 2x). Each shows the full strike, with 1 s of the ready pose before and after
(2.87 s total) at manifest timings. The ready pose is the static sprite because G0 has no idle yet. The shared
`preview.py --only sequence` (idle, strike, flinch, death) runs once all four animations exist.

## If I had another hour

- Make a fourth sample with the round 3 wording and pick per sheet again. The failures are per-variant coin
  flips (blade clipping, 6 % growth), so more samples would help more than more wording.
- Reuse this round's lessons for idle, flinch and death:
  - Plan to rely on the ground snap from the start.
  - Spell out where each prop sits in the cell, for example "the whole blade inside the cell, clear magenta above
    the tip".
  - Use concrete, visible wording for each cell rather than intent words ("leans back", "trailing back").
- Death needs a lying pose along the iso axis. With no guide, that is likely the hardest test for this method:
  expect a side-view body, not one lying on the tile.
