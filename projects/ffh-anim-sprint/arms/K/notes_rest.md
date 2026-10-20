# K (rest): keys first, then in-betweens

This file covers the animations of arm K other than the strike (the strike is in `notes.md`, by another agent).
Code: `arms/K/make_k_rest.py`. Raws, templates, cuts and review strips: `work/K/rest/`. Prompts:
`arms/K/prompts/death_p{1,2}_r{1,2}.txt`.

## death

### Method

This is the classic two-pass pipeline. Pass 1 draws only the key poses. It is a 4x2 sheet: cell 1 holds the true
sprite, cells 2-4 hold colour guides for death frames 0 (impact), 3 (kneeling) and 5 (hits the ground), and the
bottom row is left empty magenta. Pass 2 draws the in-betweens. It is an 8-cell sheet: cell 1 is the reference;
cells 4 and 6 hold the accepted pass-1 keys for frames 3 and 5 as finished art (upscaled 3x by `make_template`);
cells 2, 3, 5, 7 and 8 hold colour guides for frames 1, 2, 4, 6 and 7. The model fills those guide cells so they
connect with their neighbours. Both prompts are `gen.sheet_prompt(..., guide="colour", extra=...)`, sent with n=2
and the roster as the last image.

### What I did

- **Pass 1, round 1** (`death_p1_r1`, n=2). Both variants followed the three guides closely and left the bottom
  row empty, as asked.
  - Variant 1 kept the reference scale. Its impact key's bbox is 37-126 against the guide's 37-128. However, it
    drew a 2-4 px darker-magenta cast shadow under the kneeling and lying keys. That shadow keys out as a purple
    smear on the grass, and it cannot be removed afterwards: its colours overlap the sprite's own dark-purple
    outline colour (32,0,32).
  - Variant 2 drew every key about 5 % larger than cell 1 (impact key 93 px tall, against 89 px in variant 1), and
    its lying key reached x 13, outside the safe region.
- **Pass 1, round 2** (`death_p1_r2`). Same prompt plus one sentence: no shadow or darker patch under any figure,
  every figure the same size as cell 1, the whole sword inside its cell. Both variants came back clean, with no
  shadow.
  - **Picked `p1_r2_1`.** Its hair stays closest to the reference's tousled shape, where r2_2's impact frame has
    spikier hair. Its lying key stays inside the safe region (x 16-128), where r2_2 reaches x 14.
  - Slice: k=1.008, reference redraw dE 6.4 / IoU 0.974.
- **Pass 2, round 1** (`death_p2_r1`). The template held the picked keys in cells 4 and 6. The prompt tagged each
  cell as "in-between" or "finished key frame, keep exactly as drawn". The extra sentence said that cells 1, 4 and
  6 stay exactly as they are and that the guide cells connect smoothly with their neighbours; it also said that
  cells 7-8 keep cell 6's lying body almost unchanged, and that nothing casts a shadow. Results:
  - Variant 1 was good everywhere except frame 6 (the bounce). There the upper body bent up at the waist, half
    sitting up: the head was 16 px above the lying key instead of the beat's 2 px, and IoU with key 5 was 0.59.
  - Variant 2 had the same bounce problem (9 px, IoU 0.68). Its bottom row also drifted 12 px high (the slicer's
    ground snap corrected it).
  - How well the model kept the pasted keys: variant 1 redrew key 3 at dE 5.5 and key 5 at dE 7.9 (IoU 0.98 and
    0.96), about as faithful as its redraw of the reference cell. Variant 2 kept key 5 less well (dE 13.3).
- **Pass 2, round 2** (`death_p2_r2`). The only change was the cell 7 (frame 6) text: almost exactly the lying body
  of cell 6, with back, hips and legs in place, only the head and shoulders lifted a tiny amount, and the shield
  tilted slightly. Both variants now bend much less.
  - **Picked `p2_r2_2`** as a single variant, with no frames mixed in from other variants.
  - Frame 6 is flatter: IoU 0.84 with key 5 (r1 had 0.59 and 0.68).
  - Frame 7 (corpse) nearly repeats key 5 (dE 9.0 / IoU 0.96), so there is no pop between the pass-1 key and
    the held last frame. In r2_1, frame 7 moved the shield (dE 16.1 against key 5).
  - Frame 2 is clean, where r2_1's frame 2 has a magenta fringe along the hair and shoulder.
  - Frame 4 shows the sword clearly flying out of the hand.
- **Final death frames:** frames 0, 3 and 5 are the pass-1 keys from `p1_r2_1`; frames 1, 2, 4, 6 and 7 are
  the pass-2 in-betweens from `p2_r2_2`. I did not use pass 2's redraws of the keys: the pass-1 keys are the
  originals the in-betweens were drawn against, and the pass-2 redraws are only near-copies (dE 6.8-9.1).
- The default slicer was used unchanged (ground_snap 20); no shared file was edited. `make_k_rest.py`
  (`p1 template; p1 slice r2; p2 template; p2 slice r2; write`) reproduces the written frames identically from the
  saved raws, which I checked pixel for pixel.

**Gates, checked by eye on `preview/death_strip3x.png` and `death_map1x.png`.** The metrics raised 0 flags, and I
agree:
- No facing flip. He faces the lower-left in frames 0-3. In frames 4-7 he falls back with his head toward the
  upper-right and his feet toward the lower-left, as the spec asks.
- No hand swap. The sword stays in the screen-left (right) hand and the shield on the screen-right (left) arm
  through frame 3. The sword leaves the hand on frame 4, which the spec allows for death frames 4-7.
- No missing prop. Sword and shield are present in all 8 frames.
- Every frame stays inside x 16-143; `write_anim` gave no warnings.

### What worked

- **Keys first gives good control over the extremes.** All three keys landed on the beat on the first round:
  - Impact: head snapped back, eyes shut, grimace, arms flung out.
  - Kneeling: slumped on both knees, sword tip on the ground, shield hanging.
  - Lying: on his back, feet to the lower-left, head to the upper-right, sword on the ground at the lower-left,
    shield flat beside him.
- **The model treats pasted finished art as fixed.** It redrew the keys at roughly the fidelity of the reference
  cell (dE 5-9, IoU 0.95-0.98). Its in-betweens match the keys' character, scale and props:
  - Frame 2 (knees buckle) leads straight into the kneel.
  - Frame 7 nearly repeats key 5.
- **The in-betweens carry real secondary motion that the guides only hint at:** the sword flung straight up on
  frame 1, the sword spinning out of the hand on frame 4, and the shield tilting up on the bounce.
- **Rewording one cell fixed one cell.** The bounce came back flatter in round 2 without changing the other cells
  much.
- **Consistency is high.** Colour drift is low (drift mean 1.18), and non-rigidity is high (mean 0.409; frames 1-7
  at 0.36-0.44), so these are redrawn poses, not transforms. The overlap index is 0.433.

### What failed

- Pass 1, round 1 drew a cast shadow under the low figures even though the shared prompt already says "no
  shadows". It took a second, explicit sentence to stop it, and pass 2 needed the same sentence.
- **Frame 6 (bounce) is still bigger than the beat.** The topmost pixel is 9 px above key 5 (73 against 82),
  where the beat asks for a 2 px head-and-shoulders lift. In round 2 it is a small arch with the shield tilting up,
  not a sit-up, and at 80 ms it reads as a bounce. It is still not the 2 px nudge the beat describes, and I had no
  round left to push it further.
- **The corpse is flatter than the isometric ground axis.** The feet-to-head line runs at about 18 degrees (boots
  centre about (52,112), head centre about (108,94), read off a grid view), where the axis is 26.6 degrees (96:48). On the map tile it still reads as lying on the ground.
- **The bbox height jumps between frame 0 and frame 1** (top at y 37, then y 24), because the frame 1 sword is
  flung straight up. That is the beat ("sword arm flung up"), but the hand rises about 15 px in 120 ms.
- **Frame 4's sword hangs in mid-air** at the upper-left, next to the hand. On frame 5 it lies on the ground at
  the lower-left. That is a big jump for one 80 ms frame, though it reads as the sword dropping.
- **Faint magenta rims.** Up to about 60 low-alpha edge pixels per frame are pinkish (frames 1, 4, 6 and 7). They
  come from the shared slicer's soft key edge: the model's redraw of the reference cell on the same pass-2 sheet
  has 51 such pixels. They are invisible on the grass at 1x and faintly visible at 3x on the light strip
  background.
- **Not checked:** death frame 0 should share its pose with flinch frame 0, and K has no flinch yet. Whoever
  makes K's flinch should reuse this frame 0 (`arms/K/death/frame_00.png`), or the two will not match.

### Spend and time

- Calls: 4 (each n=2, so 8 sheet images) under tag `anim/K`: p1 r1, p1 r2, p2 r1, p2 r2. Spend $0.317
  (ledger: 0.0776 + 0.0782 + 0.0803 + 0.0809).
- Time: 6.8 minutes (406 s) from the first template to the written and checked animation, in
  `timings.jsonl`. It does not include about 10 minutes of reading the brief and tools beforehand, or writing
  these notes. Model latency was 18-23 s per call. That is well inside the 60-minute timebox.

### If I had another hour

- Spend a pass-3 call on a 3-cell sheet (key 5, bounce, corpse) to get the bounce down to the beat's 2 px. Or
  build frame 6 from key 5 itself with a small rig warp of the head and shoulders (that would be another arm's
  method, so I would only note it as a comparison).
- Ask for a lying key that runs along the 96:48 isometric axis. The guide's lying pose is already close
  (pelvis-to-head slope 28 degrees), but the model flattened it to about 18.
- Test whether adding frame 0 to the pass-2 sheet improves the 0-to-1 transition (the 8-cell layout has no room
  for it now). For example, use a 3x3 layout with frame 0 as an extra finished cell.
