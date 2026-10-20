# G2: one-call sprite sheet that repaints arm R's rig frames

This file covers the **strike** (made by `arms/G2/make_g2.py`). Idle, flinch and death come in a later round.

## Method

One image call per round draws the whole strike as a 4x2 sprite sheet. Cell 1 holds the true sprite at 3x. It is
the character reference and it calibrates the slicer's single scale. Cells 2-8 hold arm R's finished cut-out rig
frames 1-7 (`ak.load_anim("R", "strike")`) as pose guides, so this is an M8-style repaint of a code-made blockout. The
prompt is `gen.sheet_prompt("strike", beat texts, guide="rig")`. It asks the model to keep each draft's pose and
position but redraw it as clean pixel art: repair the joints and let the hair, tunic and limbs bend naturally. The
roster sheet is the second image, n=2, and the frozen contract is used with no deviations. A later round may only
reword the per-cell texts or add one sentence via `extra=`, and R's frames never change. The best single variant is
sliced with `sheet.slice_sheet` (default settings) and written with `ak.write_anim`.

Code: `arms/G2/make_g2.py` (`template` / `gen` / `slice` / `write` / `seq`). It rebuilds everything from the raws in
`work/G2/raw/` and stops with an error if `arms/R/<anim>/manifest.json` is missing. Prompts are in
`arms/G2/prompts/strike_r{1,2,3}.txt`. The template is `work/G2/template_strike.png`. R's folder is read, never
written.

## What I did (strike)

- **Round 1** (`strike_r1`, n=2): plain beat texts with `guide="rig"`. Both variants kept the same character in all
  seven cells and followed the drafts closely. r1_2 was clearly closer to R. Its silhouette IoU against R's frames
  1-7 was 0.83, 0.44, 0.84, 0.59, 0.71, 0.84, 0.94; r1_1's was 0.65, 0.47, 0.75, 0.49, 0.65, 0.79, 0.71. r1_1 also
  drew its bottom row 12 px high, so it needed a 12 px ground snap, and its standing frames are 88 px tall, against
  86 px for the sprite. Two defects:
  - Both variants drew the cell 4 blade at full length, longer than R's foreshortened blade. Frame 3's tip landed
    at x 15 in r1_2 (1 px outside the safe region; `write_anim` warned) and at x 13 in r1_1.
  - Both moved the wind-up body (frame 2) about 8 px left of the draft and shortened its blade. r1_2's frame 2
    bounding box spans x 51-108, against R's 59-120.
- **Round 2** (`strike_r2`): I reworded cell 3 (keep the body where the draft puts it, and the blade sticking out
  right as far as the draft's) and cell 4 (blade no longer than the draft's, tip where the draft's tip ends). I added
  one sentence: keep each figure at its draft's position and every sword as long as its draft's sword. Results:
  - r2_2 fixed both problems. Frame 3's tip came back to x 21, and the frame 2 IoU rose to 0.67. It also had the
    lowest colour drift so far.
  - What r2_2 left: frame 4's blade is flatter than R's, pointing left and only slightly down, where the beat says
    "slashing down-left". Frames 6-7 have a rounder head and fuller hair than cell 1, so frame 7 pops into idle.
    Frame 7's sword also leans further left than the ready angle.
  - r2_1 drifted more in colour (mean 1.15 against about 0.9) and followed the wind-up worse (IoU 0.52).
- **Round 3** (`strike_r3`): round 2's wording, plus rewordings of two more cells. Cell 5 asks for the blade at the
  draft's steep down-left angle. Cell 8 asks for almost exactly cell 1's pose, with the sword at cell 1's angle. The
  one extra sentence was extended to also ask for cell 1's head, face and hair. It is still one sentence. Results:
  - r3_1 followed R best of all six variants: IoU 0.92, 0.87, 0.89, 0.69, 0.68, 0.89, 0.96.
  - r3_2's frame 7 was closest to the sprite (IoU 0.95). But its frame 1 hair grew noticeably, and its frame 2
    wind-up arm is drawn big over the head.
- **Picked `r3_1`** (round 3, variant 1). It is a single variant; no frames are mixed. Reasons:
  - It keeps the character the most consistent where it matters most, on the return to idle. Frame 7's silhouette
    IoU with the sprite is 0.92 (r2_2: 0.90), and its head and face are closer to cell 1's than r2_2's.
  - It has the closest pose fidelity to R's drafts. The sword-tip arc tracks R's within 1-5 px on every frame.
    G2: (97,17), (119,66), (21,51), (21,111), (39,122), (40,57), (46,56). R: (95,16), (119,66), (22,50), (20,106),
    (34,121), (40,59), (46,54).
  - Every frame is inside the safe region (min x 20), and the ground snaps were only 1-3 px.
  - Against r2_2 I gave up some wind-up coil: r2_2's frame 2 is more crouched and wider-legged, which reads a little
    better as "coiled".
- **Slicer:** default settings. The scale calibration was k 1.004 in every variant, and the reference-cell redraw
  error was ΔE 5.2-6.5 (r2_2 was the outlier at 9.3). No shared file was edited.
- **Gates by eye** on `preview/strike_strip3x.png` and `strike_map1x.png`: metrics flagged nothing (0 gate flags),
  and I agree.
  - No facing flip: he faces the lower-left in every frame.
  - No hand swap: the sword stays in the right hand (screen-left) and the shield on the left arm (screen-right).
  - Both props are present in all 8 frames.
  - In frame 2 the sword hand is hidden behind the head and the blade shows to the right of it. That is the wind-up
    R's draft draws, not a swap.
- **Extra deliverable (user request): the full-sequence GIF/MP4.** `python3 arms/G2/make_g2.py seq strike` writes
  `arms/G2/sequence_strike_3x.{gif,mp4}` (one tile, 3x) and `sequence_strike_1x.{gif,mp4}` (3x3 grass map at 1x; the
  MP4 is stored at 2x NEAREST). Each plays 1 s of the ready pose, the strike at the manifest timings, then 1 s of the
  ready pose, 2.87 s in all. It uses preview.py's scene and encoders, with the same layout as G1's strike sequence.
  When G2 has all four animations, `python3 lib/preview.py --arm G2 --only sequence` gives the standard full
  sequence.

## What worked

- **The guide is followed very closely.** With R's frames as drafts the model keeps every pose, the position in the
  cell and the sword-tip arc within a few pixels: frame 1-7 IoU against R is 0.68-0.96. That is tighter than G1
  reached against its flat blockouts (0.50-0.74). A rig frame carries the real character's pixels, so the model
  barely has to invent anything.
- **The character is the same in every frame,** with the sprite's palette and outline style. Colour drift is mean
  0.80 ΔE, about twice R's 0.41 but low for a redraw. Standing height is 86 px like the sprite. At 1x on the map
  strip the strike reads cleanly: raise, wind-up behind the head, a level swing, a low lunge, the blade down to the
  ground at the lower-left, and a recovery.
- **The repaint does what the method promises at the joints, though R left it little to repair.** R's frames were
  already clean (it had fixed its seams in code), so the visible gain is modest. The rotated-part look is gone: limbs
  are redrawn as continuous shapes with hand-drawn shading, and the contact frame is lower and
  leaning harder than R's draft (frame 4 top y 46 against R's 42), which matches "body low and leaning in" better.
  The motion stays clearly non-rigid: nonrigid 0.34-0.42 on frames 1-5, mean 0.295.
- **It is cheap and quick:** three n=2 calls, about 20 s each.

## What failed

- **The repaint smooths away some of R's animation work.** Overlapping action drops from R's 1.30 to 1.09. The peak
  frames are steel 2, tunic 5, hair 3, wood 3, against R's steel 2, tunic 5, hair 4, wood 2. R's one-frame hair lag
  on the whip forward is mostly gone. Frame 5 does not show the hair trailing back that the beat asks for, and the
  wording in round 3 did not bring it back.
- **Small redraw pops on entry and exit.** Frames 1-7 are the model's drawing, not the sprite's pixels. The hair is
  a little fuller and rounder, and the face is drawn slightly differently. Frame 0 to frame 1 and frame 7 back to
  idle each show a small change of the head (frame 7 silhouette IoU with the sprite 0.92). R has no such pop.
  Boil (frame-to-frame change) is mean 95.
- **Frame 2 (wind-up):** like R's draft, the sword hand is hidden behind the head. At 1x the blade seems to come out
  of the right side of the head, and the body is less coiled than in r2_2.
- **Frame 4 (contact):** the shield is not visibly "swung back for balance". It stays at the side, as in R's draft.
  The model follows the draft rather than the beat text where the two disagree.
- **Frames 6 and 7 are nearly the same drawing.** Frame 7 is the ready pose with the sword a touch lower, so the
  settle is barely visible.
- **Wording control is coarse.** Each round fixed the targeted defect in one variant and introduced a new one in the
  other: r1 tip outside the safe region; r2_2 flat blade at contact and a pop on the return; r3_2 hair growth in
  frame 1. Picking a variant is still a choice between trade-offs.

## Spend and time

- Three calls (rounds 1-3, n=2 each), six output images. Ledger spend tagged `anim/G2`: **$0.236** (0.078 + 0.079 +
  0.079). Latency was 17.7-21.3 s per call.
- Time for the strike, from template to checks: **477 s, about 8 min** (`arms/G2/timings.jsonl`). This excludes
  reading the brief.

## If I had another hour

- **Test the frame 2 trade-off directly.** Mix r2_2's frame 2 (more coiled) into r3_1, or reword cell 3 once more
  to ask for a crouch, and check at 1x whether the mixed redraw shows a pop between frames 1-2-3.
- **Repaint over R's hair lag explicitly.** Give R's hair-lag frames a cell text that names the trailing hair
  ("the hair still swept to the upper-right as in the draft") to see whether the overlap survives the repaint.
- **Try the paste-back finish pass** (`finish.paste_back`) on frames 6-7 against the sprite's head, to remove the
  return-to-idle pop. That is a round-2 finish step, so it is not used here.
