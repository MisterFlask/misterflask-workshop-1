"""Rig helpers: rigid moving-least-squares (MLS) mesh-free warp driven by poses.json joints, plus part utilities.

  import rig
  f = rig.warp_to_pose(ak.base_canvas(), POSE_REST, POSE_TARGET)        # bends the whole sprite, feet pinned
  f = rig.mls_warp(img, src_pts, dst_pts)                               # generic: src points move to dst points

MLS (Schaefer et al. 2006, rigid variant) deforms the image so each control point lands on its target while
everything in between moves as rigidly as possible. It bends the body without cutting it into parts, so there
are no hidden regions to fill; it suits subtle motion (breathing, recoil). Big rotations (a sword swinging over
the head) tear or smear it - that is what cut-out parts (part_layer + ak.transform) are for.
The warp is computed backward (for each output pixel, where does it come from) by running MLS from the target
points to the source points, then sampling the premultiplied source bilinearly.
"""
import json
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402

JOINTS = ["head", "neck", "chest", "pelvis", "r_sh", "r_el", "r_ha", "tip", "l_sh", "l_el", "l_ha",
          "r_hip", "r_kn", "r_ft", "l_hip", "l_kn", "l_ft"]
BONES = [("head", "neck"), ("neck", "chest"), ("chest", "pelvis"), ("r_sh", "r_el"), ("r_el", "r_ha"), ("r_ha", "tip"),
         ("l_sh", "l_el"), ("l_el", "l_ha"), ("r_hip", "r_kn"), ("r_kn", "r_ft"), ("l_hip", "l_kn"), ("l_kn", "l_ft")]


def mls_map(shape, p, q, alpha=1.0):
    """Rigid MLS evaluated at every pixel of `shape` (h, w), mapping points near p to near q. Returns (h, w, 2) xy."""
    h, w = shape
    ys, xs = np.mgrid[0:h, 0:w]
    v = np.stack([xs.ravel(), ys.ravel()], 1).astype(np.float64)
    p = np.asarray(p, np.float64)
    q = np.asarray(q, np.float64)
    d2 = ((v[:, None, :] - p[None]) ** 2).sum(-1)
    wgt = 1.0 / (d2 ** alpha + 1e-6)
    sw = wgt.sum(1, keepdims=True)
    ps = (wgt @ p) / sw
    qs = (wgt @ q) / sw
    ph = p[None] - ps[:, None]
    qh = q[None] - qs[:, None]
    d = v - ps
    a = ph[..., 0] * d[:, None, 0] + ph[..., 1] * d[:, None, 1]
    b = ph[..., 0] * d[:, None, 1] - ph[..., 1] * d[:, None, 0]
    fx = (wgt * (qh[..., 0] * a - qh[..., 1] * b)).sum(1)
    fy = (wgt * (qh[..., 0] * b + qh[..., 1] * a)).sum(1)
    norm = np.hypot(fx, fy) + 1e-9
    dl = np.hypot(d[:, 0], d[:, 1])
    out = np.stack([dl * fx / norm, dl * fy / norm], 1) + qs
    hit = d2.min(1) < 1e-6  # exactly on a control point
    if hit.any():
        out[hit] = q[d2[hit].argmin(1)]
    return out.reshape(h, w, 2)


def mls_warp(im, src_pts, dst_pts, alpha=1.0, order=1):
    """Warp `im` so src_pts move to dst_pts (backward-mapped)."""
    a = np.asarray(im.convert("RGBA")).astype(np.float32)
    pm = np.dstack([a[..., :3] * a[..., 3:4] / 255.0, a[..., 3]])
    src = mls_map(a.shape[:2], dst_pts, src_pts, alpha)  # output pixel -> source location
    coords = [src[..., 1], src[..., 0]]
    out = np.dstack([ndimage.map_coordinates(pm[..., c], coords, order=order, mode="constant", cval=0.0)
                     for c in range(4)])
    A = np.clip(out[..., 3], 0, 255)
    rgb = np.clip(out[..., :3] / np.maximum(A[..., None] / 255.0, 1e-4), 0, 255)
    res = np.dstack([rgb, A])
    res[A < 2] = 0
    return Image.fromarray(res.astype(np.uint8), "RGBA")


def pose_points(pose, joints=JOINTS, bone_samples=1):
    pts = [tuple(pose[j][:2]) for j in joints if j in pose]
    for a, b in BONES:
        if a in pose and b in pose and a in joints and b in joints:
            for k in range(1, bone_samples + 1):
                t = k / (bone_samples + 1)
                pts.append((pose[a][0] + (pose[b][0] - pose[a][0]) * t, pose[a][1] + (pose[b][1] - pose[a][1]) * t))
    if "shield" in pose:
        cx, cy, rx, ry = pose["shield"]
        pts += [(cx, cy), (cx, cy - ry), (cx, cy + ry)]
    return pts


def warp_to_pose(im, pose_src, pose_dst, joints=JOINTS, extra_src=(), extra_dst=(), bone_samples=1):
    """Bend `im` (drawn in pose_src) toward pose_dst using every joint (+ bone midpoints) as an MLS handle."""
    s = pose_points(pose_src, joints, bone_samples) + list(extra_src)
    d = pose_points(pose_dst, joints, bone_samples) + list(extra_dst)
    return mls_warp(im, s, d)


# ---------------------------------------------------------------- cut-out parts
def part_layer(im, mask):
    """Copy of `im` keeping only pixels where mask (bool HxW or L image) is set."""
    a = np.asarray(im.convert("RGBA")).copy()
    m = np.asarray(mask) > 0 if not isinstance(mask, np.ndarray) else mask
    a[~m] = 0
    return Image.fromarray(a, "RGBA")


def polygon_mask(size, polys, feather=0):
    m = Image.new("L", size, 0)
    d = ImageDraw.Draw(m)
    for poly in polys:
        d.polygon([tuple(p) for p in poly], fill=255)
    return np.asarray(m) > 0


def place_part(canvas, part, pivot_src, pivot_dst, angle=0.0, sx=1.0, sy=1.0):
    """Rotate/scale a part layer about pivot_src, move pivot_src to pivot_dst, composite onto canvas (in place)."""
    t = ak.transform(part, angle=angle, sx=sx, sy=sy, pivot=pivot_src,
                     dx=pivot_dst[0] - pivot_src[0], dy=pivot_dst[1] - pivot_src[1])
    canvas.alpha_composite(t)
    return canvas


def angle_of(a, b):
    """Screen angle (deg, counter-clockwise positive, y down) of the vector a->b."""
    return float(np.degrees(np.arctan2(-(b[1] - a[1]), b[0] - a[0])))
