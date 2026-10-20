"""Round-2 finish pass, applied uniformly to whichever frames the recipe picks (round-1 arms carry no FX).

  import finish
  f = finish.palette_snap(frame)                 # opaque pixels -> nearest colour the static sprite actually uses
  f = finish.paste_back(frame, base, tol=18)     # pixels that barely changed -> exactly the sprite's pixels
  f = finish.smear(frame, tip_from, tip_to, hilt_from, hilt_to)   # swept-blade arc drawn behind the figure
  f = finish.flash(frame, 0.65)                  # hit flash (normally an engine overlay; baked here for previews)
  f = finish.spark(frame, (x, y), r=6)           # solid-pixel impact star
  frames = finish.apply_strike_fx(frames)        # smear on the swing + contact frames, spark on the hit frame
  frames = finish.apply_hit_fx(frames)           # flash on frame 0 (flinch / death)

Every helper returns a new 160x128 RGBA image and never touches the canvas outside the figure's neighbourhood.
"""
import sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402
import metrics  # noqa: E402

STEEL = [(236, 244, 252), (198, 214, 232), (150, 172, 200)]


def palette_snap(frame, alpha_min=200):
    """Snap opaque pixels to the nearest (OKLab) colour present in the static sprite. Frame 0 maps to itself;
    colours a redraw invented (a shifted blue, a greener tan) are pulled back onto the sprite's own ramps."""
    a = np.asarray(frame.convert("RGBA")).copy()
    sp = np.asarray(ak.sprite()).reshape(-1, 4)
    cols = np.unique(sp[sp[:, 3] > 200][:, :3], axis=0)
    tree = metrics.sprite_tree()
    m = a[..., 3] > alpha_min
    if m.any():
        _, idx = tree.query(metrics.oklab(a[..., :3][m]))
        a[..., :3][m] = cols[idx]
    return Image.fromarray(a, "RGBA")


def paste_back(frame, base=None, tol=18.0):
    """Where a frame matches the base sprite within `tol` (mean abs RGB, alpha-aware), use the base's exact pixels.
    Stops idle 'boiling' when a redraw re-renders unchanged areas slightly differently."""
    base = base or ak.base_canvas()
    f = np.asarray(frame.convert("RGBA")).astype(np.float32)
    b = np.asarray(base).astype(np.float32)
    d = np.abs(f[..., :3] - b[..., :3]).mean(-1) + np.abs(f[..., 3] - b[..., 3]) * 0.5
    keep = d < tol
    keep = ndimage.binary_opening(keep, iterations=1)
    out = f.copy()
    out[keep] = b[keep]
    return Image.fromarray(out.astype(np.uint8), "RGBA")


def blade_ends(frame):
    """(hilt, tip) of the biggest steel blob: tip = farthest from the body centre, hilt = its far end."""
    pm = metrics.part_masks(frame)
    st = pm["steel"]
    if st.sum() < 10:
        return None
    lab, n = ndimage.label(st)
    sizes = ndimage.sum(st, lab, range(1, n + 1))
    blade = lab == (int(np.argmax(sizes)) + 1)
    ys, xs = np.nonzero(blade)
    tip = metrics.tip_point(frame)
    if tip is None:
        return None
    k = int(np.argmax((xs - tip[0]) ** 2 + (ys - tip[1]) ** 2))
    return (int(xs[k]), int(ys[k])), tip


def smear(frame, hilt0, tip0, hilt1, tip1, steps=8, alpha=200, direction="ccw"):
    """Fill the area the blade swept between two frames with steel tones, BEHIND the figure. The tip travels on an
    arc around the moving hilt (angle interpolated in `direction`: "ccw" = counter-clockwise on screen, the way a
    lower-left-facing overhead slash turns; "cw"; or "short" for the smaller turn)."""
    import math
    a0 = math.atan2(-(tip0[1] - hilt0[1]), tip0[0] - hilt0[0])
    a1 = math.atan2(-(tip1[1] - hilt1[1]), tip1[0] - hilt1[0])
    L0, L1 = math.dist(hilt0, tip0), math.dist(hilt1, tip1)
    da = a1 - a0
    if direction == "ccw":
        da = da % (2 * math.pi)
    elif direction == "cw":
        da = -((-da) % (2 * math.pi))
    else:
        da = (da + math.pi) % (2 * math.pi) - math.pi
    lay = Image.new("RGBA", frame.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(lay)
    pts_tip, pts_mid, pts_hilt = [], [], []
    for k in range(steps + 1):
        t = k / steps
        h = (hilt0[0] + (hilt1[0] - hilt0[0]) * t, hilt0[1] + (hilt1[1] - hilt0[1]) * t)
        a, L = a0 + da * t, L0 + (L1 - L0) * t
        pts_hilt.append(h)
        pts_tip.append((h[0] + L * math.cos(a), h[1] - L * math.sin(a)))
        pts_mid.append((h[0] + 0.55 * L * math.cos(a), h[1] - 0.55 * L * math.sin(a)))
    d.polygon(pts_tip + pts_mid[::-1], fill=STEEL[0] + (alpha,))
    d.polygon(pts_mid + pts_hilt[::-1], fill=STEEL[1] + (int(alpha * 0.55),))
    d.line(pts_tip, fill=STEEL[2] + (255,), width=1)
    out = lay
    out.alpha_composite(frame)
    return out


def flash(frame, amount=0.65, colour=(255, 255, 255)):
    a = np.asarray(frame.convert("RGBA")).astype(np.float32)
    a[..., :3] = a[..., :3] * (1 - amount) + np.array(colour, np.float32) * amount
    return Image.fromarray(a.astype(np.uint8), "RGBA")


def spark(frame, at, r=6, colour=(255, 246, 196), core=(255, 255, 255)):
    out = frame.copy()
    d = ImageDraw.Draw(out)
    x, y = at
    for dx, dy, L in [(1, 0, r), (-1, 0, r), (0, 1, r), (0, -1, r), (1, 1, r * 0.6), (-1, -1, r * 0.6), (1, -1, r * 0.6), (-1, 1, r * 0.6)]:
        d.line([(x, y), (x + dx * L, y + dy * L)], fill=colour + (255,), width=2)
    d.rectangle([x - 1, y - 1, x + 1, y + 1], fill=core + (255,))
    return out


def apply_strike_fx(frames, swing=3, contact=4, direction="ccw"):
    """Smear on the swing frame (from the wind-up blade to the swing blade) and on contact (swing -> contact);
    impact spark at the contact frame's blade tip."""
    out = list(frames)
    for i in (swing, contact):
        a, b = blade_ends(frames[i - 1]), blade_ends(frames[i])
        if a and b:
            out[i] = smear(frames[i], a[0], a[1], b[0], b[1], alpha=150 if i == contact else 210, direction=direction)
    be = blade_ends(frames[contact])
    if be:
        out[contact] = spark(out[contact], be[1], r=7)
    return out


def apply_hit_fx(frames, i=0, amount=0.6):
    out = list(frames)
    out[i] = flash(frames[i], amount)
    return out
