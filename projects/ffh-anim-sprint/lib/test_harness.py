"""Harness self-tests (no API): template -> fake model answer (scaled 1.05 about the ground, shifted) -> slice back.

  python lib/test_harness.py
"""
import sys
from pathlib import Path
import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402
import sheet  # noqa: E402
import metrics  # noqa: E402


def fake_model(template_path, geom, k=1.05, dx=6, dy=-4):
    """Scale every cell by k about its ground anchor and shift by (dx, dy): what a model that redraws at a slightly
    different size would return."""
    t = Image.open(template_path).convert("RGBA")
    out = Image.new("RGBA", t.size, (255, 0, 255, 255))
    s0 = geom["scale"]
    gx = geom["inner"][0] + (ak.ANCHOR[0] - geom["region"][0]) * s0
    gy = geom["inner"][1] + (ak.ANCHOR[1] - geom["region"][1]) * s0
    for c in geom["cells"]:
        cell = t.crop((c["x"], c["y"], c["x"] + c["w"], c["y"] + c["h"]))
        big = cell.resize((round(c["w"] * k), round(c["h"] * k)), Image.NEAREST)
        ox = c["x"] + gx - gx * k + dx
        oy = c["y"] + gy - gy * k + dy
        tmp = Image.new("RGBA", t.size, (0, 0, 0, 0))
        tmp.paste(big, (int(round(ox)), int(round(oy))))
        box = (c["x"], c["y"], c["x"] + c["w"], c["y"] + c["h"])
        out.paste(tmp.crop(box), box[:2], tmp.crop(box))
    return out


def main():
    frames, _ = ak.load_anim("A0", "strike")
    geom = sheet.make_template(frames, out=ak.ROOT / "work/test/template.png")
    fake = fake_model(ak.ROOT / "work/test/template.png", geom)
    fake.convert("RGB").save(ak.ROOT / "work/test/fake.png")
    got, diag = sheet.slice_sheet(ak.ROOT / "work/test/fake.png", geom, n=8)
    print("calibration", {k: diag[k] for k in ("k", "dx", "dy")}, "warnings", diag["warnings"], "ref", diag.get("ref_redraw_dE"))
    worst = 0
    for i, (a, b) in enumerate(zip(frames, got)):
        ma, mb = metrics.mask(a), metrics.mask(b)
        ba, bb = ak.bbox(a, 128), ak.bbox(b, 128)
        off = max(abs(x - y) for x, y in zip(ba, bb))
        worst = max(worst, off)
        print(f"cell {i}: IoU {metrics.iou(ma, mb):.3f} bbox {ba} -> {bb} (max edge offset {off}px)")
    assert abs(diag["k"] - 1.05) < 0.03, diag
    assert worst <= 2, worst
    # frame-0 contract
    f0, _ = ak.load_anim("A0", "idle")
    assert np.array_equal(np.asarray(f0[0]), np.asarray(ak.base_canvas())), "idle frame 0 must equal the sprite"
    print("OK")


if __name__ == "__main__":
    main()
