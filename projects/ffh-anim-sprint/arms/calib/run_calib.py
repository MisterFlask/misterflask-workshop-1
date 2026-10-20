"""P1 calibration: one strike sheet (G1 layout) across models / guide styles / fidelity / quality.

  python arms/calib/run_calib.py templates      # build templates only
  python arms/calib/run_calib.py round1         # sunburst n=2, flare, gpt-image-2, sunburst-grey, sunburst-fidelity
  python arms/calib/run_calib.py high --model X # X at high quality
  python arms/calib/run_calib.py slice          # slice every raw into arms/calib/<run>/ + review sheet
"""
import json, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import beats, gen, guides, sheet  # noqa: E402

W = ROOT / "work/calib"
POSES = json.loads((ROOT / "poses.json").read_text())


def templates():
    out = {}
    for style in ("colour", "grey"):
        cells = [ak.base_canvas()] + [guides.render_guide(p, style) for p in POSES["anims"]["strike"][1:]]
        out[style] = sheet.make_template(cells, out=W / f"template_strike_{style}.png")
    return out


def prompt(style):
    texts = [(i + 1, beats.BEATS["strike"][i]["text"]) for i in range(1, 8)]
    return gen.sheet_prompt("strike", texts, guide=style)


def round1():
    templates()
    t = lambda s: W / f"template_strike_{s}.png"
    calls = [
        dict(tag="anim/calib", images=[t("colour"), gen.ROSTER], prompt=prompt("colour"), out=W / "raw/sunburst.png", n=2),
        dict(tag="anim/calib", images=[t("colour"), gen.ROSTER], prompt=prompt("colour"), out=W / "raw/flare.png", n=1,
             model="gpt-image-2.5-flare"),
        dict(tag="anim/calib", images=[t("colour"), gen.ROSTER], prompt=prompt("colour"), out=W / "raw/gi2.png", n=1,
             model="gpt-image-2"),
        dict(tag="anim/calib", images=[t("grey"), gen.ROSTER], prompt=prompt("grey"), out=W / "raw/sunburst_grey.png", n=1),
        dict(tag="anim/calib", images=[t("colour"), gen.ROSTER], prompt=prompt("colour"), out=W / "raw/sunburst_fid.png",
             n=1, input_fidelity="high"),
    ]
    t0 = time.time()
    res = gen.parallel(calls, workers=5)
    for r in res:
        print(json.dumps({k: r.get(k) for k in ("ok", "files", "latency", "usd", "dropped_params", "error")}))
    print("wall", round(time.time() - t0, 1))


def high(model):
    templates()
    r = gen.edit(tag="anim/calib", images=[W / "template_strike_colour.png", gen.ROSTER], prompt=prompt("colour"),
                 out=W / f"raw/{model.split('-')[-1]}_high.png", n=1, model=model, quality="high")
    print(json.dumps({k: r.get(k) for k in ("ok", "files", "latency", "usd", "dropped_params", "error")}))


def slice_all():
    geom = json.loads((W / "template_strike_colour.geom.json").read_text())
    rows = {}
    for raw in sorted((W / "raw").glob("*.png")):
        frames, diag = sheet.slice_sheet(raw, geom, n=8)
        name = raw.stem
        d = W / "cut" / name
        d.mkdir(parents=True, exist_ok=True)
        strip = ak.Image.new("RGBA", (ak.W * 8, ak.H), (222, 226, 214, 255)) if hasattr(ak, "Image") else None
        from PIL import Image
        strip = Image.new("RGBA", (ak.W * 8, ak.H), (222, 226, 214, 255))
        for i, f in enumerate(frames):
            f.save(d / f"frame_{i:02d}.png")
            strip.alpha_composite(f, (i * ak.W, 0))
        strip.resize((strip.width * 2, strip.height * 2), Image.NEAREST).save(W / f"strip_{name}.png")
        rows[name] = {k: diag[k] for k in ("k", "dx", "dy", "warnings")} | {"ref": diag.get("ref_redraw_dE")}
        print(name, json.dumps(rows[name]))
    (W / "slice_diag.json").write_text(json.dumps(rows, indent=1))


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "templates":
        templates()
    elif cmd == "round1":
        round1()
    elif cmd == "high":
        high(sys.argv[sys.argv.index("--model") + 1])
    elif cmd == "slice":
        slice_all()
