"""Composing unit figures in the atlas look (docs/ART.md 10.2).

Two kinds of material, both at atlas 2's crop scale (a sprite pixel is ~2.9 crop pixels), so that
quantize.py turns them into sprites exactly as it turns the cut atlas figures:

- atlas crops, recoloured along a palette ramp, mirrored, scaled, patched (a rider erased, a back
  painted over the gap) and combined;
- painted parts: a shape mask (ellipses, polygons, tapered strokes) shaded as a lit dome, top-left
  light, posterized to a ramp of palette colours, with a dark rim so overlapping parts stay apart.

Everything is RGBA float arrays (0..255). Layers are placed on a Canvas by their feet (the bottom
centre of their opaque box) relative to the canvas anchor."""
import json, math, os
import numpy as np
from PIL import Image, ImageDraw
from colorlib import rgb_to_lab, lab_to_rgb, rgb_of
from pal_v3 import BASE, EXTRAS

HEX = {k: h for (_, k, n, h, f) in BASE}
HEX.update({k: h for (k, n, h, f) in EXTRAS})


def col(k):
    """A palette key (or '#rrggbb') -> rgb float array."""
    return rgb_of(HEX[k] if len(k) == 1 else k)


def ramp(keys):
    """'cICR' -> list of rgb, dark to light."""
    return [col(k) for k in keys]


# ------------------------------------------------------------------ small image maths
def box1(a, r, axis):
    if r <= 0:
        return a
    pad = [(0, 0)] * a.ndim
    pad[axis] = (r + 1, r)
    c = np.cumsum(np.pad(a, pad), axis=axis)
    n = a.shape[axis]
    hi = np.take(c, range(2 * r + 1, 2 * r + 1 + n), axis=axis)
    lo = np.take(c, range(0, n), axis=axis)
    return (hi - lo) / (2 * r + 1)


def blur(a, sigma):
    """Gaussian-ish blur (three box passes per axis)."""
    if sigma <= 0:
        return a
    w = math.sqrt(4 * sigma * sigma + 1)
    r = max(1, int(round((w - 1) / 2)))
    for _ in range(3):
        a = box1(box1(a, r, 0), r, 1)
    return a


BAYER = np.array([[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]) / 16.0 - 0.47


def bayer(h, w):
    return np.tile(BAYER, (h // 4 + 1, w // 4 + 1))[:h, :w]


# ------------------------------------------------------------------ shapes
class Shape:
    """A mask drawn at 4x supersampling. Coordinates are in layer pixels (floats)."""
    SS = 4

    def __init__(self, w, h):
        self.w, self.h = int(w), int(h)
        self.im = Image.new('L', (self.w * self.SS, self.h * self.SS), 0)
        self.d = ImageDraw.Draw(self.im)

    def _p(self, pts):
        s = self.SS
        return [(x * s, y * s) for x, y in pts]

    def ellipse(self, cx, cy, rx, ry, fill=255, rot=0):
        if rot == 0:
            s = self.SS
            self.d.ellipse([(cx - rx) * s, (cy - ry) * s, (cx + rx) * s, (cy + ry) * s], fill=fill)
        else:
            t = math.radians(rot)
            pts = [(cx + rx * math.cos(a) * math.cos(t) - ry * math.sin(a) * math.sin(t),
                    cy + rx * math.cos(a) * math.sin(t) + ry * math.sin(a) * math.cos(t))
                   for a in np.linspace(0, 2 * math.pi, 48, endpoint=False)]
            self.d.polygon(self._p(pts), fill=fill)
        return self

    def poly(self, pts, fill=255):
        self.d.polygon(self._p(pts), fill=fill)
        return self

    def rect(self, x0, y0, x1, y1, fill=255):
        s = self.SS
        self.d.rectangle([x0 * s, y0 * s, x1 * s, y1 * s], fill=fill)
        return self

    def stroke(self, pts, r0, r1=None, fill=255, smooth=True):
        """A tapered stroke through pts (Catmull-Rom smoothed), radius r0 -> r1."""
        r1 = r0 if r1 is None else r1
        P = catmull(pts) if smooth and len(pts) > 2 else [tuple(p) for p in pts]
        if len(P) == 1:
            return self.ellipse(P[0][0], P[0][1], r0, r0, fill)
        seg = [math.dist(P[i], P[i + 1]) for i in range(len(P) - 1)]
        tot = sum(seg) or 1.0
        acc = 0.0
        for i in range(len(P) - 1):
            ra = r0 + (r1 - r0) * acc / tot
            acc += seg[i]
            rb = r0 + (r1 - r0) * acc / tot
            self.capsule(P[i], P[i + 1], ra, rb, fill)
        return self

    def capsule(self, a, b, ra, rb, fill=255):
        (x0, y0), (x1, y1) = a, b
        dx, dy = x1 - x0, y1 - y0
        L = math.hypot(dx, dy) or 1e-6
        nx, ny = -dy / L, dx / L
        self.d.polygon(self._p([(x0 + nx * ra, y0 + ny * ra), (x1 + nx * rb, y1 + ny * rb),
                                (x1 - nx * rb, y1 - ny * rb), (x0 - nx * ra, y0 - ny * ra)]), fill=fill)
        self.ellipse(x0, y0, ra, ra, fill)
        self.ellipse(x1, y1, rb, rb, fill)
        return self

    def mask(self):
        m = np.asarray(self.im.resize((self.w, self.h), Image.BOX), np.float64) / 255.0
        return m


def catmull(pts, n=8):
    P = [tuple(map(float, p)) for p in pts]
    P = [P[0]] + P + [P[-1]]
    out = []
    for i in range(1, len(P) - 2):
        p0, p1, p2, p3 = P[i - 1], P[i], P[i + 1], P[i + 2]
        for k in range(n):
            t = k / n
            t2, t3 = t * t, t * t * t
            out.append(tuple(0.5 * (2 * p1[j] + (-p0[j] + p2[j]) * t + (2 * p0[j] - 5 * p1[j] + 4 * p2[j] - p3[j]) * t2
                                    + (-p0[j] + 3 * p1[j] - 3 * p2[j] + p3[j]) * t3) for j in (0, 1)))
    out.append(P[-2])
    return out


# ------------------------------------------------------------------ layers
class Layer:
    def __init__(self, a):
        self.a = np.asarray(a, np.float64).copy()
        self._lab = None

    @property
    def h(self):
        return self.a.shape[0]

    @property
    def w(self):
        return self.a.shape[1]

    @property
    def alpha(self):
        return self.a[..., 3] / 255.0

    def copy(self):
        return Layer(self.a)

    def lab(self):
        if self._lab is None:
            self._lab = rgb_to_lab(self.a[..., :3])
        return self._lab

    def _dirty(self):
        self._lab = None
        return self

    def flip(self):
        self.a = self.a[:, ::-1].copy()
        return self._dirty()

    def scale(self, f, nearest=False):
        h, w = max(1, int(round(self.h * f))), max(1, int(round(self.w * f)))
        al = self.alpha
        pre = np.concatenate([self.a[..., :3] * al[..., None], al[..., None] * 255], -1)
        method = Image.NEAREST if nearest else (Image.BOX if f < 1 else Image.BILINEAR)
        ch = [np.asarray(Image.fromarray(pre[..., c].astype(np.float32), 'F').resize((w, h), method)) for c in range(4)]
        A = np.clip(ch[3] / 255.0, 0, 1)
        rgb = np.stack(ch[:3], -1) / np.maximum(A[..., None], 1e-6)
        self.a = np.concatenate([np.clip(rgb, 0, 255), (A * 255)[..., None]], -1)
        return self._dirty()

    def pad(self, n):
        self.a = np.pad(self.a, ((n, n), (n, n), (0, 0)))
        return self._dirty()

    def trim(self, thr=8):
        ys, xs = np.nonzero(self.a[..., 3] > thr)
        if len(ys):
            self.a = self.a[ys.min():ys.max() + 1, xs.min():xs.max() + 1].copy()
        return self._dirty()

    def feet(self):
        """(x, y) of the bottom centre of the opaque box."""
        ys, xs = np.nonzero(self.a[..., 3] > 127)
        return ((xs.min() + xs.max()) / 2.0, float(ys.max()))

    # ---- selections (boolean masks)
    def sel(self, hue=None, cmin=None, cmax=None, lmin=None, lmax=None, region=None, opaque=True):
        lab = self.lab()
        L, a, b = lab[..., 0], lab[..., 1], lab[..., 2]
        C = np.hypot(a, b)
        H = (np.degrees(np.arctan2(b, a)) + 360) % 360
        m = np.ones(L.shape, bool)
        if opaque:
            m &= self.a[..., 3] > 0
        if hue is not None:
            h0, h1 = hue
            m &= (H >= h0) & (H <= h1) if h0 <= h1 else ((H >= h0) | (H <= h1))
        if cmin is not None:
            m &= C >= cmin
        if cmax is not None:
            m &= C <= cmax
        if lmin is not None:
            m &= L >= lmin
        if lmax is not None:
            m &= L <= lmax
        if region is not None:
            m &= region
        return m

    def region(self, fn):
        """A boolean mask from a Shape drawing function fn(shape)."""
        s = Shape(self.w, self.h)
        fn(s)
        return s.mask() > 0.5

    def rect(self, x0, y0, x1, y1):
        """Region by fractions of the layer box."""
        m = np.zeros((self.h, self.w), bool)
        m[int(y0 * self.h):int(math.ceil(y1 * self.h)), int(x0 * self.w):int(math.ceil(x1 * self.w))] = True
        return m

    # ---- colour
    def recolor(self, m, keys, lo=4, hi=96, gamma=1.0, keepL=False, shift=0.0):
        """Map the selected pixels onto a palette ramp (dark -> light) by their lightness rank, or,
        with keepL, keep their lightness and take hue and chroma from the ramp."""
        if not m.any():
            return self
        lab = self.lab()
        R = ramp(keys)
        RL = rgb_to_lab(np.array(R))
        L = lab[..., 0][m]
        if keepL:
            # nearest ramp entries by lightness, blended
            order = np.argsort(RL[:, 0])
            rl = RL[order]
            t = np.interp(L, rl[:, 0], np.arange(len(rl)))
        else:
            l0, l1 = np.percentile(L, lo), np.percentile(L, hi)
            t = np.clip((L - l0) / max(1e-6, l1 - l0), 0, 1) ** gamma
            t = np.clip(t + shift, 0, 1) * (len(R) - 1)
        i0 = np.floor(t).astype(int)
        i1 = np.minimum(i0 + 1, len(R) - 1)
        f = (t - i0)[:, None]
        rl = RL if not keepL else RL[np.argsort(RL[:, 0])]
        newlab = rl[i0] * (1 - f) + rl[i1] * f
        if keepL:
            newlab[:, 0] = L
        rgb = lab_to_rgb(newlab)
        a = self.a.copy()
        a[..., :3][m] = rgb
        self.a = a
        return self._dirty()

    def tint(self, m, key, amount=0.5):
        c = col(key)
        self.a[..., :3][m] = self.a[..., :3][m] * (1 - amount) + c * amount
        return self._dirty()

    def erase(self, m, soft=0):
        al = self.a[..., 3]
        if soft:
            k = blur(m.astype(float), soft)
            self.a[..., 3] = al * (1 - np.clip(k * 2, 0, 1))
        else:
            self.a[..., 3] = np.where(m, 0, al)
        return self._dirty()

    def keep(self, m):
        self.a[..., 3] = np.where(m, self.a[..., 3], 0)
        return self._dirty()

    def over(self, other, x=0, y=0):
        """Composite another layer onto this one at integer offset (x, y) (top-left)."""
        H, W = self.h, self.w
        oh, ow = other.h, other.w
        x, y = int(round(x)), int(round(y))
        x0, y0 = max(0, x), max(0, y)
        x1, y1 = min(W, x + ow), min(H, y + oh)
        if x1 <= x0 or y1 <= y0:
            return self
        src = other.a[y0 - y:y1 - y, x0 - x:x1 - x]
        dst = self.a[y0:y1, x0:x1]
        sa = src[..., 3:4] / 255.0
        da = dst[..., 3:4] / 255.0
        oa = sa + da * (1 - sa)
        rgb = (src[..., :3] * sa + dst[..., :3] * da * (1 - sa)) / np.maximum(oa, 1e-6)
        self.a[y0:y1, x0:x1] = np.concatenate([rgb, oa * 255], -1)
        return self._dirty()


def blank(w, h):
    return Layer(np.zeros((int(h), int(w), 4)))


# ------------------------------------------------------------------ painting
LIGHT = np.array([-0.55, -0.72, 0.55])
LIGHT = LIGHT / np.linalg.norm(LIGHT)


def paint(shape, keys, sigma=None, rim='0', rimw=1.6, amb=0.22, bulge=1.0, dither=0.0,
          light=LIGHT, gloss=None, flat=None, vgrad=0.0):
    """Shade a Shape (or a float mask) as a lit dome and posterize it to a palette ramp.

    sigma   roundness radius (default: from the shape's thickness)
    rim     outline key ('' for none), rimw its width in layer pixels
    gloss   a key for a small specular highlight; flat: ramp index to use without shading
    vgrad   extra darkening toward the bottom (0..1)"""
    m = shape.mask() if isinstance(shape, Shape) else np.asarray(shape, np.float64)
    h, w = m.shape
    R = ramp(keys)
    if sigma is None:
        # typical half-thickness: mean distance-ish via blur of the mask
        area = m.sum()
        per = max(1.0, (np.abs(np.diff(m > 0.5, axis=0)).sum() + np.abs(np.diff(m > 0.5, axis=1)).sum()))
        sigma = max(1.5, 1.2 * area / per)
    hgt = np.sqrt(np.clip(blur(m, sigma), 0, 1)) * sigma * bulge
    gy, gx = np.gradient(hgt)
    n = np.stack([-gx, -gy, np.ones_like(gx)], -1)
    n /= np.linalg.norm(n, axis=-1, keepdims=True)
    lam = np.clip(n @ light, 0, 1)
    v = amb + (1 - amb) * lam
    if vgrad:
        ys = np.nonzero(m.any(1))[0]
        if len(ys):
            t = np.clip((np.arange(h) - ys.min()) / max(1, ys.max() - ys.min()), 0, 1)
            v = v - vgrad * t[:, None] * 0.5
    t = np.clip(v, 0, 1) * (len(R) - 1) + dither * bayer(h, w)
    idx = np.clip(np.round(t), 0, len(R) - 1).astype(int)
    if flat is not None:
        idx[:] = flat
    rgb = np.array(R)[idx]
    if gloss:
        g = (lam > 0.985) & (m > 0.6)
        rgb[g] = col(gloss)
    if rim:
        inner = blur((m > 0.5).astype(float), max(0.6, rimw * 0.55))
        edge = (m > 0.5) & (inner < 0.83)
        rgb[edge] = col(rim)
    a = np.concatenate([rgb, (m * 255)[..., None]], -1)
    return Layer(a)


def part(w, h, draw, keys, **kw):
    """paint() a Shape drawn by draw(shape)."""
    s = Shape(w, h)
    draw(s)
    return paint(s, keys, **kw)


# ------------------------------------------------------------------ canvas
class Canvas(Layer):
    """A working surface; (ax, ay) is where the figure's feet go."""

    def __init__(self, w=240, h=220, ax=None, ay=None):
        super().__init__(np.zeros((h, w, 4)))
        self.ax = w // 2 if ax is None else ax
        self.ay = h - 12 if ay is None else ay

    def put(self, layer, x=0, y=0, anchor='feet'):
        """Place layer so its anchor lands at canvas anchor + (x, y). anchor: 'feet', 'center',
        'tl' (top-left) or an (fx, fy) point in the layer."""
        if anchor == 'feet':
            fx, fy = layer.feet()
        elif anchor == 'center':
            fx, fy = layer.w / 2.0, layer.h / 2.0
        elif anchor == 'tl':
            fx, fy = 0.0, 0.0
        else:
            fx, fy = anchor
        return self.over(layer, self.ax + x - fx, self.ay + y - fy)

    def under(self, layer, x=0, y=0, anchor='feet'):
        top = Layer(self.a)
        self.a[:] = 0
        self.put(layer, x, y, anchor)
        return self.over(top, 0, 0)

    def save(self, path):
        out = self.copy().trim(100)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        Image.fromarray(np.clip(np.round(out.a), 0, 255).astype(np.uint8), 'RGBA').save(path)
        return out


# ------------------------------------------------------------------ atlas crops
class Crops:
    def __init__(self, work):
        self.work = work
        self.scales = json.load(open(f'{work}/scales.json'))

    def get(self, src):
        """A crop at atlas 2's scale."""
        atlas = src.rsplit('_', 1)[0]
        a = np.asarray(Image.open(f'{self.work}/crops/{src}.png').convert('RGBA'), np.float64)
        L = Layer(a)
        f = self.scales[atlas] / self.scales['atlas2']
        if abs(f - 1) > 0.01:
            L.scale(f)
        return L

    def debase(self, L, rows=0.16):
        """Drop hex-base residue (grass and its pale rim) from the bottom rows of a crop."""
        h = L.h
        y0 = int(h * (1 - rows))
        reg = np.zeros((L.h, L.w), bool)
        reg[y0:] = True
        grass = L.sel(hue=(85, 150), cmin=18, region=reg)
        rim = L.sel(hue=(60, 100), cmin=20, lmin=55, region=reg)
        L.erase(grass | rim)
        # the base's dark outline survives as thin lines: an opening (radius 2) in the base rows
        op = L.a[..., 3] > 127
        k = opening(op, 2)
        L.a[..., 3] = np.where(reg & ((op & ~k) | (L.a[..., 3] < 128)), 0, L.a[..., 3])
        return L._dirty()


def erode(m, r):
    out = m.copy()
    for _ in range(r):
        e = out.copy()
        e[1:] &= out[:-1]; e[:-1] &= out[1:]; e[:, 1:] &= out[:, :-1]; e[:, :-1] &= out[:, 1:]
        out = e
    return out


def dilate(m, r):
    out = m.copy()
    for _ in range(r):
        e = out.copy()
        e[1:] |= out[:-1]; e[:-1] |= out[1:]; e[:, 1:] |= out[:, :-1]; e[:, :-1] |= out[:, 1:]
        out = e
    return out


def opening(m, r):
    return dilate(erode(m, r), r)
