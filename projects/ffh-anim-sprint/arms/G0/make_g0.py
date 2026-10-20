"""G0 - one-call sprite sheet with NO pose guides: the model poses the character from the beat text alone.

Template: cell 1 = the true sprite at 3x (it also calibrates the slicer's scale), cells 2-8 empty magenta.
Prompt: gen.sheet_prompt(anim, beat texts, guide="none"). Images: [template, roster]. n=2 per call.
Up to 3 rounds per animation; a later round may only reword the per-cell texts or add one sentence via extra=.

  python arms/G0/make_g0.py template strike
  python arms/G0/make_g0.py gen strike r1          # one n=2 call -> work/G0/raw/strike_r1_{1,2}.png
  python arms/G0/make_g0.py slice strike r1        # cut every variant of that round + review strips
  python arms/G0/make_g0.py write strike r1_2      # write the chosen variant with ak.write_anim
  python arms/G0/make_g0.py seq strike             # arms/G0/sequence_strike_{3x,1x}.{gif,mp4} (idle hold, anim, idle)
"""
import json, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import beats, gen, sheet  # noqa: E402
from PIL import Image  # noqa: E402

ARM = "G0"
TAG = "anim/G0"
W = ROOT / "work/G0"
A = ROOT / "arms/G0"
GROUND_SNAP = 40


def beat_texts(anim):
    return [(i + 1, beats.BEATS[anim][i]["text"]) for i in range(1, beats.COUNTS[anim])]


def _reword(texts, new):
    return [(c, new.get(c, t)) for c, t in texts]


# Per-round prompt data. A round may only change the per-cell wording or add ONE sentence via extra.
ROUNDS = {
    "strike": {
        "r1": dict(texts=beat_texts("strike"), extra=""),
        # r2: r1 drew the bottom row ~28 px above the ground line, one variant drew cells 2-8 ~6 % larger than
        # cell 1, cell 4's blade left its cell, and frame 6 already had the sword fully back at the ready angle.
        "r2": dict(texts=_reword(beat_texts("strike"), {
            3: "wind-up extreme: body coiled back and slightly crouched, the sword hand raised beside the head and the "
               "blade cocked behind the head, hanging down behind his shoulders and back, shield tucked in close",
            4: "swing: the front foot takes a step toward the lower-left, the torso whips forward, the sword arm "
               "sweeps forward over the head and the blade is level at shoulder height, pointing to the lower-left, "
               "its tip still inside the cell",
            7: "recover: body rises, front foot pulls back halfway, the sword lifts back only halfway toward the "
               "ready angle (blade pointing forward and up, not yet upright)",
        }), extra="Draw every figure, including the whole bottom row, exactly as large as the figure in cell 1, with "
                  "its boots on the same ground line near the bottom of its cell as in cell 1, and keep every sword "
                  "fully inside its own cell."),
    },
}


def template(anim):
    n = beats.COUNTS[anim]
    return sheet.make_template([ak.base_canvas()] + [None] * (n - 1), out=W / f"template_{anim}.png")


def prompt(anim, rnd):
    r = ROUNDS[anim][rnd]
    return gen.sheet_prompt(anim, r["texts"], guide="none", extra=r.get("extra", ""))


def generate(anim, rnd):
    tpl = W / f"template_{anim}.png"
    if not tpl.exists():
        template(anim)
    p = prompt(anim, rnd)
    (A / "prompts").mkdir(parents=True, exist_ok=True)
    (A / "prompts" / f"{anim}_{rnd}.txt").write_text(p, encoding="utf-8")
    r = gen.edit(tag=TAG, images=[tpl, gen.ROSTER], prompt=p, out=W / f"raw/{anim}_{rnd}.png", n=2)
    print(json.dumps({k: r.get(k) for k in ("ok", "files", "latency", "usd", "dropped_params", "error")}))
    return r


def variants(anim, rnd):
    return sorted((W / "raw").glob(f"{anim}_{rnd}_*.png")) or sorted((W / "raw").glob(f"{anim}_{rnd}.png"))


def cut(anim, raw):
    geom = json.loads((W / f"template_{anim}.geom.json").read_text())
    # ground_snap=40 instead of the default 20: with no guide in the empty cells the model has no ground line to
    # follow there and drew the whole bottom row ~28 canvas px too high in round 1 (every strike beat has a foot
    # on the ground, so snapping the lowest opaque row onto the ground row is still the right correction)
    frames, diag = sheet.slice_sheet(raw, geom, n=beats.COUNTS[anim], ground_snap=GROUND_SNAP)
    return frames, diag


def slice_round(anim, rnd):
    out = {}
    for raw in variants(anim, rnd):
        frames, diag = cut(anim, raw)
        name = raw.stem
        d = W / "cut" / name
        d.mkdir(parents=True, exist_ok=True)
        strip = Image.new("RGBA", (ak.W * len(frames), ak.H), (222, 226, 214, 255))
        for i, f in enumerate(frames):
            f.save(d / f"frame_{i:02d}.png")
            strip.alpha_composite(f, (i * ak.W, 0))
        strip.resize((strip.width * 2, strip.height * 2), Image.NEAREST).save(W / f"strip_{name}.png")
        row = {k: diag.get(k) for k in ("k", "dx", "dy", "warnings", "ground_snap", "ref_redraw_dE")}
        row["bboxes"] = [ak.bbox(f, 40) for f in frames]
        out[name] = row
        print(name, json.dumps(row))
    (W / f"slice_diag_{anim}_{rnd}.json").write_text(json.dumps(out, indent=1))
    return out


def write(anim, variant):
    raw = W / "raw" / f"{anim}_{variant}.png"
    frames, diag = cut(anim, raw)
    man = ak.write_anim(ARM, anim, frames, note=f"G0 {anim}: variant {variant} (raw {raw.name}), no pose guides")
    print(json.dumps({"written": variant, "warnings": man.get("warnings"), "slice": diag.get("warnings"),
                      "ground_snap": diag.get("ground_snap"), "k": diag.get("k")}))


def seq(anim):
    """Full sequence of one animation on a tile: 1 s of the ready pose, the animation (manifest timings), 1 s of the
    ready pose. GIF + MP4 at 3x (one tile) and 1x (3x3 map, MP4 stored at 2x). The ready pose is the static sprite
    (idle frame 0) because this arm has no idle yet."""
    import preview as pv
    frames, man = ak.load_anim(ARM, anim)
    ms = [x["ms"] for x in man["frames"]]
    if man.get("hold_last"):
        ms = ms[:-1] + [ms[-1] + 1500]
    base = ak.base_canvas()
    timeline = [(base, 1000)] + list(zip(frames, ms)) + [(base, 1000)]
    g = pv.grass()

    def draw3(f):
        sc = Image.new("RGBA", (232, 200), (54, 92, 150, 255))
        sc.alpha_composite(g, (20, 64))
        pv.unit_on(sc, f, 20 + 96, 64 + 54)
        return sc.resize((sc.width * 3, sc.height * 3), Image.NEAREST)

    def draw1(f):
        X0, Y0 = 20 + 2 * 96, 70
        sc = Image.new("RGBA", (6 * 96 + 40, 6 * 48 + 110), (54, 92, 150, 255))
        order = sorted(((r, c) for r in range(3) for c in range(3)), key=lambda t: (t[0] + t[1], t[0]))
        for r, c in order:
            sc.alpha_composite(g, (X0 + (c - r) * 96, Y0 + (c + r) * 48))
        pv.unit_on(sc, f, X0 + 96, Y0 + 2 * 48 + 54)
        return sc

    f3 = [(draw3(f), m) for f, m in timeline]
    f1 = [(draw1(f), m) for f, m in timeline]
    pv.save_gif(f3, A / f"sequence_{anim}_3x.gif")
    pv.save_mp4(f3, A / f"sequence_{anim}_3x.mp4")
    pv.save_gif(f1, A / f"sequence_{anim}_1x.gif")
    pv.save_mp4(f1, A / f"sequence_{anim}_1x.mp4", scale=2)
    print("sequence ms", sum(m for _, m in timeline))


if __name__ == "__main__":
    cmd, anim = sys.argv[1], sys.argv[2]
    if cmd == "template":
        template(anim)
    elif cmd == "gen":
        generate(anim, sys.argv[3])
    elif cmd == "slice":
        slice_round(anim, sys.argv[3])
    elif cmd == "write":
        write(anim, sys.argv[3])
    elif cmd == "seq":
        seq(anim)
