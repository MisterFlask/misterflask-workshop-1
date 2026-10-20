"""R - idle, flinch and death from the same cut-out rig as the strike (make_r.py, parts/, rig.json).

Every pixel comes from the static sprite or from code: the part layers in arms/R/parts/ (cut from the sprite, hidden
areas filled in code by make_r.py), the rigid MLS warp (lib/rig.py), affine part transforms, and small code-drawn
touches (closed eyes). No image-model calls.

  idle    whole-sprite rigid MLS (rig.warp_to_pose) from idle frame 0 to each idle pose, feet pinned, for a
          sub-pixel breath; the sword and shield are separate layers that follow a frame later (overlapping action).
  flinch  parts + MLS (make_r.pose_frame) to the flinch poses: head snapped back up-right, arms flung out, knees bent.
  death   parts + a whole-body affine per frame (fit to the poses.json torso joints) for the stagger, the knee buckle,
          the kneel, the fall back and the corpse lying along the iso ground axis; the sword leaves the hand on
          frame 4 and lies on the ground at the lower-left.

  python arms/R/make_r_rest.py idle|flinch|death|all [--debug]
"""
import json, sys, time
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402
import rig  # noqa: E402
import make_r as R  # noqa: E402  (rig code shared with the strike; not modified)

ARM = "R"
HERE = ROOT / "arms" / ARM
WORK = ROOT / "work" / ARM / "rest"
POSES = ak.POSES["anims"]
P0 = R.P0
OUTLINE = R.OUTLINE


def load_parts():
    return {n: Image.open(HERE / "parts" / f"{n}.png").convert("RGBA") for n in R.PRIORITY}


def comp(layers, order):
    out = Image.new("RGBA", (ak.W, ak.H), (0, 0, 0, 0))
    for n in order:
        if n in layers and layers[n] is not None:
            out.alpha_composite(layers[n])
    return out


def shift(im, dx, dy):
    """Sub-pixel translate (bilinear, premultiplied); integer shifts are exact."""
    if dx == int(dx) and dy == int(dy):
        out = Image.new("RGBA", im.size, (0, 0, 0, 0))
        out.alpha_composite(im, (int(dx), int(dy))) if dx >= 0 and dy >= 0 else out.paste(
            im.transform(im.size, Image.AFFINE, (1, 0, -dx, 0, 1, -dy), resample=Image.NEAREST))
        return out
    return ak.transform(im, dx=dx, dy=dy, resample=Image.BILINEAR)


# ================================================================ IDLE
def mls_warp_sharp(im, src_pts, dst_pts, sharpen=0.0, order=1):
    """rig.mls_warp (same rigid MLS map, backward-mapped, premultiplied bilinear), with an optional sharpening of the
    displacement field: a displacement near a whole pixel snaps to it, so the head and shoulders move by exactly 1 px
    (crisp) and the legs by 0, and the sub-pixel blend is confined to a narrow band of the belly. sharpen=0 is exactly
    rig.mls_warp."""
    a = np.asarray(im.convert("RGBA")).astype(np.float32)
    pm = np.dstack([a[..., :3] * a[..., 3:4] / 255.0, a[..., 3]])
    src = rig.mls_map(a.shape[:2], dst_pts, src_pts)  # output pixel -> source location
    if sharpen > 0:
        ys, xs = np.mgrid[0:a.shape[0], 0:a.shape[1]]
        disp = src - np.dstack([xs, ys])
        fl = np.floor(disp)
        fr = disp - fl
        k = 1.0 + sharpen
        fr = np.clip((fr - 0.5) * k + 0.5, 0.0, 1.0)
        src = np.dstack([xs, ys]) + fl + fr
    coords = [src[..., 1], src[..., 0]]
    out = np.dstack([ndimage.map_coordinates(pm[..., c], coords, order=order, mode="constant", cval=0.0)
                     for c in range(4)])
    A = np.clip(out[..., 3], 0, 255)
    rgb = np.clip(out[..., :3] / np.maximum(A[..., None] / 255.0, 1e-4), 0, 255)
    res = np.dstack([rgb, A])
    res[A < 2] = 0
    return Image.fromarray(res.astype(np.uint8), "RGBA")


def idle_layers(parts):
    """Split the static sprite into body / sword / shield layers. The body keeps the sprite's own pixels; under the
    shield it gets the rig's code-filled sleeve/torso/thigh pixels, so a shield that lifts reveals no hole."""
    base = ak.base_canvas()
    _, label = R.segment(base)
    a = np.asarray(base).copy()
    sw = label == R.PRIORITY.index("sword")
    sh = label == R.PRIORITY.index("shield")
    sword = a.copy(); sword[~sw] = 0
    shield = a.copy(); shield[~sh] = 0
    body = a.copy(); body[sw | sh] = 0
    body = Image.fromarray(body, "RGBA")
    order0 = [n for g in ak.POSES["default_order"] for n in R.GROUP[g] if n not in ("sword", "shield")]
    under = comp(parts, order0)
    ua = np.asarray(under).copy()
    ua[~(sh & (a[..., 3] == 255))] = 0  # fill only under the shield's opaque pixels (its soft edge keeps the sprite's)
    filled = Image.fromarray(ua, "RGBA")
    filled.alpha_composite(body)
    swl = Image.fromarray(sword, "RGBA")
    grip = np.asarray(parts["sword"]).copy()
    grip[(a[..., 3] < 255) | sw] = 0  # the painted grip only where the opaque fist hides it
    swl.alpha_composite(Image.fromarray(grip, "RGBA"))
    return filled, swl, Image.fromarray(shield, "RGBA"), base


def idle(parts, sharpen=3.0, debug=False):
    I = POSES["idle"]
    body, sword, shield, base = idle_layers(parts)
    frames = [base]
    for i in range(1, len(I)):
        src = rig.pose_points(I[0])
        dst = rig.pose_points(I[i])
        b = mls_warp_sharp(body, src, dst, sharpen=sharpen)
        # sword and shield are rigid layers that follow the joints a frame late (the poses hold them still on frame 1,
        # lift them on frame 2 and keep them up on frame 3): whole-pixel shifts, so they stay crisp
        tdy = I[i]["tip"][1] - I[0]["tip"][1]
        hdy = I[i]["r_ha"][1] - I[0]["r_ha"][1]
        sdy = I[i]["shield"][1] - I[0]["shield"][1]
        # the hilt stays in the fist (moves with the hand); the blade above the guard is lifted to the tip's height
        # and drawn over a copy at the hand's height, so the extra rise does not open a gap at the guard
        hilt = R.hilt_layer(sword)
        ha = np.asarray(hilt)[..., 3] > 0
        bl = np.asarray(sword).copy(); bl[ha] = 0
        blade = Image.fromarray(bl, "RGBA")
        f = Image.new("RGBA", (ak.W, ak.H), (0, 0, 0, 0))
        for dy in sorted({hdy, tdy}, reverse=True):
            f.alpha_composite(shift(blade, 0, dy))
        f.alpha_composite(shift(hilt, 0, hdy))
        f.alpha_composite(b)
        f.alpha_composite(shift(shield, 0, sdy))
        frames.append(f)
    return frames


def decrumb(f, min_size=8, faint=90, solid=150):
    """Remove what floats once parts separate: tiny detached specks, and faint soft-edge pixels (the sprite's
    anti-aliasing between two parts) that are left with no solid pixel next to them."""
    a = np.asarray(f).copy()
    al = a[..., 3]
    lab, n = ndimage.label(al > 0, structure=np.ones((3, 3)))
    if n > 1:
        sizes = ndimage.sum(np.ones_like(lab), lab, range(1, n + 1))
        for c in np.nonzero(sizes < min_size)[0]:
            a[lab == c + 1] = 0
    near = ndimage.maximum_filter(a[..., 3], size=3)
    a[(a[..., 3] < faint) & (near < solid)] = 0
    return Image.fromarray(a, "RGBA")


def springs(seq, k_hair=0.55, k_shield=0.6, start=None):
    """One-frame springs (as in the strike): the hair chases the head and the shield chases the pose's shield centre,
    so both trail behind fast moves and catch up a frame later. Returns per-frame (hair_lag, shield_lag)."""
    head_xy = [np.array(q["head"], float) for q in seq]
    shield_xy = [np.array(q["shield"][:2], float) for q in seq]
    hair = np.array(start["head"] if start else seq[0]["head"], float)
    shl = np.array(start["shield"][:2] if start else seq[0]["shield"][:2], float)
    out = []
    for i in range(len(seq)):
        hair = hair + (head_xy[i] - hair) * k_hair
        shl = shl + (shield_xy[i] - shl) * k_shield
        out.append((np.clip(hair - head_xy[i], -3, 3), np.clip(shl - shield_xy[i], -2, 2)))
    return out


def flinch_frames(parts):
    F = POSES["flinch"]
    shut = dict(parts, head=shut_eyes(parts["head"]))
    lags = springs(F, start=P0)
    frames = []
    for i, p in enumerate(F):
        hl, sl = lags[i]
        if i == 0:
            sl = np.zeros(2)  # the impact frame hits the pose exactly (it is also death frame 0)
        if i == 3:  # settle: almost no lag left so idle resumes without a pop
            hl, sl = hl * 0.3, sl * 0.3
        pp = shut if p.get("eyes") == "shut" else parts
        squash = {0: (1.0, 1.02), 1: (1.0, 0.97)}.get(i, (1.0, 1.0))
        f, _ = R.pose_frame(pp, p, squash=squash, hair_lag=tuple(hl), shield_lag=tuple(sl))
        frames.append(decrumb(R.fit_ground(f)))
    return frames


def flinch(parts):
    return flinch_frames(parts)


def death(parts):
    raise NotImplementedError


# ================================================================ closed eyes (code-drawn, sprite palette)
SKIN_LIT, SKIN_MID, SKIN_SHADE = (255, 214, 128), (240, 178, 96), (214, 134, 58)
LID = (58, 26, 16)


def shut_eyes(head):
    """Paint the eyes squeezed shut on the head layer (rest coordinates): eye whites and pupils become skin from the
    sprite's own face colours, and a dark lid line with a crease is drawn across each eye."""
    a = np.asarray(head).copy()

    def put(x, y, c):
        if a[y, x, 3] > 0:
            a[y, x, :3] = c
            a[y, x, 3] = 255
    # near eye (x 81-84, y 55-58): brow-lid row and eye become skin, lid line on row 57, crease above it
    for x in range(81, 85):
        put(x, 55, SKIN_SHADE)
        put(x, 56, SKIN_MID)
        put(x, 57, LID)
        put(x, 58, SKIN_LIT)
    put(80, 57, (150, 80, 36))
    put(82, 56, SKIN_SHADE)
    put(83, 56, SKIN_SHADE)
    # far eye (x 88-89, y 54-56)
    for y in (54, 55):
        put(88, y, SKIN_MID)
        put(89, y, SKIN_SHADE)
    put(88, 56, LID)
    put(89, 56, LID)
    put(88, 57, SKIN_MID)
    return Image.fromarray(a, "RGBA")


# ================================================================ main
NOTES = {"idle": "R rig: sprite split into body/sword/shield; body bent by rigid MLS (sharpened displacement), sword "
                 "and shield follow a frame late (make_r_rest.py)",
         "flinch": "R rig: cut-out parts + MLS torso posed to the flinch joints (make_r_rest.py)",
         "death": "R rig: cut-out parts + per-frame whole-body affine to the death joints (make_r_rest.py)"}


def main(which, debug=False):
    parts = load_parts()
    for anim in which:
        t0 = time.time()
        frames = {"idle": idle, "flinch": flinch, "death": death}[anim](parts)
        man = ak.write_anim(ARM, anim, frames, note=NOTES[anim])
        print(anim, "written", json.dumps(man.get("warnings", [])))


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    which = ["idle", "flinch", "death"] if not args or args == ["all"] else args
    main(which, debug="--debug" in sys.argv)
