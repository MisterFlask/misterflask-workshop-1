# ffh-anim-sprint

Research sprint (October 2026): how to make bespoke **idle, strike, flinch and death** animations for one M7
isometric unit sprite (the swordsman), comparing several methods under one shared spec.

Status: in progress. The full write-up (methods, results, recommended pipeline) lands here and in the HTML report
when the sprint finishes.

- `spec.md`, `beats.md`, `poses.json` (from `lib/poses_src.py`): the shared contract every method fills.
- `lib/`: harness. `animkit` (frame I/O), `sheet` (model templates + slicer), `guides` (pose blockouts),
  `rig` (MLS warp, cut-out helpers), `metrics` (stiffness/consistency metrics + gates), `preview`
  (strips, onion skins, GIF/MP4 sequences, combat vignette), `test_harness`.
- `m7/`: the parts of the M7 art-mode package this sprint uses (cutout, compose, imggen, the roster sprites).
- `arms/<ARM>/`: one folder per method, with frames, manifests, sheets, previews, `sequence_*.{gif,mp4}`,
  `vignette.{gif,mp4}`, notes and timings.

Image-API calls go through `m7/lib/imggen.py` (spend ledger `spend.jsonl`, caps in `budget.json`, $20 total).
