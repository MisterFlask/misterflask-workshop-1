"""Magenta-fringe cleanup and anchor re-fit for M7 sprites.

The model's background is near-#FF00FF; keying leaves a few pink pixels around glows and thin parts.
The sprint learned (the hard way) that any GLOBAL hue rule also eats legitimate purple/pink art
(a lich's robe, a mountain's purple shadows). So every rule here is restricted to:
  - semi-transparent pixels (alpha < 200), or
  - near-pure #FF00FF, or
  - opaque pixels within 3 px of transparency (edge-only recolour).

clean(img, kind) -> img
  unit     : remove semi-transparent magenta-ish fringe + near-pure magenta   (sprint: demagenta2.py)
  building : same removal + edge-only recolour of opaque magenta-ish pixels to a neutral dark
             purple-brown, then alpha snap (<70 -> 0, >180 -> 255)            (sprint: buildings demagenta(), made edge-only)
  feature  : same removal + edge-only recolour to `edge_colour` (default dark green (60,96,36))
             (sprint: terrain edge-only rule)
refit(img, kind) re-trims and re-places on the spec canvas so the lowest opaque pixel touches the anchor
(the sprint's building cleanup could leave empty bottom rows -> sprite floating above its footprint).
"""
import numpy as np
from PIL import Image
from scipy import ndimage


def _rgba(img):
    return np.asarray(img.convert("RGBA")).astype(np.int32).copy()


def clean(img, kind="unit", edge_colour=(60, 96, 36)):
    a = _rgba(img)
    r, g, b, al = a[..., 0], a[..., 1], a[..., 2], a[..., 3]
    magish = (r > 120) & (b > 110) & (g < np.minimum(r, b) - 40) & (al > 0)
    pure = (r > 215) & (b > 215) & (g < 90)
    kill = (magish & (al < 200)) | (pure & (al > 0))
    a[kill, 3] = 0
    stats = {"removed": int(kill.sum()), "recoloured": 0}
    if kind in ("building", "feature"):
        al = a[..., 3]
        near_edge = ndimage.binary_dilation(al < 200, iterations=3)
        if kind == "building":
            m = (al > 0) & (r > 150) & (b > 120) & (g < 0.6 * np.minimum(r, b)) & near_edge
            a[m, 0] = np.clip(g[m] + 40, 0, 255); a[m, 2] = np.clip(g[m] + 30, 0, 255)
        else:
            m = (al > 0) & (r > g + 45) & (b > g + 45) & near_edge
            a[m, :3] = edge_colour
        stats["recoloured"] = int(m.sum())
        if kind == "building":
            al = a[..., 3]
            al[al < 70] = 0; al[al > 180] = 255
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8), "RGBA"), stats


def refit(img, kind, canvas_w=None):
    """Trim to alpha>8 and re-place bottom-centre on the spec canvas (unit: >=96x120, 2 px bottom pad;
    building: >=192 wide; city: 384 wide; feature: >=192 wide)."""
    bb = img.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
    if not bb:
        return img
    s = img.crop(bb)
    if kind == "unit":
        W, H, pad = max(96, s.width), max(120, s.height + 2), 2
    elif kind == "city":
        W, H, pad = max(384, s.width), s.height, 0
    else:
        W, H, pad = max(canvas_w or 192, s.width), s.height, 0
    c = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    c.alpha_composite(s, ((W - s.width) // 2, H - s.height - pad))
    return c


def magenta_report(img):
    a = _rgba(img)
    r, g, b, al = a[..., 0], a[..., 1], a[..., 2], a[..., 3]
    return int(((al > 0) & (r > g + 50) & (b > g + 50)).sum())
