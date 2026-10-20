# Animating M7 units: idle, strike, flinch, death

This is the working method for giving an M7 isometric unit sprite (the art-mode sprites made with the M7 package) its
four combat animations. It came out of a blind bake-off of seven methods on the swordsman in October 2026: after fixes,
it had the top mean score in all four animations in a second blind round against the strongest of the others. Those
sets predate rule 1 below. The recipe as it stands now (rule 1, a second pose set, later fixes) has been compared blind
only with its own earlier sets (evidence at the end). Follow it as written. If you change it, re-test it the same way
(see "Changing the method").

Everything below runs from `projects/ffh-anim-sprint/`. In a sparse agent worktree (see the root `CLAUDE.md`) run
`git sparse-checkout add projects/ffh-anim-sprint` first.

## Quick start

For a unit `<unit>` (the steps below explain each one):

1. `python3 lib/recipe.py --grid <sprite.png> --out final_<unit>`: read the joints from `work/final_<unit>/grid.png`.
2. Decide which way the sprite looks, lower-left or lower-right (step 1b). Rule 1 depends on it.
3. Write `units/<unit>.json` from `units/ogre.json` or `units/hyborem.json`, replacing everything unit-specific.
4. `--show-beats`, then `--check-guides` (both free): reword texts that name props the unit lacks; check the guides
   strike toward the facing.
5. `python3 lib/recipe.py --unit units/<unit>.json --out final_<unit>`: three image calls, about $0.23.
6. Pick variants by eye in `work/final_<unit>/variants_*.png`, then `--rebuild --pick strike=… flinch=… death=…`.
7. `python3 lib/preview.py --arm final_<unit> --root final_<unit>`, run the step 4 checklist, commit.

## The rules

1. **The strike goes where the sprite is looking.** Every unit strikes toward the side its face and eyes point to: the
   neighbouring tile at the lower-left or at the lower-right. Its flinch is a hit from that side, and its death falls
   away from it, because the attacker stands where the unit looks. Sprites differ: the ogre and Hyborem look
   lower-left, the swordsman and the worker lower-right. So decide the facing for every unit with care (step 1b; Hyborem
   was misread at first glance) and record it in the unit file; the recipe then picks the poses, the beat texts and the
   prompt that strike that way. The sprint's swordsman sets (`final/`, `final_nofx/`, `final_v1_nofx/`) and every
   method set in `arms/` break this rule (he looks lower-right and strikes lower-left), so do not copy any of them.
2. **Big actions (strike, flinch, death): the image model draws them on one guided sprite sheet per animation.**
   Cell 1 of the 4×2 sheet is the real sprite; the next cells hold a flat blockout of each frame's pose (6 for the
   strike, 3 for the flinch, 7 for the death) and the rest stay empty. The model redraws the character in each pose;
   code keys, scales and places the frames.
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
| `final_<unit>/` | the shipping look: a white hit flash on frame 0 of flinch and death; on the strike, an impact spark on the hit frame at the outermost point of the largest steel-coloured patch in it (a blade, a club's spikes, or light steel armour if that is the largest patch), and a smear arc on the swing frame and on the hit frame, each only when a steel-coloured blade at least 20 px long shows in that frame and the one before it. A dark weapon such as Hyborem's flaming blade gets neither. |
| `final_<unit>_nofx/` | the same frames without effects, for engines that draw their own effects |

Each holds, per animation `idle`, `strike`, `flinch`, `death`:

- `<anim>/frame_NN.png`: 160×128 RGBA frames, soft-edged like the M7 sprites.
- `<anim>/manifest.json`: `unit`, `facing`, `loop`, `hold_last`, and per frame `ms` and `events` (`"hit"` on strike frame 4).
- `sheet_<anim>.png`: the frames side by side, 160 px apart.
- `unit_poses.json`: the poses the guides and warps were made from (for QA and metrics).
- after step 4, in `final_<unit>/` only: `metrics.json`, the sequence and vignette files and `preview/`.

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

`spec.md` is the sprint's contract (with a correction for rule 1). `beats.md` is the beat sheet: its main tables are the
far set (lower-left units such as the ogre and Hyborem; the sprint's swordsman sets used them too, against rule 1), and
its last section is the near set (the swordsman as he really is).

## Before you start

```bash
cd projects/ffh-anim-sprint
pip install pillow numpy scipy requests      # and ffmpeg on PATH, for the preview MP4s
```

- **API key.** In a Claude Code cloud session the proxy injects the OpenAI key. Elsewhere, `export OPENAI_API_KEY=...`.
- **Budget.** Every image call goes through `m7/lib/imggen.py`, which logs it to `spend.jsonl` and refuses a call that
  would exceed any of three caps in `budget.json`: the unit's tag (`anim/units`, $5 at the time of writing), the root
  tag `anim` ($20) and `total` ($20). The check uses a pessimistic estimate of about $0.25 per sheet call; `spend.jsonl`
  then logs about $0.08 per call (the returned token usage at deliberately high prices, so the real bill is at most
  that). For bulk work raise every cap that binds. A unit file without a `tag` is billed to `anim/final`.
- **The sprite.** An M7 unit PNG at most 128 px wide and 128 px tall, feet at or near the bottom (M7 leaves 2 px; the
  recipe stands the redrawn frames on the sprite's own feet row, so a few more empty rows, like Hyborem's 4, are fine),
  looking lower-left or lower-right, holding one weapon. Do not mirror it: step 1b records which way it looks. Paths in
  unit files and on the command line are relative to `projects/ffh-anim-sprint/`.

## Step 1: describe the unit

**1a. Render the grid (free).**
```bash
python3 lib/recipe.py --grid <the sprite's path> --out final_<unit>
```
This writes `work/final_<unit>/grid.png`: the sprite on the 160×128 animation canvas at 6×, with grid lines every 8 px
and labels every 16, and prints where the sprite sits on the canvas (a 96×120 sprite at (32, 8); Hyborem, 105 wide, at
(27, 8)). Read every joint position from the grid in **canvas** coordinates.

**1b. Decide which way the unit looks.** This is easy to get wrong at a glance: Hyborem was first read as lower-right,
and three independent blind checks then read lower-left. So crop the head and zoom it to 8× or more, and write down
which side of the head's centre the eye (or eyes), the nose and the mouth sit on, and which side the ear, the back of
the hair, a hood or a topknot sit on. Face features on the left and ear or hair on the right: **lower-left**; the
reverse: **lower-right**. For a face drawn straight at the viewer, use the hair or hood, then the chest and the feet.
If you can, have a second agent judge the facing from the sprite alone, without telling it your answer. Write it as
`"facing"`; the recipe refuses a unit file without it.

**1c. Write `units/<unit>.json`.** Start from `units/ogre.json` (lower-left, a club, no shield) or `units/hyborem.json`
(lower-left, a greatsword, no shield), and replace everything unit-specific: `name`, `sprite`, `facing`, `description`,
`hands`, `scale`, `head_r`, `limb`, `joints`, and every `beats` text. Both examples look lower-left, so their overrides
name the lower-left and the upper-right; overrides go to the model exactly as written and are never mirrored, so the
recipe refuses a unit whose overrides point the wrong way for its facing. Do not copy `units/swordsman.json`: it uses
the source poses unchanged (`same_as_source`), which only fits the swordsman.

| Key | What to put |
|---|---|
| `name` | the unit key, e.g. `"ogre"`; it goes into the manifests |
| `sprite` | path relative to `projects/ffh-anim-sprint/`, e.g. `"m7/roster/units/unit_ogre_embers.png"` |
| `facing` | `"lower-left"` or `"lower-right"` (step 1b) |
| `tag` | `"anim/units"` (check it: a copied file may carry another tag) |
| `description` | one line the model must keep: body, colours, armour, the weapon and **which hand** (`"... a spiked club in his RIGHT hand"`). Name the FfH unit, then describe what it looks like. |
| `hands` | one sentence, e.g. `"the club stays in his right hand on the screen-left side and his left hand is an empty fist on the screen-right side"` |
| `guide_style` | `"grey"` for every unit except the swordsman (its blockouts use the swordsman's own colours) |
| `scale` | the figure's height ÷ 86 (the swordsman's), measured from the top of the head or hair (not horns, hat or weapon) to the soles; e.g. 98 px → 1.14. If a hat or helmet hides the top of the head, estimate it: the top of the head is about as far above the eyes as the chin is below them. It scales how far the body travels (the swordsman's moves), not the pose angles. |
| `head_r` | head radius in px (the swordsman's is 13); it sizes the guide's head, face and hair cap |
| `limb` | limb thickness relative to the swordsman: 1.0 for human builds, about 1.4 for hulking ones |
| `joints` | frame-0 positions, below; every joint is required except `shield` |
| `beats` | rewordings of beat texts, step 1d |

Optional keys: `palette` (overrides guide colours), `defringe: false` (turns the magenta de-fringe off), `sharpen`
(warp sharpening, default 3.0). `legacy`, `same_as_source` and `poses_from` are for the sprint's own sets; leave them out.

Joints, in canvas coordinates `[x, y]`, placed **on the sprite as drawn**. `r_` is the weapon arm and the leg on the
same side of the body, wherever they are on screen; `l_` is the other arm and leg. For a weapon held in both hands
(a hoe, a spear, a polearm), the weapon hand `r_ha` is the one at the end of the shaft, farthest from the weapon's
head, so that `r_ha` to `tip` runs along the whole weapon; the other hand is `l_ha`. The poses swing one-handed, so say
in `hands` that the other hand holds the shaft lower down at rest and lets go during the action, and describe it as an
empty fist in every override that mentions it (strike 2–5, flinch 0–1, death 1–3), or the model gets a text and a guide
that disagree. No two-handed unit has been generated yet, so check its strike closely in step 3:

| Joint | Where |
|---|---|
| `head` | centre of the head, not counting horns or hat |
| `neck`, `chest`, `pelvis` | base of the neck, middle of the chest, middle of the belt line |
| `r_sh`, `r_el`, `r_ha` | weapon arm: shoulder, elbow, hand (the fist on the grip) |
| `tip` | the weapon's far end (blade tip or club head), wherever it rests, even over the shoulder; for a head set across the shaft (hoe, axe, hammer, pick), where the shaft meets the head |
| `l_sh`, `l_el`, `l_ha` | the other shoulder, elbow, hand |
| `shield` | `[cx, cy, rx, ry]`, **only if the unit carries a shield**; leave it out otherwise |
| `r_hip`, `r_kn`, `r_ft`, `l_hip`, `l_kn`, `l_ft` | hips, knees, ankles |

How the poses are made: the facing and the weapon's screen side pick a source pose set, which is retargeted onto your
joints. Each bone keeps your unit's frame-0 angle and length and turns as the source bone turns; in the frames the model
draws, the weapon arm and weapon swing to the source's actual direction once the source bone has turned well away
from rest, so a weapon carried differently (a club on the shoulder) still swings where the beat sheet says. The warped
frames copy the source's small joint movements (the breath). `--show-beats` prints which set your unit gets.

| Sprite looks | Weapon arm on screen (`r_sh` left or right of `l_sh`) | Source poses |
|---|---|---|
| lower-left | left (the far hand) | far set, `poses.json` (the ogre, Hyborem) |
| lower-right | left (the near hand) | near set, `poses_near.json` (the swordsman) |
| lower-left | right (the near hand) | near set, mirrored |
| lower-right | right (the far hand) | far set, mirrored |

Approximate joints are fine; the model redraws the whole figure. What matters is that `r_ha` to `tip` follows the
weapon and that the feet sit on the feet.

**1d. Check the beat texts (free).**
```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit> --show-beats
```
This prints the facing, the pose set, the smear direction (`cw` or `ccw`: which way the blade turns on screen; it is
set by the pose set and needs no action) and the text each sheet cell will get (strike frames 1–6, flinch 0–2, death
1–7), marking texts that name a shield, sword or blade and texts you have overridden (`own`). The defaults already point
the right way for your unit's facing. Override every text that names a prop your unit does not have (a club instead of
a sword, an empty fist or a cape instead of a shield) and any other feature it lacks (hair under a hat, a cape), as a
string keyed by frame number. A unit without a sword and shield needs most texts overridden (the worker: 15 of 16):
```json
"beats": {"strike": {"2": "...", "4": "..."}, "flinch": {"0": "..."}, "death": {"5": "..."}}
```
Write overrides for the unit's real facing and keep every direction the default text gives (toward the facing for the
strike, away from it for the flinch and the fall). `units/ogre.json` and `units/hyborem.json` have worked rewordings.

**1e. Check the guides (free).**
```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit> --check-guides
```
Open `work/final_<unit>/guides_check.png` (every cell is labelled). Row 1 starts with your frame-0 skeleton over the
sprite: bones in red, the weapon arm and weapon in yellow, joints as white dots; each should sit on the sprite's body
part. The rest of row 1 is the strike poses the model will be asked to draw. Row 2 starts with the frame-0 blockout over
the sprite in a red tint, then the flinch; row 3 is the death. Check:
- the strike reaches toward the unit's facing, the flinch recoils away from it and the death falls away from it;
- frame 0 lines up and no pose looks broken; fix the joints until it does;
- each guide cell against its beat text from 1d: if your weapon rests at a different angle from the swordsman's (on the
  shoulder, or pointing down like a hoe), an early frame such as strike 1 may hold it differently from the text; then
  reword the text to describe what the guide shows, never the direction of the blow;
- every guide draws the weapon as a light sword blade with a crossguard, held or dropped (death 4–7, where it lies
  where the swordsman drops his); that is expected, and every text that shows the weapon must name it (club, hoe,
  greatsword) so the model draws it.

If it warns that the guide weapon was shortened, the weapon is too long for the area the model is shown (x 16–143,
y 0–127 of the canvas) in those frames. Carry on: the model draws it at full length, so in step 3 reject variants where
it is cut off. Do not move `tip` to silence the warning. A clamp at the bottom means the full weapon would reach below
the feet; the beat text ("nearly touches the ground") is what keeps the model from drawing it there.

## Step 2: run the recipe

```bash
python3 lib/recipe.py --unit units/<unit>.json --out final_<unit>
```
This sends three sheet calls in parallel (strike, flinch, death), with two variants each. It takes about 20–25 s of
model calls (longer if the API rate-limits) plus a few seconds to cut, warp and assemble, for about $0.23. It writes
`final_<unit>/` and `final_<unit>_nofx/`. Raws, templates and diagnostics go to `work/final_<unit>/`, which is not
committed.

## Step 3: pick the variants by eye

The recipe picks a variant per sheet by a score (reference-cell fidelity, scale, ground snaps, and gate flags that only
make sense for the swordsman's colours). **That score cannot see colour smudges, off-model faces or a strike in the
wrong direction, so always look.** For each sheet open `work/final_<unit>/variants_<sheet>.png`: one row per variant at
2×, cell 1 the real sprite, frames shown as they will be used but cropped to x 16–143 (a weapon that reaches past
x 143 looks cut there; the frames themselves run to x 159). A cell label says when the frame was slid sideways onto its
guide (`slid -24`: the model drew the figure 24 px off) or moved up or down onto the ground by 4 px or more
(`ground +19`). Choose the variant with fewer of these defects:

- **wrong direction:** the strike lands on the side away from where the face looks, the face turns the other way, or
  the hit comes from the wrong side;
- **colour smudges:** a patch of the wrong colour on clothing (tan on blue shorts), or an extra strap;
- **size pop:** a figure noticeably bigger than the sprite (often frame 1 or the hit frames), or swollen hair;
- **wrong pose:** the weapon in the wrong hand, a missing or doubled prop, or a pose that ignores the guide;
- **off-model face:** a different face, squeezed manga eyes, or a front-facing head;
- **floating:** a frame labelled `FLOATS n` hangs n px above the ground (the recipe snaps up to 40 px; a held corpse
  that floats stays floating forever);
- **sideways slide:** the feet jump between frames even after the slide (compare with frame 0);
- **cropping:** a weapon or limb cut off at the canvas edge;
- **magenta or purple specks** on outlines or seams (the recipe recolours most of them, except for units with purple
  art such as the lich or the Sheaim mage, where de-fringing is off: pick the cleanest variant, re-roll if both have
  specks);
- **for a lower-right unit, a flip in strike frames 3–5:** the torso turns into the lunge and the shield swings behind
  the back, which some judges read as the figure turning round (see Known limits); prefer the variant where the face
  and chest still clearly point lower-right;
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
- `vignette.{gif,mp4}`: two copies side by side on one screen row; the attacker (blue banner) strikes toward the
  mirrored defender (red), who stands on the side the attacker faces;
- `preview/`: per animation `<anim>_strip3x.png` (frames at 3× with timings and the hit marked), `<anim>_map1x.png`
  (game scale), an onion skin with the weapon-tip trail (`_onion.png`), a silhouette strip (`_sil.png`), a change heat
  map (`_diff.png`) and GIFs (`_1x.gif`, `_3x.gif`, not committed).

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

`python3 lib/metrics.py --arm final_<unit> --root final_<unit>` prints a summary per animation and writes every number
to `final_<unit>/metrics.json` (`jitter`, for idle and flinch, is only in that file). Only `nonrigid`, `jitter` and
`boil` are palette-independent; the gates, `drift_dE`, `overlap` and `arc` look for the swordsman's colours, so ignore
them for other units (the gates are heuristics even for the swordsman).

**Commit** `units/<unit>.json`, `final_<unit>/` (frames, manifests, sheets, `unit_poses.json`, `metrics.json`, the
sequence and vignette files and the preview stills) and `final_<unit>_nofx/` (frames, manifests, sheets,
`unit_poses.json`). `.gitignore` already leaves out `work/` and the per-animation preview GIFs.

## Walk cycle (experimental)

`lib/walk.py` applies the same method to a 6-frame walk, toward the unit's facing (rule 1):
```bash
python3 lib/walk.py --unit units/<unit>.json --out final_<unit> --check-guides     # walk guides, free
python3 lib/walk.py --unit units/<unit>.json --out final_<unit>                    # 1 call, about $0.08
python3 lib/walk.py --unit units/<unit>.json --out final_<unit> --rebuild --pick 2 # re-cut with a variant, free
```
The poses come from a planted-foot walk rig on the unit's frame-0 skeleton: each foot travels along the facing's ground
diagonal, plants for half the cycle and swings with a lift for the other half; each leg is solved in a small 3D ground
frame, so a leg pointing into the scene stays straight instead of bending. The model draws the six frames on one
guided sheet, which is sliced, de-fringed and slid onto the guides like the strike. It writes `final_<unit>/walk/`
(110 ms frames, looping; the manifest adds `ground_speed_px_per_s` and `ground_direction`, so the engine moves the unit at
the speed the stride implies, 54.5 px/s on screen), `sheet_walk.png`, `preview/walk_strip3x.png` and
`walk_clip.{gif,mp4}`: the unit walking two tiles along its facing, turning (mirrored) and walking two tiles down the
other diagonal, then idling. Pick the variant by eye in `work/final_<unit>/variants_walk.png`, and compare figure
heights with the sprite: one variant's height bounced 9 px between frames.

Tried once, on the swordsman (`final_swordsman/walk/`, variant 2), and not judged. Its frames are 2–6 px taller than the
idle, a small pop when he starts or stops, and two pairs of frames (2–3, 5–0) are near-twins, a slight hold at each
footfall.

## Known limits

- **Canvas size.** The canvas is 160×128 with a safe region of x 16–143. A unit taller than about 110 px, or one with a
  long weapon, gets clipped: the ogre's club and Hyborem's greatsword are cut off at the top or the side in some frames.
  The recipe refuses sprites taller or wider than 128. A bigger canvas means changing `W`, `H`, `ANCHOR`, `SAFE_X` and
  `SPRITE_OFFSET` in `lib/animkit.py`; `canvas`, `anchor`, `sprite_offset` and every joint in `lib/poses_src.py` and
  `lib/poses_near_src.py`, then regenerating `poses.json` and `poses_near.json` (`lib/guides.py` reads the canvas size
  from `poses.json`); `REGION`, `GROUND_Y` and `SCALE` in `lib/sheet.py` (128 × 3 already fills a 384 px cell, so the
  cell size from `default_geom` and the image size in `lib/gen.py` `CONTRACT` change too);
  the guide-weapon clamp (x 18–142, y 3–125) in `recipe.retarget`; the cell sizes in `recipe.review_sheet`; and the
  scene sizes in `lib/preview.py`. That is untested. The sideways slide (step 3) can push a long weapon further past
  the edge, as with Hyborem's greatsword and the ogre's club on the hit frames.
- **Purple or magenta art** (Sheaim, liches, infernal fire). The model's background is magenta and is keyed out. The
  de-fringe step turns itself off when 1% or more of the sprite's opaque pixels are purple (the lich and the Sheaim
  mage), but keying can still eat purple art. The magenta key is assumed by the prompt, the template background
  (`sheet.make_template`) and `sheet.slice_sheet`; another key colour is not supported yet.
- **Body plans.** The poses are a biped swinging a one-handed melee weapon. Archers and casters (the elf archer, the
  lich, the Sheaim mage, the priest) have no poses yet. A shot or a cast needs a new pose file in the `poses.json`
  schema, written for the set that `--show-beats` names for the unit (the recipe mirrors it as needed), pointed to by
  the unit file's `poses_from`, with all 16 beat texts overridden and the shot or cast aimed toward the facing (rule 1).
  Do not edit `lib/poses_src.py`, `lib/poses_near_src.py` or `lib/beats.py` for this: every melee unit uses them. This
  is untested; if you were not asked to build it, say so rather than animating a caster with the sword swing. Mounted
  units, animals and ships are untested.
- **Facing.** Lower-left and lower-right only (see the engine contract).
- **The near set** (`poses_near.json`: lower-right with the weapon in the near hand, or mirrored) was written after the
  sprint for the strike-direction rule and has not been through a bake-off like the far set; it has been judged only as
  guides and in the post-rule checks. Its weakest point is the turn into the strike: the far arm swings the shield
  behind the back on the 50 ms swing frame, and judges in both checks read that as the shield vanishing and reappearing
  behind him or swapping arms, and one judge per round as the figure turning round and slashing behind himself (while
  the direction judges all read the strike as lower-right). Keeping the shield on the far side of the body through
  frames 3–5 is the obvious change (in `lib/poses_near_src.py`); it is untested.
- **Remaining model defects.** Even the best variants carry small smudges, a corpse a little flatter than the isometric
  axis, and occasionally a death bounce that is too big. The pick in step 3 is the control.
- **Model choice.** `gpt-image-2.5-sunburst` at medium quality, 1536×1024, two variants per call (`lib/gen.py`
  `CONTRACT`). In calibration gpt-image-2.5-flare redrew the reference cell and gpt-image-2 rescaled everything by 19%.
  High quality was only marginally crisper, at twice the price.

## Changing the method

If you improve the recipe, re-test it before relying on it. Run the old and the new version on the same unit, then have
judges compare them blind: letters instead of names, a frame-ordering test, and scores for motion, consistency, style
and appeal (`lib/judging.py`; `JUDGE_BRIEF.md` is the sprint's brief, which says the swordsman faces lower-left, so
rewrite its first paragraph with the unit's real facing first; `judging/post_rule/make_packets.py` builds the blind
packets used after the sprint). Agents judging stills are a useful filter. A person watching the motion is the real
test.

## Why this method (evidence)

The October 2026 sprint ran seven methods on the swordsman under one beat sheet (`arms/*/`). Three judge agents scored
every animation blind; this recipe's first build was entered as an unnamed letter.

| Round 1, weighted score /10 | idle | strike | flinch | death |
|---|---|---|---|---|
| this recipe (first build) | 7.80 | 6.65 | 7.37 | 7.28 |
| sheet, no pose guides | 3.80 | 7.60 | 6.18 | 7.03 |
| model repaints rig frames | 5.78 | 7.35 | 7.20 | 7.18 |
| sheet with pose blockouts | 4.13 | 7.17 | 6.02 | 7.00 |
| keys, then in-betweens | – | 7.20 | – | 7.62 |
| chained per-frame calls | – | 6.63 | – | – |
| cut-out rig + mesh warp | 8.07 | 4.95 | 5.20 | 3.97 |
| whole-sprite tween (the stiff look) | 6.28 | 4.58 | 5.03 | 3.62 |

In round 1 the first build led only the flinch. After three fixes that cost no new image calls (picking the better
strike variant, sharpened warps, de-fringing), fresh blind judges compared it with its first build and the two best
other methods per animation, and gave it the top mean score in all four: 7.90 / 7.68 / 7.62 / 7.85. Only the death was
ranked first by every judge, and the idle and strike leads (0.2 and 0.3) are too small to call.

- Report, with every animation playable: https://claude.ai/artifact/3eWLaH1v9PHpu6G6FGKExb
- Data: `judging/` (scores, judges' JSON and keys), `spend.jsonl`, and the method arms' `notes.md` (the A0 control has
  only its script, `arms/A0/make_a0.py`).

### After the sprint

- **Rule 1** (the strike goes where the sprite looks) was added after the sprint. Sprites that look lower-right with
  the weapon in the near hand needed a second pose set (`poses_near.json`): three agents each designed one, three blind
  judges scored the rendered guides, and the winner (mean 7.83 of 10 in strike, flinch and death; the others 7.33 and
  5.33 in the strike) was refined from the judges' notes (`judging/near_set/panel.json`).
- **A cold test** of this guide on the ogre, by an agent that read nothing else, found four gaps that are now fixed in
  the recipe: a weapon carried differently from the swordsman's (a club on the shoulder) swung the wrong way, other units
  did not breathe in the idle, the model's sideways placement made the ogre hop backwards while striking, and purple
  specks survived inside the figure.
- **Blind verification, round A** (`judging/post_rule/round_a.json`): three judges read where each set's sprite looks and
  where it strikes, under neutral letters. They read Hyborem as looking lower-left, against this guide's first call of
  lower-right, so his first post-rule set struck the wrong way. The round also exposed a prompt bug (the per-cell facing
  sentence was sent unformatted) and gaps in this guide found by a second cold run (on the worker, up to step 1e). The
  bug and the guide gaps are fixed (a two-handed unit has still not been generated), and the three worked examples were
  made again.
- **Blind verification, round B** (`judging/post_rule/round_b.json`), on the regenerated sets: every new set strikes
  where its sprite looks (3 of 3 judges each), and the sprint's swordsman set fails again (3 of 3). Old set → new set,
  weighted scores, idle / strike / flinch / death (gaps under about 0.5 are within judge noise):

  | Unit (old set) | Old | New |
  |---|---|---|
  | ogre (the cold-test agent's set) | 5.25 / 6.83 / 6.10 / 6.38 | 7.23 / 5.92 / 7.27 / 6.93 |
  | swordsman (the sprint's judged set) | 7.17 / 6.03 / 6.52 / 6.52 | 7.17 / 6.28 / 7.30 / 7.13 |
  | Hyborem (`final_hyborem_v1/`) | 5.70 / 6.60 / 6.97 / 6.98 | 7.33 / 7.73 / 7.33 / 5.63 |

  The two clear losses, the ogre's strike and Hyborem's death, were frames floating 21–22 px above the ground: the
  slicer only snapped up to 20 px and left them, and the pick by eye missed them. It now snaps up to 40 px and the
  variant sheets label any float; both sets were re-cut from the same picks, so every redrawn frame now stands on the
  sprite's ground row (Hyborem's on his own feet row, 2 px above the others, because his PNG has 4 empty rows under the
  feet), but the re-cut sets were not judged again. The swordsman's idle is unchanged because both sets use the same
  warp of his sprite.

## Worked examples

- `units/swordsman.json` → `final_swordsman/`: looks lower-right, sword in the near hand, shield; near set, colour
  guides.
- `units/ogre.json` → `final_ogre/`: looks lower-left, a long club resting on the shoulder, no shield; far set
  retargeted, beat texts reworded (club for the sword).
- `units/hyborem.json` → `final_hyborem/`: looks lower-left, a long greatsword, no shield; far set retargeted, beat texts
  reworded (fist and cape for the shield).
- Sprint sets, kept only to reproduce the sprint (`"legacy": true`): `units/swordsman_judged.json` → `final/` breaks rule
  1 (its `facing` is the direction that set strikes, not where he looks; `final/`'s manifests predate the `facing`
  field); `units/hyborem_v1.json` → `final_hyborem_v1/` keeps the rule but has the sprint's processing (no idle breath,
  no sideways snap).
