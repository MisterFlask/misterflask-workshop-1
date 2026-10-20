"""K (rest) - keys first, then in-betweens: the classic pipeline, for the DEATH animation.

The strike of arm K is made by arms/K/make_k.py (another agent); this script only covers death.

Pass 1 (keys): 4x2 sheet, cell 1 = the true sprite (ak.base_canvas()), cells 2, 3, 4 = colour guides for death
frames 0 (impact), 3 (kneeling) and 5 (hits the ground), cells 5-8 empty magenta. One n=2 call per round, at most
2 rounds. Pick one variant by eye and slice the three keys.
Pass 2 (in-betweens): 4x2 sheet, cell 1 = the true sprite, cells 2-8 = death frames 1-7. Cells 4 and 6 (frames 3
and 5) hold the accepted pass-1 keys as finished art; cells 2, 3, 5, 7, 8 (frames 1, 2, 4, 6, 7) hold colour
guides. One n=2 call per round, at most 2 rounds.
Final: frame 0 and keys 3, 5 from pass 1; frames 1, 2, 4, 6, 7 from pass 2.

  python3 arms/K/make_k_rest.py p1 template
  python3 arms/K/make_k_rest.py p1 gen r1          # -> work/K/rest/raw/death_p1_r1_{1,2}.png
  python3 arms/K/make_k_rest.py p1 slice r1        # cut every variant + review strips
  python3 arms/K/make_k_rest.py p2 template        # uses KEYS (the picked pass-1 variant) below
  python3 arms/K/make_k_rest.py p2 gen r1
  python3 arms/K/make_k_rest.py p2 slice r1
  python3 arms/K/make_k_rest.py write              # assemble the final frames (PICK below) with ak.write_anim
"""
import json, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import beats, gen, guides, sheet  # noqa: E402
from PIL import Image  # noqa: E402

ARM = "K"
TAG = "anim/K"
ANIM = "death"
W = ROOT / "work/K/rest"
A = ROOT / "arms/K"
POSES = ak.POSES["anims"][ANIM]

P1_FRAMES = [0, 3, 5]            # death frames on pass-1 cells 2, 3, 4
P2_FRAMES = [1, 2, 3, 4, 5, 6, 7]  # death frames on pass-2 cells 2..8
P2_KEYS = [3, 5]                 # frames pasted as finished art on pass 2 (cells 4 and 6)

# Choices made by eye (filled in as the work goes; see notes_rest.md)
KEYS = None   # pass-1 variant whose keys are accepted, e.g. "p1_r1_2"
PICK = None   # pass-2 variant whose in-betweens are used, e.g. "p2_r1_1"


def guide(i):
    return guides.render_guide(POSES[i], "colour")


def text(i):
    return beats.BEATS[ANIM][i]["text"]


# ---------------------------------------------------------------- pass 1: keys
P1_EXTRA = {
    "r1": ("Only cells 2, 3 and 4 get a figure: they are three key poses of the death animation, not consecutive "
           "frames. Cells 5, 6, 7 and 8 (the whole bottom row) are deliberately empty and must stay plain flat "
           "magenta with nothing drawn in them."),
}


def p1_texts():
    return [(2 + j, f"key frame {i + 1} of 8, {text(i)}") for j, i in enumerate(P1_FRAMES)]


def p1_template():
    cells = [ak.base_canvas()] + [guide(i) for i in P1_FRAMES] + [None] * 4
    return sheet.make_template(cells, out=W / "template_death_p1.png")


def p1_prompt(rnd):
    return gen.sheet_prompt(ANIM, p1_texts(), guide="colour", extra=P1_EXTRA[rnd])


# ---------------------------------------------------------------- pass 2: in-betweens
P2_EXTRA = {
    "r1": ("Cells 4 and 6 are not blockouts: they already hold finished pixel art of the character (the key poses, "
           "death frames 4 and 6). Cells 1, 4 and 6 must stay exactly as they are, pixel for pixel. The blockout "
           "cells 2, 3, 5, 7 and 8 get the in-between frames: draw each one so it connects smoothly with its "
           "neighbouring cells (same character, same size, same sword and shield, and each pose part-way between "
           "the cells before and after it), so the eight cells play as one continuous fall."),
}


def p2_texts():
    out = []
    for j, i in enumerate(P2_FRAMES):
        tag = "finished key frame, keep exactly as drawn" if i in P2_KEYS else "in-between"
        out.append((2 + j, f"frame {i + 1} of 8 ({tag}), {text(i)}"))
    return out


def keys_frames():
    """The accepted pass-1 frames, by death frame number."""
    if not KEYS:
        raise SystemExit("set KEYS to the picked pass-1 variant first")
    d = W / "cut" / f"death_{KEYS}"
    return {i: Image.open(d / f"frame_{j + 1:02d}.png").convert("RGBA") for j, i in enumerate(P1_FRAMES)}


def p2_template():
    keys = keys_frames()
    cells = [ak.base_canvas()] + [keys[i] if i in P2_KEYS else guide(i) for i in P2_FRAMES]
    return sheet.make_template(cells, out=W / "template_death_p2.png")


def p2_prompt(rnd):
    return gen.sheet_prompt(ANIM, p2_texts(), guide="colour", extra=P2_EXTRA[rnd])


# ---------------------------------------------------------------- shared
def generate(pas, rnd):
    tpl = W / f"template_death_{pas}.png"
    if not tpl.exists():
        (p1_template if pas == "p1" else p2_template)()
    p = (p1_prompt if pas == "p1" else p2_prompt)(rnd)
    (A / "prompts").mkdir(parents=True, exist_ok=True)
    (A / "prompts" / f"death_{pas}_{rnd}.txt").write_text(p, encoding="utf-8")
    r = gen.edit(tag=TAG, images=[tpl, gen.ROSTER], prompt=p, out=W / f"raw/death_{pas}_{rnd}.png", n=2)
    print(json.dumps({k: r.get(k) for k in ("ok", "files", "latency", "usd", "dropped_params", "error")}))
    return r


def cut(pas, raw):
    geom = json.loads((W / f"template_death_{pas}.geom.json").read_text())
    return sheet.slice_sheet(raw, geom, n=8)


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
    out = {}
    for raw in sorted((W / "raw").glob(f"death_{pas}_{rnd}_*.png")):
        frames, diag = cut(pas, raw)
        name = raw.stem
        d = W / "cut" / name
        d.mkdir(parents=True, exist_ok=True)
        for i, f in enumerate(frames):
            f.save(d / f"frame_{i:02d}.png")
        labs = (["ref"] + [f"d{i}" for i in P1_FRAMES] + ["empty"] * 4) if pas == "p1" else \
               (["ref"] + [f"d{i}" + ("K" if i in P2_KEYS else "") for i in P2_FRAMES])
        review_strip(frames, W / f"strip_{name}.png", labs)
        row = {k: diag.get(k) for k in ("k", "dx", "dy", "warnings", "ground_snap", "ref_redraw_dE")}
        row["bboxes"] = [ak.bbox(f, 40) for f in frames]
        out[name] = row
        print(name, json.dumps(row))
    (W / f"slice_diag_death_{pas}_{rnd}.json").write_text(json.dumps(out, indent=1))
    return out


def final_frames():
    keys = keys_frames()
    if not PICK:
        raise SystemExit("set PICK to the picked pass-2 variant first")
    d = W / "cut" / f"death_{PICK}"
    p2 = {i: Image.open(d / f"frame_{j + 1:02d}.png").convert("RGBA") for j, i in enumerate(P2_FRAMES)}
    return [keys[i] if i in keys else p2[i] for i in range(8)]


def write():
    frames = final_frames()
    man = ak.write_anim(ARM, ANIM, frames,
                        note=f"K death: frames 0, 3, 5 from pass-1 keys {KEYS}; frames 1, 2, 4, 6, 7 from pass-2 "
                             f"in-betweens {PICK}")
    print(json.dumps({"written": [KEYS, PICK], "warnings": man.get("warnings")}))


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[0] == "write":
        write()
    elif a[1] == "template":
        (p1_template if a[0] == "p1" else p2_template)()
    elif a[1] == "gen":
        generate(a[0], a[2])
    elif a[1] == "slice":
        slice_round(a[0], a[2])
