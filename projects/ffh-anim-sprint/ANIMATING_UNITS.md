# Animating M7 units: idle, strike, flinch, death

This is the working method for giving an M7 isometric unit sprite (the 96×120 art-mode sprites made with the
M7 package) its four combat animations. It won a blind bake-off of seven methods in October 2026; the evidence is
at the end. Follow it as written. If you change it, re-test it the same way (see "Changing the method").

Everything below runs from `projects/ffh-anim-sprint/`. In a sparse agent worktree (see the root `CLAUDE.md`) run
`git sparse-checkout add projects/ffh-anim-sprint` first.

## The rule

- **Big actions (strike, flinch, death): the image model draws them on one guided sprite sheet per animation.**
  Cell 1 of the sheet is the real sprite; every other cell holds a flat blockout of that frame's pose. The model
  redraws the character in each pose; code keys, scales and places the frames.
- **Small motion (idle, and the last frame of the strike and flinch): the real sprite's own pixels, bent by a
  sharpened mesh warp.** No redraw, so nothing shimmers, and the animation lands back on the exact static sprite.
- **Do not** animate actions by moving, rotating or cutting up the static sprite. That is the stiff look the
  sprint set out to fix, and it came last for strike, flinch and death with every judge.
- **Do not** let the image model draw the idle. A one-pixel breath is smaller than the difference between any two
  of its drawings, so every model-drawn idle boiled.
- **Do not** generate frames one call at a time, each from the previous frame. Drift accumulates, and it costs
  several times more.

`lib/recipe.py` does all of this. Your job is to describe the unit, run the recipe, pick variants by eye and check
the result.

## What you produce

For a unit `<unit>` you write `units/<unit>.json` and get two folders:

| Folder | Contents |
|---|---|
| `final_<unit>/` | the shipping look: frames with a smear arc and impact spark on the strike and a white hit flash on frame 0 of flinch and death |
| `final_<unit>_nofx/` | the same frames without effects, for engines that draw their own effects |

Each holds, per animation `idle`, `strike`, `flinch`, `death`:

- `<anim>/frame_NN.png`: 160×128 RGBA frames, soft-edged like the M7 sprites.
- `<anim>/manifest.json`: per-frame `ms`, `events` (`"hit"` on strike frame 4), `loop`, `hold_last`.
- `sheet_<anim>.png`: the frames side by side, 160 px apart.

| Animation | Frames | Timing | Notes |
|---|---|---|---|
| idle | 4 | 250 ms each, loops | frame 0 is the static sprite, unchanged |
| strike | 8 | about 870 ms | frame 0 is the static sprite; `hit` on frame 4; returns to idle |
| flinch | 4 | about 440 ms | frame 0 is the impact; returns to idle |
| death | 8 | about 1.2 s | frame 0 is the flinch impact; last frame (corpse) is held |

**Engine contract:**
- Anchor: canvas point (80, 126) goes on the tile centre (`tile_x + 96, tile_y + 54`), exactly like a static M7
  unit (its bottom-centre, feet 2 px above the bottom).
- The banner goes left of the **idle frame-0** bounding box, fixed per unit, so it does not jump while the unit
  animates.
- Facing: everything faces lower-left. Mirror the frames for lower-right; the 160-wide canvas mirrors about the
  anchor. Upper facings need back views, which this method does not make.

`spec.md` and `beats.md` are the full contract and the frame-by-frame beat sheet.

## Before you start

```bash
cd projects/ffh-anim-sprint
pip install pillow numpy scipy requests
```

- **API key.** In a Claude Code cloud session the proxy injects the OpenAI key. Elsewhere, `export OPENAI_API_KEY=...`.
- **Budget.** Every image call goes through `m7/lib/imggen.py`. It refuses a call that would exceed the caps in
  `budget.json` and logs every call to `spend.jsonl`. Set `"tag": "anim/units"` in your unit file (cap $5 at the
  time of writing). Raise that cap, or the `total`, for bulk work. One unit costs about $0.23; a re-roll of one sheet
  costs about $0.08.
- **The sprite.** An M7 unit PNG: at most 128 px wide and 128 px tall, feet 2 px above the bottom, facing lower-left,
  holding its weapon in the hand on the **screen-left** side (the far hand). All melee units in the M7 roster do. Put it
  anywhere under the project; the unit file names the path. If it faces right, mirror it first.

## Step 1: describe the unit

**1a. Read the joints.**
```bash
python3 lib/recipe.py --grid m7/roster/units/unit_<unit>.png --out final_<unit>
```
This writes `work/final_<unit>/grid.png`: the sprite on the 160×128 animation canvas at 6×, with grid lines every
8 px and labels every 16. Read joint positions from it in **canvas** coordinates. A 96×120 sprite sits at offset
(32, 8), so sprite pixel (x, y) is canvas (x+32, y+8).

**1b. Write `units/<unit>.json`.** Copy `units/hyborem.json` and edit it:

| Key | What to put |
|---|---|
| `name` | the unit key, e.g. `"ogre"` |
| `sprite` | path from the project root, e.g. `"m7/roster/units/unit_ogre_embers.png"` |
| `tag` | `"anim/units"` |
| `description` | one line the model must keep: body, colours, armour, the weapon and **which hand** (`"... a spiked club in his RIGHT hand"`). Name the FfH unit, then describe what it looks like. |
| `hands` | one sentence, e.g. `"the club stays in his right hand on the screen-left side and his left hand is an empty fist on the screen-right side"` |
| `guide_style` | `"grey"` for every unit except the swordsman (its blockouts use the swordsman's own colours) |
| `scale` | figure height ÷ 86 (the swordsman's height), e.g. 1.25 for a 106 px figure; it scales body movement |
| `head_r` | head radius in px on the sprite (the swordsman's is 13) |
| `limb` | limb thickness relative to the swordsman: 1.0 for human builds, about 1.4 for hulking ones |
| `joints` | frame-0 positions, below |
| `beats` | rewordings of beat texts that mention props this unit does not have, below |

Joints (canvas coordinates, `[x, y]`). `r_` is the weapon side (screen-left, the far side); `l_` is the other side
(screen-right, the near side):

| Joint | Where |
|---|---|
| `head` | centre of the head, not counting horns or hat |
| `neck`, `chest`, `pelvis` | base of the neck, middle of the chest, middle of the belt line |
| `r_sh`, `r_el`, `r_ha` | weapon-side shoulder, elbow, hand (the fist on the grip) |
| `tip` | the weapon's tip (the far end of the blade or club head) |
| `l_sh`, `l_el`, `l_ha` | other shoulder, elbow, hand |
| `shield` | `[cx, cy, rx, ry]`, **only if the unit carries a shield**; leave it out otherwise |
| `r_hip`, `r_kn`, `r_ft`, `l_hip`, `l_kn`, `l_ft` | hips, knees, ankles (screen-left leg is `r_`) |

The animation is the swordsman's beat sheet retargeted onto these joints: every bone keeps the unit's own frame-0
angle and length and turns by however much the swordsman's matching bone turns. Approximate joints are fine; the
model redraws the whole figure. What matters is that `r_ha` to `tip` follows the weapon, and that the feet sit on the
feet.

Beat texts: the recipe sends the sheet text for strike frames 1–6, flinch frames 0–2 and death frames 1–7 (`lib/beats.py`
has every default). Override any that name the shield, or the sword when the weapon is different, as a string keyed
by frame number:
```json
"beats": {"strike": {"2": "...", "4": "..."}, "flinch": {"0": "...", "1": "..."}, "death": {"1": "...", "5": "...", "6": "...", "7": "..."}}
```
For a shieldless unit these mention the shield: strike 2 and 4, flinch 0 and 1, death 1, 3, 5, 6 and 7. `units/hyborem.json`
has worked rewordings (an empty fist and a cape instead of a shield, a greatsword instead of a sword).

**1c. Check the guides (free).**
```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit> --check-guides
```
Open `work/final_<unit>/guides_check.png`. Row 1 starts with the frame-0 blockout laid over the sprite: limbs, head and
weapon should lie on the sprite's. The rest of row 1 is the strike poses the model will be asked to draw; row 2 is the
flinch and row 3 the death.
Fix the joints until frame 0 lines up and no pose looks broken.

## Step 2: run the recipe

```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit>
```
This sends three sheet calls in parallel (strike, flinch, death), with two variants each. It takes about 21 s of
model calls plus about 6 s to cut, warp and assemble, for about $0.23. It writes `final_<unit>/` and
`final_<unit>_nofx/`. Raws, templates and diagnostics go to `work/final_<unit>/`, which is not committed.

## Step 3: pick the variants by eye

The recipe picks a variant per sheet by a score (reference-cell fidelity, scale, ground snaps, gate flags). **That
score cannot see colour smudges or off-model faces, so always look.** For each sheet open
`work/final_<unit>/variants_<sheet>.png`: one row per variant at 2×, cell 1 the real sprite. Choose the variant with
fewer of these, which are the defects seen in the sprint:

- **colour smudges:** a patch of the wrong colour on clothing (tan on blue shorts), or an extra strap;
- **size pop:** a figure noticeably bigger than the sprite (often frame 1 or the hit frames), or swollen hair;
- **wrong pose:** the weapon in the wrong hand, a facing flip, a missing prop, or a pose that ignores the guide;
- **off-model face:** a different face, squeezed manga eyes, or a front-facing head;
- **cropping:** a weapon cut off at the canvas edge;
- **death:** a corpse lying side-on instead of along the tile's diagonal, or a bounce (frame 6) that reads as a sit-up.

Then re-cut with your picks. This is free, with no API calls:
```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit> --rebuild --pick strike=2 flinch=1 death=1
```
If both variants of a sheet are bad, regenerate only that sheet: `--anims strike` (one call, about $0.08). For a
flinch or death re-roll pass `--anims flinch death`, because death frame 0 comes from the flinch sheet. Then pick
again.

## Step 4: preview, check, hand over

```bash
python3 lib/preview.py --arm final_<unit> --root final_<unit>
```
This writes, in `final_<unit>/`:
- `sequence_3x.{gif,mp4}`: idle ×2, strike, idle, flinch, idle, death, then a 1.5 s corpse hold;
- `sequence_1x.{gif,mp4}`: the same on a 3×3 map at game scale;
- `vignette.{gif,mp4}`: two copies fighting;
- `preview/`: strips per animation (`<anim>_strip3x.png`, `<anim>_map1x.png`, onion skins).

Watch the 1× sequence. Check:

- [ ] idle loops with no pop at 3 → 0 and the feet stay put;
- [ ] strike reads at 1×: wind-up, swing, contact on the hit frame, follow-through, a clean settle into idle;
- [ ] flinch snaps back on frame 0 and returns to idle without a pop;
- [ ] death ends on a corpse lying on the tile, not floating;
- [ ] the same character in every frame: face, outfit, colours, weapon, size;
- [ ] no magenta specks on outlines (the recipe removes the fringe, but check at 3×);
- [ ] nothing important is cut off at the canvas edge.

`python3 lib/metrics.py --arm final_<unit> --root final_<unit>` gives numbers. Its automatic gates (facing, hand swap,
missing sword) look for the swordsman's colours, so ignore their flags for other palettes.

**Commit** `units/<unit>.json`, `final_<unit>/` and `final_<unit>_nofx/` (frames, manifests, sheets and the sequence and
vignette files). `.gitignore` already leaves out `work/` and the per-animation preview GIFs.

## Known limits

- **Canvas size.** The canvas is 160×128 with a safe region of x 16–143. A unit taller than about 110 px, or one with a
  long weapon, gets clipped: Hyborem's greatsword is cut off at the top in strike frame 1 and reaches the left edge on
  the swing. The recipe refuses sprites taller than 128 or wider than 128. A bigger canvas means changing `W`, `H`,
  `ANCHOR` and `SAFE_X` in `lib/animkit.py`, `REGION` and `GROUND_Y` in `lib/sheet.py`, and the template cell size. That
  is untested, so judge the first results carefully.
- **Purple or magenta art** (Sheaim, liches, infernal fire). The model's background is magenta and is keyed out. The
  recipe's de-fringe step skips itself when the sprite contains purple, but keying can still eat purple art. M7's
  static pipeline uses a cyan key for such units; this recipe does not support one yet (the prompt and
  `sheet.key_sheet` assume magenta).
- **Body plans.** The poses are a biped swinging a one-handed melee weapon. Archers and casters need their own beat
  sheet and poses (a draw-and-release or a cast, edited in `lib/poses_src.py` and `lib/beats.py`). Mounted units,
  animals and ships are untested.
- **Facing.** Lower-left only (see the engine contract).
- **Remaining model defects.** Even the best variants carry small smudges, a corpse a little flatter than the
  isometric axis, and occasionally a death bounce that is too big. The pick in step 3 is the control.
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
| cut-out rig + mesh warp | 8.07 | 4.95 | 5.20 | 3.97 |
| whole-sprite tween (the stiff look) | 6.28 | 4.58 | 5.03 | 3.62 |

After three fixes that cost no new image calls (picking the better strike variant, sharpened warps, de-fringing), fresh
blind judges gave the recipe the top mean score in all four animations: 7.90 / 7.68 / 7.62 / 7.85. Only the death was
ranked first by every judge, and the idle and strike leads (0.2 and 0.3) are too small to call. On a second unit,
Hyborem, the first calls gave a set that looks usable (`final_hyborem/`, not judged).

- Report, with every animation playable: https://claude.ai/artifact/3eWLaH1v9PHpu6G6FGKExb
- Data: `judging/` (scores, judges' JSON and keys), `spend.jsonl` and each arm's `notes.md`.
- Worked examples: `units/swordsman.json` → `final/`, `units/hyborem.json` → `final_hyborem/`.
