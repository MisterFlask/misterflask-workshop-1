"""G2 (rest): idle, flinch and death as one-call sprite sheets whose non-reference cells hold arm R's cut-out rig
frames as pose guides (an M8-style repaint). The strike is made by make_g2.py (another agent); this file never
touches it.

Two 4x2 sheets (384x512 cells, cell 1 = the true sprite at 3x: character reference + the slicer's single scale):
  SHEET A: cells 2-4 = R's idle frames 1-3, cells 5-8 = R's flinch frames 0-3
           (cell texts prefixed "idle frame k:" / "flinch frame k:", anim="IDLE (cells 2-4) and FLINCH (cells 5-8)")
  SHEET B: cells 2-8 = R's death frames 1-7; death frame 0 = the flinch frame 0 accepted from sheet A.
Prompt: gen.sheet_prompt(..., guide="rig"). Images [template, gen.ROSTER], n=2, tag anim/G2. Up to 3 rounds per
sheet; a later round only rewords cell texts or adds ONE sentence via extra=. R's frames are read, never edited.

  python3 arms/G2/make_g2_rest.py template A            # work/G2/rest/template_A.png (+ .geom.json)
  python3 arms/G2/make_g2_rest.py gen A r1              # one n=2 call -> work/G2/rest/raw/sheetA_r1_{1,2}.png
  python3 arms/G2/make_g2_rest.py slice A r1            # cut every variant + review strips + numbers
  python3 arms/G2/make_g2_rest.py write idle            # write the PICKED variant (see PICK) with ak.write_anim
  python3 arms/G2/make_g2_rest.py all                   # rebuild idle, flinch, death from the saved raws
"""
import json, sys
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import beats, gen, sheet, metrics  # noqa: E402
from PIL import Image  # noqa: E402

ARM = "G2"
TAG = "anim/G2"
W = ROOT / "work/G2/rest"
A = ROOT / "arms/G2"

SHEET_ANIM = {"A": "IDLE (cells 2-4) and FLINCH (cells 5-8)", "B": "death"}
# which (anim, frame) each sheet cell 2..8 holds (cell index i = 1..7 in 0-based slicer order)
CELLS = {"A": [("idle", 1), ("idle", 2), ("idle", 3), ("flinch", 0), ("flinch", 1), ("flinch", 2), ("flinch", 3)],
         "B": [("death", k) for k in range(1, 8)]}
# chosen variants (raw stems without the "sheetX_" prefix), filled in as rounds are judged
PICK = {"idle": "r2_2", "flinch": "r1_1", "death": "r2_1"}


def rig(anim):
    man = ROOT / "arms/R" / anim / "manifest.json"
    if not man.exists():
        raise SystemExit(f"arms/R/{anim}/manifest.json does not exist: G2 needs R's rig frames as guides")
    return ak.load_anim("R", anim)[0]


def guides(sh):
    return [ak.base_canvas()] + [rig(a)[k] for a, k in CELLS[sh]]


def beat_texts(sh):
    out = []
    for c, (a, k) in enumerate(CELLS[sh], start=2):
        t = beats.BEATS[a][k]["text"]
        out.append((c, f"{a} frame {k}: {t}" if sh == "A" else t))
    return out


def _reword(texts, new):
    return [(c, new.get(c, t)) for c, t in texts]


# Per-round prompt data. A later round may only change per-cell wording or add ONE sentence via extra.
ROUNDS = {"A": {"r1": dict(texts=beat_texts("A"), extra="")},
          "B": {"r1": dict(texts=beat_texts("B"), extra="")}}
# A r2: r1 posed every cell like R, but idle cells 2-3 were full redraws (new face and expression, a bigger shield,
# magenta-tinged outline): idle boil mean 32.7 / 32.9 against R's 10.9, 1494-1900 px changed per step. Cell 4 came
# back almost as the sprite (IoU 0.979 / 0.973). R's idle drafts ARE the sprite's own pixels moved by 1-2 px, so the
# idle cells now say the draft is already finished and must be copied unchanged, as cell 1 is; one extra sentence.
# Flinch texts unchanged (r1_1's flinch is usable: IoU vs R 0.65/0.61/0.80/0.96, drift 1.06).
ROUNDS["A"]["r2"] = dict(
    texts=_reword(beat_texts("A"), {
        2: "idle frame 1: breathing in; this draft is already finished art, the reference sprite with the chest and "
           "head raised by one pixel: copy the draft exactly as it is, pixel for pixel, as you do with cell 1",
        3: "idle frame 2: top of the breath; this draft is already finished art, the reference sprite with the chest "
           "and head one pixel up and the sword tip two pixels up: copy the draft exactly as it is, pixel for pixel, "
           "as you do with cell 1",
        4: "idle frame 3: breathing out; this draft is already finished art, the reference sprite with the sword tip "
           "one pixel up: copy the draft exactly as it is, pixel for pixel, as you do with cell 1",
    }),
    extra="Cells 2-4 need no repair at all: they differ from cell 1 by only one or two pixels, so reproduce them "
          "unchanged, with exactly cell 1's face, hair, sword and shield, and redraw only cells 5-8.")
# A r3: r2 halved the idle redraw (boil mean 23.6 / 20.4, 678-1481 px changed per step; r2_2's frames 1-3 are R's
# drafts slightly softened, with a thin magenta-tinged outline and small eye/mouth changes). r2's flinch frames 0-1
# grew the head and hair (both variants) against r1_1. Keep r2's idle texts; reword flinch cells 5-6 to keep cell 1's
# head size; the one extra sentence also asks for cell 1's outline with no pink or purple edge pixels.
ROUNDS["A"]["r3"] = dict(
    texts=_reword(ROUNDS["A"]["r2"]["texts"], {
        5: "flinch frame 0: impact: hit from the lower-left; head snaps back toward the upper-right, eyes squeezed "
           "shut, torso jolts back, both arms jerk outward (shield flung out to the right, sword arm thrown up and "
           "out); the head and hair stay exactly as small as in cell 1, only tipped back",
        6: "flinch frame 1: recoil extreme: leaning back further, knees bent, sword dipping, shield still held out; "
           "the head and hair stay exactly as small as in cell 1",
    }),
    extra="Cells 2-4 need no repair at all: they differ from cell 1 by only one or two pixels, so reproduce them "
          "unchanged, with exactly cell 1's face, hair, sword, shield and crisp dark outline (no pink or purple edge "
          "pixels), and redraw only cells 5-8.")

# B r2: both r1 variants follow the standing drafts (frames 1-3: stagger with the sword flung up, knees buckling,
# kneeling with the sword tip on the ground) but redraw the lying frames as a side-view body lying almost level
# (head only ~20 px above the feet over ~100 px, R's draft rises 96:45 along the iso ground axis), the shield upright
# and face-on beside the head instead of flat, and the far arm raised as a fist on frames 4 and 6 (R's draft has the
# arm on the far side of the chest; the model amplified it into a wave). Both drew the bottom row high (r1_1 by 21 px,
# just past the slicer's 20 px ground snap, so its lying frames float; r1_2 by 14-16 px). Reword cells 6-8, one
# sentence on the bottom row's position and slant.
ROUNDS["B"]["r2"] = dict(
    texts=_reword(beat_texts("B"), {
        6: "hits the ground: lying on his back along the draft's diagonal, feet at the lower-left and head at the "
           "upper-right, the body rising toward the upper-right as steeply as the draft's body; the sword lies on the "
           "ground at the lower-left; the shield lies flat on the ground beside him, a thin ellipse as in the draft; "
           "both arms lie on the ground, not raised",
        7: "bounce: the same lying pose on the same diagonal, only the head and shoulders lifted a little off the "
           "ground and the flat shield rocking; both arms stay down on the ground",
        8: "corpse: lying flat and still on his back along the same diagonal as the draft, feet at the lower-left and "
           "head at the upper-right, eyes closed, arms on the ground, sword and shield lying flat on the ground beside "
           "him",
    }),
    extra="In the bottom row keep each lying body exactly where its draft lies in the cell, resting on the same ground "
          "line as the feet in cell 1 and slanting up toward the upper-right exactly as much as the draft does.")

# B r3: r2 fixed the lying frames in both variants (body slanting up to the upper-right, shield flat as an ellipse
# beside him, arms down on frames 5 and 7; drift r2_2 0.96-1.07, r2_1 1.07-1.35; IoU vs R 0.52-0.79). Left: frame 4
# (falling back) still raises the empty sword hand as a fist above the head in both variants, which reads as a wave;
# R's draft has the arm up on the far side, and the beat only says the sword slips out of the hand. Reword cell 5;
# keep r2's cells 6-8 and r2's extra sentence.
ROUNDS["B"]["r3"] = dict(
    texts=_reword(ROUNDS["B"]["r2"]["texts"], {
        5: "falling back: torso tips back toward the upper-right, the sword has slipped out of the open hand and is "
           "dropping to the ground at the lower-left, legs slide forward; the empty sword arm trails limply back "
           "behind the body, the hand open and not raised above the head",
    }),
    extra=ROUNDS["B"]["r2"]["extra"])


def template(sh):
    return sheet.make_template(guides(sh), out=W / f"template_{sh}.png")


def prompt(sh, rnd):
    r = ROUNDS[sh][rnd]
    return gen.sheet_prompt(SHEET_ANIM[sh], r["texts"], guide="rig", extra=r.get("extra", ""))


def save_prompt(sh, rnd, p):
    (A / "prompts").mkdir(parents=True, exist_ok=True)
    names = ["idle", "flinch"] if sh == "A" else ["death"]
    for a in names:
        (A / "prompts" / f"{a}_sheet{sh}_{rnd}.txt").write_text(p, encoding="utf-8")


def generate(sh, rnd):
    tpl = W / f"template_{sh}.png"
    if not tpl.exists():
        template(sh)
    p = prompt(sh, rnd)
    save_prompt(sh, rnd, p)
    r = gen.edit(tag=TAG, images=[tpl, gen.ROSTER], prompt=p, out=W / f"raw/sheet{sh}_{rnd}.png", n=2)
    print(json.dumps({k: r.get(k) for k in ("ok", "files", "latency", "usd", "dropped_params", "error")}))
    return r


def variants(sh, rnd):
    return sorted((W / "raw").glob(f"sheet{sh}_{rnd}_*.png")) or sorted((W / "raw").glob(f"sheet{sh}_{rnd}.png"))


def cut(sh, raw):
    geom = json.loads((W / f"template_{sh}.geom.json").read_text())
    return sheet.slice_sheet(raw, geom, n=8)


def _iou(a, b):
    return round(float(metrics.iou(metrics.mask(a), metrics.mask(b))), 3)


def _changed(a, b, thr=40):
    """opaque-union pixels whose premultiplied RGBA changes by more than thr levels (max channel)."""
    x = np.asarray(a).astype(np.float32)
    y = np.asarray(b).astype(np.float32)
    px = np.dstack([x[..., :3] * x[..., 3:4] / 255, x[..., 3:4]])
    py = np.dstack([y[..., :3] * y[..., 3:4] / 255, y[..., 3:4]])
    d = np.abs(px - py).max(-1)
    u = (x[..., 3] > 40) | (y[..., 3] > 40)
    return int((d[u] > thr).sum()), int(u.sum())


def numbers(sh, frames):
    """Review numbers for one variant: IoU vs R's draft per cell, drift, and (sheet A) the idle loop's boil."""
    g = guides(sh)
    row = {"iou_vs_R": [_iou(frames[i], g[i]) for i in range(1, 8)],
           "drift": [d["mean"] if d else None for d in metrics.drift(frames[1:8])],
           "bbox": [ak.bbox(f, 40) for f in frames]}
    if sh == "A":
        base = ak.base_canvas()
        loop = [base] + frames[1:4] + [base]
        row["idle_boil"] = metrics.boil(loop)
        row["idle_boil_mean"] = round(float(np.mean(row["idle_boil"])), 1)
        row["idle_changed_px"] = [_changed(loop[i], loop[i + 1])[0] for i in range(4)]
        row["idle_iou_vs_sprite"] = [_iou(f, base) for f in frames[1:4]]
        Rb = [base] + rig("idle")[1:4] + [base]
        row["R_idle_boil"] = metrics.boil(Rb)
    return row


def slice_round(sh, rnd):
    out = {}
    g = guides(sh)
    for raw in variants(sh, rnd):
        frames, diag = cut(sh, raw)
        name = raw.stem
        d = W / "cut" / name
        d.mkdir(parents=True, exist_ok=True)
        strip = Image.new("RGBA", (ak.W * 8, ak.H * 2), (222, 226, 214, 255))
        for i, f in enumerate(frames):
            f.save(d / f"frame_{i:02d}.png")
            strip.alpha_composite(g[i], (i * ak.W, 0))
            strip.alpha_composite(f, (i * ak.W, ak.H))
        strip.save(W / f"strip_{name}_1x.png")
        strip.resize((strip.width * 2, strip.height * 2), Image.NEAREST).save(W / f"strip_{name}.png")
        row = {k: diag.get(k) for k in ("k", "dx", "dy", "warnings", "ground_snap", "ref_redraw_dE")}
        row.update(numbers(sh, frames))
        out[name] = row
        print(name, json.dumps(row))
    (W / f"slice_diag_{sh}_{rnd}.json").write_text(json.dumps(out, indent=1))
    return out


def frames_for(anim, variant=None):
    """The canvas frames of one animation from a sheet variant (e.g. 'r1_2')."""
    if anim in ("idle", "flinch"):
        v = variant or PICK[anim]
        fr, diag = cut("A", W / "raw" / f"sheetA_{v}.png")
        if anim == "idle":
            return [ak.base_canvas()] + fr[1:4], diag, f"sheetA_{v}"
        return fr[4:8], diag, f"sheetA_{v}"
    v = variant or PICK["death"]
    fr, diag = cut("B", W / "raw" / f"sheetB_{v}.png")
    f0, _, src0 = frames_for("flinch")
    return [f0[0]] + fr[1:8], diag, f"sheetB_{v} (frame 0 = flinch frame 0 from {src0})"


def write(anim, variant=None):
    frames, diag, src = frames_for(anim, variant)
    man = ak.write_anim(ARM, anim, frames, note=f"G2 {anim}: {src}, repaint of R rig frames")
    print(json.dumps({"written": anim, "src": src, "warnings": man.get("warnings"), "slice": diag.get("warnings"),
                      "ground_snap": diag.get("ground_snap"), "k": diag.get("k")}))


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "template":
        template(sys.argv[2])
    elif cmd == "gen":
        generate(sys.argv[2], sys.argv[3])
    elif cmd == "slice":
        slice_round(sys.argv[2], sys.argv[3])
    elif cmd == "write":
        write(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)
    elif cmd == "all":
        for a in ("idle", "flinch", "death"):
            write(a)
