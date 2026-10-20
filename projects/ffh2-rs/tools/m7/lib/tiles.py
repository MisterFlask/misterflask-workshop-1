"""M7 terrain helpers (ported unchanged in behaviour from the sprint's methods/M7/terrain/work/tt.py,
inset.py and post.py).

- lattice_tile + finish_tile: turn a full-frame iso ground swatch into a seamless 192x128 tile
- inset_tile: put a cut-out object (farm plot, hill cluster, oasis...) inside a finished base tile
- lift_shadow: lift a feature sprite and add the lower-right silhouette drop shadow
- tilegrid / view: review helpers
"""
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

import cutout as co


def dmask():
    """Exact 192x96 diamond (pixel-centre test) inside a 192x128 tile canvas."""
    a = np.zeros((128, 192), np.uint8)
    ys, xs = np.mgrid[0:128, 0:192]
    inside = (np.abs(xs + 0.5 - 96) / 96 + np.abs(ys + 0.5 - 48) / 48) <= 1.0 + 1e-6
    a[inside] = 255
    return a


def pixelate(rgb, p):
    """Block-average p x p (grid anchored at 0,0) and repeat. p must divide 48 so blocks line up across tiles."""
    if p <= 1:
        return rgb
    H, W = rgb.shape[:2]
    h2, w2 = -(-H // p), -(-W // p)
    pad = np.pad(rgb, ((0, h2 * p - H), (0, w2 * p - W), (0, 0)), mode="edge")
    small = pad.reshape(h2, p, w2, p, 3).mean((1, 3))
    return np.repeat(np.repeat(small, p, 0), p, 1)[:H, :W]


def smooth_noise(shape, sigma, seed):
    r = np.random.default_rng(seed).random(shape)
    r = ndimage.gaussian_filter(r, sigma, mode="wrap")
    return (r.argsort(axis=None).argsort().reshape(shape) + 0.5) / r.size  # rank -> uniform


def lattice_tile(src, k, cx=0.5, cy=0.5, m=0.25, seed=1, ss=3, p=3):
    """Sample an iso-camera swatch (HxWx3 float) into a lattice-periodic 192x96 diamond texture.

    Lattice coords: a runs along u=(96,-48), b along v=(96,48) -- exactly the screen offsets between
    neighbouring iso tiles -- so a texture that is 1-periodic in (a,b) tiles seamlessly. Near a=0 / b=0
    each art-pixel block stochastically samples from a or a+1 (a dithered cross-fade, no ghosting).
    k = source px per output px (bigger k -> smaller, denser features)."""
    H, W = src.shape[:2]
    C = np.array([cx * W, cy * H])
    oh, ow = 96 * ss, 192 * ss
    ys, xs = np.mgrid[0:oh, 0:ow]
    x = (xs + 0.5) / ss
    y = (ys + 0.5) / ss
    a = np.mod((x / 96 - (y - 48) / 48) / 2, 1.0)
    b = np.mod((x / 96 + (y - 48) / 48) / 2, 1.0)
    blk = max(1, p * ss)
    nsh = (-(-oh // blk), -(-ow // blk))
    ra = np.repeat(np.repeat(smooth_noise(nsh, 1.2, seed), blk, 0), blk, 1)[:oh, :ow]
    rb = np.repeat(np.repeat(smooth_noise(nsh, 1.2, seed + 7), blk, 0), blk, 1)[:oh, :ow]
    wa = np.clip(a / m, 0, 1); wb = np.clip(b / m, 0, 1)
    A = np.where(ra > wa, a + 1, a)
    B = np.where(rb > wb, b + 1, b)
    off = 0.5 + m / 2
    sx = C[0] + k * ((A - off) * 96 + (B - off) * 96)
    sy = C[1] + k * (-(A - off) * 48 + (B - off) * 48)
    out = np.zeros((oh, ow, 3), np.float32)
    for c in range(3):
        out[..., c] = ndimage.map_coordinates(src[..., c], [sy - 0.5, sx - 0.5], order=1, mode="reflect")
    out = out.reshape(96, ss, 192, ss, 3).mean((1, 3))
    return pixelate(out, p)


def make_skirt(kind="earth", p=3, depth=30, seed=3):
    """Side face under the diamond (visible at map edges and above dropped water tiles)."""
    rng = np.random.default_rng(seed)
    img = np.zeros((128, 192, 4), np.float32)
    nb = 192 // p + 1
    lip = rng.integers(1, 3, nb)
    for x in range(192):
        ytop = 48 + (x if x < 96 else 191 - x) / 2.0
        left = x < 96
        col = x // p
        for y in range(int(ytop), min(128, int(ytop + depth) + 1)):
            d = y - ytop
            if d < 0:
                continue
            row = int(d) // p
            n = (hash((col, row, seed)) % 1000) / 1000.0
            if kind == "earth":
                if d < lip[col] * p:
                    c = np.array([92, 140, 40]) if left else np.array([70, 112, 32])
                    if d >= (lip[col] - 1) * p:
                        c = c * 0.78
                else:
                    t = d / depth
                    c = np.array([156, 104, 60]) * (1 - 0.35 * t)
                    if not left:
                        c = c * 0.74
                    band = (int(d + (x % 97) * 0.07) // (p * 2)) % 3
                    c = c * (1.0 - 0.07 * band)
                    if n > 0.9:
                        c = c * 0.7
                    elif n < 0.08:
                        c = np.minimum(c * 1.25, 255)
            elif kind == "water":
                t = d / depth
                if d < p:
                    c = np.array([70, 140, 230]) if left else np.array([50, 115, 205])
                else:
                    c = np.array([30, 92, 196]) * (1 - 0.5 * t)
                    if not left:
                        c = c * 0.78
                    if n > 0.93:
                        c = np.minimum(c * 1.4, 255)
            else:  # generic material skirt: kind = "r,g,b" body colour (sand, snow, rock, lava ...)
                body = np.array([float(v) for v in kind.split(",")])
                t = d / depth
                c = body * (1 - 0.4 * t)
                if d < p:
                    c = np.minimum(body * 1.15, 255)
                if not left:
                    c = c * 0.76
                band = (int(d + (x % 97) * 0.07) // (p * 2)) % 3
                c = c * (1.0 - 0.06 * band)
                if n > 0.9:
                    c = c * 0.75
            img[y, x, :3] = c
            img[y, x, 3] = 255
    return img


def finish_tile(top_rgb, out, skirt="earth", p=3, seed=3):
    """Skirt first, then every diamond pixel overwritten with the top texture (alpha 255)."""
    img = make_skirt(skirt, p, seed=seed) if skirt != "none" else np.zeros((128, 192, 4), np.float32)
    m = dmask()
    top = np.zeros((128, 192, 3), np.float32); top[:96] = top_rgb
    sel = m > 0
    img[sel, :3] = top[sel]; img[sel, 3] = 255
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(np.clip(img, 0, 255).astype(np.uint8), "RGBA").save(out)


def adjust(rgb, sat=1.0, bright=1.0):
    if sat == 1.0 and bright == 1.0:
        return rgb
    g = rgb.mean(-1, keepdims=True)
    return np.clip((g + (rgb - g) * sat) * bright, 0, 255)


def swatch_to_tile(raw, out, k=1.8, p=3, m=0.25, skirt="earth", seed=1, sat=1.0, bright=1.0, cx=0.5, cy=0.5):
    src = np.asarray(Image.open(raw).convert("RGB")).astype(np.float32)
    if k * 192 * (1 + m) > min(src.shape[:2]) * 1.0:
        raise SystemExit(f"k={k} samples a window wider than the swatch; use k <= {min(src.shape[:2]) / (192 * (1 + m)):.1f}")
    top = adjust(lattice_tile(src, k, cx=cx, cy=cy, m=m, seed=seed, p=p), sat, bright)
    finish_tile(top, out, skirt, p)


def inset_tile(raw, base, out, width=168, bottom_y=92, p=2, skirt="earth", key="#ff00ff"):
    """Cut a magenta-background object, scale to `width`, pixelate, paste centred inside the base tile's
    diamond with its bottom at `bottom_y` (the diamond's bottom vertex is y=96), re-mask, re-skirt.
    Width > ~170 gets clipped at the side vertices. Returns (w, h, x, y); y < 0 means the top was cropped."""
    im = co.remove_bg(Image.open(raw).convert("RGBA"), key, 48, 30)
    im = co.trim(co.clean_specks(im))
    im = co.scale_to(im, w=width)
    im = co.pixelate(im, p)
    b = Image.open(base).convert("RGBA").crop((0, 0, 192, 96))
    canvas = b.copy()
    x = (192 - im.width) // 2; y = bottom_y - im.height
    if y >= 0:
        canvas.alpha_composite(im, (x, y))
    else:
        canvas.alpha_composite(im.crop((0, -y, im.width, im.height)), (x, 0))
    top = np.asarray(canvas.convert("RGB")).astype(np.float32)
    finish_tile(top, out, skirt, p)
    # The object alone on the tile canvas, clipped to the diamond: the game's overlay (ovl_*), drawn over
    # whatever base tile the plot has, so one improvement or hill set serves every terrain.
    ov = Image.new("RGBA", (192, 128), (0, 0, 0, 0))
    ov.alpha_composite(im if y >= 0 else im.crop((0, -y, im.width, im.height)), (x, max(0, y)))
    a = np.asarray(ov).copy()
    a[..., 3] = np.minimum(a[..., 3], dmask())
    Image.fromarray(a, "RGBA").save(overlay_path(out))
    return im.width, im.height, x, y


def overlay_path(out):
    """Where inset_tile writes the overlay of the tile it writes to `out`."""
    out = Path(out)
    return out.with_name(out.stem + ".ovl.png")


def pair_variant(a_png, b_png, out, p=3, inner=0.5, outer=0.78, seed=5):
    """Variant b made to tile with its base a: b keeps its own top face inside, and takes a's pixels near the
    diamond's edges (a dithered cross-fade, p-px blocks, between `inner` and `outer` of the way from the centre
    to the edge), skirt included. a tiles with itself, so a|b, b|a and b|b neighbours all meet on a's border."""
    A = np.asarray(Image.open(a_png).convert("RGBA"))
    B = np.asarray(Image.open(b_png).convert("RGBA"))
    ys, xs = np.mgrid[0:128, 0:192]
    d = np.abs(xs + 0.5 - 96) / 96 + np.abs(ys + 0.5 - 48) / 48
    w = np.clip((d - inner) / (outer - inner), 0, 1)
    blocks = (-(-128 // p), -(-192 // p))
    r = np.repeat(np.repeat(smooth_noise(blocks, 1.0, seed), p, 0), p, 1)[:128, :192]
    take_b = (dmask() > 0) & (r >= w)
    o = A.copy()
    o[take_b] = B[take_b]
    Image.fromarray(o, "RGBA").save(out)
    return int(take_b.sum())


def lift_shadow(inp, out, lift=6, dx=4, dy=3, alpha=0.35, colour=(12, 40, 10)):
    """Lift a feature `lift` px above its anchor and put a dark silhouette copy offset (dx, dy) under it."""
    im = Image.open(inp).convert("RGBA")
    W, H = im.width + 2 * dx, im.height + lift
    c = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    a = np.asarray(im)[..., 3].astype(np.float32)
    sh = np.zeros((im.height, im.width, 4), np.uint8); sh[..., :3] = colour; sh[..., 3] = (a > 100) * 255 * alpha
    c.alpha_composite(Image.fromarray(sh, "RGBA"), (dx + dx, dy))
    c.alpha_composite(im, (dx, 0))
    x0 = (W - im.width) // 2
    c.crop((x0, 0, x0 + im.width, H)).save(out)


def view(inp, out, s=3, bg=(70, 100, 40)):
    im = Image.open(inp).convert("RGBA")
    b = Image.new("RGBA", im.size, tuple(bg) + (255,))
    b.alpha_composite(im)
    b.resize((im.width * s, im.height * s), Image.NEAREST).save(out)


def tilegrid(inp, out, n=3):
    """n x n self-tiling check on a dark background: any seam shows as a line."""
    t = Image.open(inp).convert("RGBA")
    W = 96 * 2 * n + 192; H = 48 * 2 * n + 128
    sc = Image.new("RGBA", (W, H), (20, 30, 50, 255))
    for s in range(2 * n - 1):
        for r in range(n):
            c = s - r
            if 0 <= c < n:
                sc.alpha_composite(t, ((c - r) * 96 + (n - 1) * 96 + 96, (c + r) * 48 + 20))
    sc.save(out)
