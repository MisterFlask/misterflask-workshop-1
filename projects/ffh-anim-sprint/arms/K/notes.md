# K: keys first, then in-betweens

This file covers the arm-K **strike**. The arm-K death (another agent, same method) is in `notes_rest.md`.
Code: `arms/K/make_k.py`. Raws, templates, cuts, review strips and slice diagnostics: `work/K/strike/`. Prompts:
`arms/K/prompts/strike_p1_r1.txt`, `strike_p2_r1.txt`, `strike_p2_r2.txt`.

## strike

### Method

This is the classic key-then-in-between pipeline, done as two image-model passes on the 8-cell layout of the final
strip. Pass 1 draws only the keys. Cell 1 holds the true sprite. Cells 3, 5 and 6 hold colour guides for frames 2
(wind-up extreme), 4 (contact) and 5 (follow-through). Cells 2, 4, 7 and 8 stay empty magenta. Pass 2 uses the same
layout. Cell 1 is again the sprite, and the accepted keys are pasted into cells 3, 5 and 6 as finished art. Colour
guides for frames 1, 3, 6 and 7 go in cells 2, 4, 7 and 8, and the model draws the in-betweens there so they
connect with their finished neighbours. Every prompt is `gen.sheet_prompt("strike", ..., guide="colour", extra=...)`,
sent under the frozen contract (sunburst, medium, 1536x1024, n=2) with the roster as the last image.

### What I did

- **Pass 1, round 1** (one n=2 call). Both variants left the four empty cells empty, kept the reference cell
  (k = 1.004, IoU 0.98, dE 5.5 / 6.7) and followed all three guides. Both drew the bottom row 8-11 px high, and the
  slicer's ground snap corrected it. I picked **variant 1** (`p1_r1_1`) by eye. Its contact is lower and leans in
  more. Its follow-through blade reaches almost to the ground at the lower-left, as the beat asks; variant 2's
  stops higher. Its wind-up is more crouched. Colour drift was equal (1.1 vs 1.0). Variant 2 had cleaner edges (8
  vs 43 magenta-fringe pixels on the wind-up), but pose fidelity came first. I did not need a second round.
- **Pass 2, round 1** (one n=2 call). Both variants kept the pasted keys almost unchanged: the silhouette IoU
  between the pass-1 keys and pass 2's redraws was 0.98. Both drew in-betweens that follow the guides and read as
  one strike. Variant 1 was usable, with two problems. Its settle frame 7 came back 2 px taller than the sprite and
  6 % bigger in area (IoU 0.82 with frame 0), which makes a pop when idle resumes. Its swing blade (frame 3) poked
  1 px past the safe region; variant 2's poked 8 px past.
- **Pass 2, round 2** (one n=2 call). The prompt was round 1 plus one sentence: cells 7 and 8 at exactly the size of
  cell 1, and the level blade in cell 4 no farther left than the sword tip in cell 5. Variant 1 got the settle frame
  right (86 px tall like the sprite, IoU 0.90 with frame 0, frame 7 to frame 0 change 47 against 66-67 for the other
  variants), but its swing blade
  poked 5 px out (x 11). Variant 2 kept the swing inside (x 18), but its settle frame was no better than round 1.
- **Final frames.** Frame 0 is the sprite. Frames 2, 4 and 5 are the **pass-1 keys** (`p1_r1_1`), not pass 2's
  redraws of them. Frames 1 and 3 come from **pass-2 round-2 variant 2**; frames 6 and 7 from **pass-2 round-2
  variant 1**. So the in-betweens mix two variants of the same call. Both were drawn against the same pasted keys,
  and at 1x and 3x I could not see a style difference between them. I mixed them to get both the swing inside the
  safe region and the settle frame that matches the sprite. `PICK` in `make_k.py` records this.
- Timings are the `beats.md` values, unchanged. No FX.
- Gate check by eye on `preview/strike_strip3x.png` and `strike_map1x.png`. The metrics raised no gate flags, and
  I confirm there are none. Every frame faces the lower-left. The sword is in the right hand on the screen-left
  side, except frame 2, where the beat puts the blade behind the shoulders at screen-right. The shield stays on the
  left arm at screen-right. Sword and shield are present in all 8 frames. Frame 0 is pixel-identical to the sprite
  (`write_anim` enforces it).
- Metrics (`metrics.json`): nonrigid mean 0.333 (frames 1-5: 0.36-0.45; 6: 0.18; 7: 0.10). Overlap index 0.5:
  steel and tunic peak on frame 4, hair and shield on frame 5. Colour drift mean 1.18. No gate flags. The
  sword-tip trail (`strike_onion.png`) runs up-right (1), down behind the shoulders (2), out level to the left (3),
  down-left (4, 5), back up (6, 7). It is a clean loop with no backtracking.
- Deliverables: `strike/` frames and manifest, and `sheet_strike.png`. The previews in `preview/` include
  `strike_1x.gif` and `strike_3x.gif`, plus `strike_1x.mp4` and `strike_3x.mp4` (the strike alone at manifest
  timings, plus a 400 ms hold on frame 0, made by `make_k.py mp4`). The full sequence is
  `sequence_{1x,3x}.{gif,mp4}` (idle x2, strike, idle, flinch, idle, death, corpse hold). Idle and flinch are not
  made yet, so the sequence shows the static sprite for them. It uses the arm-K death that existed when I ran it.

### What worked

- **Key-only sheets work.** The model left all four empty cells empty in both variants and spent its effort on
  three well-posed keys. Pass 1 did not need a second round.
- **Pasted finished keys are respected.** Pass 2 redrew them almost exactly (IoU 0.98) instead of reinterpreting
  them. The in-betweens picked up the keys' face, hair, outfit and proportions, so the whole strike reads as one
  character at 1x.
- The poses follow the beats well. The wind-up is coiled, the swing is level, the contact lunges low toward the
  lower-left, and the follow-through tip ends near the ground. Recover and settle come back to the ready pose with
  the sword a little low.
- A one-sentence prompt fix in round 2 fixed the settle-frame size (frame 7 now matches the sprite's height).
- It is cheap and fast: 3 calls, about 7 minutes from the first template to a written, checked strike.

### What failed

- **Magenta fringe** on every generated frame (1-7). Keyed outline pixels keep a magenta/purple cast: about 200-430
  outline pixels per frame have a strong magenta tint, against 12 in the sprite. There are also 4-25 near-pure
  magenta pixels per frame in small enclosed pockets, for example between the hand and the hilt on frames 3 and 5. At 3x it shows as a purplish rim; at 1x it is barely visible. I left it for the shared round-2 finish
  pass (`palette_snap`), so every arm gets the same treatment.
- **The in-betweens run a little large.** Frames 1, 3 and 6 have 10-12 % more opaque pixels than the sprite, mostly
  fluffier hair (the calibration saw the same); frame 7 has 5 % more. Frame 1's raised blade reaches y = 11.
- **Frame 3 (swing)** follows its guide (blade level, front foot stepping), but the torso is nearly upright, so the
  "torso whips forward" part of the beat is weak. The blade is held at head height, not mid-sweep.
- **Frame 4 (contact)**: the shield arm is held out to the right, not clearly swung back for balance. **Frame 5**:
  the hair does not visibly trail back.
- **Frame 0 to frame 1** jumps from the original sprite to model art (redraw dE about 6-7 against the sprite). This
  pop is inherent to any redraw arm.
- The pass-2 in-betweens come from two variants (see above). I saw no style break, but it is a mix.

### Spend and time

- Strike: 3 sheet calls, n=2 (6 images): pass 1 round 1, pass 2 rounds 1 and 2. Ledger cost **$0.240**: 0.0784 +
  0.0805 + 0.0812. Latency about 17-19 s per call.
- The tag `anim/K` also carries the arm-K death (another agent): $0.557 for the tag in total when the strike was
  written. Cap $2.40.
- Time: 414 s, about 7 minutes (`timings.jsonl`), within the 60-minute timebox.

### If I had another hour

- Run a pass-1 round 2 aimed at the contact key: shield arm swung back, hair trailing, a more forward torso. The
  keys set everything that follows, so this is where a round buys the most.
- Ask pass 2 for a swing frame with the torso pitched forward and the blade at shoulder height mid-sweep, and give
  the size constraint for all four in-between cells, not only cells 7 and 8.
- Try a third pass that pastes all seven accepted frames back in and asks only for consistency fixes (size, hair).
  This tests whether a clean-up pass stays as faithful as pass 2 did.
- For the other animations: reuse `make_k.py`. Idle and flinch need only `ANIM`, the key and in-between frame lists,
  the key names and the prompt extras changed; the template, slicing, review and write steps carry over.
