# G0 (rest): idle, flinch and death as one-call sprite sheets, no pose guides

The strike is in `notes.md`. This file covers the other three animations.

## Method

This uses the same method as the strike. Each round is one n=2 image call that draws a 4x2 sprite sheet. Cell 1 holds
the true sprite at 3x: it is the character reference, and it also sets the slicer's single scale. The other cells are
empty magenta, so the model poses the character from the cell text alone (`gen.sheet_prompt(..., guide="none")`). The
images are [template, `gen.ROSTER`], and the result is sliced with `sheet.slice_sheet(..., ground_snap=48)`, the strike
setting. Two sheets cover three animations:
- **Sheet A**: cells 2-4 are idle frames 1-3 and cells 5-8 are flinch frames 0-3.
- **Sheet B**: cells 2-8 are death frames 1-7. Death frame 0 is the accepted flinch frame 0 from sheet A.

Each sheet got 3 rounds. Later rounds only reworded cells or swapped the one `extra=` sentence.

Code: `arms/G0/make_g0_rest.py` (`template` / `gen` / `slice` / `write`). It rebuilds every frame from the raws in
`work/G0/rest/raw/`.
- Prompts are in `arms/G0/prompts/{idle,flinch}_sheetA_r{1,2,3}.txt`. The idle and flinch files are identical copies
  of the same sheet-A prompt.
- Death prompts are in `arms/G0/prompts/death_sheetB_r{1,2,3}.txt`.

Round 1 already used the strike's lessons:
- It reused the strike's picked `extra` sentence (same size as cell 1, boots on cell 1's ground line, swords inside the
  cell).
- It used the strike's ground snap.
- Its death text said in visible terms that the lying body runs diagonally, boots at the lower-left and head at the
  upper-right.

## What I did

### Idle (sheet A, cells 2-4)

- **r1:** the beat texts, each prefixed with "idle frame k:".
  - Variant 1: the idle cells were full redraws, not copies. Frames 1-2 were 4 px taller (head top at y 36 against
    the sprite's 40). The sword fist moved up and in toward the body, the blade stood more upright, and the stance was
    wider.
  - Variant 2: the model redrew cell 1 itself 14 % larger and 49 px higher (k=1.14). Its idle silhouettes matched the
    sprite with IoU 0.64-0.70 only.
- **r2:** each idle cell reworded to "an exact copy of the figure in cell 1 (same size, stance, sword angle and
  sword-hand position, shield), except breathing in: ...".
  - Variant 1: silhouette IoU against the sprite was 0.87, 0.74 and 0.73.
  - Variant 2: IoU 0.86, 0.80 and 0.89. Its head top is 1 px up in frames 1-2 and back in frame 3, exactly as the beats
    ask.
- **r3:** "a tracing of cell 1". The cell text named cell 1's actual pose: sword fist low beside the right hip, blade
  leaning out to the upper-left. The extra sentence was swapped for one that asks for cells 2-4 as near-identical
  tracings and keeps the bottom-row size clause.
  - Variant 1: IoU 0.84, 0.75 and 0.84.
  - Variant 2: IoU 0.83, 0.81 and 0.93.
  - The sword fist still moved up and in. Neither variant beat r2 variant 2 over the whole loop. I counted the pixels
    that change by more than 48 in any channel across the four transitions 0>1>2>3>0:
    - r2_2: 7487 px
    - r3_2: 8148 px
    - r3_1: 9556 px
- **Picked `A_r2_2`** cells 2-4. Frame 0 is the static sprite, enforced by `write_anim`.
  - This variant redrew the whole sheet 16 % large (k=1.163). That is just outside the slicer's 0.85-1.15 warning band,
    but the slicer's calibration from cell 1 scales it back correctly: the idle frames come out 86-87 px tall against
    the sprite's 86.
  - Ground snaps on this sheet: none in the top row, +26 px for every bottom-row cell.

### Flinch (sheet A, cells 5-8)

- **r1:** the beat texts, each prefixed with "flinch frame k:". Both variants were usable: eyes shut, arms flung out,
  a recoil crouch with the sword dipping, a recover and a settle. In neither did the head move back toward the
  upper-right.
- **r2:** frames 0 and 1 reworded. Frame 0: head knocked back toward the upper-right, chin up, clearly further right
  than in cell 1. Frame 1: leaning back further toward the upper-right, sword dipping to the lower-left. Boots stay
  planted.
  - Variant 1 has the strongest lean-back in frame 1.
  - Variant 2 tips the head back in frame 0 and returns closest to the ready pose: its frame 3 overlaps the sprite with
    IoU 0.87, against 0.73 for variant 1.
- **r3:** flinch wording unchanged from r2. Variant 1 has a magenta patch beside the shield in frame 1. Variant 2 is
  similar to r2, with its eyes still shut in frame 2.
- **Picked `A_r2_2`** cells 5-8: the same variant as idle, so the whole sheet A result comes from one image and no
  frames are mixed. Variant 2 also has the best settle back into idle, and its frame 1 sword dips as the beat says.

### Death (sheet B, cells 2-8; frame 0 = flinch frame 0 from `A_r2_2` cell 5)

- **r1:** the beat texts. Cells 6-8 described the lying body running diagonally from the boots at the lower-left to
  the head at the upper-right, "along the edge of an isometric floor tile".
  - The poses followed the beats well, and the lying body really does run on a diagonal: slope 0.29-0.41 by PCA of the
    body mask, against the iso axis's 0.5.
  - **Motion lines:** both variants drew arcs around the shield in cell 7, because "the shield rocks" invites them.
    These are FX and are not allowed.
  - **Shadows:** variant 1 drew dark-magenta contact shadows under the body in cells 5-8. The keyer keeps them.
  - **Size:** variant 2 drew cells 2-4 larger.
  - **Pose:** the knees were drawn up, so he is half sitting.
- **r2:** cell 2 gained the hit direction. Cell 6 asked for straight legs and no shadow under him. Cell 7 changed to
  "the shield tilted up a little on its rim (no motion lines)".
  - Both variants came out clean: no arcs and no shadows.
  - Variant 2 lies closest to the iso axis: slope 0.47, 0.63 (the bounce) and 0.45.
- **r3:** cell 6 asked for the shield lying flat, "seen from above as a wide oval", and cell 8 for straight legs.
  - The shields did come out flat, but both bodies turned flatter and more side-on (slope 0.20-0.28).
  - Variant 1's lying body is about 14 % smaller in area: a size pop.
  - Variant 2's frame 7 sword tip is at x 15, outside the safe region.
- **Picked `B_r2_2`** cells 2-8, with frame 0 from `A_r2_2` cell 5.
  - Ground snaps: none in the top row; +37, +32, +32 and +32 px for cells 5-8.
  - The slicer warns "cell 4 has more than one large figure". That cell is death frame 4, where the sword is drawn as
    a separate piece slipping out of the hand, as the beat asks.

### Gates, by eye

I checked `preview/{idle,flinch,death}_strip3x.png` and `_map1x.png`. The metrics flag nothing (0 gate flags for all
three) and I agree:
- **Facing:** he faces the lower-left in every standing frame. The corpse's head is at the upper-right and its feet at
  the lower-left.
- **Hands:** the sword stays in the screen-left (right) hand and the shield on the screen-right (left) arm. This holds
  in all idle and flinch frames and in death frames 0-3.
- **Props:** sword and shield are present in every frame. In death frames 4-7 the sword is free (in the air in frame
  4, on the ground at the lower-left in frames 5-7), which the beats allow.
- **Frame 0:** idle frame 0 is the static sprite. `write_anim` raised no safe-region warnings: every frame stays inside
  x 16-143.

The fringe left by keying is single magenta-tinted outline pixels, as in the strike:
- idle: 20-31 px per frame
- flinch: 36-44 px per frame
- death: 42-103 px per frame
- strike, measured the same way: 54-68 px per frame

## What worked

- **Flinch and death read well at 1x without any guide.**
  - Flinch: a clear impact (eyes shut, arms thrown out, sword up), a recoil crouch with the sword dipped, a recover,
    then a settle that hands back to idle smoothly.
  - Death: a stagger back with the sword flung up, buckling knees, a slumped kneel with the sword tip on the ground,
    a backward fall with the sword slipping away, a corpse, a bounce, and a still corpse with eyes closed.
  - Non-rigidity is 0.41-0.49 in every redrawn flinch and death frame, so these are real poses.
  - Colour drift is low: mean 0.81 for flinch and 0.95 for death.
- **The diagonal lying pose works better than the strike notes feared.** Describing the body as running "from his
  boots at the lower-left up to his head at the upper-right, along the edge of an isometric floor tile" gave a corpse
  lying on the tile, not a side-view figure tipped over. r2_2's body slope is about 0.46 against the iso 0.5.
- **Rewording removed whole defect classes in one round.** Motion arcs and contact shadows were gone in both r2
  variants of sheet B once cell 7 said "tilted up a little on its rim (no motion lines)" and cell 6 said "no shadow".
- **Planted feet:** ground snapping kept the lowest row at y 125 in every idle and flinch frame. Foot x std is 1.0 px
  for idle and 1.46 px for flinch.
- **Identity:** the character stayed consistent across both sheets. Death frame 0 comes from sheet A and frame 1 from
  sheet B, and that seam does not show at 1x.

## What failed

### Idle

- **It boils clearly.** Each of frames 1-3 is a fresh redraw, not a copy of the sprite. Every round asked for copies
  ("exact copy", then "tracing"), but in every variant the model:
  - moved the sword fist up and in toward the body and stood the blade more upright;
  - drew a slightly wider stance and a slightly larger shield;
  - changed the face.
  The model keeps cell 1 itself almost untouched (redraw IoU 0.91-0.98), but it will not repeat it in another cell.
- **Numbers for the picked r2_2:**
  - Silhouette IoU against the sprite: 0.86, 0.80 and 0.89.
  - Pixels changed per transition: 1489-2271 out of about 3400 opaque pixels.
  - Metric boil mean: 50.8. For comparison, the guided G1 idle is 43.9 and the transform-based A0 idle is 16.6.
- **At 1x in the map strip:** this reads as a twitch once a second, not breathing.
  - The sword jumps and the legs shift at frame 0>1 and again at 3>0, where the true sprite meets the redraws.
  - Frames 1>2>3 shimmer less but still visibly: the face, the hair outline and the shield rim move.
- **The beats themselves come out only roughly:** the head goes up 1 px in frames 1-2 and back in frame 3, as asked.
  But the sword-tip trail is (47,52) > (44,54) > (42,47) > (44,50): the tip wanders 3-5 px sideways and rises 5 px in
  frame 2, where the beat asks for 2 px.

### Flinch

- **Frame 0:** the head tips back with the eyes shut but stays roughly where it was. The beat wants it snapped toward
  the upper-right, and r2's explicit wording ("clearly further right than in cell 1") did not move it.
- **Frame 1:** a knees-bent crouch with the head turned back, not a clear lean back.
- **Frame 2:** the sword still points down-left. As a halfway point between the dip and the ready angle it is
  defensible, but it means the blade travels up only in frame 3.
- **Scale and position:** sheet A was drawn 16 % too large and its bottom row 26 px too high. Calibration and the
  ground snap correct both, but this method depends entirely on that correction.

### Death

- **The shield stands on its rim** beside him in frames 5-7 instead of lying flat. Asking for a flat oval (r3) fixed
  the shield but flattened the body toward a side view, so I did not take it.
- **The bounce is too big.** In frame 6 the head and shoulders lift about 11 px (bbox top 37 against 48 in frames 5
  and 7), where the beat asks for 2 px. At 80 ms it reads as a sit-up bounce.
- **The knees stay slightly bent** in the corpse.
- **Frame 3>4 is a big jump**, from kneeling to mid-air with the legs thrown forward. That is acceptable at 80 ms, but
  nothing in the method controls spacing.
- **Corpse placement:** the slicer's ground snap puts the corpse's lowest pixel (the sword or the boots) on the ground
  row, so the body sits on the front-left half of the tile with the head near the tile's right corner. It reads as
  lying on the tile, but the placement comes from the snap rule, not from an aimed position.

## Spend and time

- **Calls:** 6 calls (n=2 each, 12 images), all tagged `anim/G0`, all in `spend.jsonl`. Ledger total **$0.4723**.
  - Sheet A: $0.0781, $0.0792 and $0.0794.
  - Sheet B: $0.0782, $0.0786 and $0.0788.
  - About 18-21 s per call; rounds of A and B ran in parallel.
  - G0's tag total, including the strike, is now $0.7068 against the $1.2 cap.
- **Time:** about 15 minutes (878 s) of wall-clock for all three animations, including previews, the sequence and vignette
  renders, and these notes. `timings.jsonl` holds one row per animation, split in consecutive windows:
  - idle and flinch get 30 % of the batch each, and half of sheet A's cost each;
  - death gets 40 % of the batch and sheet B's cost.
  The split is approximate, because the two sheets were worked on interleaved.
- **Deliverables:** `python3 lib/preview.py --arm G0 --only sequence` and `--only vignette` were run once all four
  animations existed: `arms/G0/sequence_{1x,3x}.{gif,mp4}` and `arms/G0/vignette.{gif,mp4}`.

## If I had another hour

- **Idle:** stop spending rounds on it. Three wordings moved silhouette IoU only between 0.80 and 0.93, and the
  redrawn sword and stance pop is the same in every variant. A one-call sheet without guides cannot make a breathing
  idle out of redraws. The honest G0 result is "boils"; a usable idle needs frames derived from the sprite itself
  (transforms or a rig), which is another arm's method.
- **Flinch:** one more sample of the r2 wording, hoping for a variant whose frame 0 head actually moves to the
  upper-right. The failures here are per-variant, as in the strike.
- **Death:**
  - One more sample of the r2 wording.
  - Try a bounce wording in visible terms ("only his head lifts a finger's width off the ground").
  - Ask for the shield "lying on the ground, tilted" rather than "flat oval", so the body does not flatten with it.
