# Animating M7 units: idle, strike, flinch, death

This is the working method for giving an M7 isometric unit sprite (the art-mode sprites made with the M7 package) its
four combat animations. It came out of a blind bake-off of seven methods in October 2026: after fixes, it had the top
mean score in all four animations in a second blind round against the strongest of the others (evidence at the end).
Follow it as written. If you change it, re-test it the same way (see "Changing the method").

Everything below runs from `projects/ffh-anim-sprint/`. In a sparse agent worktree (see the root `CLAUDE.md`) run
`git sparse-checkout add projects/ffh-anim-sprint` first.

## The rules

1. **The strike goes where the sprite is looking.** Every unit strikes toward the side its face and eyes point to: the
   neighbouring tile at the lower-left or at the lower-right. Its flinch is a hit from that side, and its death falls
   away from it, because the attacker stands where the unit looks. Sprites differ: the ogre looks lower-left, while the
   swordsman, Hyborem, the worker, the elf archer, the priest and the hill giant look lower-right. So decide the facing
   for every unit (step 1b) and record it in the unit file; the recipe then picks poses, beat texts and prompt that
   strike that way. The sprint's judged swordsman set (`final/`) breaks this rule (he looks lower-right and strikes
   lower-left), so do not copy it.
2. **Big actions (strike, flinch, death): the image model draws them on one guided sprite sheet per animation.**
   Cell 1 of the sheet is the real sprite; every other cell holds a flat blockout of that frame's pose. The model
   redraws the character in each pose; code keys, scales and places the frames.
3. **Small motion (idle, and the last frame of the strike and flinch): the real sprite's own pixels, bent by a
   sharpened mesh warp.** No redraw, so nothing shimmers, and the animation lands back on the exact static sprite.
4. **Do not** animate actions by moving, rotating or cutting up the static sprite. That is the stiff look the sprint
   set out to fix: the whole-sprite tween came last in strike, flinch and death with every judge, and the cut-out rig
   second to last.
5. **Do not** let the image model draw the idle. A one-pixel breath is smaller than the difference between any two of
   its drawings, so every model-drawn idle boiled.
6. **Do not** generate frames one call at a time, each from the previous frame. Drift accumulates, and it costs
   several times more.

`lib/recipe.py` does all of this. Your job is to describe the unit, run the recipe, pick variants by eye and check the
result.

## What you produce

For a unit `<unit>` you write `units/<unit>.json` and get two folders:

| Folder | Contents |
|---|---|
| `final_<unit>/` | the shipping look: a white hit flash on frame 0 of flinch and death, plus a smear arc and an impact spark on the strike when the weapon is a steel-coloured blade (other weapons get no strike effects) |
| `final_<unit>_nofx/` | the same frames without effects, for engines that draw their own effects |

Each holds, per animation `idle`, `strike`, `flinch`, `death`:

- `<anim>/frame_NN.png`: 160×128 RGBA frames, soft-edged like the M7 sprites.
- `<anim>/manifest.json`: `unit`, `facing`, `loop`, `hold_last`, and per frame `ms` and `events` (`"hit"` on strike frame 4).
- `sheet_<anim>.png`: the frames side by side, 160 px apart.
- `unit_poses.json`: the poses the guides and warps were made from (for QA and metrics).

| Animation | Frames | Timing | Notes |
|---|---|---|---|
| idle | 4 | 250 ms each, loops | frame 0 is the static sprite, unchanged |
| strike | 8 | 870 ms | frame 0 is the static sprite; `hit` on frame 4; returns to idle |
| flinch | 4 | 440 ms | frame 0 is the impact; returns to idle |
| death | 8 | 1180 ms, last frame held | frame 0 is the flinch impact; last frame is the corpse |

**Engine contract:**
- Anchor: canvas point (80, 126) goes on the tile centre (`tile_x + 96, tile_y + 54`), exactly like a static M7 unit
  (its bottom-centre, feet 2 px above the bottom).
- The banner goes left of the **idle frame-0** bounding box, fixed per unit, so it does not jump while the unit animates.
- Facing: the manifest's `facing` is the way the unit looks and strikes in these frames. To fight toward the other
  lower diagonal, mirror every frame; the 160-wide canvas mirrors about the anchor. Upper facings need back views, which
  this method does not make.

`spec.md` and `beats.md` are the full contract and the swordsman's frame-by-frame beat sheet.

## Before you start

```bash
cd projects/ffh-anim-sprint
pip install pillow numpy scipy requests      # and ffmpeg on PATH, for the preview MP4s
```

- **API key.** In a Claude Code cloud session the proxy injects the OpenAI key. Elsewhere, `export OPENAI_API_KEY=...`.
- **Budget.** Every image call goes through `m7/lib/imggen.py`, which logs it to `spend.jsonl` and refuses a call that
  would exceed any of three caps in `budget.json`: the unit's tag (`anim/units`, $5 at the time of writing), the root
  tag `anim` ($20) and `total` ($20). The check uses a pessimistic estimate of about $0.25 per sheet call; the bill is
  about $0.08. For bulk work raise every cap that binds. A unit file without a `tag` is billed to `anim/final`.
- **The sprite.** An M7 unit PNG at most 128 px wide and 128 px tall, feet 2 px above the bottom, looking lower-left or
  lower-right, holding one weapon in one hand. Do not mirror it: step 1b records which way it looks.

## Step 1: describe the unit

**1a. Render the grid (free).**
```bash
python3 lib/recipe.py --grid <the sprite's path> --out final_<unit>
```
This writes `work/final_<unit>/grid.png`: the sprite on the 160×128 animation canvas at 6×, with grid lines every 8 px
and labels every 16, and prints where the sprite sits on the canvas (a 96×120 sprite at (32, 8); Hyborem, 105 wide, at
(27, 8)). Read every joint position from the grid in **canvas** coordinates.

**1b. Decide which way the unit looks.** Zoom into the head on the grid. If the eyes, the nose and the turn of the face
point screen-left, the unit looks **lower-left**; screen-right, **lower-right**. For a face drawn straight at the
viewer, use the hair or hood (it sits on the back of the head), the chest and the feet. Write it as `"facing"`; the
recipe refuses a unit file without it.

**1c. Write `units/<unit>.json`.** Start from `units/ogre.json` (a lower-left unit with a club, no shield) or
`units/hyborem.json` (lower-right, greatsword, no shield):

| Key | What to put |
|---|---|
| `name` | the unit key, e.g. `"ogre"`; it goes into the manifests |
| `sprite` | path from the project root, e.g. `"m7/roster/units/unit_ogre_embers.png"` |
| `facing` | `"lower-left"` or `"lower-right"` (step 1b) |
| `tag` | `"anim/units"` (check it: a copied file may carry another tag) |
| `description` | one line the model must keep: body, colours, armour, the weapon and **which hand** (`"... a spiked club in his RIGHT hand"`). Name the FfH unit, then describe what it looks like. |
| `hands` | one sentence, e.g. `"the club stays in his right hand on the screen-left side and his left hand is an empty fist on the screen-right side"` |
| `guide_style` | `"grey"` for every unit except the swordsman (its blockouts use the swordsman's own colours) |
| `scale` | the figure's height ÷ 86 (the swordsman's), measured from the top of the head or hair (not horns, hat or weapon) to the soles; e.g. 98 px → 1.14. It scales how far the body travels (the swordsman's moves), not the pose angles. |
| `head_r` | head radius in px (the swordsman's is 13) |
| `limb` | limb thickness relative to the swordsman: 1.0 for human builds, about 1.4 for hulking ones |
| `joints` | frame-0 positions, below; every joint is required except `shield` |
| `beats` | rewordings of beat texts, step 1d |

Optional keys: `palette` (overrides guide colours), `defringe: false` (turns the magenta de-fringe off), `sharpen`
(warp sharpening, default 3.0). `legacy`, `same_as_source` and `poses_from` are for the sprint's own sets; leave them out.

Joints, in canvas coordinates `[x, y]`, placed **on the sprite as drawn**. `r_` is the weapon arm and the leg on the
same side of the body, wherever they are on screen; `l_` is the other arm and leg:

| Joint | Where |
|---|---|
| `head` | centre of the head, not counting horns or hat |
| `neck`, `chest`, `pelvis` | base of the neck, middle of the chest, middle of the belt line |
| `r_sh`, `r_el`, `r_ha` | weapon arm: shoulder, elbow, hand (the fist on the grip) |
| `tip` | the weapon's far end (blade tip or club head), wherever it rests, even over the shoulder |
| `l_sh`, `l_el`, `l_ha` | the other shoulder, elbow, hand |
| `shield` | `[cx, cy, rx, ry]`, **only if the unit carries a shield**; leave it out otherwise |
| `r_hip`, `r_kn`, `r_ft`, `l_hip`, `l_kn`, `l_ft` | hips, knees, ankles |

How the poses are made: the facing and the weapon's screen side pick a source pose set, which is retargeted onto your
joints. Each bone keeps your unit's frame-0 angle and length and turns as the source bone turns; in the frames the model
draws, the weapon arm and weapon swing to the source's actual direction once the source bone has turned well away
from rest, so a weapon carried differently (a club on the shoulder) still swings where the beat sheet says. The warped
frames copy the source's small joint movements (the breath). `--show-beats` prints which set your unit gets.

| Sprite looks | Weapon hand on screen | Source poses |
|---|---|---|
| lower-left | left (the far hand) | far set, `poses.json` (the ogre) |
| lower-right | left (the near hand) | near set, `poses_near.json` (the swordsman, Hyborem) |
| lower-left | right (the near hand) | near set, mirrored |
| lower-right | right (the far hand) | far set, mirrored |

Approximate joints are fine; the model redraws the whole figure. What matters is that `r_ha` to `tip` follows the
weapon and that the feet sit on the feet.

**1d. Check the beat texts (free).**
```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit> --show-beats
```
This prints the facing, the pose set, and the text each sheet cell will get (strike frames 1–6, flinch 0–2, death 1–7),
marking texts that name a shield, sword or blade. The defaults already point the right way for your unit's facing.
Override every text that names a prop your unit does not have (a club instead of a sword, an empty fist or a cape
instead of a shield), as a string keyed by frame number:
```json
"beats": {"strike": {"2": "...", "4": "..."}, "flinch": {"0": "..."}, "death": {"5": "..."}}
```
Write overrides for the unit's real facing and keep every direction the default text gives (toward the facing for the
strike, away from it for the flinch and the fall). `units/ogre.json` and `units/hyborem.json` have worked rewordings.

**1e. Check the guides (free).**
```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit> --check-guides
```
Open `work/final_<unit>/guides_check.png`. Row 1 starts with the frame-0 blockout laid over the sprite in a red tint:
limbs, head and weapon should lie on the sprite's. The rest of row 1 is the strike poses the model will be asked to
draw, row 2 the flinch and row 3 the death. Check that the strike reaches toward the unit's facing, and fix the joints
until frame 0 lines up and no pose looks broken. If it warns that the guide weapon was shortened, the weapon is too
long for the 160×128 canvas in those frames: the model will draw it full length and it will be cut off at the edge
(see Known limits).

## Step 2: run the recipe

```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit>
```
This sends three sheet calls in parallel (strike, flinch, death), with two variants each. It takes about 21 s of model
calls plus about 6 s to cut, warp and assemble, for about $0.23. It writes `final_<unit>/` and `final_<unit>_nofx/`.
Raws, templates and diagnostics go to `work/final_<unit>/`, which is not committed.

## Step 3: pick the variants by eye

The recipe picks a variant per sheet by a score (reference-cell fidelity, scale, ground snaps, and gate flags that only
make sense for the swordsman's colours). **That score cannot see colour smudges, off-model faces or a strike in the
wrong direction, so always look.** For each sheet open `work/final_<unit>/variants_<sheet>.png`: one row per variant at
2×, cell 1 the real sprite, frames shown as they will be used. A cell label says when the frame was slid sideways onto
its guide (`slid -24`: the model drew the figure 24 px off) or snapped down to the ground by 4 px or more. Choose the
variant with fewer of these defects:

- **wrong direction:** the strike lands on the side away from where the face looks, the face turns the other way, or
  the hit comes from the wrong side;
- **colour smudges:** a patch of the wrong colour on clothing (tan on blue shorts), or an extra strap;
- **size pop:** a figure noticeably bigger than the sprite (often frame 1 or the hit frames), or swollen hair;
- **wrong pose:** the weapon in the wrong hand, a missing or doubled prop, or a pose that ignores the guide;
- **off-model face:** a different face, squeezed manga eyes, or a front-facing head;
- **sideways slide:** the feet jump between frames even after the slide (compare with frame 0);
- **cropping:** a weapon or limb cut off at the canvas edge;
- **magenta or purple specks** on outlines or seams (the recipe recolours most of them);
- **death:** a corpse lying side-on instead of along the tile's diagonal, or a bounce (frame 6) that reads as a sit-up.

Then re-cut with your picks. This is free, with no API calls:
```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit> --rebuild --pick strike=2 flinch=1 death=1
```
If both variants of a sheet are bad, regenerate that sheet. A re-roll overwrites the sheet's previous variants.
- strike: `--anims strike` (one call, about $0.08);
- flinch: `--anims flinch` (one call);
- death: `--anims flinch death` (two calls: the death sheet always regenerates the flinch sheet too, because death frame
  0 is flinch frame 0).

Then finish with the full `--rebuild --pick strike=… flinch=… death=…`, so every folder is re-cut from the same picks.

## Step 4: preview, check, hand over

```bash
python3 lib/preview.py --arm final_<unit> --root final_<unit>
```
This writes, in `final_<unit>/`:
- `sequence_3x.{gif,mp4}`: idle ×2, strike, idle, flinch, idle, death, then a 1.5 s corpse hold;
- `sequence_1x.{gif,mp4}`: the same on a 3×3 map at game scale;
- `vignette.{gif,mp4}`: two copies fighting, the attacker on the side it faces;
- `preview/`: per animation `<anim>_strip3x.png` (frames at 3× with timings and the hit marked), `<anim>_map1x.png`
  (game scale), an onion skin and a silhouette strip.

If you cannot play GIFs, read the strips: `preview/<anim>_strip3x.png` for detail and `preview/<anim>_map1x.png` for
game scale. Check:

- [ ] the strike lands where the unit looks: on the hit frame (strike 4) the weapon is out on the `facing` side of the
      body, the front foot steps that way and the face still looks that way;
- [ ] the flinch recoils away from the facing side and the corpse falls away from it;
- [ ] idle loops with no pop at 3 → 0 and the feet stay put;
- [ ] strike reads at 1×: wind-up, swing, contact on the hit frame, follow-through, a clean settle into idle;
- [ ] flinch snaps back on frame 0 and returns to idle without a pop;
- [ ] death ends on a corpse lying on the tile, not floating;
- [ ] the same character in every frame: face, outfit, colours, weapon, size;
- [ ] no magenta specks on outlines (check at 3×; the held corpse frame matters most);
- [ ] nothing important is cut off at the canvas edge.

`python3 lib/metrics.py --arm final_<unit> --root final_<unit>` gives numbers. Only `nonrigid`, `jitter` and `boil` are
palette-independent; the gates, `drift_dE`, `overlap` and `arc` look for the swordsman's colours, so ignore them for
other units.

**Commit** `units/<unit>.json`, `final_<unit>/` (frames, manifests, sheets, `unit_poses.json`, the sequence and vignette
files and the preview stills) and `final_<unit>_nofx/` (frames, manifests, sheets). `.gitignore` already leaves out
`work/` and the per-animation preview GIFs.

## Known limits

- **Canvas size.** The canvas is 160×128 with a safe region of x 16–143. A unit taller than about 110 px, or one with a
  long weapon, gets clipped: the ogre's club and Hyborem's greatsword are cut off at the top or the side in some frames.
  The recipe refuses sprites taller or wider than 128. A bigger canvas means changing `W`, `H`, `ANCHOR`, `SAFE_X` and
  `SPRITE_OFFSET` in `lib/animkit.py`; `canvas`, `anchor` and `sprite_offset` in `poses.json` (via `lib/poses_src.py`,
  which `lib/guides.py` reads) and `poses_near.json`; `REGION`, `GROUND_Y` and `SCALE` in `lib/sheet.py` (128 × 3 already
  fills a 384 px cell, so the cell size from `default_geom` and the image size in `lib/gen.py` `CONTRACT` change too);
  the guide-weapon clamp (x 18–142, y 3–125) in `recipe.retarget`; the cell sizes in `recipe.review_sheet`; and the
  scene sizes in `lib/preview.py`. That is untested. The sideways slide (step 3) can push a long weapon further past
  the edge: Hyborem's greatsword in strike frames 3 and 4.
- **Purple or magenta art** (Sheaim, liches, infernal fire). The model's background is magenta and is keyed out. The
  de-fringe step turns itself off when 1% or more of the sprite's opaque pixels are purple (the lich and the Sheaim
  mage), but keying can still eat purple art. The magenta key is assumed by the prompt, the template background
  (`sheet.make_template`) and `sheet.slice_sheet`; another key colour is not supported yet.
- **Body plans.** The poses are a biped swinging a one-handed melee weapon. Archers and casters need their own beat sheet
  and poses (a draw-and-release or a cast, in `lib/poses_src.py`, `lib/poses_near_src.py` and `lib/beats.py`). Mounted
  units, animals and ships are untested.
- **Facing.** Lower-left and lower-right only (see the engine contract).
- **The near set** (`poses_near.json`, lower-right) was written after the sprint for the strike-direction rule. It has
  not been through a blind bake-off like the far set.
- **Remaining model defects.** Even the best variants carry small smudges, a corpse a little flatter than the isometric
  axis, and occasionally a death bounce that is too big. The pick in step 3 is the control.
- **Model choice.** `gpt-image-2.5-sunburst` at medium quality, 1536×1024, two variants per call (`lib/gen.py`
  `CONTRACT`). In calibration gpt-image-2.5-flare redrew the reference cell and gpt-image-2 rescaled everything by 19%.
  High quality was only marginally crisper, at twice the price.

## Changing the method

If you improve the recipe, re-test it before relying on it. Run the old and the new version on the same unit, then have
judges compare them blind: letters instead of names, a frame-ordering test, and scores for motion, consistency, style
and appeal (`lib/judging.py`, `JUDGE_BRIEF.md`). Agents judging stills are a useful filter. A person watching the motion
is the real test.

## Why this method (evidence)

The October 2026 sprint ran seven methods on the swordsman under one beat sheet (`arms/*/`). Three judge agents scored
every animation blind; this recipe's first build was entered as an unnamed letter.

| Round 1, weighted score /10 | idle | strike | flinch | death |
|---|---|---|---|---|
| this recipe (first build) | 7.80 | 6.65 | 7.37 | 7.28 |
| sheet, no pose guides | 3.80 | 7.60 | 6.18 | 7.03 |
| model repaints rig frames | 5.78 | 7.35 | 7.20 | 7.18 |
| keys, then in-betweens | – | 7.20 | – | 7.62 |
| cut-out rig + mesh warp | 8.07 | 4.95 | 5.20 | 3.97 |
| whole-sprite tween (the stiff look) | 6.28 | 4.58 | 5.03 | 3.62 |

In round 1 the first build led only the flinch. After three fixes that cost no new image calls (picking the better
strike variant, sharpened warps, de-fringing), fresh blind judges compared it with its first build and the two best
other methods per animation, and gave it the top mean score in all four: 7.90 / 7.68 / 7.62 / 7.85. Only the death was
ranked first by every judge, and the idle and strike leads (0.2 and 0.3) are too small to call.

- Report, with every animation playable: https://claude.ai/artifact/3eWLaH1v9PHpu6G6FGKExb
- Data: `judging/` (scores, judges' JSON and keys), `spend.jsonl`, and the method arms' `notes.md`.

### After the sprint

- **Rule 1** (the strike goes where the sprite looks) was added after the sprint. Most roster sprites look lower-right,
  so a second pose set was made for them (`poses_near.json`): three agents each designed one, three blind judges scored
  the rendered guides, and the winner (7.83 of 10 in strike, flinch and death, against 7.33 and 5.33 for the others in
  the strike) was refined from the judges' notes.
- **A cold test** of this guide on the ogre, by an agent that read nothing else, found four gaps that are now fixed in
  the recipe: a weapon carried differently from the swordsman's (a club on the shoulder) swung the wrong way, other units
  did not breathe in the idle, the model's sideways placement made the ogre hop backwards while striking, and purple
  specks survived inside the figure.
- VERIFY-PLACEHOLDER

## Worked examples

- `units/swordsman.json` → `final_swordsman/`: looks lower-right, sword in the near hand, shield; near set, colour
  guides.
- `units/hyborem.json` → `final_hyborem/`: looks lower-right, a long greatsword, no shield; near set retargeted, grey
  guides, beat texts reworded (fist and cape for the shield).
- `units/ogre.json` → `final_ogre/`: looks lower-left, a long club resting on the shoulder, no shield; far set
  retargeted, beat texts reworded (club for the sword).
- Pre-rule sets, kept only to reproduce the sprint (they break rule 1; `"legacy": true`): `units/swordsman_judged.json`
  → `final/`, `units/hyborem_v1.json` → `final_hyborem_v1/`.
