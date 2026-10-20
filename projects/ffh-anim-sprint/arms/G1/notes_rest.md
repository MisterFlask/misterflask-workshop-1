# G1 (rest): idle, flinch and death from one-call sheets with colour pose guides

The strike was made by a separate G1 agent (`notes.md`, `make_g1.py`). This file covers idle, flinch and death.
Code: `arms/G1/make_g1_rest.py` (`template` / `gen` / `slice` / `write`). It rebuilds every frame from the raws in
`work/G1/rest/raw/`. Prompts are in `arms/G1/prompts/`. Sheet A served two animations, so each sheet A prompt is
saved twice with the same text, as `idle_sheetA_rN.txt` and `flinch_sheetA_rN.txt`. The death prompt is
`death_sheetB_r1.txt`.

Two sheets, both 4x2 with 384x512 cells. Cell 1 of each holds the true sprite at 3x.
- **Sheet A:** cells 2-4 = idle frames 1-3, cells 5-8 = flinch frames 0-3. Each cell text starts with
  "idle frame k:" or "flinch frame k:", and `anim="IDLE (cells 2-4) and FLINCH (cells 5-8)"`.
- **Sheet B:** cells 2-8 = death frames 1-7. Death frame 0 is the flinch frame 0 accepted from sheet A.

The guides in every non-reference cell are `guides.render_guide(pose, "colour")` from `poses.json`, and they stayed
the same in every round. Every call used the contract: sunburst, medium, 1536x1024, n=2, images
[template, `gen.ROSTER`], tag `anim/G1`. Slicing used `sheet.slice_sheet` with its defaults (ground snap 20). I
changed no shared file.

## Idle

### Method
Sheet A, cells 2-4: the model redraws the reference character on the three idle breathing blockouts. They differ
from the frame-0 blockout by 1-2 px of chest, head, sword and shield. Frame 0 is the static sprite (`write_anim`
enforces this). Frames 1-3 are the model's cells 2-4 from one variant, sliced at the sheet's single scale.

### What I did
- **Round 1** (`sheetA_r1`): plain beat texts. Both variants posed every cell correctly. However, cells 2-4 were
  full redraws: a different face and expression, fluffier hair, a reshaped shield and sword hand. Mean idle boil
  (frame-to-frame change, `metrics.boil`) was 41 for variant 1 and 46 for variant 2. For comparison, the sprite
  against the model's own redraw of cell 1 scores 15-16, and A0's transform idle scores about 16.
- **Round 2** (`sheetA_r2`): cells 2-4 reworded to "Copy cell 1 exactly, pixel for pixel ... only the head and
  chest are one pixel higher". One sentence added: cells 2-4 are a subtle breathing loop and must look like exact
  copies of cell 1. The flinch texts were unchanged. No effect: boil was 44 (r2_1) and 61 (r2_2).
- **Round 3** (`sheetA_r3`): cells 2-4 reworded as "a duplicate of cell 1 nudged by a pixel". The extra sentence
  became "where the blockout and cell 1 disagree, follow cell 1 ... not by redrawing the face, hair, sword or
  shield". Still no effect: boil was 58 (r3_1) and 42 (r3_2).
- **Picked `r2_1`** as a single variant, with the same variant used for flinch. Mean boil including the 3→0 loop
  step:

  | Variant | r1_1 | r2_1 | r3_2 | r1_2 | r2_2 | r3_1 |
  |---|---|---|---|---|---|---|
  | Mean boil | 39.1 | 39.6 | 40.5 | 47.5 | 53.7 | 54.6 |

  The top three are a statistical tie. By eye at 1x and 3x, r2_1's frames 1-3 agree with each other best (same
  sword angle and the same shield). In r1_1, frame 3's shield is drawn bigger. In r3_2, frame 1's shield is bigger.
  r2_1 also gave the best flinch, so the whole of sheet A comes from one variant.

### What worked
- Pose and size: frames 1-3 are the same character at the same scale (k=1.004). The feet stay planted (bottom row
  125 in every frame, feet-x std 0.22). The breath is there: head and chest up in frames 1-2, and the sword and
  shield peak in frame 2 (steel and wood peak frame 2, tunic peak frame 1, overlap index 0.43).
- Gates: no facing flip, no hand swap, nothing missing. Frame 0 is pixel-identical to the sprite. The metrics
  flagged 0 gates, and I confirm that by eye on `preview/idle_strip3x.png` and `idle_map1x.png`.

### What failed
- **It boils, heavily.** Every step swaps the drawing. 37-64 % of the figure's opaque pixels change by more than
  40 levels per step: 0→1 2445 px, 1→2 1521, 2→3 2621, 3→0 1495, out of about 4100. The diff heat map lights the
  whole figure on 0→1 and 2→3.
- **What is visible at 1x in the map strip:**
  - The face and eye pixels change.
  - The hair outline changes: hair displacement is 3.8-4.4 px, against a beat of 1 px.
  - The blade looks more upright in frames 1-3 than in the sprite, because the hilt is drawn further right. So the
    sword flicks at 0→1 and again at 3→0.
  - The redraws carry a thin purple fringe along the outlines (65-80 magenta-tinted edge pixels per frame against 0
    in the sprite), so frame 0 looks crisper than frames 1-3.
- At 1x the loop reads as a once-a-second shimmer of the whole figure plus a sword twitch, not as a breath.
- The sword tip rises 1, 5 and 3 px in frames 1-3, where the beat asks for 0, 2 and 1.
- Wording cannot fix this. Two rounds of "copy cell 1 pixel for pixel" changed nothing measurable. The model always
  redraws a guided cell from scratch, and the redraw differs from the original sprite much more than the 1-2 px
  breath it is meant to show. With this method, idle is the weakest of the four animations.

### Spend and time
Sheet A only (it also produced the flinch): 3 calls, n=2 each, 6 images, $0.2365. See the flinch section for the
time.

### If I had another hour
A redraw-based method cannot do a 1-px breath. The honest fix lies outside G1: keep frames 1-3 as the sprite
itself plus a small transform or rig move, and use the model only for big poses. Within G1, the next step would be
to compose idle from the model's redraw of cell 1 in the same sheet (frame 0 would still have to be the true
sprite, so the pop at 0→1 stays). Alternatively, apply a palette snap or paste back onto the sprite, but that is
the round-2 finish pass, not this method.

## Flinch

### Method
Sheet A, cells 5-8: four flinch blockouts (impact, recoil extreme, recovering, settle) on the same sheet as idle.
The sliced cells become flinch frames 0-3 unchanged.

### What I did
Same three calls as idle (the flinch texts never changed). All six variants gave usable flinches with correct
facing and hands. Picked **`r2_1`**, the same variant as idle, with no mixed frames. Frame 0 matches the beat best:
the head is tilted back toward the upper-right with the eyes shut, the sword arm is thrown up and out, and the
shield is flung to the right. Frame 1 leans further back, with the knees bent and the sword dipping. In r1_1, the
next best, frame 0 is a grimace with the head barely back. In r1_2 the hair in frame 0 has a pink keying fringe.

### What worked
- The poses read at 1x as a clean hit reaction: snap back, recoil, halfway, settle. Peak displacement of the steel,
  tunic, hair and wood all falls on frame 1, as the recoil extreme should.
- The character stays consistent across the four frames (drift mean 0.94). Non-rigidity is 0.25-0.39, so these
  are real new poses. All frames stay inside x 37-129.
- 0 gate flags, confirmed by eye: he faces the lower-left, the sword is in the screen-left hand and the shield on
  the screen-right arm in every frame, and both props are present.

### What failed
- Flinch frame 3 (settle) → idle frame 0 (the true sprite) has the same redraw pop as idle 3→0: face, hair and
  blade angle change at once.
- Frame 2 has a dark brown strap hanging at the right hip that the sprite does not have.
- The planted feet slide a little (feet-x std 1.9 px). The ground row holds at 125.
- 60-75 purple-tinted edge pixels per frame. They show at 3x around the shield rim and hair, and are hard to see at
  1x.
- The metrics flag no overlapping action (overlap index 0): every part peaks on frame 1. The blockouts already put
  every part's extreme on frame 1, so the model followed them.

### Spend and time
Sheet A: 3 calls, $0.2365, shared with idle. Wall clock for sheet A, from template to written and checked, was
509 s (about 8.5 min), from `timings.jsonl`. That figure overlaps with death, because sheets A and B round 1 ran in
parallel. Well inside the 60-minute timebox.

### If I had another hour
Nothing urgent. A fourth sample might give a settle frame closer to the sprite's own drawing, which would soften the
return to idle.

## Death

### Method
Sheet B: cells 2-8 hold death blockouts 1-7: stagger, buckle, kneel, fall back, hit the ground, bounce, corpse. In
the last four the body lies along the iso axis. Frames 1-7 = sheet B cells 2-8. Frame 0 = flinch frame 0 from sheet
A `r2_1` (the beat sheet makes them the same pose).

### What I did
- **Round 1 only** (`sheetB_r1`, n=2). Both variants followed every blockout.
- **Picked `r1_1`.** Variant 2's frame 4 (falling back) was drawn about 22 canvas px above the ground line. That is
  beyond the slicer's 20 px snap, so it would float. Its lying frames also carry pink keying fringe at the shield,
  and its sword reaches x 16.
- r1_1 had no defect worth another round. The ground snaps for frames 4-7 were +13 and +14 px, inside the default
  20, because the model drew the bottom row a little high.
- Death mixes sheets by design, not by choice: frame 0 comes from sheet A r2_1 and frames 1-7 from sheet B r1_1.

### What worked
- The whole fall reads at 1x on the tile:
  1. Impact.
  2. Stagger back with the sword flung up.
  3. Knees buckling with the sword lowering.
  4. Kneeling with the sword tip on the ground and the shield hanging.
  5. Tipping back, with the sword already out of the hand and lying at the lower-left.
  6. Lying on his back with the feet to the lower-left, the head to the upper-right and the shield flat beside him.
  7. Bounce.
  8. Corpse.
- The guides got the lying pose onto the tile, the case G0's notes expected to be hardest without guides. Frame 0
  (sheet A) and frame 1 (sheet B) are visibly the same character, so the seam between the two sheets does not pop.
- 0 gate flags, confirmed by eye: no facing flip, the sword stays in the screen-left hand through frame 3, and in
  frames 4-7 it lies on the ground, which the gates allow from frame 4. The sword and shield are present in every
  frame. Everything stays inside x 25-130.

### What failed
- The corpse lies a little flatter than the blockout. The principal axis of frames 5-7 is 10-13°, against 13.5-17°
  for the guide and 26.6° for the iso axis. It still reads as lying on the tile, but closer to side view than
  intended.
- Frame 6 (bounce): instead of a 2 px lift of the head and shoulders, the model raised the right fist and redrew the
  arm. The bounce reads as an arm twitch. Frames 5→6→7 also boil in the hair and arm (boil about 68 per step),
  though at 80 ms this hardly shows. The held corpse frame is steady.
- Frame 4 shows the sword already lying on the ground rather than slipping out of the hand.
- 67-126 purple-tinted edge pixels per frame (most in frame 1, around the raised sword). The ground snap was +13 to
  +14 px on frames 4-7, so the model's bottom row was slightly high. The snap fixed it.

### Spend and time
Sheet B: 1 call, n=2, $0.0781. It ran in parallel with sheet A round 1 and was written and checked inside the same
509 s window (second row in `timings.jsonl`).

### If I had another hour
Run a second sheet B round with frame 6 reworded ("the same pose as cell 7, only the head and shoulders a little
higher; arms unchanged") and one sentence asking for the body along the tile's diagonal. Then pick per sheet again.

## Totals for this agent
4 calls (8 images), all tagged `anim/G1`: $0.0780 + $0.0781 + $0.0793 + $0.0792 = **$0.3146** from the ledger. Raws,
templates, slice diagnostics and review strips are in `work/G1/rest/`.
