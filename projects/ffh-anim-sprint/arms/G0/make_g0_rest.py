"""G0 (rest) - idle, flinch and death as one-call sprite sheets with NO pose guides.

Same method as make_g0.py (strike): template cell 1 = the true sprite at 3x (character reference + the slicer's
scale calibration), every other cell empty magenta; prompt = gen.sheet_prompt(..., guide="none"); images =
[template, gen.ROSTER]; n=2; sliced with sheet.slice_sheet(ground_snap=48). Up to 3 rounds per sheet; a later round
may only reword cell texts or add one sentence via extra=.

  SHEET A: cell 1 reference, cells 2-4 idle frames 1-3, cells 5-8 flinch frames 0-3 (one call, two animations)
  SHEET B: cell 1 reference, cells 2-8 death frames 1-7; death frame 0 = the accepted flinch frame 0 from sheet A

  python arms/G0/make_g0_rest.py template A|B
  python arms/G0/make_g0_rest.py gen A r1           # one n=2 call -> work/G0/rest/raw/A_r1_{1,2}.png
  python arms/G0/make_g0_rest.py slice A r1         # cut every variant + review strips (3x and 1x on grass)
  python arms/G0/make_g0_rest.py write idle r1_2    # idle   <- sheet A variant (cells 2-4; frame 0 = static sprite)
  python arms/G0/make_g0_rest.py write flinch r1_1  # flinch <- sheet A variant (cells 5-8)
  python arms/G0/make_g0_rest.py write death r1_2 r1_1   # death <- sheet B variant, frame 0 = flinch f0 of A variant
"""
import json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import beats, gen, sheet  # noqa: E402
from PIL import Image  # noqa: E402

ARM = "G0"
TAG = "anim/G0"
W = ROOT / "work/G0/rest"
A = ROOT / "arms/G0"
# Same reach as the strike (make_g0.py): with no guide in the empty cells the model ignores the ground line, mostly
# in the bottom row. Every idle/flinch/death beat keeps feet, knees or the lying body on the ground.
GROUND_SNAP = 48

SIZE_SENTENCE = ("Draw every figure, including the whole bottom row, exactly as large as the figure in cell 1, with "
                 "its boots on the same ground line near the bottom of its cell as in cell 1, and keep every sword "
                 "fully inside its own cell.")  # reused from the strike's picked prompt (strike_r2)

SHEETS = {
    "A": {"anim": "IDLE (cells 2-4) and FLINCH (cells 5-8)", "prompt_names": ["idle", "flinch"]},
    "B": {"anim": "death", "prompt_names": ["death"]},
}

# ---------------------------------------------------------------- per-round prompt data
_IDLE_R1 = {
    2: "idle frame 1: " + beats.BEATS["idle"][1]["text"],
    3: "idle frame 2: " + beats.BEATS["idle"][2]["text"],
    4: "idle frame 3: " + beats.BEATS["idle"][3]["text"],
}
_FLINCH_R1 = {
    5: "flinch frame 0: " + beats.BEATS["flinch"][0]["text"],
    6: "flinch frame 1: " + beats.BEATS["flinch"][1]["text"],
    7: "flinch frame 2: " + beats.BEATS["flinch"][2]["text"],
    8: "flinch frame 3: " + beats.BEATS["flinch"][3]["text"],
}
_DEATH_R1 = {i + 1: beats.BEATS["death"][i]["text"] for i in range(1, 8)}
# the strike lessons: name where each prop is, and say the lying body runs along the tile diagonal in visible terms
_DEATH_R1[6] = ("hits the ground: lying flat on his back on the ground, his body running diagonally across the cell "
                "from his boots at the lower-left up to his head at the upper-right (along the edge of an isometric "
                "floor tile); the sword lies on the ground at the lower-left near his boots; the shield lies flat "
                "beside him")
_DEATH_R1[7] = ("bounce: the same diagonal lying pose as cell 6, but his head and shoulders lifted slightly off the "
                "ground, the shield rocking")
_DEATH_R1[8] = ("corpse: the same diagonal lying pose as cell 6, settled flat and still on his back, eyes closed, "
                "sword and shield on the ground beside him")


def _cells(d):
    return sorted(d.items())


ROUNDS = {
    "A": {"r1": dict(texts=_cells({**_IDLE_R1, **_FLINCH_R1}), extra=SIZE_SENTENCE)},
    "B": {"r1": dict(texts=_cells(_DEATH_R1), extra=SIZE_SENTENCE)},
}


def _reword(texts, new):
    return [(c, new.get(c, t)) for c, t in texts]


# r2 (sheet A): in both r1 variants the idle cells were full redraws, not copies: the figure ~4 px (5 %) taller, the
# sword hand moved in toward the body and the blade more upright, the shield bigger -> a pop at frame 0->1 and heavy
# boil. The flinch impact had eyes shut and arms out but the head did not move back toward the upper-right.
ROUNDS["A"]["r2"] = dict(texts=_reword(ROUNDS["A"]["r1"]["texts"], {
    2: "idle frame 1: an exact copy of the figure in cell 1 (same size, same stance and boots, same sword angle and "
       "sword-hand position, same shield), except breathing in: his chest and head sit one pixel higher and his "
       "shoulders lift slightly; the sword has not moved yet",
    3: "idle frame 2: the same exact copy of cell 1, at the top of the breath: chest and head still one pixel higher; "
       "the sword tip two pixels higher and the shield one pixel higher; nothing else changes",
    4: "idle frame 3: the same exact copy of cell 1, breathing out: chest and head back down exactly as in cell 1; "
       "the sword tip still one pixel high, settling",
    5: "flinch frame 0: impact, hit from the lower-left: his head is knocked back toward the upper-right (chin up, "
       "head clearly further right than in cell 1), eyes squeezed shut, torso jolted back, both arms jerk outward: the "
       "shield flung out to the right, the sword arm thrown up and out to the left with the sword still in his hand; "
       "boots stay planted",
    6: "flinch frame 1: recoil extreme: leaning back further toward the upper-right, knees bent, the sword dipping "
       "down toward the lower-left, shield still held out to the right; boots stay planted",
}), extra=SIZE_SENTENCE)
# r2 (sheet B): both r1 variants drew motion arcs around the shield in cell 7 ("rocking" invites them: FX, not
# allowed) and r1_1 drew dark magenta contact shadows under the lying body (cells 5-8) that the keyer keeps; the lying
# body had its knees drawn up (half sitting). Cell 2 gets the hit direction it lacked.
ROUNDS["B"]["r2"] = dict(texts=_reword(ROUNDS["B"]["r1"]["texts"], {
    2: "stagger back: knocked back by a hit from the lower-left, he steps back with his near (left) foot toward the "
       "upper-right, body thrown back, the sword arm flung up with the sword still in his hand, shield drooping",
    6: "hits the ground: lying flat on his back with his legs straight, his body running diagonally across the cell "
       "from his boots at the lower-left up to his head at the upper-right (along the edge of an isometric floor "
       "tile); the sword lies on the ground at the lower-left near his boots; the shield lies flat beside him; no "
       "shadow under him",
    7: "bounce: the same diagonal lying pose as cell 6, but his head and shoulders lifted slightly off the ground and "
       "the shield tilted up a little on its rim (no motion lines)",
}), extra=SIZE_SENTENCE)


# r3 (sheet A): r2's "exact copy" wording helped (r2_2 idle silhouette IoU vs the sprite 0.86/0.80/0.89, head 1 px up in
# frames 1-2 and back in 3, as beated) but every idle cell still moved the sword fist up and in toward the body, stood
# the blade more upright and widened the stance. r3 names cell 1's actual sword/shield/stance, and the extra sentence is
# swapped (still one sentence) for one asking for tracings of cell 1 plus the bottom-row size/sword clause.
ROUNDS["A"]["r3"] = dict(texts=_reword(ROUNDS["A"]["r2"]["texts"], {
    2: "idle frame 1: a tracing of cell 1: the same pose, size and place in the cell, the sword fist low beside his "
       "right hip with the blade leaning out to the upper-left at the same angle as in cell 1, the shield at the same "
       "height, the same boots and stance; only his chest and head are one pixel higher (breathing in)",
    3: "idle frame 2: the same tracing of cell 1, top of the breath: chest and head one pixel higher, the sword tip "
       "two pixels higher and the shield one pixel higher; nothing else changes",
    4: "idle frame 3: the same tracing of cell 1, breathing out: chest and head back exactly as in cell 1, the sword "
       "tip still one pixel higher, settling",
}), extra="Cells 2, 3 and 4 must look almost identical to cell 1 at exactly the same size, like tracings with only a "
          "one- or two-pixel breathing change, and every figure in the bottom row must be exactly as large as the "
          "figure in cell 1 with its sword fully inside its cell.")


# r3 (sheet B): r2 fixed the motion arcs and the shadows (both variants clean) and r2_2 lies close to the iso axis
# (body slope ~0.46 vs 0.5). Left over: the shield stands up on its rim beside him instead of lying flat, and the knees
# stay a little bent. One more sample with those two cells made more concrete.
ROUNDS["B"]["r3"] = dict(texts=_reword(ROUNDS["B"]["r2"]["texts"], {
    6: "hits the ground: lying flat on his back with his legs straight, his body running diagonally across the cell "
       "from his boots at the lower-left up to his head at the upper-right (along the edge of an isometric floor "
       "tile); the sword lies on the ground at the lower-left near his boots; the shield lies flat on the ground "
       "beside his left side, seen from above as a wide oval; no shadow under him",
    8: "corpse: the same diagonal lying pose as cell 6, settled flat and still on his back with his legs straight, "
       "eyes closed, sword and shield lying flat on the ground beside him",
}), extra=SIZE_SENTENCE)


# ---------------------------------------------------------------- template / generate
def template(sh):
    return sheet.make_template([ak.base_canvas()] + [None] * 7, out=W / f"template_{sh}.png")


def prompt(sh, rnd):
    r = ROUNDS[sh][rnd]
    return gen.sheet_prompt(SHEETS[sh]["anim"], r["texts"], guide="none", extra=r.get("extra", ""))


def generate(sh, rnd):
    tpl = W / f"template_{sh}.png"
    if not tpl.exists():
        template(sh)
    p = prompt(sh, rnd)
    (A / "prompts").mkdir(parents=True, exist_ok=True)
    for name in SHEETS[sh]["prompt_names"]:  # sheet A's prompt is saved under both idle_ and flinch_ (same text)
        (A / "prompts" / f"{name}_sheet{sh}_{rnd}.txt").write_text(p, encoding="utf-8")
    r = gen.edit(tag=TAG, images=[tpl, gen.ROSTER], prompt=p, out=W / f"raw/{sh}_{rnd}.png", n=2)
    print(json.dumps({k: r.get(k) for k in ("ok", "files", "latency", "usd", "dropped_params", "error")}))
    return r


def variants(sh, rnd):
    return sorted((W / "raw").glob(f"{sh}_{rnd}_*.png")) or sorted((W / "raw").glob(f"{sh}_{rnd}.png"))


def cut(sh, variant):
    raw = W / "raw" / f"{sh}_{variant}.png"
    geom = json.loads((W / f"template_{sh}.geom.json").read_text())
    frames, diag = sheet.slice_sheet(raw, geom, n=8, ground_snap=GROUND_SNAP)
    return frames, diag


def anim_frames(anim, variant, f0_variant=None):
    """idle: [static, A1, A2, A3]; flinch: [A4..A7]; death: [flinch f0 of A f0_variant] + [B1..B7]."""
    if anim == "idle":
        fr, diag = cut("A", variant)
        return [ak.base_canvas()] + fr[1:4], diag
    if anim == "flinch":
        fr, diag = cut("A", variant)
        return fr[4:8], diag
    if anim == "death":
        fr, diag = cut("B", variant)
        fa, _ = cut("A", f0_variant)
        return [fa[4]] + fr[1:8], diag
    raise ValueError(anim)


# ---------------------------------------------------------------- review
def review_strips(frames, name):
    import preview as pv
    strip = Image.new("RGBA", (ak.W * len(frames), ak.H), (222, 226, 214, 255))
    for i, f in enumerate(frames):
        strip.alpha_composite(f, (i * ak.W, 0))
    strip.resize((strip.width * 2, strip.height * 2), Image.NEAREST).save(W / f"strip_{name}.png")
    pv.map1x(frames).save(W / f"map_{name}.png")


def slice_round(sh, rnd):
    import numpy as np
    out = {}
    base = np.asarray(ak.base_canvas()).astype(np.float32)
    for raw in variants(sh, rnd):
        v = raw.stem.split("_", 1)[1]
        frames, diag = cut(sh, v)
        d = W / "cut" / raw.stem
        d.mkdir(parents=True, exist_ok=True)
        for i, f in enumerate(frames):
            f.save(d / f"frame_{i:02d}.png")
        review_strips(frames, raw.stem)
        row = {k: diag.get(k) for k in ("k", "dx", "dy", "warnings", "ground_snap", "ref_redraw_dE")}
        row["bboxes"] = [ak.bbox(f, 40) for f in frames]
        if sh == "A":  # idle boil proxy: mean |RGBA diff| vs the static sprite over the union of opaque pixels
            boil = []
            for f in frames[1:4]:
                a = np.asarray(f).astype(np.float32)
                m = (a[..., 3] > 40) | (base[..., 3] > 40)
                boil.append(round(float(np.abs(a - base)[m].mean()), 1))
            row["idle_vs_static_absdiff"] = boil
        out[raw.stem] = row
        print(raw.stem, json.dumps(row))
    (W / f"slice_diag_{sh}_{rnd}.json").write_text(json.dumps(out, indent=1))
    return out


def write(anim, variant, f0_variant=None):
    frames, diag = anim_frames(anim, variant, f0_variant)
    src = {"idle": f"sheet A {variant} cells 2-4 (frame 0 = static sprite)", "flinch": f"sheet A {variant} cells 5-8",
           "death": f"sheet B {variant} cells 2-8, frame 0 = flinch frame 0 (sheet A {f0_variant} cell 5)"}[anim]
    man = ak.write_anim(ARM, anim, frames, note=f"G0 {anim}: {src}, no pose guides")
    print(json.dumps({"written": anim, "variant": variant, "warnings": man.get("warnings"),
                      "slice": diag.get("warnings"), "ground_snap": diag.get("ground_snap"), "k": diag.get("k")}))


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "template":
        template(sys.argv[2])
    elif cmd == "prompt":
        print(prompt(sys.argv[2], sys.argv[3]))
    elif cmd == "gen":
        generate(sys.argv[2], sys.argv[3])
    elif cmd == "slice":
        slice_round(sys.argv[2], sys.argv[3])
    elif cmd == "write":
        write(sys.argv[2], sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else None)
