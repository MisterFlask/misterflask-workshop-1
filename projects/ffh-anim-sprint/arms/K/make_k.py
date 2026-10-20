"""K - keys first, then in-betweens (the classic key -> in-between pipeline), for the STRIKE animation.

(The death animation of arm K is made by arms/K/make_k_rest.py, another agent, in work/K/rest/.)

Pass 1 (keys): 4x2 sheet laid out like the final 8-frame strip: cell 1 = the true sprite (ak.base_canvas()),
cells 3, 5, 6 = colour guides for strike frames 2 (wind-up extreme), 4 (contact) and 5 (follow-through); cells 2, 4,
7, 8 empty magenta. One n=2 call per round, at most 2 rounds. Pick one variant by eye and slice the three keys.
Pass 2 (in-betweens): same layout, cell 1 = the true sprite, cells 3, 5, 6 = the accepted keys pasted as finished
art, cells 2, 4, 7, 8 = colour guides for frames 1, 3, 6, 7. One n=2 call per round, at most 2 rounds.
Final: frame 0 = the sprite (write_anim enforces it), frames 2, 4, 5 = pass-1 keys, frames 1, 3, 6, 7 = pass-2
in-betweens.

  python3 arms/K/make_k.py p1 template
  python3 arms/K/make_k.py p1 gen r1           # -> work/K/strike/raw/strike_p1_r1_{1,2}.png
  python3 arms/K/make_k.py p1 slice r1         # cut every variant + review strips
  python3 arms/K/make_k.py p2 template         # uses KEYS (the picked pass-1 variant) below
  python3 arms/K/make_k.py p2 gen r1
  python3 arms/K/make_k.py p2 slice r1
  python3 arms/K/make_k.py write               # assemble the final frames (PICK below) with ak.write_anim
  python3 arms/K/make_k.py mp4                 # arms/K/preview/strike_{3x,1x}.mp4 (strike alone, manifest timings)
"""
import json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import beats, gen, guides, sheet  # noqa: E402
from PIL import Image  # noqa: E402

ARM = "K"
TAG = "anim/K"
ANIM = "strike"
W = ROOT / "work/K/strike"
A = ROOT / "arms/K"
POSES = ak.POSES["anims"][ANIM]

KEY_FRAMES = [2, 4, 5]          # strike frames drawn as keys (they sit in cells 3, 5, 6 = frame + 1)
INB_FRAMES = [1, 3, 6, 7]       # in-betweens (cells 2, 4, 7, 8)
KEY_NAMES = {2: "wind-up extreme", 4: "contact", 5: "follow-through"}

# Choices made by eye (see notes.md)
KEYS = "p1_r1_1"   # pass-1 variant whose keys are accepted (round 1 variant 1, see notes.md)
# in-betweens by strike frame -> pass-2 variant. Mixed from the two round-2 variants (same call, same pasted keys):
# r2_2 for frames 1 and 3 (its swing blade stays inside the safe region; r2_1's pokes 5 px out), r2_1 for frames 6
# and 7 (its settle frame matches the sprite's height, IoU 0.90 vs 0.81, so the smallest pop when idle resumes).
PICK = {1: "p2_r2_2", 3: "p2_r2_2", 6: "p2_r2_1", 7: "p2_r2_1"}


def guide(i):
    return guides.render_guide(POSES[i], "colour")


def text(i):
    return beats.BEATS[ANIM][i]["text"]


def cell_of(frame):
    return frame + 1   # 1-based cell number in the 8-cell layout


# ---------------------------------------------------------------- pass 1: keys
P1_EXTRA = {
    "r1": ("Only cells 3, 5 and 6 get a figure: they are the three KEY poses of the strike (wind-up, contact, "
           "follow-through), each drawn in the cell it will occupy in the finished eight-frame strip. Cells 2, 4, 7 "
           "and 8 are deliberately empty (their in-between frames are drawn later) and must stay plain flat magenta "
           "#FF00FF with nothing drawn in them. Draw no shadow or darker patch on the ground under any figure, and "
           "draw every figure at exactly the same size as cell 1, with the whole sword inside its cell."),
}


def p1_texts():
    return [(cell_of(i), f"key pose ({KEY_NAMES[i]}, frame {i + 1} of 8): {text(i)}") for i in KEY_FRAMES]


def p1_template():
    cells = [None] * 8
    cells[0] = ak.base_canvas()
    for i in KEY_FRAMES:
        cells[cell_of(i) - 1] = guide(i)
    return sheet.make_template(cells, out=W / "template_strike_p1.png")


def p1_prompt(rnd):
    return gen.sheet_prompt(ANIM, p1_texts(), guide="colour", extra=P1_EXTRA[rnd])


# ---------------------------------------------------------------- pass 2: in-betweens
P2_EXTRA = {
    "r1": ("Cells 3, 5 and 6 are not blockouts: they already hold finished pixel art of the character (the key poses "
           "of the strike: wind-up, contact and follow-through). Cells 1, 3, 5 and 6 must stay exactly as they are, "
           "pixel for pixel. Only the blockout cells 2, 4, 7 and 8 get new drawings, the in-between frames: draw each "
           "one in the pose of its blockout so it connects smoothly with the finished frames before and after it "
           "(same character, same size, same face, hair, outfit, sword and shield, the pose part-way between its "
           "neighbours), so the eight cells play as one continuous sword strike. Cell 8 is almost exactly the "
           "cell-1 pose with the sword a touch lower. Draw no shadow or darker patch on the ground under any figure: "
           "the magenta touching each figure's outline stays pure #FF00FF."),
}
# r2: r1 variant 1 was usable as is (picked as the fallback), but its settle frame (cell 8) came back 2 px taller and
# 6 % bigger in area than cell 1 (a pop when idle resumes), and the level blade of cell 4 poked 1 px (variant 2: 8 px)
# past the left edge of the safe region. r2 = r1 plus one sentence on size and the blade.
P2_EXTRA["r2"] = P2_EXTRA["r1"] + (
    " Draw cells 7 and 8 at exactly the same size as cell 1 (same height, same head size, the hair no bigger): cell 8 "
    "should look almost identical to cell 1. Keep the level blade in cell 4 short enough that its tip stays well "
    "inside the cell, no farther left than the sword tip in cell 5.")


def p2_texts():
    out = []
    for i in range(1, 8):
        if i in KEY_FRAMES:
            out.append((cell_of(i), f"finished key frame ({KEY_NAMES[i]}), keep exactly as drawn: {text(i)}"))
        else:
            out.append((cell_of(i), f"in-between to draw: {text(i)}"))
    return out


def keys_frames():
    """The accepted pass-1 key frames, by strike frame number."""
    if not KEYS:
        raise SystemExit("set KEYS to the picked pass-1 variant first")
    d = W / "cut" / f"strike_{KEYS}"
    return {i: Image.open(d / f"frame_{cell_of(i) - 1:02d}.png").convert("RGBA") for i in KEY_FRAMES}


def p2_template():
    keys = keys_frames()
    cells = [ak.base_canvas()] + [keys[i] if i in KEY_FRAMES else guide(i) for i in range(1, 8)]
    return sheet.make_template(cells, out=W / "template_strike_p2.png")


def p2_prompt(rnd):
    return gen.sheet_prompt(ANIM, p2_texts(), guide="colour", extra=P2_EXTRA[rnd])


# ---------------------------------------------------------------- shared
def generate(pas, rnd):
    tpl = W / f"template_strike_{pas}.png"
    if not tpl.exists():
        (p1_template if pas == "p1" else p2_template)()
    p = (p1_prompt if pas == "p1" else p2_prompt)(rnd)
    (A / "prompts").mkdir(parents=True, exist_ok=True)
    (A / "prompts" / f"strike_{pas}_{rnd}.txt").write_text(p, encoding="utf-8")
    r = gen.edit(tag=TAG, images=[tpl, gen.ROSTER], prompt=p, out=W / f"raw/strike_{pas}_{rnd}.png", n=2)
    print(json.dumps({k: r.get(k) for k in ("ok", "files", "latency", "usd", "dropped_params", "error")}))
    return r


def review_strip(frames, out, labels=None, scale=2):
    from PIL import ImageDraw
    strip = Image.new("RGBA", (ak.W * len(frames), ak.H), (222, 226, 214, 255))
    for i, f in enumerate(frames):
        strip.alpha_composite(f, (i * ak.W, 0))
    big = strip.resize((strip.width * scale, strip.height * scale), Image.NEAREST)
    if labels:
        d = ImageDraw.Draw(big)
        for i, t in enumerate(labels):
            d.text((i * ak.W * scale + 4, 4), t, fill=(0, 0, 0, 255))
    big.save(out)


def slice_round(pas, rnd):
    geom = json.loads((W / f"template_strike_{pas}.geom.json").read_text())
    out = {}
    for raw in sorted((W / "raw").glob(f"strike_{pas}_{rnd}_*.png")):
        frames, diag = sheet.slice_sheet(raw, geom, n=8)
        name = raw.stem
        d = W / "cut" / name
        d.mkdir(parents=True, exist_ok=True)
        for i, f in enumerate(frames):
            f.save(d / f"frame_{i:02d}.png")
        if pas == "p1":
            labs = ["ref"] + [f"f{c - 1} KEY" if (c - 1) in KEY_FRAMES else "empty" for c in range(2, 9)]
        else:
            labs = ["ref"] + [f"f{i}" + (" key" if i in KEY_FRAMES else " inb") for i in range(1, 8)]
        review_strip(frames, W / f"strip_{name}.png", labs)
        row = {k: diag.get(k) for k in ("k", "dx", "dy", "warnings", "ground_snap", "ref_redraw_dE")}
        row["bboxes"] = [ak.bbox(f, 40) for f in frames]
        out[name] = row
        print(name, json.dumps(row))
    (W / f"slice_diag_strike_{pas}_{rnd}.json").write_text(json.dumps(out, indent=1))
    return out


def final_frames():
    keys = keys_frames()
    if not PICK:
        raise SystemExit("set PICK to the picked pass-2 variant first")
    p2 = {i: Image.open(W / "cut" / f"strike_{PICK[i]}" / f"frame_{i:02d}.png").convert("RGBA") for i in INB_FRAMES}
    frames = [ak.base_canvas()]
    for i in range(1, 8):
        frames.append(p2[i] if i in INB_FRAMES else keys[i])
    return frames


def write():
    frames = final_frames()
    inb = ", ".join(f"{i}: {PICK[i]}" for i in INB_FRAMES)
    man = ak.write_anim(ARM, ANIM, frames,
                        note=f"K strike: frames 2, 4, 5 = pass-1 keys {KEYS}; in-betweens from pass 2 ({inb})")
    print(json.dumps({"written": [KEYS, PICK], "warnings": man.get("warnings")}))


def mp4():
    """The strike on its own (manifest timings, one loop + a short hold on frame 0) as MP4 next to the preview GIFs."""
    import preview
    frames, man = ak.load_anim(ARM, ANIM)
    ms = [x["ms"] for x in man["frames"]]
    seq = [(f, m) for f, m in zip(frames, ms)] + [(frames[0], 400)]
    one = [(preview.tile_cell(f), m) for f, m in seq]
    pv = A / "preview"
    pv.mkdir(parents=True, exist_ok=True)
    preview.save_mp4(one, pv / "strike_1x.mp4", scale=2)
    preview.save_mp4([(c, m) for c, m in one], pv / "strike_3x.mp4", scale=3)
    print("wrote", pv / "strike_1x.mp4", pv / "strike_3x.mp4")


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[0] == "write":
        write()
    elif a[0] == "mp4":
        mp4()
    elif a[1] == "template":
        (p1_template if a[0] == "p1" else p2_template)()
    elif a[1] == "gen":
        generate(a[0], a[2])
    elif a[1] == "slice":
        slice_round(a[0], a[2])
