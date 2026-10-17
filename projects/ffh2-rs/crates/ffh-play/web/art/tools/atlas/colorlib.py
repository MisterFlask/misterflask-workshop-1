"""Small colour toolkit (numpy only): sRGB<->Lab, k-means in Lab, swatch sheets."""
import numpy as np
from PIL import Image, ImageDraw, ImageFont


def srgb_to_linear(c):
    c = c / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(c):
    c = np.clip(c, 0, 1)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(c, 1 / 2.4) - 0.055) * 255.0


_M = np.array([[0.4124564, 0.3575761, 0.1804375],
               [0.2126729, 0.7151522, 0.0721750],
               [0.0193339, 0.1191920, 0.9503041]])
_WHITE = np.array([0.95047, 1.0, 1.08883])


def rgb_to_lab(rgb):
    """rgb: (...,3) array 0..255 -> Lab (...,3)."""
    lin = srgb_to_linear(np.asarray(rgb, dtype=np.float64))
    xyz = lin @ _M.T / _WHITE
    eps = 216 / 24389
    kap = 24389 / 27
    f = np.where(xyz > eps, np.cbrt(xyz), (kap * xyz + 16) / 116)
    L = 116 * f[..., 1] - 16
    a = 500 * (f[..., 0] - f[..., 1])
    b = 200 * (f[..., 1] - f[..., 2])
    return np.stack([L, a, b], axis=-1)


def lab_to_rgb(lab):
    lab = np.asarray(lab, dtype=np.float64)
    fy = (lab[..., 0] + 16) / 116
    fx = fy + lab[..., 1] / 500
    fz = fy - lab[..., 2] / 200
    eps = 216 / 24389
    kap = 24389 / 27
    def finv(f):
        f3 = f ** 3
        return np.where(f3 > eps, f3, (116 * f - 16) / kap)
    xyz = np.stack([finv(fx), np.where(lab[..., 0] > kap * eps, fy ** 3, lab[..., 0] / kap), finv(fz)], axis=-1) * _WHITE
    lin = xyz @ np.linalg.inv(_M).T
    return np.clip(np.round(linear_to_srgb(lin)), 0, 255)


def hexof(rgb):
    r, g, b = [int(round(v)) for v in rgb]
    return f"#{r:02x}{g:02x}{b:02x}"


def rgb_of(hexs):
    h = hexs.lstrip('#')
    return np.array([int(h[i:i + 2], 16) for i in (0, 2, 4)], dtype=np.float64)


def lch(rgb):
    L, a, b = rgb_to_lab(np.asarray(rgb, dtype=np.float64))
    C = float(np.hypot(a, b))
    H = float(np.degrees(np.arctan2(b, a)) % 360)
    return float(L), C, H


def kmeans_lab(pixels_rgb, k, iters=25, seed=1):
    """pixels_rgb: (N,3). Returns (centers_rgb (k,3) as member means, counts (k,))."""
    rng = np.random.default_rng(seed)
    X = rgb_to_lab(pixels_rgb)
    n = len(X)
    # k-means++ init
    centers = [X[rng.integers(n)]]
    d2 = np.sum((X - centers[0]) ** 2, axis=1)
    for _ in range(1, k):
        p = d2 / d2.sum()
        centers.append(X[rng.choice(n, p=p)])
        d2 = np.minimum(d2, np.sum((X - centers[-1]) ** 2, axis=1))
    C = np.array(centers)
    for _ in range(iters):
        dist = ((X[:, None, :] - C[None, :, :]) ** 2).sum(-1)
        lab = dist.argmin(1)
        newC = np.array([X[lab == j].mean(0) if np.any(lab == j) else C[j] for j in range(k)])
        if np.allclose(newC, C, atol=0.05):
            C = newC
            break
        C = newC
    dist = ((X[:, None, :] - C[None, :, :]) ** 2).sum(-1)
    lab = dist.argmin(1)
    counts = np.bincount(lab, minlength=k)
    rgbc = np.array([pixels_rgb[lab == j].mean(0) if counts[j] else lab_to_rgb(C[j]) for j in range(k)])
    order = np.argsort(-counts)
    return rgbc[order], counts[order]


def swatch_sheet(rows, path, sw=46, label_w=170):
    """rows: list of (title, [(rgb, text), ...]). Writes a PNG sheet."""
    font = ImageFont.load_default()
    maxn = max(len(r[1]) for r in rows)
    cols = min(maxn, 10)
    lines_per_row = [(len(r[1]) + cols - 1) // cols for r in rows]
    H = sum(16 + l * (sw + 26) for l in lines_per_row) + 10
    W = cols * (sw + 64) + 20
    im = Image.new('RGB', (W, H), (24, 24, 28))
    d = ImageDraw.Draw(im)
    y = 6
    for (title, items), nl in zip(rows, lines_per_row):
        d.text((8, y), title, fill=(230, 230, 230), font=font)
        y += 16
        for i, (rgb, txt) in enumerate(items):
            cx = 10 + (i % cols) * (sw + 64)
            cy = y + (i // cols) * (sw + 26)
            d.rectangle([cx, cy, cx + sw, cy + sw], fill=tuple(int(v) for v in rgb), outline=(0, 0, 0))
            for li, t in enumerate(str(txt).split('\n')):
                d.text((cx, cy + sw + 2 + li * 11), t, fill=(210, 210, 210), font=font)
        y += nl * (sw + 26)
    im.save(path)
