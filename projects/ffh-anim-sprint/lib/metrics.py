"""Animation metrics and gates. Each metric answers one question (see spec/plan):

  nonrigid[i]  is frame i just frame 0 moved?   1 - best silhouette IoU after a whole-sprite fit (rotate/scale/shear/shift)
  overlap      does everything move in lockstep?  spread (std, frames) of the peak-displacement frame of tracked parts
  arc          sword-tip trail (steel pixel farthest from the body centre) + spacing between frames
  drift_dE     did colours drift?                OKLab distance (x100) of opaque pixels to the static sprite's colours
  jitter       do planted feet slide?            std of the lowest opaque row / foot x for idle & flinch
  boil         frame-to-frame change             mean alpha-weighted RGB change between consecutive frames
  gates        facing flip / hand swap / missing sword (heuristic flags, confirmed by eye)

  python lib/metrics.py --arm G1 [--anim strike]   # writes arms/<arm>/metrics.json
"""
import argparse, json, sys
from pathlib import Path
import numpy as np
from PIL import Image
from scipy import ndimage
from scipy.spatial import cKDTree

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402

ANIMS = ["idle", "strike", "flinch", "death"]


def oklab(rgb):
    c = np.asarray(rgb, np.float32) / 255.0
    c = np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    l = 0.4122214708 * c[..., 0] + 0.5363325363 * c[..., 1] + 0.0514459929 * c[..., 2]
    m = 0.2119034982 * c[..., 0] + 0.6806995451 * c[..., 1] + 0.1073969566 * c[..., 2]
    s = 0.0883024619 * c[..., 0] + 0.2817188376 * c[..., 1] + 0.6299787005 * c[..., 2]
    l, m, s = np.cbrt(l), np.cbrt(m), np.cbrt(s)
    return np.stack([0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
                     1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
                     0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s], -1)


_TREE = None


def sprite_tree():
    global _TREE
    if _TREE is None:
        a = np.asarray(ak.sprite()).reshape(-1, 4)
        cols = np.unique(a[a[:, 3] > 200][:, :3], axis=0)
        _TREE = cKDTree(oklab(cols))
    return _TREE


def mask(im, thr=128):
    return np.asarray(im)[..., 3] > thr


def iou(a, b):
    u = (a | b).sum()
    return (a & b).sum() / u if u else 1.0


def _affine_fit(m0, m1, angles, scales, shears, shift=1):
    """Best IoU of m1 against whole-sprite transforms of m0 (rotation, uniform scale, x-shear; centroids aligned,
    +-shift px search). A frame that is just frame 0 moved/rotated/leaned scores ~1, i.e. nonrigid ~0."""
    if m0.sum() == 0 or m1.sum() == 0:
        return 0.0
    h, w = m1.shape
    c0 = np.array(ndimage.center_of_mass(m0))[::-1]  # x, y
    c1 = np.array(ndimage.center_of_mass(m1))[::-1]
    src = Image.fromarray((m0 * 255).astype(np.uint8))
    best = 0.0
    for a in angles:
        t = np.radians(a)
        R = np.array([[np.cos(t), np.sin(t)], [-np.sin(t), np.cos(t)]])
        for sc in scales:
            for sh in shears:
                M = R @ np.array([[1, sh], [0, 1]]) * sc
                Mi = np.linalg.inv(M)
                # out = M (in - c0) + c1  ->  in = Mi (out - c1) + c0
                off = c0 - Mi @ c1
                m = np.asarray(src.transform((w, h), Image.AFFINE, (Mi[0, 0], Mi[0, 1], off[0], Mi[1, 0], Mi[1, 1], off[1]),
                                             resample=Image.BILINEAR)) > 127
                for dy in range(-shift, shift + 1):
                    for dx in range(-shift, shift + 1):
                        v = iou(np.roll(np.roll(m, dy, 0), dx, 1), m1)
                        if v > best:
                            best = v
    return best


def nonrigid(frames, wide=False):
    m0 = mask(frames[0])
    angles = list(range(-100, 101, 4)) if wide else list(range(-30, 31, 2))
    scales = [0.95, 1.0, 1.05]
    shears = [-0.1, 0.0, 0.1] if wide else [-0.16, -0.08, 0.0, 0.08, 0.16]
    return [round(1.0 - _affine_fit(m0, mask(f), angles, scales, shears), 3) for f in frames]


# ---------------------------------------------------------------- colour-tracked parts
def part_masks(im):
    a = np.asarray(im).astype(np.int32)
    r, g, b, al = a[..., 0], a[..., 1], a[..., 2], a[..., 3]
    op = al > 160
    mx, mn = np.maximum(np.maximum(r, g), b), np.minimum(np.minimum(r, g), b)
    steel = op & (b >= r) & (b >= g - 12) & (mx - mn < 70) & (mn > 95)
    blue = op & (b > r + 60) & (b > g + 40)
    hair = op & (r > g + 25) & (g > b) & (r < 150) & (mx > 70) & (b < 70)
    wood = op & (r > g + 40) & (g > b + 15) & (r > 120) & (r < 200)
    skin = op & (r > 225) & (g > 160) & (b > 90) & (b < 175) & (r > b + 60)
    return {"steel": steel, "tunic": blue, "hair": hair, "wood": wood, "skin": skin}


def centroid(m):
    return None if m.sum() < 6 else tuple(float(v) for v in ndimage.center_of_mass(m)[::-1])


def tip_point(im):
    pm = part_masks(im)
    st = pm["steel"]
    if st.sum() < 10:
        return None
    lab, n = ndimage.label(st)
    sizes = ndimage.sum(st, lab, range(1, n + 1))
    blade = lab == (int(np.argmax(sizes)) + 1)
    body = mask(im) & ~blade
    cy, cx = ndimage.center_of_mass(body) if body.sum() else ndimage.center_of_mass(mask(im))
    ys, xs = np.nonzero(blade)
    k = int(np.argmax((xs - cx) ** 2 + (ys - cy) ** 2))
    return (int(xs[k]), int(ys[k]))


def overlap(frames):
    parts = ["steel", "tunic", "hair", "wood"]
    c0 = {p: centroid(part_masks(frames[0])[p]) for p in parts}
    disp = {p: [] for p in parts}
    for f in frames:
        pm = part_masks(f)
        for p in parts:
            c = centroid(pm[p])
            disp[p].append(None if c is None or c0[p] is None else round(float(np.hypot(c[0] - c0[p][0], c[1] - c0[p][1])), 2))
    peaks = {p: int(np.nanargmax([np.nan if v is None else v for v in d])) for p, d in disp.items()
             if any(v is not None and v > 0 for v in d)}
    spread = float(np.std(list(peaks.values()))) if len(peaks) > 1 else 0.0
    return {"displacement": disp, "peak_frame": peaks, "overlap_index": round(spread, 3)}


def drift(frames):
    tree = sprite_tree()
    out = []
    for f in frames:
        a = np.asarray(f).reshape(-1, 4)
        px = a[a[:, 3] > 200][:, :3]
        if len(px) == 0:
            out.append(None)
            continue
        d, _ = tree.query(oklab(px))
        out.append({"mean": round(float(d.mean() * 100), 2), "p95": round(float(np.percentile(d, 95) * 100), 2)})
    return out


def jitter(frames):
    bottoms, feet_x = [], []
    for f in frames:
        m = mask(f, 60)
        ys = np.nonzero(m.any(1))[0]
        if not len(ys):
            continue
        bottoms.append(int(ys[-1]))
        low = m[max(0, ys[-1] - 5): ys[-1] + 1]
        xs = np.nonzero(low.any(0))[0]
        feet_x.append((int(xs[0]), int(xs[-1])))
    fx = np.array(feet_x, float) if feet_x else np.zeros((1, 2))
    return {"bottom_std": round(float(np.std(bottoms)), 2), "feet_x_std": round(float(fx.std(0).mean()), 2),
            "bottoms": bottoms}


def boil(frames):
    out = []
    for i in range(1, len(frames)):
        a = np.asarray(frames[i - 1]).astype(np.float32)
        b = np.asarray(frames[i]).astype(np.float32)
        wa, wb = a[..., 3:4] / 255, b[..., 3:4] / 255
        d = np.abs(a[..., :3] * wa - b[..., :3] * wb).sum(-1) / 3 + np.abs(a[..., 3] - b[..., 3]) / 2
        out.append(round(float(d[(a[..., 3] > 0) | (b[..., 3] > 0)].mean()), 2))
    return out


def _pose_blade_vs_shield(anim, i):
    """Expected sign of (blade centre x - shield centre x) from poses.json, or None if it is ambiguous."""
    try:
        p = ak.POSES["anims"][anim][i]
    except (KeyError, IndexError):
        return None
    hilt, tip = (p["sword"] if "sword" in p else (p["r_ha"], p["tip"]))
    bx = (hilt[0] + tip[0]) / 2
    d = bx - p["shield"][0]
    return None if abs(d) < 8 else (1 if d > 0 else -1)


def gates(anim, frames, poses=None):
    """Heuristic flags, confirmed by eye: facing flip (the face/skin centroid should be left of the hair centroid),
    hand swap (blade vs shield on the opposite sides from the beat-sheet pose), missing sword."""
    flags = []
    for i, f in enumerate(frames):
        pm = part_masks(f)
        lying = anim == "death" and i >= 4
        hair, skin, steel, wood = (centroid(pm[k]) for k in ("hair", "skin", "steel", "wood"))
        if pm["steel"].sum() < 15:
            flags.append(f"frame {i}: little or no steel (sword missing?)")
        if lying or hair is None or skin is None:
            continue
        if skin[0] > hair[0] + 6:
            flags.append(f"frame {i}: face is right of the hair (facing flip?)")
        exp = _pose_blade_vs_shield(anim, i)
        if steel and wood and exp is not None:
            got = steel[0] - wood[0]
            if abs(got) > 6 and (1 if got > 0 else -1) != exp:
                flags.append(f"frame {i}: blade and shield on the wrong sides (hands swapped?)")
    return flags


def anim_metrics(frames, anim):
    m = {"nonrigid": nonrigid(frames, wide=(anim == "death")), "overlap": overlap(frames), "drift_dE": drift(frames),
         "boil": boil(frames), "arc": [tip_point(f) for f in frames], "gates": gates(anim, frames)}
    if anim in ("idle", "flinch"):
        m["jitter"] = jitter(frames)
    nr = [v for v in m["nonrigid"][1:]]
    m["summary"] = {"nonrigid_mean": round(float(np.mean(nr)), 3) if nr else 0.0,
                    "overlap_index": m["overlap"]["overlap_index"],
                    "drift_mean": round(float(np.mean([d["mean"] for d in m["drift_dE"] if d])), 2),
                    "boil_mean": round(float(np.mean(m["boil"])), 2) if m["boil"] else 0.0,
                    "n_gate_flags": len(m["gates"])}
    return m


def arm_metrics(arm, anims=ANIMS, root=None):
    d = Path(root) if root else ak.ROOT / "arms" / arm
    mp = d / "metrics.json"
    out = json.loads(mp.read_text()) if mp.exists() else {}  # merge: agents may compute one animation at a time
    for anim in anims:
        if not (d / anim / "manifest.json").exists():
            continue
        frames, _ = ak.load_anim(arm, anim, root=root)
        out[anim] = anim_metrics(frames, anim)
    mp.write_text(json.dumps(out, indent=1))
    return {k: v for k, v in out.items() if k in anims}


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm", required=True); ap.add_argument("--anim", action="append"); ap.add_argument("--root")
    a = ap.parse_args()
    res = arm_metrics(a.arm, a.anim or ANIMS, a.root)
    print(json.dumps({k: v["summary"] | {"gates": v["gates"]} for k, v in res.items()}, indent=1))
