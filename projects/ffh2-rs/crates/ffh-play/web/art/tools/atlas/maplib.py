"""Painting map sprites in the atlas look (docs/ART.md 5-6): features, relief, landmarks,
improvements and resources.

A `Pic` is a sprite-sized canvas painted at K times its resolution with composelib's lit domes;
coordinates are in sprite pixels (pixel (i, j) spans [i, i+1] x [j, j+1]). `Pic.rows()` turns it
into sprite text exactly as quantize.py turns a unit: area-average in linear light, nearest palette
colour in Lab, isolated pixels dropped, the outline enforced (4.2), the colour count reduced to the
namespace's budget. `rewrite()` replaces named sprites in a .spr file, keeping their metadata."""
import math, os, re, sys
import numpy as np
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from colorlib import rgb_to_lab, rgb_of, srgb_to_linear, linear_to_srgb
from composelib import Shape, blank, paint, col, Layer, LIGHT
from pal_v3 import BASE, EXTRAS

K = 6
PAL = [(k, h) for (_, k, n, h, f) in BASE] + [(k, h) for (k, n, h, f) in EXTRAS]
KEYS = [k for k, _ in PAL]
PRGB = np.array([rgb_of(h) for _, h in PAL])
PLAB = rgb_to_lab(PRGB)
KI = {k: i for i, k in enumerate(KEYS)}
LUM = PLAB[:, 0]
TRANS = -1


class S:
    """A Shape whose coordinates are sprite pixels."""

    def __init__(self, w, h, k):
        self.k = k
        self.sh = Shape(w * k, h * k)

    def ellipse(self, cx, cy, rx, ry, fill=255, rot=0):
        k = self.k
        self.sh.ellipse(cx * k, cy * k, rx * k, ry * k, fill, rot)
        return self

    def poly(self, pts, fill=255):
        k = self.k
        self.sh.poly([(x * k, y * k) for x, y in pts], fill)
        return self

    def rect(self, x0, y0, x1, y1, fill=255):
        k = self.k
        x1, y1 = max(x0, x1 * k - 0.25), max(y0, y1 * k - 0.25)
        self.sh.rect(x0 * k, y0 * k, x1, y1, fill)
        return self

    def stroke(self, pts, r0, r1=None, fill=255, smooth=True):
        k = self.k
        self.sh.stroke([(x * k, y * k) for x, y in pts], r0 * k, (r0 if r1 is None else r1) * k, fill, smooth)
        return self

    def mask(self):
        return self.sh.mask()


class Pic:
    def __init__(self, w, h, ax=None, ay=None, k=K):
        self.w, self.h, self.k = w, h, k
        self.ax = w // 2 if ax is None else ax
        self.ay = h - 1 if ay is None else ay
        self.L = blank(w * k, h * k)
        self.flat = []   # (mask fn, key) parts painted unshaded

    def add(self, draw, keys, sigma=None, rim='', rimw=0.7, **kw):
        """Paint a part: draw(s) with s an S; sigma and rimw in sprite pixels."""
        s = S(self.w, self.h, self.k)
        draw(s)
        lay = paint(s.mask(), keys, sigma=None if sigma is None else sigma * self.k, rim=rim, rimw=rimw * self.k, **kw)
        self.L.over(lay)
        return self

    def solid(self, draw, key):
        """A flat part in one colour (no shading, no rim)."""
        return self.add(draw, key + key, rim='', flat=0)

    def under(self, draw, keys, **kw):
        top = Layer(self.L.a)
        self.L = blank(self.w * self.k, self.h * self.k)
        self.add(draw, keys, **kw)
        self.L.over(top)
        return self

    # ------------------------------------------------------------ quantize
    def indices(self, outline=True, budget=8, avoid='', clean=True):
        k = self.k
        a = self.L.a
        al = a[..., 3] / 255.0
        lin = srgb_to_linear(a[..., :3])
        def rs(ch):
            return ch.reshape(self.h, k, self.w, k).mean(axis=(1, 3))
        A = rs(al)
        rgb = np.stack([rs(lin[..., c] * al) for c in range(3)], -1) / np.maximum(A[..., None], 1e-6)
        rgb = np.clip(np.round(linear_to_srgb(rgb)), 0, 255)
        lab = rgb_to_lab(rgb)
        allowed = np.array([i for i, kk in enumerate(KEYS) if kk not in avoid])
        d = ((lab[..., None, :] - PLAB[None, None, allowed, :]) ** 2).sum(-1)
        idx = allowed[d.argmin(-1)]
        idx[lab[..., 0] < 9] = KI['0']
        idx = np.where(A >= 0.5, idx, TRANS)
        if clean:
            idx = drop_isolated(idx)
        if outline:
            idx = enforce_outline(idx)
        idx = reduce_colours(idx, budget)
        return idx

    def rows(self, **kw):
        idx = self.indices(**kw)
        return [''.join('.' if v == TRANS else KEYS[v] for v in row) for row in idx]


def drop_isolated(idx):
    H, W = idx.shape
    out = idx.copy()
    for y in range(H):
        for x in range(W):
            if idx[y, x] == TRANS:
                continue
            if not any(0 <= y + dy < H and 0 <= x + dx < W and idx[y + dy, x + dx] != TRANS
                       for dy, dx in ((-1, 0), (1, 0), (0, -1), (0, 1))):
                out[y, x] = TRANS
    return out


def enforce_outline(idx):
    """Dark edge pixels become void; light edges get a void pixel outside them (inside the frame)."""
    H, W = idx.shape
    out = idx.copy()
    for y in range(H):
        for x in range(W):
            v = idx[y, x]
            if v == TRANS:
                continue
            edge = False
            for dy, dx in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                yy, xx = y + dy, x + dx
                if not (0 <= yy < H and 0 <= xx < W) or idx[yy, xx] == TRANS:
                    edge = True
                    if 0 <= yy < H and 0 <= xx < W and LUM[v] >= 34:
                        out[yy, xx] = KI['0']
            if edge and LUM[v] < 34:
                out[y, x] = KI['0']
    return out


def reduce_colours(idx, maxc):
    idx = idx.copy()
    while True:
        vals, cnt = np.unique(idx[idx >= 0], return_counts=True)
        keep = [(v, c) for v, c in zip(vals, cnt) if v != KI['0']]
        if len(keep) <= maxc:
            return idx
        v0 = min(keep, key=lambda t: t[1])[0]
        others = [v for v, _ in keep if v != v0]
        tgt = min(others, key=lambda v: float(((PLAB[v] - PLAB[v0]) ** 2).sum()))
        idx[idx == v0] = tgt


# ------------------------------------------------------------ .spr files
def parse(path):
    """[(name, header, meta lines, rows)] plus the text around them, in order."""
    lines = open(path).read().split('\n')
    items, i = [], 0
    while i < len(lines):
        m = re.match(r'^=\s*(\S+)\s+(\d+)x(\d+)\s+(-?\d+),(-?\d+)(.*)$', lines[i])
        if not m:
            items.append(('text', lines[i]))
            i += 1
            continue
        name, w, h, ax, ay, rest = m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4)), int(m.group(5)), m.group(6)
        i += 1
        meta = []
        while i < len(lines) and lines[i].startswith(':'):
            meta.append(lines[i])
            i += 1
        rows = lines[i:i + h]
        i += h
        items.append(('sprite', dict(name=name, w=w, h=h, ax=ax, ay=ay, rest=rest, meta=meta, rows=rows)))
    return items


def bright_near(rows, x, y, keys='L8lFWE', r=4):
    best = None
    for yy, row in enumerate(rows):
        for xx, c in enumerate(row):
            if c in keys:
                d = (xx - x) ** 2 + (yy - y) ** 2
                if d <= r * r and (best is None or d < best[0]):
                    best = (d, xx, yy)
    return best


def rewrite(path, new, header=None):
    """new: {name: (rows, dict(w=, h=, ax=, ay=, note=, lights=[(table, radius)]))}. Lights already on
    the sprite move to the nearest bright pixel; `lights` replaces them."""
    items = parse(path)
    out = []
    for kind, it in items:
        if kind == 'text':
            out.append(it)
            continue
        sp = it
        if sp['name'] in new:
            rows, o = new[sp['name']]
            w, h = len(rows[0]), len(rows)
            ax, ay = o.get('ax', sp['ax']), o.get('ay', sp['ay'])
            meta = []
            for m in sp['meta']:
                if m.startswith(': note') and o.get('note'):
                    m = ': note ' + o['note']
                if m.startswith(': light') and 'lights' not in o:
                    mm = re.match(r': light (-?\d+),(-?\d+) (\w+) (\d+)', m)
                    if mm:
                        b = bright_near(rows, int(mm.group(1)), int(mm.group(2)))
                        if b:
                            m = f': light {b[1]},{b[2]} {mm.group(3)} {mm.group(4)}'
                if m.startswith(': light') and 'lights' in o:
                    continue
                meta.append(m)
            for (tb, rad, lx, ly) in o.get('lights', []):
                meta.append(f': light {lx},{ly} {tb} {rad}')
            out.append(f'= {sp["name"]} {w}x{h} {ax},{ay}{sp["rest"]}')
            out += meta + rows
        else:
            out.append(f'= {sp["name"]} {sp["w"]}x{sp["h"]} {sp["ax"]},{sp["ay"]}{sp["rest"]}')
            out += sp['meta'] + sp['rows']
    text = '\n'.join(out)
    if header:
        text = header(text)
    open(path, 'w').write(text)


def preview(sprites, path, scale=6, bg=(111, 165, 52), cols=10):
    """sprites: [(name, rows)] -> a sheet on grass at scale."""
    from PIL import ImageDraw, ImageFont
    cw = max(len(r[0]) for _, r in sprites) * scale + 10
    ch = max(len(r) for _, r in sprites) * scale + 22
    n = len(sprites)
    sheet = Image.new('RGB', (min(cols, n) * cw, ((n + cols - 1) // cols) * ch), (40, 40, 46))
    d = ImageDraw.Draw(sheet)
    font = ImageFont.load_default()
    for i, (name, rows) in enumerate(sprites):
        w, h = len(rows[0]), len(rows)
        img = Image.new('RGB', (w, h), bg)
        px = img.load()
        for y, row in enumerate(rows):
            for x, c in enumerate(row):
                if c in KI:
                    px[x, y] = tuple(int(v) for v in PRGB[KI[c]])
                elif c in '#%':
                    px[x, y] = (200, 39, 43) if c == '#' else (143, 30, 31)
                elif c == '@':
                    px[x, y] = (162, 118, 218)
        X, Y = (i % cols) * cw, (i // cols) * ch
        sheet.paste(img.resize((w * scale, h * scale), Image.NEAREST), (X + 5, Y + 2))
        d.text((X + 5, Y + h * scale + 6), name.split('/')[-1][:20], fill=(230, 230, 230), font=font)
    sheet.save(path)


class Px:
    """A sprite painted at its own resolution, like hand pixel art. Each part is a crisp shape
    (supersampled coverage of at least half a pixel), shaded per pixel by the lit-dome normal of its
    blurred mask and posterized to its ramp (dark to light). Where a part lies over an earlier one,
    its lower and right edge takes its darkest tone (sep), so lobes and tiers read apart without
    black lines inside the figure. rows() then outlines the whole silhouette once: each edge pixel
    takes its part's outline key (void by default)."""

    def __init__(self, w, h, ax=None, ay=None, scale=1.0):
        self.w, self.h = w, h
        self.ax = w // 2 if ax is None else ax
        self.ay = h - 1 if ay is None else ay
        self.scale = scale        # recipe coordinates are multiplied by this (icons drawn once at 16, rendered at 8)
        self.idx = np.full((h, w), TRANS, np.int32)
        self.okey = np.full((h, w), KI['0'], np.int32)
        self.nooutline = np.zeros((h, w), bool)

    def mask(self, draw, ss=4):
        from PIL import ImageDraw
        s = S(self.w, self.h, 1)
        s.k = self.scale
        s.sh.SS = ss
        s.sh.im = Image.new('L', (self.w * ss, self.h * ss), 0)
        s.sh.d = ImageDraw.Draw(s.sh.im)
        draw(s)
        return s.mask()

    def add(self, draw, keys, sigma=None, okey='0', gloss=None, flat=None, amb=0.3, light=None, sep=True,
            vgrad=0.0, outline=True, **_):
        cov = self.mask(draw)
        m = cov >= 0.5
        if not m.any():
            return self
        R = [KI[k] for k in keys]
        if flat is not None:
            v = np.full(m.shape, float(flat) / max(1, len(R) - 1))
        else:
            from composelib import blur
            if sigma is None:
                area = m.sum()
                per = max(1.0, (np.abs(np.diff(m, axis=0)).sum() + np.abs(np.diff(m, axis=1)).sum()))
                sigma = max(0.7, 1.0 * area / per)
            hgt = np.sqrt(np.clip(blur(cov, sigma), 0, 1)) * sigma * 1.6
            gy, gx = np.gradient(hgt)
            n = np.stack([-gx, -gy, np.ones_like(gx)], -1)
            n /= np.linalg.norm(n, axis=-1, keepdims=True)
            L = LIGHT if light is None else np.asarray(light, float) / np.linalg.norm(light)
            v = amb + (1 - amb) * np.clip(n @ L, 0, 1)
            if vgrad:
                ys = np.nonzero(m.any(1))[0]
                t = np.clip((np.arange(self.h) - ys.min()) / max(1, ys.max() - ys.min()), 0, 1)
                v = v - vgrad * t[:, None]
            # stretch the part's values over its ramp so every part uses its whole range
            lo, hi = np.percentile(v[m], 3), np.percentile(v[m], 97)
            v = np.clip((v - lo) / max(1e-6, hi - lo), 0, 1)
        t = np.clip(v, 0, 1) * (len(R) - 1)
        ci = np.array(R)[np.clip(np.round(t), 0, len(R) - 1).astype(int)]
        if gloss:
            g = m & (v >= np.percentile(v[m], 95)) & (v > 0.85)
            ci[g] = KI[gloss]
        if sep:
            below = np.zeros_like(m); below[:-1] = self.idx[1:] != TRANS
            right = np.zeros_like(m); right[:, :-1] = self.idx[:, 1:] != TRANS
            nb_out_d = np.zeros_like(m); nb_out_d[:-1] = ~m[1:]; nb_out_d[-1] = True
            nb_out_r = np.zeros_like(m); nb_out_r[:, :-1] = ~m[:, 1:]; nb_out_r[:, -1] = True
            sepm = m & ((nb_out_d & below) | (nb_out_r & right))
            ci[sepm] = R[0]
        self.idx = np.where(m, ci, self.idx)
        self.okey = np.where(m, KI[okey], self.okey)
        self.nooutline = np.where(m, not outline, self.nooutline)
        return self

    def solid(self, draw, key, outline=False):
        return self.add(draw, key + key, flat=0, outline=outline, sep=False)

    def put(self, x, y, key, outline=True, okey='0'):
        """Set one pixel (pixel-rule generators); key '.' clears it."""
        x, y = int(x * self.scale), int(y * self.scale)
        if 0 <= x < self.w and 0 <= y < self.h:
            self.idx[y, x] = TRANS if key == '.' else KI[key]
            self.okey[y, x] = KI[okey]
            self.nooutline[y, x] = not outline
        return self

    def under(self, draw, keys, **kw):
        top, ok, no = self.idx.copy(), self.okey.copy(), self.nooutline.copy()
        self.idx[:] = TRANS
        self.add(draw, keys, **kw)
        keep = top != TRANS
        self.idx = np.where(keep, top, self.idx)
        self.okey = np.where(keep, ok, self.okey)
        self.nooutline = np.where(keep, no, self.nooutline)
        return self

    def rows(self, budget=8, outline=True, **_):
        idx = self.idx.copy()
        if outline:
            # outline the silhouette where it is thick; thin strokes (fronds, twigs) keep their colour
            m = idx != TRANS
            pad = np.pad(m, 1)
            interior = pad[:-2, 1:-1] & pad[2:, 1:-1] & pad[1:-1, :-2] & pad[1:-1, 2:] & m
            edge = m & ~interior & ~self.nooutline
            ip = np.pad(interior, 1)
            near = ip[:-2, 1:-1] | ip[2:, 1:-1] | ip[1:-1, :-2] | ip[1:-1, 2:]
            idx = np.where(edge & near, self.okey, idx)
        idx = reduce_colours(idx, budget)
        return [''.join('.' if v == TRANS else KEYS[v] for v in row) for row in idx]
