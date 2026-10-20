"""C - chained per-frame generation: every frame k = 1..N-1 is drawn by its own image call that sees the previous
accepted frame, so the character's consistency is carried forward one frame at a time.

For each k the template is a 2-cell sheet (1536x1024, cells 768x1024, canvas region at 5x, ground row 980):
cell 1 = frame k-1 as accepted (frame 0 = ak.base_canvas()), cell 2 = guides.render_guide(pose_k, "colour").
Images: [template, the true sprite at 5x NEAREST on flat magenta, gen.ROSTER]. n=2 per call, at most 2 calls per
frame. Each variant is sliced with sheet.slice_sheet(raw, geom, n=2, ref_index=0, ref_canvas=prev_frame) and
frame [1] is the candidate. The chosen candidate becomes frame k and the template of frame k+1 is built from it.

  python3 arms/C/make_c.py ref                     # work/C/ref_sprite_5x.png (once)
  python3 arms/C/make_c.py gen strike 3 r1         # one n=2 call for frame 3 -> work/C/raw/strike_f3_r1_{1,2}.png
  python3 arms/C/make_c.py slice strike 3 r1       # cut both variants + review image work/C/review_strike_f3_r1.png
  python3 arms/C/make_c.py accept strike 3 r1_2    # frame 3 := that variant (recorded in arms/C/chain_strike.json)
  python3 arms/C/make_c.py write strike            # ak.write_anim from the accepted chain
  python3 arms/C/make_c.py seq strike              # arms/C/sequence_strike_{3x,1x}.{gif,mp4}
  python3 arms/C/make_c.py rebuild strike          # re-slice every accepted raw in order (reproduces the frames)

Everything after `gen` is reproducible from the raws in work/C/raw/ plus arms/C/chain_strike.json.
"""
import json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import beats, gen, guides, metrics, sheet  # noqa: E402
import numpy as np  # noqa: E402
from PIL import Image, ImageDraw  # noqa: E402

ARM = "C"
TAG = "anim/C"
W = ROOT / "work/C"
A = ROOT / "arms/C"
POSES = json.loads((ROOT / "poses.json").read_text())
REF = W / "ref_sprite_5x.png"
GEOM = sheet.default_geom(cols=2, rows=1, size=(1536, 1024), scale=5, ground_y=980)

# Per-(frame, round) extra sentence, only used when a frame needs a second call (reason recorded next to it).
EXTRA = {
    # r1 (both variants) drew the level blade longer than the blockout's and lower, tip at x 10-12 (outside the safe
    # region x 16-143); hair had grown to ~1.25x the sprite's pixel count over the chain
    ("strike", 3, "r2"): "The blade is exactly as long as the sword in cell 1 and in the original sprite, and its tip "
                         "ends where the blockout's blade tip ends, well inside the cell with clear magenta to its "
                         "left; the arm is raised as in the blockout. Keep the hair exactly as small and neat as in "
                         "the original sprite.",
    # r1 (both variants) again drew the blade longer than the blockout's: tip at x 12-14, outside the safe region
    ("strike", 4, "r2"): "The blade is exactly as long as the sword in the original sprite, no longer, and its tip "
                         "ends where the blockout's blade tip ends, well inside the cell with clear magenta to its "
                         "left. Keep the hair exactly as small and neat as in the original sprite.",
}


def make_ref():
    b = ak.base_canvas()
    bg = Image.new("RGBA", b.size, (255, 0, 255, 255))
    bg.alpha_composite(b)
    bg.resize((b.width * 5, b.height * 5), Image.NEAREST).convert("RGB").save(REF)
    return REF


def chain_path(anim):
    return A / f"chain_{anim}.json"


def load_chain(anim):
    p = chain_path(anim)
    return json.loads(p.read_text()) if p.exists() else {}


def accepted(anim, k):
    """Frame k as accepted (frame 0 = the static sprite)."""
    if k == 0:
        return ak.base_canvas()
    p = W / "accepted" / anim / f"frame_{k:02d}.png"
    if not p.exists():
        raise SystemExit(f"frame {k} of {anim} not accepted yet")
    return Image.open(p).convert("RGBA")


def guide(anim, k):
    return guides.render_guide(POSES["anims"][anim][k], "colour")


def template(anim, k):
    out = W / f"template_{anim}_f{k}.png"
    return sheet.make_template([accepted(anim, k - 1), guide(anim, k)], geom=GEOM, out=out), out


def prompt(anim, k, rnd="r1"):
    b = beats.BEATS[anim]
    n = len(b)
    prev_txt = b[k - 1]["text"]
    if k - 1 == 0:
        prev_txt = "the ready pose, exactly the original sprite"
    p = (
        "The first attached image is a two-cell animation template from my isometric fantasy strategy game, on a flat "
        "magenta background (there are no visible grid lines). "
        f"The left cell (cell 1) is frame {k} of {n} of a {anim.upper()} animation (frame 1 is the ready pose) "
        f"of {gen.CHARACTER}, drawn in {gen.STYLE}, in a 3/4 view facing the lower-left. Cell 1 shows: {prev_txt}. "
        f"The right cell (cell 2) holds a flat-colour blockout mannequin that shows the pose of the NEXT frame, "
        f"frame {k + 1}: {b[k]['text']}. "
        "Draw that next frame in cell 2: the same character as in cell 1, in exactly the pose of the blockout, at the "
        "same position and size in the cell, feet on the same ground line as cell 1. The blockout is only a pose "
        "guide: replace it entirely with fully detailed pixel art; do not keep its flat colours or simplified shapes. "
        "Everything the pose does not change must match cell 1 exactly: the same face and hair, the same outfit and "
        "colours, the same body proportions and size, the same sword and shield, the same pixel size and outline "
        "style, so that cell 1 and cell 2 play back as two consecutive frames of one animation. He faces the "
        "lower-left; the sword stays in his right hand (the far arm, on the screen-left side) and the round shield on "
        "his left arm (on the screen-right side). Leave cell 1 exactly as it is. "
        "The second attached image is the original sprite of this character, enlarged: it is the ground truth for his "
        "design, colours, proportions and the size of his hair, sword and shield. The third attached image is a "
        "roster sheet of other units from the same game: match its outline colour, shading and pixel size. "
        "Each cell holds exactly one figure, fully inside its cell. No grid lines, frames, numbers, text, ground, "
        "shadows, motion lines, smears, sparks or any other effects. Background: flat solid pure magenta #FF00FF "
        "filling the whole image, with no gradient."
    )
    extra = EXTRA.get((anim, k, rnd))
    return p + (" " + extra if extra else "")


def generate(anim, k, rnd):
    if not REF.exists():
        make_ref()
    _, tpl = template(anim, k)
    p = prompt(anim, k, rnd)
    (A / "prompts").mkdir(parents=True, exist_ok=True)
    (A / "prompts" / f"{anim}_f{k}_{rnd}.txt").write_text(p, encoding="utf-8")
    r = gen.edit(tag=TAG, images=[tpl, REF, gen.ROSTER], prompt=p, out=W / f"raw/{anim}_f{k}_{rnd}.png", n=2)
    print(json.dumps({k_: r.get(k_) for k_ in ("ok", "files", "latency", "usd", "dropped_params", "error")}))
    return r


def cut(anim, k, raw):
    prev = accepted(anim, k - 1)
    geom = json.loads((W / f"template_{anim}_f{k}.geom.json").read_text())
    frames, diag = sheet.slice_sheet(raw, geom, n=2, ref_index=0, ref_canvas=prev)
    return frames[1], diag


def guide_iou(f, g):
    return round(float(metrics.iou(metrics.mask(f, 128), metrics.mask(g, 60))), 3)


def stats(anim, k, f, diag):
    prev = accepted(anim, k - 1)
    bb = ak.bbox(f, 40)
    d = metrics.drift([f])[0]
    pm = metrics.part_masks(f)
    pm0 = metrics.part_masks(ak.base_canvas())
    return {"k": diag.get("k"), "dx": diag.get("dx"), "dy": diag.get("dy"), "ref_redraw": diag.get("ref_redraw_dE"),
            "ground_snap": diag.get("ground_snap"), "warnings": diag.get("warnings"), "bbox": bb,
            "height": (bb[3] - bb[1]) if bb else None, "guide_iou": guide_iou(f, guide(anim, k)),
            "drift": d, "tip": metrics.tip_point(f), "pose_tip": POSES["anims"][anim][k].get("tip"),
            "boil_vs_prev": metrics.boil([prev, f])[0],
            "parts_px": {p: int(pm[p].sum()) for p in pm}, "parts_px_sprite": {p: int(pm0[p].sum()) for p in pm0},
            "gates": [g for g in metrics.gates(anim, [ak.base_canvas()] * k + [f]) if g.startswith(f"frame {k}:")]}


def _on(f, bg=(222, 226, 214, 255)):
    c = Image.new("RGBA", f.size, bg)
    c.alpha_composite(f)
    return c


def slice_round(anim, k, rnd):
    raws = sorted((W / "raw").glob(f"{anim}_f{k}_{rnd}_*.png"))
    prev, g = accepted(anim, k - 1), guide(anim, k)
    cands, rows = [], {}
    for raw in raws:
        f, diag = cut(anim, k, raw)
        name = raw.stem
        d = W / "cut" / anim
        d.mkdir(parents=True, exist_ok=True)
        f.save(d / f"{name}.png")
        rows[name] = stats(anim, k, f, diag)
        cands.append((name, f))
        print(name, json.dumps(rows[name]))
    # review: prev | guide | candidates | onion(prev red, cand) at 3x
    tiles = [("prev f%d" % (k - 1), _on(prev)), ("guide f%d" % k, _on(g))]
    for name, f in cands:
        tiles.append((name, _on(f)))
    for name, f in cands:
        on = Image.new("RGBA", f.size, (255, 255, 255, 255))
        pa = np.asarray(prev).astype(np.float32)
        tint = np.zeros_like(pa)
        tint[..., 0] = 230
        tint[..., 1] = 80
        tint[..., 2] = 80
        tint[..., 3] = pa[..., 3] * 0.55
        on.alpha_composite(Image.fromarray(tint.astype(np.uint8), "RGBA"))
        fc = np.asarray(f).copy()
        fc[..., 3] = (fc[..., 3] * 0.75).astype(np.uint8)
        on.alpha_composite(Image.fromarray(fc, "RGBA"))
        tiles.append(("onion " + name.split("_")[-1], on))
    s = 3
    rv = Image.new("RGBA", (len(tiles) * ak.W * s, ak.H * s + 16), (40, 40, 48, 255))
    dr = ImageDraw.Draw(rv)
    for i, (lab, t) in enumerate(tiles):
        rv.alpha_composite(t.resize((ak.W * s, ak.H * s), Image.NEAREST), (i * ak.W * s, 16))
        dr.text((i * ak.W * s + 4, 2), lab, fill=(255, 255, 255, 255))
    rv.save(W / f"review_{anim}_f{k}_{rnd}.png")
    (W / f"slice_diag_{anim}_f{k}_{rnd}.json").write_text(json.dumps(rows, indent=1))
    return rows


def accept(anim, k, variant, why=""):
    raw = W / "raw" / f"{anim}_f{k}_{variant}.png"
    f, diag = cut(anim, k, raw)
    d = W / "accepted" / anim
    d.mkdir(parents=True, exist_ok=True)
    f.save(d / f"frame_{k:02d}.png")
    ch = load_chain(anim)
    ch[str(k)] = {"variant": variant, "raw": raw.name, "why": why, "k": diag.get("k"),
                  "ground_snap": diag.get("ground_snap"), "ref_redraw": diag.get("ref_redraw_dE")}
    # anything accepted after k was built on the old frame k: drop it
    for kk in list(ch):
        if int(kk) > k:
            ch.pop(kk)
            p = d / f"frame_{int(kk):02d}.png"
            if p.exists():
                p.unlink()
    chain_path(anim).write_text(json.dumps(ch, indent=1))
    print(json.dumps({"accepted": k, "variant": variant, "diag_k": diag.get("k"), "snap": diag.get("ground_snap")}))


def rebuild(anim):
    """Re-slice every accepted raw in chain order (frame k's slice uses rebuilt frame k-1 as its reference)."""
    ch = load_chain(anim)
    for k in range(1, beats.COUNTS[anim]):
        if str(k) not in ch:
            break
        template(anim, k)  # template of k is rebuilt from the (rebuilt) frame k-1
        accept(anim, k, ch[str(k)]["variant"], ch[str(k)].get("why", ""))
    chain_path(anim).write_text(json.dumps(ch, indent=1))


def safe_clip(f):
    """Zero the alpha of any pixel outside the safe region x 16..143 (spec). Returns (frame, pixels removed)."""
    a = np.asarray(f).copy()
    out = np.zeros(a.shape[1], bool)
    out[:ak.SAFE_X[0]] = True
    out[ak.SAFE_X[1]:] = True
    n = int((a[:, out, 3] > 0).sum())
    a[:, out] = 0
    return Image.fromarray(a, "RGBA"), n


def write(anim):
    frames, clipped = [], {}
    for k in range(beats.COUNTS[anim]):
        f, n = safe_clip(accepted(anim, k))
        if n:
            clipped[k] = n
        frames.append(f)
    ch = load_chain(anim)
    note = "C %s: chained per-frame generation; " % anim + ", ".join(f"f{k}={v['variant']}" for k, v in ch.items())
    if clipped:
        note += "; safe-region clip (px removed per frame): " + json.dumps(clipped)
        print("safe-region clip", clipped)
    man = ak.write_anim(ARM, anim, frames, note=note)
    print(json.dumps({"written": anim, "warnings": man.get("warnings") if man else None}))


def seq(anim):
    """Full sequence of one animation on a tile: 1 s of the ready pose, the animation (manifest timings), 1 s of the
    ready pose, as GIF + MP4 at 3x (one tile) and 1x (3x3 map, MP4 stored at 2x). The ready pose is the static sprite
    (idle frame 0) because this arm has no idle yet. Same drawing and encoders as preview.py (and G1's seq)."""
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
    cmd = sys.argv[1]
    if cmd == "ref":
        print(make_ref())
    elif cmd == "template":
        print(template(sys.argv[2], int(sys.argv[3]))[1])
    elif cmd == "prompt":
        print(prompt(sys.argv[2], int(sys.argv[3]), sys.argv[4] if len(sys.argv) > 4 else "r1"))
    elif cmd == "gen":
        generate(sys.argv[2], int(sys.argv[3]), sys.argv[4])
    elif cmd == "slice":
        slice_round(sys.argv[2], int(sys.argv[3]), sys.argv[4])
    elif cmd == "accept":
        accept(sys.argv[2], int(sys.argv[3]), sys.argv[4], " ".join(sys.argv[5:]))
    elif cmd == "rebuild":
        rebuild(sys.argv[2])
    elif cmd == "write":
        write(sys.argv[2])
    elif cmd == "seq":
        seq(sys.argv[2])
