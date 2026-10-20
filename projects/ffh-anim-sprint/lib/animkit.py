"""Shared animation I/O and image helpers. Every arm writes its frames through write_anim().

  import animkit as ak
  base = ak.base_canvas()                      # 160x128 canvas with the static sprite at (32, 8)
  ak.write_anim("G1", "strike", frames)        # frames: list of 160x128 RGBA; timings/events from beats.py
  frames, man = ak.load_anim("G1", "strike")

Helpers: premultiplied resize/rotate/affine (no dark fringes on soft edges), bbox/trim, snap_ground, and the
round-2 finish pass (palette_snap, paste_back, smear, flash, spark live in finish.py).
"""
import json, sys, time
from pathlib import Path
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "m7" / "lib"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import beats  # noqa: E402

W, H = 160, 128
ANCHOR = (80, 126)
SPRITE_OFFSET = (32, 8)
SAFE_X = (16, 144)
SPRITE_PATH = ROOT / "m7/roster/units/unit_swordsman.png"
POSES = json.loads((ROOT / "poses.json").read_text())


def sprite():
    return Image.open(SPRITE_PATH).convert("RGBA")


def base_canvas(sp=None):
    c = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    c.alpha_composite(sp or sprite(), SPRITE_OFFSET)
    return c


def arm_dir(arm):
    d = ROOT / "arms" / arm
    d.mkdir(parents=True, exist_ok=True)
    return d


# ---------------------------------------------------------------- premultiplied geometry helpers
def _premul(im):
    a = np.asarray(im.convert("RGBA")).astype(np.float32)
    al = a[..., 3:4] / 255.0
    return np.dstack([a[..., :3] * al, a[..., 3:4]])


def _unpremul(pm):
    A = np.clip(pm[..., 3], 0, 255)
    al = np.maximum(A / 255.0, 1e-4)
    rgb = np.clip(pm[..., :3] / al[..., None], 0, 255)
    out = np.dstack([rgb, A])
    out[A < 1] = 0
    return Image.fromarray(out.astype(np.uint8), "RGBA")


def _per_channel(pm, fn):
    return np.dstack([np.asarray(fn(Image.fromarray(pm[..., i], "F"))) for i in range(4)])


def resize(im, size, resample=Image.LANCZOS):
    return _unpremul(_per_channel(_premul(im), lambda c: c.resize(size, resample)))


def scale(im, s, resample=Image.LANCZOS):
    return resize(im, (max(1, round(im.width * s)), max(1, round(im.height * s))), resample)


def affine(im, matrix, size=None, resample=Image.BICUBIC):
    """matrix = (a, b, c, d, e, f) mapping OUTPUT (x, y) -> INPUT (a x + b y + c, d x + e y + f), as PIL expects."""
    size = size or im.size
    return _unpremul(_per_channel(_premul(im), lambda ch: ch.transform(size, Image.AFFINE, matrix, resample=resample)))


def transform(im, angle=0.0, sx=1.0, sy=1.0, shx=0.0, pivot=ANCHOR, dx=0.0, dy=0.0, resample=Image.BICUBIC):
    """Rotate (deg, + = counter-clockwise on screen), scale and shear about `pivot`, then translate by (dx, dy)."""
    t = np.radians(angle)
    c, s = np.cos(t), np.sin(t)
    R = np.array([[c, s], [-s, c]])  # screen y points down: + angle turns counter-clockwise
    M = R @ np.array([[1, shx], [0, 1]]) @ np.diag([sx, sy])
    Mi = np.linalg.inv(M)
    px, py = pivot
    # out = M (in - pivot) + pivot + d  ->  in = Mi (out - pivot - d) + pivot
    ox, oy = px + dx, py + dy
    a, b, d_, e = Mi[0, 0], Mi[0, 1], Mi[1, 0], Mi[1, 1]
    cc = px - a * ox - b * oy
    ff = py - d_ * ox - e * oy
    return affine(im, (a, b, cc, d_, e, ff), resample=resample)


def bbox(im, thr=8):
    return im.getchannel("A").point(lambda v: 255 if v > thr else 0).getbbox()


def snap_ground(frame, target_bottom=ANCHOR[1] - 1, max_px=3):
    """Shift a frame vertically by up to max_px so its lowest opaque row sits on target_bottom (feet planted)."""
    bb = bbox(frame, 60)
    if not bb:
        return frame, 0
    dy = target_bottom - (bb[3] - 1)
    if dy == 0 or abs(dy) > max_px:
        return frame, 0
    out = Image.new("RGBA", frame.size, (0, 0, 0, 0))
    out.alpha_composite(frame, (0, dy))
    return out, dy


def mirror(frame):
    from PIL import ImageOps
    return ImageOps.mirror(frame)  # 160 wide, anchor x=80 -> stays on the anchor


# ---------------------------------------------------------------- writing / loading
def write_anim(arm, anim, frames, ms=None, events=None, force_frame0=True, note=None):
    """Validate and write one animation: frames, manifest.json and sheet_<anim>.png.
    For idle and strike, frame 0 is replaced by the static sprite (spec) unless force_frame0=False."""
    n = beats.COUNTS[anim]
    if len(frames) != n:
        raise ValueError(f"{anim} needs {n} frames, got {len(frames)}")
    ms = ms or beats.ms(anim)
    events = events or beats.events(anim)
    frames = [f.convert("RGBA") for f in frames]
    warns = []
    for i, f in enumerate(frames):
        if f.size != (W, H):
            raise ValueError(f"frame {i} is {f.size}, expected {(W, H)}")
        bb = bbox(f, 40)
        if bb and (bb[0] < SAFE_X[0] or bb[2] > SAFE_X[1]):
            warns.append(f"frame {i} leaves the safe region x {SAFE_X}: bbox {bb}")
    if anim in ("idle", "strike") and force_frame0:
        frames[0] = base_canvas()
    d = arm_dir(arm) / anim
    d.mkdir(parents=True, exist_ok=True)
    for old in d.glob("frame_*.png"):
        old.unlink()
    man = {"unit": "swordsman", "arm": arm, "anim": anim, "canvas": [W, H], "anchor": list(ANCHOR),
           "loop": beats.LOOP[anim], "hold_last": beats.HOLD_LAST[anim], "frames": []}
    for i, f in enumerate(frames):
        fn = f"frame_{i:02d}.png"
        f.save(d / fn)
        man["frames"].append({"file": fn, "ms": int(ms[i]), "events": list(events[i])})
    if note:
        man["note"] = note
    if warns:
        man["warnings"] = warns
    (d / "manifest.json").write_text(json.dumps(man, indent=1))
    sheet = Image.new("RGBA", (W * n, H), (0, 0, 0, 0))
    for i, f in enumerate(frames):
        sheet.alpha_composite(f, (i * W, 0))
    sheet.save(arm_dir(arm) / f"sheet_{anim}.png")
    return man


def load_anim(arm, anim, root=None):
    d = (Path(root) if root else ROOT / "arms" / arm) / anim
    man = json.loads((d / "manifest.json").read_text())
    frames = [Image.open(d / fr["file"]).convert("RGBA") for fr in man["frames"]]
    return frames, man


def log_time(arm, what, t0, t1=None, **extra):
    """Append a timing row to arms/<arm>/timings.jsonl (t0/t1 = time.time() values)."""
    t1 = t1 or time.time()
    row = dict(what=what, start=round(t0, 1), end=round(t1, 1), seconds=round(t1 - t0, 1), **extra)
    with open(arm_dir(arm) / "timings.jsonl", "a") as f:
        f.write(json.dumps(row) + "\n")
    return row


def grid_view(im, out, scale=6, bg=(126, 170, 60, 255), step=8):
    """Save `im` enlarged with a coordinate grid (lines every `step` px, labels every 16) for annotating joints/masks."""
    from PIL import ImageDraw
    base = Image.new("RGBA", im.size, bg)
    base.alpha_composite(im.convert("RGBA"))
    big = base.resize((im.width * scale, im.height * scale), Image.NEAREST)
    d = ImageDraw.Draw(big)
    for x in range(0, im.width, step):
        d.line([(x * scale, 0), (x * scale, big.height)], fill=(255, 255, 0, 170) if x % 16 == 0 else (255, 255, 255, 80))
        if x % 16 == 0:
            d.text((x * scale + 2, 2), str(x), fill=(0, 0, 0, 255))
    for y in range(0, im.height, step):
        d.line([(0, y * scale), (big.width, y * scale)], fill=(255, 255, 0, 170) if y % 16 == 0 else (255, 255, 255, 80))
        if y % 16 == 0:
            d.text((2, y * scale + 2), str(y), fill=(0, 0, 0, 255))
    big.save(out)
    return out
