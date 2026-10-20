"""G2 - one-call sprite sheet whose cells 2-N hold arm R's cut-out rig frames as pose guides (an M8-style repaint of
a code-made blockout).

Template: cell 1 = the true sprite at 3x (character reference + the slicer's scale calibration), cells 2-N = arm R's
written frames 1..N-1 (ak.load_anim("R", anim); R's folder is read, never edited). Prompt:
gen.sheet_prompt(anim, beat texts, guide="rig"). Images: [template, roster]. n=2 per call. Up to 3 rounds per
animation; a later round may only reword the per-cell texts or add one sentence via extra=, and always keeps the same
R rig frames as guides.

  python3 arms/G2/make_g2.py template strike
  python3 arms/G2/make_g2.py gen strike r1          # one n=2 call -> work/G2/raw/strike_r1_{1,2}.png
  python3 arms/G2/make_g2.py slice strike r1        # cut every variant of that round + review strips
  python3 arms/G2/make_g2.py write strike r1_2      # write the chosen variant with ak.write_anim
  python3 arms/G2/make_g2.py seq strike             # arms/G2/sequence_strike_{3x,1x}.{gif,mp4}
"""
import json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import beats, gen, sheet  # noqa: E402
from PIL import Image  # noqa: E402

ARM = "G2"
TAG = "anim/G2"
W = ROOT / "work/G2"
A = ROOT / "arms/G2"


def beat_texts(anim):
    return [(i + 1, beats.BEATS[anim][i]["text"]) for i in range(1, beats.COUNTS[anim])]


def _reword(texts, new):
    return [(c, new.get(c, t)) for c, t in texts]


# Per-round prompt data. A round may only change the per-cell wording or add ONE sentence via extra.
ROUNDS = {
    "strike": {
        "r1": dict(texts=beat_texts("strike"), extra=""),
        # r2: r1_2 (the better variant: silhouette IoU vs R 0.83/0.44/0.84/0.59/0.71/0.84/0.94 on frames 1-7) drew the
        # cell 4 blade longer than R's foreshortened blade, so frame 3's tip lands on x 15 (1 px outside the safe
        # region; r1_1: x 13). Both variants also moved the cell 3 wind-up body ~8 px left of the draft and shortened
        # its blade (bbox x 51..108 vs R 59..120, IoU 0.44-0.47). Reword cells 3 and 4, one sentence on draft fidelity.
        "r2": dict(texts=_reword(beat_texts("strike"), {
            3: "wind-up extreme (hold): body coiled back and slightly crouched, the sword cocked behind the head with "
               "the blade hanging down behind the shoulders, shield tucked in; keep the body exactly where the draft "
               "puts it and let the blade stick out to the right of the head as far as the draft's blade does",
            4: "swing: the front foot steps toward the lower-left, the torso whips forward, the arm sweeps over the "
               "head and the blade is level, pointing to the lower-left; the blade is no longer than the draft's "
               "blade and its tip ends where the draft's tip ends, well inside the cell",
        }), extra="Keep each figure at exactly the position of its draft in the cell, and keep every sword exactly as "
                  "long as the draft's sword in that cell, with its tip where the draft's tip is."),
    },
}
# r3: r2_2 fixed both r1 problems (frame 3 tip x 21; wind-up IoU vs R 0.67) and has the lowest colour drift (0.78).
# Left: frame 4's blade is flatter than R's (points left, only slightly down, where the beat says slashing down-left),
# and frames 6-7 redraw the head rounder with fuller hair and a different face than cell 1, so 7 -> idle pops; r2_2's
# frame 7 sword also leans further left than the ready angle. Reword cells 5 and 8; the one extra sentence now also
# asks for cell 1's head and hair (merged into r2's sentence, so there is still one).
ROUNDS["strike"]["r3"] = dict(
    texts=_reword(ROUNDS["strike"]["r2"]["texts"], {
        5: "contact: arm fully extended toward the lower-left, the blade slashing steeply down-left at the same angle "
           "as the draft's blade, front knee bent, body low and leaning in, shield arm swung back for balance",
        8: "settle: almost exactly the reference pose of cell 1, with the sword at the same angle as in cell 1, only a "
           "touch lower",
    }),
    extra="Keep each figure at exactly the position of its draft in the cell, keep every sword exactly as long as the "
          "draft's sword with its tip where the draft's tip is, and keep the head, face and hair exactly as small "
          "and neat as in cell 1.")


def rig_frames(anim):
    """Arm R's frames for this animation (frame 0 is the static sprite). Raises if R has not written it."""
    man = ROOT / "arms/R" / anim / "manifest.json"
    if not man.exists():
        raise SystemExit(f"arms/R/{anim}/manifest.json does not exist: G2 needs R's rig frames as guides")
    frames, _ = ak.load_anim("R", anim)
    return frames


def template(anim):
    R = rig_frames(anim)
    n = beats.COUNTS[anim]
    cells = [ak.base_canvas()] + R[1:n]
    return sheet.make_template(cells, out=W / f"template_{anim}.png")


def prompt(anim, rnd):
    r = ROUNDS[anim][rnd]
    return gen.sheet_prompt(anim, r["texts"], guide="rig", extra=r.get("extra", ""))


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
    return sheet.slice_sheet(raw, geom, n=beats.COUNTS[anim])


def slice_round(anim, rnd):
    out = {}
    guides_row = [ak.base_canvas()] + rig_frames(anim)[1:beats.COUNTS[anim]]
    for raw in variants(anim, rnd):
        frames, diag = cut(anim, raw)
        name = raw.stem
        d = W / "cut" / name
        d.mkdir(parents=True, exist_ok=True)
        # review strip: R rig guides on top, sliced repaint below (2x)
        strip = Image.new("RGBA", (ak.W * len(frames), ak.H * 2), (222, 226, 214, 255))
        for i, f in enumerate(frames):
            f.save(d / f"frame_{i:02d}.png")
            strip.alpha_composite(guides_row[i], (i * ak.W, 0))
            strip.alpha_composite(f, (i * ak.W, ak.H))
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
    man = ak.write_anim(ARM, anim, frames, note=f"G2 {anim}: variant {variant} (raw {raw.name}), repaint of R rig frames")
    print(json.dumps({"written": variant, "warnings": man.get("warnings"), "slice": diag.get("warnings"),
                      "ground_snap": diag.get("ground_snap"), "k": diag.get("k")}))


def seq(anim):
    """Full sequence of one animation on a tile: 1 s of the ready pose, the animation (manifest timings), 1 s of the
    ready pose. GIF + MP4 at 3x (one tile) and 1x (3x3 map, MP4 stored at 2x). The ready pose is the static sprite
    (idle frame 0) because this arm has no idle yet. Uses preview.py's own drawing and encoders (same layout as the
    G1 strike sequence, so the arms' sequence files are directly comparable)."""
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
