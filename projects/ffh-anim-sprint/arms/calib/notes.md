# P1 calibration: one strike sheet, several models and settings

Template: `review/template_strike_colour.png` (and `_grey`): a 4×2 sheet, cell 1 = the true sprite at 3×, cells
2–8 = the strike blockouts from `poses.json`. Prompt: `gen.sheet_prompt("strike", …, guide="colour")` (logged in
`spend.jsonl`). Second image: `m7/anchors/anchor_roster_r1.png`. Six calls, 26 s wall-clock in parallel, **$0.355
total** (ledger, pessimistic prices).

| Run | Model | Settings | $ | Latency | Cell-1 redraw (k / IoU / ΔE) | Verdict |
|---|---|---|---|---|---|---|
| sunburst_1/_2 | gpt-image-2.5-sunburst | medium, n=2, colour guides | 0.078 | 19.6 s | 1.004 / 0.972 / 7.5 | Followed every guide; same character in all cells; cell 1 nearly untouched |
| sunburst_grey | sunburst | medium, grey mannequins | 0.039 | 17.6 s | 1.004 / 0.972 / 8.0 | As good as colour guides; slightly less hair growth |
| sunburst_fid | sunburst | `input_fidelity=high` | 0.039 | 16.3 s | 1.012 / 0.966 / 8.7 | **Parameter not supported** by sunburst (imggen dropped it); just another sample |
| sunburst_high | sunburst | high quality | 0.080 | 30.0 s | 1.004 / 0.978 / 5.8 | Marginally crisper, cell 1 kept a little better; 2× the price |
| flare | gpt-image-2.5-flare | medium | 0.039 | 13.0 s | 1.054 / 0.891 / 13.9 | Redrew cell 1 and drew the bottom row ~14 px off the ground line |
| gi2 | gpt-image-2 | medium | 0.080 | 25.9 s | 1.190 / 0.937 / 10.6 | Rescaled everything +19% and redrew cell 1 in its own taller style |

(The floor for "cell 1 copied perfectly" is ΔE 3.7 / IoU 0.974: nearest-upscaling the soft sprite, keying and
downscaling it again already costs that much; see `lib/test_harness.py`.)

Findings that shape the contract:

- **A guided sheet works on the first call.** All four sunburst sheets are usable strikes: the model follows the
  blockout poses closely and keeps face, outfit, sword and shield consistent across the seven new cells. Standing
  frames come back 86–87 px tall, matching the sprite (86 px). Hair comes back about a third bigger in pixel count
  than in the sprite: the redraw is a little "fluffier" than the original.
- Sunburst preserves the reference cell best, so frame 0 → frame 1 has the smallest pop.
- Rows can drift vertically (flare's bottom row); the slicer now snaps every non-reference frame's lowest opaque
  row onto the ground row (≤ 20 px), logged as `ground_snap` in the slice diagnostics.
- n=2 is supported; input tokens are billed per output image, so n=2 costs exactly twice n=1.
- Real cost is about **$0.039 per sheet image** at medium, so the $20 cap is far from binding; the binding
  constraint is the pre-call estimate (~$0.25 for an n=2 sheet call), which the per-arm caps check against.

**Frozen generation contract** (`lib/gen.py` `CONTRACT`): gpt-image-2.5-sunburst, medium, 1536×1024, n=2, no
`input_fidelity`, colour blockout guides (grey is an equal alternative), roster sheet as the second image.
