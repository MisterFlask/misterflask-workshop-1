"""G1 (rest) - IDLE, FLINCH and DEATH as one-call sprite sheets with flat-colour pose guides from poses.json.

Two sheets (4x2, 384x512 cells), each with the true sprite in cell 1 (character reference + slicer calibration):
  SHEET A: cells 2-4 = idle frames 1-3 guides, cells 5-8 = flinch frames 0-3 guides  (one call, two animations)
  SHEET B: cells 2-8 = death frames 1-7 guides; death frame 0 = the accepted flinch frame 0 from sheet A
Guides: guides.render_guide(pose, "colour"). Prompt: gen.sheet_prompt(..., guide="colour"). Images: [template, roster].
n=2 per call. Up to 3 rounds per sheet; later rounds may change the wording or add ONE sentence via extra=, the
guides never change. The strike is made by a separate G1 agent (make_g1.py); this file does not touch it.

  python arms/G1/make_g1_rest.py template A|B
  python arms/G1/make_g1_rest.py gen A r1          # one n=2 call -> work/G1/rest/raw/sheetA_r1_{1,2}.png
  python arms/G1/make_g1_rest.py slice A r1        # cut every variant of that round + review strips
  python arms/G1/make_g1_rest.py write A r1_2      # idle + flinch from that variant via ak.write_anim
  python arms/G1/make_g1_rest.py write B r1_1 --d0 r1_2   # death from that variant; frame 0 = flinch frame 0 of sheet A r1_2
"""
import json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import beats, gen, guides, sheet  # noqa: E402
from PIL import Image  # noqa: E402

ARM = "G1"
TAG = "anim/G1"
W = ROOT / "work/G1/rest"
A = ROOT / "arms/G1"
POSES = json.loads((ROOT / "poses.json").read_text())["anims"]

# sheet -> list of (anim, frame) for cells 2..8 (cell 1 = reference)
LAYOUT = {
    "A": [("idle", 1), ("idle", 2), ("idle", 3), ("flinch", 0), ("flinch", 1), ("flinch", 2), ("flinch", 3)],
    "B": [("death", k) for k in range(1, 8)],
}
ANIM_LABEL = {"A": "IDLE (cells 2-4) and FLINCH (cells 5-8)", "B": "death"}
PROMPT_NAMES = {"A": ["idle", "flinch"], "B": ["death"]}


def base_texts(s):
    return [(c + 2, f"{a} frame {k}: {beats.BEATS[a][k]['text']}") for c, (a, k) in enumerate(LAYOUT[s])]


def _reword(texts, new):
    return [(c, new.get(c, t)) for c, t in texts]


# Per-round prompt data. A round may only change the per-cell wording or add ONE sentence via extra.
ROUNDS = {"A": {"r1": dict(texts=base_texts("A"), extra="")},
          "B": {"r1": dict(texts=base_texts("B"), extra="")}}
# A r2: both r1 variants posed every cell well, but idle cells 2-4 were full redraws of the character (different face
# and expression, fluffier hair, a reshaped shield and sword hand), so against the true sprite in idle frame 0 the
# loop boils (mean frame-to-frame change 41-46 vs 15-16 for the sprite against the model's own copy of cell 1).
# r2 rewords cells 2-4 as "copy cell 1, move only what the blockout moves" and adds one sentence; flinch unchanged.
ROUNDS["A"]["r2"] = dict(
    texts=_reword(base_texts("A"), {
        2: "idle frame 1: breathing in. Copy cell 1 exactly, pixel for pixel (same face and expression, same hair, "
           "outfit, sword and shield drawing); only the head and chest are one pixel higher, as in the blockout",
        3: "idle frame 2: top of the breath. Copy cell 1 exactly, pixel for pixel; the head and chest are still one "
           "pixel higher, and the sword tip and the shield are raised by one or two pixels, as in the blockout",
        4: "idle frame 3: breathing out. Copy cell 1 exactly, pixel for pixel; the head and chest are back down and "
           "only the sword tip is still one pixel high, as in the blockout",
    }),
    extra="Cells 2, 3 and 4 are a subtle breathing loop: they must look like exact copies of the cell 1 sprite (same "
          "face and expression, same hair shape, same shading and outline pixels), differing from it only by the "
          "one- or two-pixel shifts of their blockouts.")
# A r3: "copy pixel for pixel" changed nothing (r2_1 idle boil 44, r2_2 61): cells 2-4 were still fresh redraws, and
# they follow the blockout's slightly more upright sword and rounder shield rather than cell 1's. r3 words cells 2-4
# as "a duplicate of cell 1 nudged by a pixel" and the one extra sentence says to follow cell 1 where the two disagree.
ROUNDS["A"]["r3"] = dict(
    texts=_reword(base_texts("A"), {
        2: "idle frame 1 (breathing in): a duplicate of cell 1 with the head and chest nudged up by one pixel; the "
           "same face, hair, sword angle, shield and legs as cell 1",
        3: "idle frame 2 (top of the breath): a duplicate of cell 1 with the head and chest one pixel up and the "
           "sword and shield nudged up by one or two pixels; the same face, hair, sword angle and legs as cell 1",
        4: "idle frame 3 (breathing out): a duplicate of cell 1 with only the sword nudged up by one pixel; the same "
           "face, hair, chest, shield and legs as cell 1",
    }),
    extra="For cells 2, 3 and 4, where the blockout and cell 1 disagree, follow cell 1: make them by duplicating cell "
          "1 and nudging parts by a pixel or two, not by redrawing the face, hair, sword or shield.")


def template(s):
    cells = [ak.base_canvas()] + [guides.render_guide(POSES[a][k], "colour") for a, k in LAYOUT[s]]
    return sheet.make_template(cells, out=W / f"template_sheet{s}.png")


def prompt(s, rnd):
    r = ROUNDS[s][rnd]
    return gen.sheet_prompt(ANIM_LABEL[s], r["texts"], guide="colour", extra=r.get("extra", ""))


def generate(s, rnd):
    tpl = W / f"template_sheet{s}.png"
    if not tpl.exists():
        template(s)
    p = prompt(s, rnd)
    (A / "prompts").mkdir(parents=True, exist_ok=True)
    for name in PROMPT_NAMES[s]:  # sheet A serves two animations: the same prompt is saved under both names
        (A / "prompts" / f"{name}_sheet{s}_{rnd}.txt").write_text(p, encoding="utf-8")
    r = gen.edit(tag=TAG, images=[tpl, gen.ROSTER], prompt=p, out=W / f"raw/sheet{s}_{rnd}.png", n=2)
    print(json.dumps({k: r.get(k) for k in ("ok", "files", "latency", "usd", "dropped_params", "error")}))
    return r


def variants(s, rnd):
    return sorted((W / "raw").glob(f"sheet{s}_{rnd}_*.png")) or sorted((W / "raw").glob(f"sheet{s}_{rnd}.png"))


def cut(s, raw):
    geom = json.loads((W / f"template_sheet{s}.geom.json").read_text())
    return sheet.slice_sheet(raw, geom, n=8)


def slice_round(s, rnd):
    out = {}
    for raw in variants(s, rnd):
        frames, diag = cut(s, raw)
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
    (W / f"slice_diag_sheet{s}_{rnd}.json").write_text(json.dumps(out, indent=1))
    return out


def frames_of(s, variant):
    raw = W / "raw" / f"sheet{s}_{variant}.png"
    frames, diag = cut(s, raw)
    return frames, diag, raw


def write(s, variant, d0=None, idle_override=None):
    frames, diag, raw = frames_of(s, variant)
    info = {"slice": diag.get("warnings"), "ground_snap": diag.get("ground_snap"), "k": diag.get("k"),
            "ref": diag.get("ref_redraw_dE")}
    if s == "A":
        idle = [ak.base_canvas()] + frames[1:4]
        man = ak.write_anim(ARM, "idle", idle, note=f"G1 idle: sheet A variant {variant} (raw {raw.name}), cells 2-4")
        info["idle_warnings"] = man.get("warnings")
        man = ak.write_anim(ARM, "flinch", frames[4:8],
                            note=f"G1 flinch: sheet A variant {variant} (raw {raw.name}), cells 5-8")
        info["flinch_warnings"] = man.get("warnings")
    else:
        fa, _, rawa = frames_of("A", d0)
        death = [fa[4]] + frames[1:8]
        man = ak.write_anim(ARM, "death", death,
                            note=f"G1 death: frame 0 = flinch frame 0 (sheet A {d0}, raw {rawa.name}); frames 1-7 = "
                                 f"sheet B variant {variant} (raw {raw.name}), cells 2-8")
        info["death_warnings"] = man.get("warnings")
    print(json.dumps({"written": s, "variant": variant} | info))


if __name__ == "__main__":
    cmd, s = sys.argv[1], sys.argv[2]
    if cmd == "template":
        template(s)
    elif cmd == "gen":
        generate(s, sys.argv[3])
    elif cmd == "slice":
        slice_round(s, sys.argv[3])
    elif cmd == "write":
        d0 = sys.argv[sys.argv.index("--d0") + 1] if "--d0" in sys.argv else None
        write(s, sys.argv[3], d0=d0)
