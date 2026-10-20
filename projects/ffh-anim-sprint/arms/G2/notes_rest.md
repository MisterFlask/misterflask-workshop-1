# G2 (rest): idle, flinch and death as one-call sheets that repaint arm R's rig frames

A separate G2 agent made the strike (`notes.md`, `make_g2.py`, `strike/`). This file covers idle, flinch and death.
The code is `arms/G2/make_g2_rest.py` (`template` / `gen` / `slice` / `write` / `all`). `python3
arms/G2/make_g2_rest.py all` rebuilds all three animations from the raws in `work/G2/rest/raw/`. A second run gave
byte-identical frames. The prompts are in `arms/G2/prompts/`. Sheet A served two animations, so each sheet A prompt
is saved twice with the same text, as `idle_sheetA_rN.txt` and `flinch_sheetA_rN.txt`. The death prompts are
`death_sheetB_rN.txt`. Arm R's folder is only read.

There are two 4x2 sheets with 384x512 cells. Cell 1 of each holds the true sprite at 3x (`ak.base_canvas()`).
- **Sheet A:** cells 2-4 hold R's idle frames 1-3, and cells 5-8 hold R's flinch frames 0-3. Each cell text starts
  with "idle frame k:" or "flinch frame k:", and `anim="IDLE (cells 2-4) and FLINCH (cells 5-8)"`.
- **Sheet B:** cells 2-8 hold R's death frames 1-7. Death frame 0 is the flinch frame 0 accepted from sheet A.

Every call used `gen.sheet_prompt(..., guide="rig")` and images [template, `gen.ROSTER`]. The contract was
sunburst, medium, 1536x1024, n=2, tag `anim/G2`. Each round changed only the per-cell wording and the one `extra=`
sentence. R's frames never changed. Slicing used `sheet.slice_sheet` with its defaults (one scale from cell 1, ground
snap up to 20 px). No shared file was edited. All three R animations existed, so nothing is missing.

`preview.py --only stills` also redraws the strike's preview images. They come from the same strike frames, so
nothing in the strike changed.

---

## idle

### Method
Sheet A, cells 2-4. R's idle frames are the sprite's own pixels with the head and chest raised 1 px and the sword
and shield lagging. The model was asked to keep each draft's pose and position. Frame 0 is the static sprite
(`write_anim` enforces this). Frames 1-3 are cells 2-4 of one variant, sliced at the sheet's single scale.

### What I did
- **Round 1** (`sheetA_r1`): plain beat texts.
  - Both variants posed the idle cells like R: IoU 0.90-0.99 against the drafts.
  - Cells 2-3 were full redraws, with a new face and expression, a slightly bigger shield and a magenta-tinged
    outline. Cell 4 came back almost as the sprite (IoU 0.98).
  - Mean idle boil, including the 3→0 loop step: 32.7 and 32.9. Per step, 645-2048 of about 4100 opaque pixels
    changed by more than 40 levels.
- **Round 2** (`sheetA_r2`): the three idle cells reworded. Each now says the draft is already finished art (the
  reference sprite with the chest and head one pixel up, and so on) and must be copied pixel for pixel, as cell 1 is.
  One extra sentence: cells 2-4 need no repair and should be reproduced unchanged with cell 1's face, hair, sword and
  shield. The flinch texts were unchanged.
  - The redraw halved: boil 23.6 (r2_1) and 20.4 (r2_2).
  - This is the opposite of G1's result with flat blockouts, where "copy cell 1" did nothing. A rig draft is already
    the sprite, so "copy the draft" is an instruction the model can follow.
- **Round 3** (`sheetA_r3`): round 2's idle wording, with the extra sentence also asking for a crisp dark outline
  with no pink or purple edge pixels. Flinch cells 5-6 were reworded (see flinch). Boil 22.2 (r3_1) and 20.9
  (r3_2). The fringe did not change.
- **Picked `r2_2`**, a single variant. It has:
  - the lowest boil (steps 20.8, 19.2, 24.8 and 17.0 on the 3→0 loop; 922, 678, 972 and 675 pixels changed);
  - the lowest colour drift (1.0 per frame);
  - the right timing. The head top goes 40 / 39 / 39 / 40, as in R. The sword tip y goes 52 / 52 / 50 / 52, where
    R's is 52 / 52 / 50 / 51.
- r3_2 was a near tie (20.9), but its sword tip drops 1 px on frame 1 and its hair outline shifts more.

### What worked
- **The breath reads.** The head and chest rise on frame 1, the sword tip rises 2 px on frame 2, and the body comes
  back down on frame 3.
- **The feet are rock steady:** bottom_std 0 and feet_x_std 0. Nonrigid is 0.034 / 0.046 / 0.019, so these are
  small moves of the same drawing.
- **The idle-specific wording is the clearest gain this arm found.** Repainting R's sprite-pixel drafts with
  "copy the draft exactly" drops the boil to about half of round 1 (32.8 → 20.4). G1's guided redraw stayed at about
  40.
- **Gates:** the metrics flagged none, and I confirm that by eye on `preview/idle_strip3x.png` and
  `idle_map1x.png`. There is no facing flip, the sword stays in the screen-left hand, the shield stays on the near
  arm, both props are present, and frame 0 is the sprite.

### What failed
- **It still boils, mildly.** Mean metric boil is 21.6, against R's 12.1 and G1's about 40.
- **What is visible at 1x in the map strip:** frames 1-3 are a touch softer and darker-edged than frame 0, so the
  figure gives a faint shimmer on the 0→1 and 3→0 steps. Frames 1, 2 and 3 agree with each other well.
- **At 3x:** the eyes and mouth are redrawn slightly differently from the sprite, and frame 2 has one pink pixel by
  the sword hand.
- **The diff heat:** on 0→1 the whole outline and the face light up faintly, and on 1→2 and 2→3 the sword and head
  do. The legs barely change.
- **Honest summary:** it reads as a breath with a light outline shimmer, not as a pop. It is not as clean as R's
  own frames.
- **Magenta fringe.** The model's anti-aliased edges against the magenta background survive keying (tolerance 48)
  as purple-tinted outline pixels: 457 / 470 / 444 on frames 1-3, against 190 such pixels in the sprite's own art.
  That is the main source of the remaining shimmer. Asking for no pink edge pixels (round 3) did nothing. I did not
  add a despill pass, because the method slices with `slice_sheet` as it is.
- **The shield moves the wrong way.** On frames 1-2 it drops by about 0.9 px (centroid y 85.2 → 86.1 / 86.0),
  where R's draft lifts it by 0.6 px. Its redraw also moved it slightly, so the wood peaks on frame 1, not 2.

### Spend and time
- Sheet A: 3 calls, n=2 each, 6 images, $0.2368. These calls also produced the flinch.
- Logged at 257 s (4.3 min), from the template to the checks of the written idle. This time includes judging the
  flinch cells of every sheet A variant.

### If I had another hour
- Within G2, the remaining shimmer is mostly the purple fringe. Test a despill or palette-snap of the edge pixels
  to the sprite's outline colours (`finish.palette_snap` is a round-2 finish step) and see whether boil falls toward
  R's 12.
- Alternatively, ship R's idle as it is. A 1-px breath gains nothing from a repaint, because R's frames have no
  joints to repair.

---

## flinch

### Method
Sheet A, cells 5-8: R's flinch frames 0-3 repainted in the same call as the idle. All four frames come from one
variant.

### What I did
- The flinch cells used the plain beat texts in rounds 1 and 2.
- **Round 3** reworded cells 5-6. Both round 2 variants drew frames 0-1 with a bigger head and hair, so the new
  texts asked to keep the head and hair as small as in cell 1, only tipped back. It did not help. r3 hair counts
  were 672-680 px on frame 0, against 630 for r1_1 and 524 in the sprite. Pose fidelity also dropped: IoU against R
  0.52 / 0.48 on frames 0-1.
- **Picked `r1_1`** (round 1, variant 1), a single variant. Reasons:
  - the best character consistency of the six: drift 1.06 / 1.07 / 1.05 / 1.01 and the smallest hair growth;
  - the best follow of R on the extreme frames: IoU 0.65 / 0.61 / 0.80 / 0.96;
  - the cleanest return to idle: frame 3 has silhouette IoU 0.953 with the sprite and boil 23.1 into idle frame 0,
    the lowest of the six.
- So idle (`r2_2`) and flinch (`r1_1`) come from different sheet A calls. Each animation is a single variant, and
  no frames are mixed within an animation.
- r1_1 drew its bottom row 9 px high. The slicer snapped all four flinch frames by the same 9 px, so they stay
  consistent with each other.

### What worked
- **The beat reads at 1x.**
  - Frame 0: the impact, with the head tipped back toward the upper-right and the eyes squeezed shut. Both arms are
    flung out, with the sword up-left and the shield thrown out to the right.
  - Frame 1: leaning back with the knees bent and the sword dipping level to the left, the shield still out.
  - Frame 2: halfway back.
  - Frame 3: nearly the ready pose.
- **It is the same character in every frame,** at the same scale (k 1.008), and the feet stay on row 125.
- The repaint draws the wince as a real expression, not code-drawn lids. The arms are drawn as continuous limbs
  with no rotated-part joints.
- **Gates:** none flagged, confirmed by eye on `flinch_strip3x.png` and `flinch_map1x.png`. There is no flip, the
  sword stays in the screen-left hand and the shield on the near arm in all 4 frames, and both props are present.

### What failed
- **Frame 0 is bigger than R's draft:** 4068 opaque pixels against 3278 (+24%). The model turns the torso more
  frontal as the arms fling out, and widens the stance. All six variants did this (+18% to +28%). It reads as a
  bigger jolt, but the 3/4 turn opens and closes over the flinch.
- **The feet move between frames:** feet_x_std 2.18, against R's 1.3. The stance is redrawn wider on frame 0 and
  narrower on frames 2-3, so the boots step a little at 1x.
- **Overlap index 0.0.** Every tracked part peaks on frame 1. R's one-frame shield and hair springs did not survive
  the repaint.
- **Boil is high:** 114.5 / 88.3 / 74.4, against R's mean 67.2. Each frame is a fresh drawing, so the shading on the
  tunic and legs changes from frame to frame.

### Spend and time
- No calls of its own: it uses the 3 sheet A calls ($0.2368) logged under idle.
- Logged at 31 s, for writing and checking the chosen variant. Judging the flinch cells happened inside the idle
  window above.

### If I had another hour
- Word cell 5 as "keep the body turned three-quarters to the lower-left as in the draft, only the arms flung out",
  to keep the frame 0 silhouette closer to R's.
- Name R's shield lag in the cell 6 text, to test whether a spring survives the repaint.

---

## death

### Method
Sheet B, cells 2-8: R's death frames 1-7 repainted in one call. Frame 0 is flinch frame 0 from sheet A `r1_1`,
pixel for pixel, as the spec asks.

### What I did
- **Round 1** (`sheetB_r1`): plain beat texts.
  - Both variants followed the standing drafts: the stagger with the sword flung up, the knees buckling, and the
    kneel with the sword tip on the ground.
  - Both redrew the lying frames as a side-view body lying almost level, with the head only about 20 px above the
    feet over about 100 px. R's draft rises 96:45 along the iso ground axis.
  - The shield stood upright and face-on by the head, not flat.
  - The far arm was raised as a fist on frames 4 and 6. R's draft has the arm on the far side of the chest, and the
    model turned it into a wave.
  - Both drew the bottom row high. r1_1 was 21 px high, just past the slicer's 20 px ground snap, so its lying frames
    float at y 105. r1_2 was 14-16 px high and was snapped.
- **Round 2** (`sheetB_r2`): reworded cells 6-8. The texts now ask for the body lying along the draft's diagonal,
  rising to the upper-right as steeply as the draft, with the shield flat as a thin ellipse and both arms on the
  ground. One extra sentence: keep each lying body where its draft lies, on cell 1's ground line, with the draft's
  slant.
  - Both variants fixed the slant, the flat shield and the arms on frames 5 and 7.
  - Frame 4 still raised the fist in both.
- **Round 3** (`sheetB_r3`): round 2's texts, with cell 5 reworded: the empty sword arm trails limply back, the hand
  open and not raised above the head.
  - r3_1 still raised the fist.
  - r3_2 fixed it: the arm now trails back at shoulder height with an open hand. But r3_2 drew the lying body 40%
    larger than R's (2654-3325 px against 1915-2118), with bigger hair.
- **Picked `r2_1`** (round 2, variant 1), a single variant for frames 1-7. Reasons:
  - the closest scale to R in the lying frames (+25%; r3_2 +40%);
  - the lowest boil of the four round 2-3 variants (101.8);
  - the cleanest corpse.
- I did not mix in r3_2's frame 4. It is drawn about 25% bigger than r2_1's neighbouring frames, so it would pop.

### What worked
- **At 1x on the map strip the whole beat reads.**
  - Frame 0: the impact.
  - Frame 1: the stagger back, with the sword flung high.
  - Frames 2-3: the knees buckle, then he kneels slumped with the sword tip on the ground and the head drooping.
  - Frame 4: the fall back, with the sword already dropped.
  - Frame 5: on his back with the feet toward the lower-left and the head toward the upper-right, the sword lying at
    the lower-left and the shield flat beside his head.
  - Frame 6: the bounce.
  - Frame 7: the corpse with the eyes closed.
- **The vignette works.** The mirrored defender dies with his feet toward the attacker.
- **The repaint fixes what R's notes listed as its weakest frames.**
  - The kneel (frame 3) is a real kneel on both knees, not upside-down boots behind the shield.
  - Frames 2-3 slump forward, with the head and shoulders bent over, which R's rig could not draw.
  - The corpse has a face drawn from the side with closed eyes, not a sheared upright face.
  - The legs are two clean boots, not a cluster.
- **Overlap index 0.83,** against R's 0.0. The parts settle on different frames: steel 5, tunic 6, hair 5, wood 7.
  The redraw varies a little, which accounts for part of this.
- **Character:** drift 1.06-1.35, the same outfit and colours, and the standing frames at the sprite's size.
- **Gates:** none flagged, confirmed by eye on `death_strip3x.png` and `death_map1x.png`.
  - There is no flip.
  - The sword is in the screen-left hand on frames 0-3 and on the ground at the lower-left on 4-7 (allowed).
  - The shield is on the near arm on frames 0-4 and flat beside him on 5-7.
  - No prop is missing, and every frame stays inside x 26-128.

### What failed
- **Frame 4 (falling back) raises the empty sword hand as a fist above the head.** In 80 ms at 1x it flickers like
  a wave. The model follows R's draft, which has the arm up, over the beat text. A cell rewording fixed it in one
  round 3 variant only, and that variant had other problems (scale).
- **Frame 2 (knees buckle) is already a deep crouch,** nearly as low as the kneel: top y 57, against 61 on frame 3.
  The progression from buckling to kneeling is compressed. IoU against R's frame 2 is 0.52.
- **Frame 6 (bounce) lifts the head and shoulders about 9 px,** where the beat says 2 px (top y 72 against 81 on
  frames 5 and 7). It reads as a half sit-up rather than a bounce.
- **The lying body is about 25% bigger than R's draft.** The model draws the corpse with fuller limbs and hair, but
  it stays in proportion with the standing frames.
- **Frame 0 → 1 comes from two different calls** (sheet A `r1_1`, then sheet B `r2_1`). The wince face changes to
  an open-mouthed upward look, and the hair is drawn differently. The big pose change hides most of it at 1x.
- **Boil per step is 110-138 on the falling frames,** which is expected for a fall (R: 103.7 mean). The lying
  frames 5→6→7 still change by 47.8 and 47.7, because each lying frame is a separate drawing and the shading and hair
  change between them.
- **All lying frames needed a 10-11 px ground snap** (the bottom row was drawn high again). The snap is uniform
  across frames 4-7, so they stay consistent with each other.

### Spend and time
- Sheet B: 3 calls, n=2 each, 6 images, $0.2361.
- Logged at 256 s (4.3 min), from the template to the checks.

### If I had another hour
- Change one cell of R's guide. A version of R's frame 4 with the far arm drawn down, made in code in the G2
  folder, would let the repaint follow the beat instead of the draft's raised arm. That is a change of guide, not of
  wording, so it was outside this round's rules.
- Reword cell 7 to "the same lying pose, the head lifted only slightly", to bring the bounce down to a few pixels.

---

## Spend and time (all three)

- 6 calls (3 for sheet A, 3 for sheet B), n=2 each, 12 output images. Ledger spend tagged `anim/G2` for this
  agent's calls: **$0.4729**. Latency was 17.6-20.5 s per call.
- Logged times: idle 257 s, flinch 31 s, death 256 s, about 9 minutes in all. Each is well inside its 60-minute box.
- After all four G2 animations existed, I ran `preview.py --arm G2 --only sequence` (`arms/G2/sequence_{3x,1x}.{gif,
  mp4}`, 7.99 s) and `--only vignette` (`arms/G2/vignette.{gif,mp4}`, 6.74 s).
