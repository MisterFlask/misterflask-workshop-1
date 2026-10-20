# C: chained per-frame generation

This round covers the **strike** only. Code: `arms/C/make_c.py` (`ref` / `gen` / `slice` / `accept` / `write` /
`seq` / `rebuild`). Raws are in `work/C/raw/`, and the accepted chain is in `arms/C/chain_strike.json`. Prompts are in
`arms/C/prompts/strike_f{k}_r{n}.txt`. `rebuild strike` re-slices every accepted raw in chain order and reproduces the
written frames bit for bit (checked).

## Method

Each strike frame k = 1..7 comes from its own image call, and that call sees the previous accepted frame, so
consistency is carried forward one frame at a time. The template has two cells (1536x1024, cells 768x1024, canvas
region at 5x, ground row 980):
- cell 1 is frame k-1 as accepted (frame 0 is `ak.base_canvas()`);
- cell 2 is the colour blockout of pose k from `poses.json`.

The images sent are `[template, the true sprite at 5x NEAREST on flat magenta, gen.ROSTER]`. The prompt is my own
and reuses `gen.CHARACTER` and `gen.STYLE`. It says that cell 1 is frame k of 8 of a STRIKE (with that frame's beat
text), that cell 2 is the blockout for the next frame (with that beat text), and asks the model to:
- draw the next frame in cell 2 in exactly the blockout's pose;
- keep everything else as in cell 1, and leave cell 1 unchanged;
- treat image 2 as the ground truth for the character;
- use flat magenta and no effects.

Each call is n=2, and I allowed at most 2 calls per frame. Each variant is sliced with
`sheet.slice_sheet(raw, geom, n=2, ref_index=0, ref_canvas=prev_frame)` and I take frame [1]. I picked by eye from a
review strip showing the previous frame, the guide, both candidates, and onion skins of the previous frame under
each candidate. I also used the slice numbers: guide IoU, sword-tip versus pose tip, colour drift, and the cell-1
redraw error.

## What I did (strike)

Nine sheet calls in total. Frames 1, 2, 5, 6 and 7 took one call each; frames 3 and 4 took two.

| Frame | Pick | Why |
|---|---|---|
| 1 anticipation | r1_2 | Closer to the blockout (guide IoU 0.665 vs 0.516). Lower colour drift, and the hair grew less. |
| 2 wind-up | r1_2 | The blade hangs behind the shoulders as in the blockout (tip (117,70) vs pose (118,68)). r1_1 pointed the blade forward-left and also altered cell 1 (ref dE 6.7). |
| 3 swing | r2_2 | In r1, both variants drew the level blade longer than the blockout's and at shoulder height, so the tip landed at x 10-12, outside the safe region. In r2 I added one sentence (blade as long as the sprite's sword, tip where the blockout's tip ends, arm raised as in the blockout, hair as small as the sprite's). That fixed it: tip (23,50) vs pose (22,50). r2_2 has less hair and less change from frame 2 than r2_1. |
| 4 contact | r2_2 | In r1, both variants again drew the blade too long (tip x 12-14). In r2 the same kind of length sentence brought the tip to x 18 (pose 20). r2_2 keeps the body low (78 px tall) as the beat asks. r2_1 stood taller and redrew cell 1 (ref dE 12). |
| 5 follow-through | r1_1 | The blade is steeper, with its tip closer to the pose tip ((28,121) vs (34,121); r1_2 was at (25,120)). |
| 6 recover | r1_2 | Highest guide IoU (0.70), and the stance is halfway back as the beat says. r1_1 pulled the feet together and needed a 2 px ground snap. |
| 7 settle | r1_2 | Closest to the static sprite it hands back to (silhouette IoU 0.78 vs 0.75). The sword is 3 px low. |

- There are no mixed frames: every frame is one whole variant.
- After slicing, the only edit is in `write()`: it zeroes the alpha of 6 faint blade-tip outline pixels at x 15 in
  frame 3 (the spec's safe region is x 16-143). Every frame is otherwise exactly as sliced.
- Every slice calibrated cleanly: k = 1.000-1.005, and redraws of the accepted cell 1 scored dE 2.7-4.4 with IoU
  0.98-0.99.
- No accepted frame needed a ground snap. Two rejected variants (f1 r1_1 and f6 r1_1) each needed a 2 px snap.
- `metrics.py` raises no gate flags. I checked by eye in `strike_strip3x.png` and `strike_map1x.png`:
  - every frame faces the lower-left;
  - the sword stays in the far (screen-left) hand and the shield on the near arm in all 8 frames;
  - both props are present in every frame.

  No flag needed dismissing; none applies.
- Deliverables are `arms/C/sequence_strike_{3x,1x}.{gif,mp4}`: 1 s of the ready pose, then the strike at the
  manifest timings, then 1 s of the ready pose, 2.87 s in all. They use the same drawing and encoders as `preview.py`
  and G1's `seq`.

## What worked

- **The character stays the same across all 8 frames.** Face, outfit, colours, sword and shield all hold, and the
  slicer's scale stays at k ≈ 1.00 on every call. The strike reads clearly at 1x on the tile.
- **The model followed the blockout poses closely on the first call.** Final guide IoU ranges from 0.51 to 0.70.
  Measured sword tips are within 1-6 px of the `poses.json` tips on every frame:
  - measured: (97,16) (117,70) (23,50) (18,105) (28,121) (40,52) (46,56)
  - pose: (96,18) (118,68) (22,50) (20,106) (34,121) (40,58) (46,53)
- **The model left cell 1 nearly untouched in 13 of the 18 variants** (ref dE under 4.5). So the slicer's
  calibration from the previous frame works as well as calibration from the true sprite.
- **Seeing the previous frame keeps neighbouring frames close.** Onion skins show the unchanged parts (legs on frames
  1→2, the torso on 5→6→7) staying in place.
- **The chain is cheap and fast:** each call took about 16 s and cost $0.094.

## What failed

- **Drift accumulates along the chain, which is this method's signature.**
  - Colour drift against the sprite rises frame by frame: mean 0.70, 1.01, 1.35, 1.51, 1.58, 1.36, 1.63 for frames
    1-7 (p95 from 1.7 to 9.5). G0 and G1, which draw every frame in one sheet against the true sprite, stay flat at
    about 1.0-1.15.
  - The shield grows: its wood pixel count goes from 520 in the sprite to 617 in frame 1 and 691 in frame 7. The
    "match image 2" instruction did not stop this, because each call copies cell 1 more than it copies the reference.
  - The hair jumps about 18% in the very first redraw (524 → 617 px) and then stays about that size.
- **There is a pop back to idle after frame 7.** Frame 7's silhouette IoU against the static sprite is only 0.78:
  the shield is bigger, the hair fluffier, and the body sits about 1 px right. Frame 0 → 1 has the matching pop the
  other way (from the true sprite to the model's style). A chain can never close the loop back to frame 0, because
  each frame only sees its predecessor.
- **The model ignored parts of the beats.**
  - Frames 4-5: the shield arm is not swung back for balance; the shield stays forward at chest height.
  - Frame 5: no visible hair trailing back and only a slight torso twist.
- **The model overshot the blade length** on the two frames where the blade is foreshortened (3 and 4). Each needed a
  second call with explicit length wording. Frame 3 still leaves 6 faint pixels at x 15, which I clipped.
- **Errors can't be fixed later.** Accepting frame k fixes the starting point for every later frame, so a weak early
  choice spreads through the rest. Re-rolling frame k means regenerating every frame after it (`accept` drops them).
  I was lucky that no early frame was bad.
- **The method is serial.** Seven dependent calls plus a review after each took about 10 minutes of wall-clock, with
  no parallelism. A one-sheet arm takes one call.

## Side probe: Responses API with the image_generation tool

Script: `arms/C/resp_probe.py`, outputs in `work/C/resp/`, prompts in `arms/C/prompts/resp_probe_turn{1,2}.txt`.

**Setup.**
- Model gpt-5.5 with `tools=[{"type":"image_generation","quality":"medium","size":"1536x1024"}]` and a forced
  `tool_choice`.
- Turn 1 sent the true sprite (5x on magenta, as a base64 data URL) and asked for strike frame 1 on magenta.
- Turn 2 sent `previous_response_id` and asked for frame 2.

**It works mechanically.** Both turns returned one image each, with no errors.
- Latency: 21.5 s and 20.4 s.
- The response reports the image tool's tokens separately, in `tool_usage.image_gen`:
  - turn 1: 640 in / 1568 out;
  - turn 2: 3143 in / 1568 out, because turn 2 re-feeds the previous image.
- The mainline model's own usage was about 3.4-3.8k in and 300 out per turn.
- Ledger cost, pricing every token at $10/M input and $40/M output: **$0.116 and $0.144, $0.260 for the two calls.**
  This is conservative, and the real price is likely lower.

**Consistency was poor for animation use.**
- The two frames share the costume colours and the general design: blue tunic, round shield, brown hair.
- The character is drawn about 10x larger than the sprite and fills the whole image height.
- In both frames he faces screen-right, which is the sprite's facing flipped.
- His face is redrawn older and squarer than the sprite's.
- Between turns the figure moves and grows: turn 2 is closer and larger, the blade becomes curved and is held up
  overhead instead of hanging behind the shoulders, and the hair and face change.

**Verdict.** Without a template there is no shared registration (scale, position, ground line), so frames from the
Responses chain cannot be sliced onto the canvas. Identity is held only loosely. For sprite animation it is clearly
worse than the edits endpoint with a positioned template. These images were not used for any frame.

## Spend and time

**Spend** (rows tagged `anim/C` in `spend.jsonl`):

| Item | Calls | Cost |
|---|---|---|
| Strike: sunburst edits, n=2, 3 input images | 9 | $0.847 (about $0.094 each) |
| Responses probe | 2 | $0.260 |
| **Arm total** | 11 | **$1.107** of the $2.00 cap |

**Time** (`arms/C/timings.jsonl`):
- Strike: 610 s, about 10 minutes from first reading the brief to the written, checked strike, including the probe
  running in parallel.
- Probe: 42 s of API time, plus about 4 minutes writing the script.

## If I had another hour

- **Anchor every call to the true sprite, not just the previous frame.** A 3-cell template could hold the true
  sprite, the previous frame and the new guide. That should stop the creeping shield and hair growth and the colour
  drift, while keeping frame-to-frame continuity.
- **Close the loop on the last frame.** Generate frame 7 with the true sprite in cell 1 instead of frame 6. That would
  remove the pop back to idle; frame 6 would still provide continuity through the guide.
- **Add one sentence on the shield arm for frames 4-5** ("the shield arm swings back toward the upper-right, away
  from the body") and re-roll from frame 4. Because of the chain, frames 5-7 would need re-rolling too.
- **Re-use the rig for idle, flinch and death.** `make_c.py` is generic (`gen <anim> <k> <round>`), but flinch and
  death have no static frame 0. They need a seed frame, for example a chained call from the sprite into flinch frame
  0, before the chain can start.
