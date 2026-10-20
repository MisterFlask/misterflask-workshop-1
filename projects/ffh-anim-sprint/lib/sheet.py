"""Template sheets for the image model, and the slicer that turns its answer back into canvas frames.

Layout (default): 1536x1024, 4 columns x 2 rows of 384x512 cells, NO grid lines (cutout.remove_bg floods from
the image border only, so lines would turn every cell into an enclosed pocket). Each cell shows the canvas region
x 16..144, y 0..128 at 3x (384x384), with canvas y=126 (the anchor/ground) on cell row GROUND_Y=500. Unused
pixels are pure #FF00FF. Read order is row-major; cell 0 normally holds the true sprite (the reference).

  geom = make_template([ak.base_canvas(), guide1, guide2, ...], out="work/x/template.png")
  frames, diag = slice_sheet("work/x/raw.png", geom, n=8)       # frames[i] = 160x128 canvas for cell i

slice_sheet keys the whole sheet once, assigns connected components to cells by centroid (small bits such as
sword tips join the nearest big figure), estimates ONE scale + offset for the whole sheet from the model's
redraw of the reference cell, and maps every cell into canvas coordinates with it. It never normalises a
frame's own height. Each non-reference frame is then snapped so its lowest opaque row is on the ground row (every
beat has feet, knees or a lying body on the ground; models drift whole rows up or down by up to ~15 px). diag has the
calibration, ground snaps, per-cell warnings and the reference-redraw colour error.
"""
import json, sys
from pathlib import Path
import numpy as np
from PIL import Image
from scipy import ndimage

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402
import cutout as co  # noqa: E402  (m7/lib, path set by animkit)

MAGENTA = (255, 0, 255, 255)
GROUND_Y = 500
SCALE = 3
REGION = (16, 0, 144, 128)


def default_geom(cols=4, rows=2, size=(1536, 1024), scale=SCALE, ground_y=GROUND_Y, region=REGION):
    cw, ch = size[0] // cols, size[1] // rows
    rw, rh = (region[2] - region[0]) * scale, (region[3] - region[1]) * scale
    ox_in = (cw - rw) // 2
    oy_in = ground_y - (ak.ANCHOR[1] - region[1]) * scale  # cell y of canvas row region[1]
    cells = []
    for i in range(cols * rows):
        r, c = divmod(i, cols)
        cells.append({"i": i, "x": c * cw, "y": r * ch, "w": cw, "h": ch})
    return {"size": list(size), "cols": cols, "rows": rows, "cell": [cw, ch], "scale": scale, "region": list(region),
            "ground_y": ground_y, "inner": [ox_in, oy_in], "cells": cells}


def canvas_to_sheet(geom, i, x, y):
    c = geom["cells"][i]
    s = geom["scale"]
    return (c["x"] + geom["inner"][0] + (x - geom["region"][0]) * s,
            c["y"] + geom["inner"][1] + (y - geom["region"][1]) * s)


def make_template(cells, out=None, geom=None, bg=MAGENTA, **kw):
    """cells: list of 160x128 RGBA canvases (or None for an empty cell). Returns geom (also saved next to out)."""
    geom = geom or default_geom(**kw)
    s = geom["scale"]
    x0, y0, x1, y1 = geom["region"]
    sheet = Image.new("RGBA", tuple(geom["size"]), bg)
    for i, cv in enumerate(cells):
        if cv is None:
            continue
        crop = cv.crop((x0, y0, x1, y1))
        up = crop.resize((crop.width * s, crop.height * s), Image.NEAREST)
        sx, sy = canvas_to_sheet(geom, i, x0, y0)
        sheet.alpha_composite(up, (int(sx), int(sy)))
    if out:
        out = Path(out)
        out.parent.mkdir(parents=True, exist_ok=True)
        sheet.convert("RGB").save(out)
        geom = dict(geom, template=str(out))
        out.with_suffix(".geom.json").write_text(json.dumps(geom, indent=1))
    return geom


def _ref_box(geom, i, ref_canvas):
    """Where the reference figure sits in sheet coordinates (bbox of its opaque pixels)."""
    bb = ak.bbox(ref_canvas, 60)
    a = canvas_to_sheet(geom, i, bb[0], bb[1])
    b = canvas_to_sheet(geom, i, bb[2], bb[3])
    return (a[0], a[1], b[0], b[1])


def key_sheet(raw, key="#ff00ff", tol=48.0, holes=30):
    im = Image.open(raw).convert("RGBA") if not isinstance(raw, Image.Image) else raw.convert("RGBA")
    st = {}
    keyed = co.remove_bg(im, key, tol, holes, st)
    return keyed, st


def assign_components(keyed, geom, min_big=600):
    a = np.asarray(keyed)
    solid = a[..., 3] > 40
    lab, n = ndimage.label(solid, structure=np.ones((3, 3)))
    if n == 0:
        return lab, {}, []
    idx = np.arange(1, n + 1)
    sizes = ndimage.sum(solid, lab, idx)
    cents = ndimage.center_of_mass(solid, lab, idx)
    objs = ndimage.find_objects(lab)
    cw, ch = geom["cell"]
    owner, warns = {}, []
    big = [k for k in range(n) if sizes[k] >= min_big]

    def cell_of(y, x):
        c = min(geom["cols"] - 1, max(0, int(x // cw)))
        r = min(geom["rows"] - 1, max(0, int(y // ch)))
        return r * geom["cols"] + c

    for k in big:
        cy, cx = cents[k]
        owner[k + 1] = cell_of(cy, cx)
        sl = objs[k]
        c = geom["cells"][owner[k + 1]]
        if sl[1].start < c["x"] - 8 or sl[1].stop > c["x"] + c["w"] + 8 or sl[0].start < c["y"] - 8 or sl[0].stop > c["y"] + c["h"] + 8:
            warns.append(f"component {k + 1} ({int(sizes[k])} px) of cell {owner[k + 1]} crosses its cell border")
    for k in range(n):
        if k in big or sizes[k] < 25:
            continue
        cy, cx = cents[k]
        # small bit: give it to the nearest big component (by bbox distance), else to its centroid's cell
        best, bd = None, 1e9
        for b in big:
            sl = objs[b]
            dy = max(sl[0].start - cy, 0, cy - sl[0].stop)
            dx = max(sl[1].start - cx, 0, cx - sl[1].stop)
            dd = (dx * dx + dy * dy) ** 0.5
            if dd < bd:
                best, bd = b, dd
        owner[k + 1] = owner[best + 1] if best is not None and bd < 40 else cell_of(cy, cx)
    return lab, owner, warns


def slice_sheet(raw, geom, n=None, ref_index=0, ref_canvas=None, calibrate=True, holes=30, return_redraw=False,
                ground_snap=20):
    """Returns (frames, diag). frames[i] is the 160x128 canvas for cell i (empty canvas if the cell had nothing)."""
    geom = json.loads(Path(geom).read_text()) if isinstance(geom, (str, Path)) else geom
    n = n or geom["cols"] * geom["rows"]
    im = Image.open(raw).convert("RGBA") if not isinstance(raw, Image.Image) else raw.convert("RGBA")
    if list(im.size) != geom["size"]:
        im = im.resize(tuple(geom["size"]), Image.LANCZOS)
    keyed, kst = key_sheet(im, holes=holes)
    lab, owner, warns = assign_components(keyed, geom)
    ka = np.asarray(keyed)
    layers = []
    for i in range(n):
        ids = [k for k, c in owner.items() if c == i]
        m = np.isin(lab, ids) if ids else np.zeros(lab.shape, bool)
        arr = ka.copy()
        arr[~m] = 0
        layers.append(Image.fromarray(arr, "RGBA"))
        if len([k for k in ids if (lab == k).sum() >= 600]) > 1:
            warns.append(f"cell {i} has more than one large figure")
    diag = {"key_stats": kst, "warnings": warns, "k": 1.0, "dx": 0.0, "dy": 0.0}
    s0 = geom["scale"]
    ref_canvas = ref_canvas or ak.base_canvas()
    gx = geom["inner"][0] + (ak.ANCHOR[0] - geom["region"][0]) * s0   # cell-local ground anchor
    gy = geom["inner"][1] + (ak.ANCHOR[1] - geom["region"][1]) * s0
    if calibrate and ref_index is not None:
        bb = ak.bbox(layers[ref_index], 60)
        if bb:
            c = geom["cells"][ref_index]
            exp = _ref_box(geom, ref_index, ref_canvas)
            exp_h = exp[3] - exp[1]
            k = (bb[3] - bb[1]) / exp_h
            # model drew the cell scaled by k about the ground anchor, then shifted by (dx, dy)
            ex_cx = (exp[0] + exp[2]) / 2 - c["x"]
            md_cx = (bb[0] + bb[2]) / 2 - c["x"]
            dx = md_cx - (gx + k * (ex_cx - gx))
            dy = (bb[3] - c["y"]) - (gy + k * ((exp[3] - c["y"]) - gy))
            diag.update(k=round(k, 4), dx=round(dx, 2), dy=round(dy, 2))
            if not 0.85 <= k <= 1.15:
                warns.append(f"reference redraw scale k={k:.3f} is outside 0.85-1.15")
        else:
            warns.append("reference cell is empty after keying")
    k, dx, dy = diag["k"], diag["dx"], diag["dy"]
    frames = []
    for i in range(n):
        c = geom["cells"][i]
        lay = layers[i]
        bb = ak.bbox(lay, 8)
        canvas = Image.new("RGBA", (ak.W, ak.H), (0, 0, 0, 0))
        if bb:
            crop = lay.crop(bb)
            s = 1.0 / (s0 * k)
            small = ak.resize(crop, (max(1, round(crop.width * s)), max(1, round(crop.height * s))))
            # sheet point (X, Y) -> cell-local (u, v) -> true cell-local -> canvas
            u0, v0 = bb[0] - c["x"], bb[1] - c["y"]
            ut = gx + (u0 - gx - dx) / k
            vt = gy + (v0 - gy - dy) / k
            cx = geom["region"][0] + (ut - geom["inner"][0]) / s0
            cy = geom["region"][1] + (vt - geom["inner"][1]) / s0
            canvas.alpha_composite(small, (int(round(cx)), int(round(cy))))
        if ground_snap and i != ref_index:
            # every beat keeps something on the ground (feet, knees or the lying body), so the lowest opaque row
            # belongs on the ground row; models sometimes draw a whole row of cells a few px high or low
            canvas, sdy = ak.snap_ground(canvas, max_px=ground_snap)
            if sdy:
                diag.setdefault("ground_snap", {})[i] = sdy
        frames.append(canvas)
    if calibrate and ref_index is not None:
        diag["ref_redraw_dE"] = ref_redraw_error(frames[ref_index], ref_canvas)
    if return_redraw:
        return frames, diag, layers
    return frames, diag


def ref_redraw_error(redraw, ref):
    """Mean OKLab distance (x100) between the model's redraw of the reference and the true sprite, over pixels
    opaque in both, plus silhouette IoU. Predicts a 'pop' between frame 0 and frame 1."""
    import metrics
    a = np.asarray(redraw).astype(np.float32)
    b = np.asarray(ref).astype(np.float32)
    ma, mb = a[..., 3] > 128, b[..., 3] > 128
    both = ma & mb
    iou = both.sum() / max(1, (ma | mb).sum())
    if both.sum() == 0:
        return {"dE": None, "iou": round(float(iou), 3)}
    la, lb = metrics.oklab(a[..., :3][both]), metrics.oklab(b[..., :3][both])
    return {"dE": round(float(np.linalg.norm(la - lb, axis=1).mean() * 100), 2), "iou": round(float(iou), 3)}
