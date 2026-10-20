# ffh-anim-sprint

Research sprint (20 October 2026): how to make bespoke **idle, strike, flinch and death** animations for an M7
isometric unit sprite, tested on the swordsman and checked on Hyborem. Seven methods ran on one shared beat sheet,
three judge agents scored them blind, and a hybrid recipe (built mid-trial, judged blind with the others, then fixed)
had the top mean score in all four animations in a second blind round.

## Result in one paragraph

Draw the big actions with the image model, and move the sprite's own pixels for the small ones. gpt-image-2.5-sunburst
draws a convincing strike, flinch and death on **one sprite sheet per animation** whose first cell is the real sprite
and whose other cells hold flat-colour pose blockouts. It cannot draw an idle (every redraw shimmers), so idle and the
frames next to idle are the real sprite bent by a sharpened mesh warp. A full four-animation set takes 3 image calls,
about 21 s of model calls plus 6 s of assembly, and $0.23 (`lib/recipe.py`), then a by-eye variant pick. Moving the whole sprite or its cut-out parts, which is what looked stiff
before, came last for strike, flinch and death in every judge's ranking.

| Round 1 (weighted judge score /10) | idle | strike | flinch | death |
|---|---|---|---|---|
| Recipe, first build | 7.80 | 6.65 | 7.37 | 7.28 |
| G2 rig frames repainted | 5.78 | 7.35 | 7.20 | 7.18 |
| G0 sheet, no guides | 3.80 | 7.60 | 6.18 | 7.03 |
| G1 sheet, pose blockouts | 4.13 | 7.17 | 6.02 | 7.00 |
| K keys then in-betweens | – | 7.20 | – | 7.62 |
| C chained per-frame | – | 6.63 | – | – |
| R cut-out rig + mesh warp | 8.07 | 4.95 | 5.20 | 3.97 |
| A0 whole-sprite tween (control) | 6.28 | 4.58 | 5.03 | 3.62 |

Verification round (fresh judges, fixed recipe vs its first build and the two best methods per animation): fixed recipe
7.90 / 7.68 / 7.62 / 7.85, the top mean in all four (first for every judge only in the death; the idle and strike margins,
0.2 and 0.3, are too small to call). The same frames scored up to 1.2 points apart between the two panels, so compare
within a round. Total image-API spend for the sprint: $4.45 of a $20 cap.

The full write-up, with every animation playable, is the HTML report built from `report/` and published at
https://claude.ai/artifact/3eWLaH1v9PHpu6G6FGKExb (private until shared from its Share menu).

## Make a set for a unit

```bash
cd projects/ffh-anim-sprint
pip install pillow numpy scipy requests
python3 lib/recipe.py --unit units/swordsman.json --out final                 # 3 calls; writes final/ and final_nofx/
python3 lib/recipe.py --unit units/swordsman.json --out final --rebuild --pick strike=2   # re-cut a variant, free
python3 lib/preview.py --arm final --root final                              # strips, sequence_*.gif/mp4, vignette
```

**To animate another unit, follow [`ANIMATING_UNITS.md`](ANIMATING_UNITS.md)**: the step-by-step guide for agents (unit
file and joints, the free `--grid` and `--check-guides` helpers, picking variants by eye, QA checklist, known limits).
A unit file (`units/*.json`) holds the sprite path, a one-line description of what the model must keep, the hands
sentence and the frame-0 joints in canvas pixels. Other units use grey guides and poses retargeted from the
swordsman's (`units/hyborem.json`). The OpenAI key is injected by the session proxy; elsewhere set `OPENAI_API_KEY`.

Output per animation: `<out>/<anim>/frame_NN.png` (160×128, anchor (80,126) = tile centre, as an M7 unit),
`manifest.json` (per-frame ms, `hit` event, loop/hold), `sheet_<anim>.png`. The engine should place the banner from
the idle frame-0 bounding box. Big units need a taller canvas than 128 px (Hyborem's raised greatsword clips).

## Layout

- `spec.md`, `beats.md`, `poses.json` (from `lib/poses_src.py`): the shared contract.
- `lib/`: `animkit` (frame I/O), `sheet` (templates + slicer), `guides` (pose blockouts), `gen` (image calls, prompt
  skeleton), `rig` (MLS warp), `finish` (de-fringe, smear, spark, flash, palette snap), `metrics`, `preview`
  (strips, GIF/MP4 sequences, combat vignette), `judging` (blind packets, aggregation), `recipe`.
- `arms/<ARM>/`: one folder per method with frames, manifests, sheets, notes, timings, `sequence_{3x,1x}.{gif,mp4}`
  and `vignette.{gif,mp4}`. `arms/calib/` is the model calibration.
- `final/` (recipe with FX), `final_nofx/`, `final_v1_nofx/` (first build), `final_hyborem/`.
- `judging/`: judge JSON, keys, `scores.json`, `verify_scores.json`. Packets are regenerable and not committed.
- `report/`: `build.py`, `content.py`, `player.js`, `style.css` → `report/site/` (not committed).
- `m7/`: the parts of the M7 art-mode package this sprint uses. `spend.jsonl` / `budget.json`: the image-API ledger.
- `AGENTS_BRIEF.md`, `JUDGE_BRIEF.md`: the briefs the method and judge agents worked from.
