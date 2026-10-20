# Brief for arm agents

You are running one **arm** (method) of a research sprint: making idle / strike / flinch / death animations for
one M7 isometric unit sprite, the swordsman (`m7/roster/units/unit_swordsman.png`). Several arms run in parallel,
each trying a different method on the same beats. A blind judge panel compares them later, so what matters is the
best result **your method** can produce, honestly reported. Do not switch to another arm's method if yours
struggles; write up why it struggles instead.

Work from the project root `projects/ffh-anim-sprint/` and run Python from there (`python3 ...`).

## Read first (in this order)

1. `spec.md`: canvas 160×128, anchor (80,126), frame counts, facing, files, gates.
2. `beats.md`: what each frame shows and its ms. `lib/beats.py` holds the same data.
3. `poses.json` (made by `lib/poses_src.py`): 2D joint positions for every frame of every animation.
4. `arms/calib/notes.md`: what the image-model calibration found, and the frozen generation contract.
5. The docstrings at the top of `lib/animkit.py`, `lib/sheet.py`, `lib/guides.py`, `lib/gen.py`, `lib/rig.py`,
   `lib/metrics.py`, `lib/preview.py`. `arms/A0/make_a0.py` is a complete small example arm. `arms/calib/run_calib.py`
   shows templates → model calls → slicing.

## Tools you share (do not edit them)

- `animkit`: `base_canvas()`, `write_anim(ARM, anim, frames)` (the only way to write results), `load_anim`,
  premultiplied `transform`/`resize`/`scale`, `snap_ground`, `mirror`, `log_time`, `grid_view(img, out)` (6×
  enlargement with coordinates for annotating).
- `sheet`: `make_template(cells, out=...)` → geometry; `slice_sheet(raw, geom, n=...)` → canvas frames + diagnostics
  (one global scale from the reference cell, ground snap, warnings).
- `guides`: `render_guide(pose, "colour"|"grey")` → 160×128 blockout of a poses.json pose.
- `gen`: `edit(tag=..., images=[...], prompt=..., out=..., n=2)` and `parallel([...])`; `sheet_prompt(...)` (shared
  prompt skeleton); `ROSTER` (the roster anchor image); `CONTRACT` (model, quality, size, n).
- `rig`: rigid MLS warp (`mls_warp`, `warp_to_pose`), `part_layer`, `polygon_mask`, `place_part`, `angle_of`.
- `metrics`: `python3 lib/metrics.py --arm ARM --anim strike` (non-rigidity, overlap, colour drift, gates).
- `preview`: `python3 lib/preview.py --arm ARM --only stills` → `arms/ARM/preview/<anim>_strip3x.png`,
  `_map1x.png`, `_onion.png`, `_sil.png`, `_diff.png`.

If a shared tool has a bug that blocks you, copy the function into your arm folder, fix it there, and say so in
your notes. Never edit `lib/`, `m7/`, `spec.md`, `beats.md`, `poses.json`, or another arm's folder.

## Image-model rules

- Only through `gen.edit` / `gen.parallel` (it goes through `m7/lib/imggen.py`, which enforces the caps in
  `budget.json` and logs every call to `spend.jsonl`). Tag every call `anim/<ARM>`. Do not use any other image tool.
- Contract: gpt-image-2.5-sunburst, medium, 1536×1024, n=2 per call (`gen.CONTRACT`); the roster sheet
  (`gen.ROSTER`) as the last image. Deviate only with a reason in your notes.
- Real cost is about $0.04 per output image; your cap is checked against a pessimistic pre-call estimate
  (~$0.25 per n=2 sheet call), so you have room for several rounds but not dozens.
- Up to 3 rounds per animation. Pick by eye at 1× (map strip) and 3×: consistency of the character first, then
  pose fidelity to the beat, then polish. If you mix frames from different variants, say so in your notes.
- Keep raw outputs and templates in `work/<ARM>/` (not committed) and the prompts you used in
  `arms/<ARM>/prompts/`.

## Output rules

- Round 1 frames carry **no FX**: no smears, motion lines, flashes, sparks or fades. FX are added uniformly to every
  arm later. Motion must come from the poses themselves.
- Frame 0 of idle and strike is the static sprite (`write_anim` enforces it). Stay inside x 16–143.
- Put your code in `arms/<ARM>/make_<arm>.py` (or a few scripts) so the result can be reproduced from the saved raws.
- After writing an animation, run the metrics and the stills preview, and **look** at `<anim>_strip3x.png` and
  `<anim>_map1x.png` (view big sheets at half size to save context). Check the gates.
- Time yourself: `t0 = time.time()` when you start an animation, then `ak.log_time(ARM, "<anim>", t0, calls=N,
  usd=X)` when it is written and checked. Timebox: 60 minutes per animation batch you are given.
- Write `arms/<ARM>/notes.md` with these sections: **Method** (2–4 sentences), **What I did** (rounds, calls,
  which variant you picked and why), **What worked**, **What failed** (be specific: frame numbers, defects),
  **Spend and time**, **If I had another hour**. Plain, factual prose.
