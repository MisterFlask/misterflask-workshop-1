# G1: one-call sprite sheet with flat-colour pose guides

This file covers the **strike** (made by `arms/G1/make_g1.py`). Idle, flinch and death for G1 are being made by a
separate G1 agent (`arms/G1/make_g1_rest.py`, raws in `work/G1/rest/`); see that agent's section if it adds one.

## Method

One image call per round draws the whole strike as a 4x2 sprite sheet. Cell 1 holds the true sprite at 3x: it is
the character reference and it calibrates the slicer's single scale. Cells 2-8 hold flat-colour blockouts of strike
frames 1-7 from the shared `poses.json` (`guides.render_guide(pose, "colour")`), so the model gets explicit pose
control. Prompt: `gen.sheet_prompt("strike", beat texts, guide="colour")`, with the roster sheet as the second image,
n=2. Up to 3 rounds. A later round may only reword the per-cell texts or add one sentence via `extra=`, and the
guides never change. The best single variant is sliced with `sheet.slice_sheet` (default settings) and written with
`ak.write_anim`.

Code: `arms/G1/make_g1.py` (`template` / `gen` / `slice` / `write` / `seq`). It rebuilds everything from the raws in
`work/G1/raw/`. Prompts are in `arms/G1/prompts/strike_r{1,2,3}.txt`. The template is byte-identical to the
calibration's `template_strike_colour.png`. All calls were fresh; no calibration raws were reused.

## What I did (strike)

- **Round 1** (`strike_r1`, n=2): the plain beat texts, which is exactly the calibration prompt. Both variants
  followed every blockout, kept the same character, and kept the standing height at 86 px. Two defects:
  - In both variants the model drew the sword at the sprite's full length, which is longer than the foreshortened
    blockout blade in cell 4. Frame 3's tip left the safe region: x 14 in r1_1 (9 px outside x 16) and x 9 in r1_2
    (49 px outside).
  - r1_2 also drew the raised blade in frame 1 past the top of the canvas region, so it is clipped flat on row 0
    (7 px).
- **Round 2** (`strike_r2`): I reworded cell 2 (the blade tip ends where the blockout's tip ends, with magenta above
  it) and cell 4 (the blade no longer than the blockout's, its tip ending where the blockout's ends). I added one
  sentence: keep the hair as small and neat as in cell 1, and draw the sword as long as cell 1's sword, following the
  blockout blade's length and tip position. Results:
  - Both variants stayed inside the safe region (frame 3 at x 19 and x 16) and clear of the top (frame 1 top at y 11
    and y 14).
  - In r2_2, the frame 7 sword tilts well past the ready angle: its tip is 15 px from the pose tip, which pops on the
    return to idle.
- **Round 3** (`strike_r3`): round 2's wording, plus concrete wording for cell 6 (hair swept back toward the
  upper-right, chest turned toward the lower-left) and cell 8 (sword at the same angle as in cell 1). Results:
  - The cell 6 wording changed nothing visible in either variant.
  - r3_2 added a brown strap or lacing across the tunic chest in frames 1-2, which is outfit drift.
  - r3_1 drew frames 6-7 with hair 2 px taller than the sprite (top y 38 against 40).
  - Both drew the frame 3 tip at x 15 (3-4 px outside the safe region).
- **Picked `r2_1`** (round 2, variant 1) as a single variant, with no mixed frames. Reasons:
  - Every frame is inside the safe region, and the standing frames are 85-86 px tall like the sprite.
  - It has the closest match to the guides. Silhouette IoU against the blockouts for frames 1-7 is 0.70, 0.60,
    0.74, 0.54, 0.50, 0.73, 0.74.
  - The measured sword-tip arc tracks the `poses.json` tips within 1-7 px on every frame. Metrics arc: (98,13),
    (117,71), (21,49), (19,113), (36,122), (39,57), (46,55). Pose tips: (96,18), (118,68), (22,50), (20,106),
    (34,121), (40,58), (46,53).
  - Frame 7 is the closest of all variants to the ready pose (tip 4 px from the pose tip).
- **Slicer:** default settings throughout. The scale calibration was k 1.000-1.008 in all six variants. The bottom
  row needed only 3-9 px of ground snap, inside the default 20 px reach. For r2_1 the snaps were frames 4-7: +6, +4,
  +3, +3 px. No shared file was edited.

Gates by eye on `preview/strike_strip3x.png` and `strike_map1x.png`: metrics flagged nothing (0 gate flags), and I
agree.
- No facing flip: he faces the lower-left in every frame.
- No hand swap: the sword stays in the right hand (screen-left arm) and the shield on the left arm (screen-right)
  in every frame. In frame 2 the sword hand is hidden behind the head and the blade shows to the right of the head.
  That is the wind-up the blockout draws, not a swap.
- No missing prop: sword and shield are present in every frame.
- Frame 0 is pixel-identical to the static sprite (checked), and every frame stays inside x 16-143.

## What worked

- **Pose control is close to exact.** The guides pin down the pose: sword-tip positions land within a few pixels of
  `poses.json`, and the silhouettes follow the blockouts. Every beat reads: anticipation with the sword overhead,
  wind-up with the blade behind the head, a level swing, a low contact lunge with the shield swung back, a
  follow-through with the tip at the ground, then recover and settle.
- **The guides also fix the things G0 struggled with:**
  - The ground line: the bottom row came back within 3-9 px of the ground (G0: 27-42 px off).
  - Scale: k 1.000-1.008, with no 6-7 % growth in any of the six variants.
  - Where the figure sits in the cell.
- **Consistency:** the same face, outfit, sword, shield and outline style in all 42 generated cells, apart from the
  r3_2 strap. Facing and handedness were right in every cell.
- **Metrics for r2_1:** non-rigidity mean 0.311 (frames 1-5: 0.35-0.42), so these are real redrawn poses, not
  transforms of frame 0. Colour drift mean 0.94, 0 gate flags.
- The first call was already usable. Rounds 2 and 3 only cleaned up blade length and bounds.

## What failed

- **The model ignores detail the guide does not show.** The beat asks for hair trailing back and a twisted torso in
  frame 5. The blockouts carry no hair shape or twist, and the round 3 text asking for them changed nothing in either
  variant. With this method the guide wins over the text, so any detail the guide lacks is effectively lost.
- **Blade length:** the model draws the sprite's full blade, so the foreshortened blockout blades (frame 3's is
  32 px against 36 px at rest) get overshot by 6-11 px. That pushed frame 3 out of the safe region until round 2
  spelled out the blockout tip.
- **Keying fringe:** a thin purple-tinted fringe of magenta keying leftovers runs along the outlines. It is most
  visible around the sword and the hand in frames 1, 3 and 4 at 3x and above, and hard to see at 1x. A later
  finish pass (palette snap) is the place to fix it.
- **Hair:** the hair is a little fluffier than the sprite in every redrawn frame, so frames 0 to 1 and 7 to 0 pop
  slightly in the hair. The "hair as small and neat as cell 1" sentence did not measurably help.
- **Frame 2:** the sword hand is hidden behind the head, and the blade reads as sticking out to the right of the head
  more than hanging down the back. This follows the blockout.
- **Frames 4 and 5** have similar silhouettes: both are a low lunge, with the blade lower in frame 5. That comes from
  the shared poses.
- **Frames 2 to 3:** the tip jumps from behind the head to level in front in one 50 ms frame. This matches the poses
  (it is the smear frame, which gets FX later), but nothing in this method shapes the spacing beyond the poses.

## Spend and time

- 3 calls for the strike (n=2 each, 6 images), all tagged `anim/G1`, contract settings. Ledger: $0.0780, $0.0789
  and $0.0792, **$0.2361** in total. About 18-20 s per call.
- The `anim/G1` tag total in `spend.jsonl` is higher ($0.472 when I checked). The separate G1 agent for idle, flinch
  and death shares the tag, and its 3 calls (raws in `work/G1/rest/raw/sheet{A,B}_*`) are not part of the strike.
- Time: 5.8 minutes (350 s) from start until the strike was written, checked and its sequence rendered
  (`timings.jsonl`), plus a few minutes for these notes.

Extra deliverable: `arms/G1/sequence_strike_3x.{gif,mp4}` (one tile at 3x) and `sequence_strike_1x.{gif,mp4}` (3x3
map at 1x; the MP4 is stored at 2x). Each shows the full strike, with 1 s of the ready pose before and after
(2.87 s total) at manifest timings. The ready pose is the static sprite. `python3 arms/G1/make_g1.py seq strike`
rebuilds them. The shared `preview.py --only sequence` (idle, strike, flinch, death) runs once all four animations
exist.

## If I had another hour

- Take one or two more samples with the round 2 wording and pick again. The remaining defects (hair size, fringe
  amount, frame 7 angle) vary per sample, and the wording has done what it can.
- Lessons for the other animations:
  - Rely on the default ground snap.
  - State in the prompt that blade length and tip position follow the blockout.
  - Do not expect beat details that the guide lacks (hair, facial expression, twist) to appear from the text alone.
    For flinch, "eyes squeezed shut" may need to be the one `extra=` sentence, and it may still be ignored.
- The hair pop at frames 0 to 1 and 7 to 0 could be reduced by pasting the sprite's own head onto near-ready frames
  6-7. That would be a rig or finish step from another arm's toolbox, so I did not do it here.
