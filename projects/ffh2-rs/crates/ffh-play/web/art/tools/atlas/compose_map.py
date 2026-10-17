"""Redraw the map sprites in the atlas look (docs/ART.md 5-6): python3 compose_map.py ART_DIR [PREVIEW_DIR] [GROUP...]

Groups: features, relief, landmarks, improvements, resources. Each recipe paints a sprite at K x
its size with maplib (lit domes posterized to palette ramps) and quantizes it into its frame; the
sprite keeps its name, frame, anchor and metadata (`: place`, `: flat`, `: types`; `: light` moves
to the nearest bright pixel), so the mapping and the placement rules are untouched."""
import math, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from maplib import Pic, Px, parse, rewrite, preview

ART = sys.argv[1]
PREV = sys.argv[2] if len(sys.argv) > 2 else None
ONLY = set(sys.argv[3:])
GROUPS = {}


def group(name, path, budget, outline=True):
    def deco(fn):
        g = GROUPS.setdefault(name, dict(path=path, recipes={}))
        g['recipes'][fn.__name__] = (fn, budget, outline)
        return fn
    return deco


def frame_of(path):
    return {it['name']: it for kind, it in parse(path) if kind == 'sprite'}


# ================================================================== trees and features
PINE = 'ktTm'
PINE_MOSS = 'qmoO'
CANOPY = 'ktTo'
BARK = 'hru'


def conifer(p, cx, foot, height, width, tiers=3, keys=PINE, trunk=1.5, gloss='O'):
    """A pine from its trunk foot (cx, foot): overlapping tiers, widest at the bottom, tips drooping."""
    p.add(lambda s: s.rect(cx - trunk / 2, foot - trunk * 2, cx + trunk / 2, foot), BARK, sigma=0.6, sep=False)
    top = foot - height
    base = foot - trunk * 1.3
    span = base - top
    for i in range(tiers):
        yb = base - span * (i / tiers) * 0.82
        yt = top if i == tiers - 1 else yb - span * 0.5
        w = width * (1.0 - 0.22 * i)
        pts = [(cx - w / 2, yb), (cx - w * 0.36, yb - 0.9), (cx, yt), (cx + w * 0.36, yb - 0.9), (cx + w / 2, yb), (cx, yb - 0.5)]
        p.add(lambda s, pts=pts: s.poly(pts), keys, sigma=max(1.0, w / 3.0), gloss=gloss)


def broadleaf(p, cx, foot, height, width, keys=CANOPY, trunk=2.0, lobes=None, gloss='O', roots=False):
    th = height * 0.38
    p.add(lambda s: s.poly([(cx - trunk / 2 - (0.8 if roots else 0), foot), (cx - trunk * 0.4, foot - th), (cx + trunk * 0.4, foot - th),
                            (cx + trunk / 2 + (0.8 if roots else 0), foot)]), BARK, sigma=0.8)
    cy = foot - th - (height - th) * 0.45
    lobes = lobes or [(-0.28, 0.12, 0.34), (0.28, 0.1, 0.34), (0, -0.18, 0.4), (-0.1, 0.2, 0.3), (0.14, 0.22, 0.28)]
    for (dx, dy, r) in lobes:
        p.add(lambda s, dx=dx, dy=dy, r=r: s.ellipse(cx + dx * width, cy + dy * height, r * width, r * width * 0.9),
              keys, sigma=r * width * 0.7, gloss=gloss)


def palm(p, cx, foot, height, lean=0.0, keys='qmoO', fronds=5):
    tx = cx + lean * height * 0.4
    ty = foot - height * 0.72
    p.add(lambda s: s.stroke([(cx, foot), (cx + lean * height * 0.15, foot - height * 0.35), (tx, ty)], 0.9, 0.65), 'hrij', sigma=0.6)
    for i in range(fronds):
        a = math.radians(-160 + i * (140 / max(1, fronds - 1)))
        L = height * 0.42
        ex, ey = tx + math.cos(a) * L, ty + math.sin(a) * L * 0.55 + L * 0.25
        mx, my = tx + math.cos(a) * L * 0.5, ty + math.sin(a) * L * 0.5 - 0.6
        p.add(lambda s, mx=mx, my=my, ex=ex, ey=ey: s.stroke([(tx, ty), (mx, my), (ex, ey)], 1.5, 0.5), keys, sigma=0.9)
    p.add(lambda s: s.ellipse(tx, ty + 0.3, 1.0, 0.8), 'hri', sigma=0.6)


def deadtree(p, cx, foot, height, keys='hBD'):
    p.add(lambda s: s.stroke([(cx, foot), (cx, foot - height * 0.75)], height * 0.12, height * 0.08), keys, sigma=0.7)
    p.add(lambda s: (s.stroke([(cx, foot - height * 0.5), (cx - height * 0.3, foot - height * 0.8)], height * 0.07, 0.3),
                     s.stroke([(cx, foot - height * 0.62), (cx + height * 0.32, foot - height * 0.95)], height * 0.07, 0.3),
                     s.stroke([(cx, foot - height * 0.75), (cx + height * 0.05, foot - height)], height * 0.06, 0.3)), keys, sigma=0.6)


# ---------------------------------------------------------------- pixel-rule generators
def pine_px(p, cx, foot, tiers, keys='mTtk', trunk='ru'):
    """A pine by pixel rules: tiers = [(rows, half-width at the bottom)] from the top tier down; each
    tier widens a pixel a row, its left edge lit, its right half shaded, its last row the dark
    underside with drooping tips. The void outline (rows() adds it) closes the slanted sides, but
    each tier's underside row keeps its darkest green: a forest is many overlapping pines, and a
    black bar under every tier would make it a dark lattice that outshouts the units (docs/ART.md
    4.3)."""
    lit, base, shade, deep = keys
    y = foot - 1 - sum(r for r, _ in tiers) + (len(tiers) - 1)   # tiers overlap a row
    p.put(cx, y - 1, lit)                                         # the tip
    for ti, (rows, hw) in enumerate(tiers):
        start = max(0, hw - rows + 1)
        for r in range(rows):
            half = start + r
            yy = y + r
            last = r == rows - 1
            for dx in range(-half, half + 1):
                x = cx + dx
                if last:
                    k = deep if abs(dx) < half else shade
                elif dx == -half:
                    k = lit
                elif dx < -half / 3:
                    k = base if dx > -half else lit
                elif dx <= half / 3:
                    k = base if dx <= 0 else shade
                else:
                    k = shade if dx < half else deep
                p.put(x, yy, k, okey=deep if last else '0')
            if last and half >= 2:
                p.put(cx - half - 1, yy, shade, okey=deep)
                p.put(cx + half + 1, yy, deep, okey=deep)
        y += rows - 1
    for yy in range(y + 1, foot + 1):
        p.put(cx, yy, trunk[0])
        if len(trunk) > 1 and yy == foot:
            p.put(cx, yy, trunk[1])


def palm_px(p, cx, foot, height, lean=1, crown='Oow', trunk='ri', reach=5):
    """A palm by pixel rules: a ringed trunk curving toward `lean`, five drooping two-row fronds."""
    lit, base, dark = crown
    pts = []
    for i in range(height):
        f = i / max(1, height - 1)
        pts.append((cx + round(lean * f * f * 2.5), foot - i))
    for i, (x, y) in enumerate(pts):
        p.put(x, y, trunk[i % 2])
    hx, hy = pts[-1]
    def frond(d, rise, n):
        # a two-row leaf, lit above and dark below, no outline (it would swallow it)
        for k in range(1, n + 1):
            x = hx + d * k
            y = hy - rise + (k * k) // (n + 1)
            p.put(x, y, lit if k < n - 1 else base, outline=False)
            p.put(x, y + 1, dark, outline=False)
    frond(-1, 1, reach); frond(1, 1, reach)
    frond(-1, 0, reach - 1); frond(1, 0, reach - 1)
    for k in range(1, 3):
        p.put(hx, hy - k, lit, outline=False)
        p.put(hx + lean, hy - k - 1, base, outline=False)
    p.put(hx, hy, 'h'); p.put(hx + 1, hy, 'h')


def deadtree_px(p, cx, foot, height, keys='DB'):
    lit, dark = keys
    for i in range(height - 1):
        p.put(cx, foot - i, dark)
        if height >= 7 and i < height // 2:
            p.put(cx - 1, foot - i, lit)
    top = foot - height + 1
    for (dx, dy) in ((-1, 1), (-2, 0), (1, 2), (2, 1), (3, 0), (0, 0)):
        if abs(dx) < height // 2 + 1:
            p.put(cx + dx, top + dy + (height // 4), lit if dx < 0 else dark)


def bush_px(p, x0, y0, w, h, keys='OAz'):
    lit, base, dark = keys
    for y in range(h):
        for x in range(w):
            if (x in (0, w - 1)) and y == 0 and w > 2:
                continue
            k = lit if y == 0 and x < w - 1 else (dark if y == h - 1 or x == w - 1 else base)
            p.put(x0 + x, y0 + y, k, outline=False)
    for x in range(w):
        if x not in (0, w - 1):
            p.put(x0 + x, y0 + h, '0', outline=False)


def mote_px(p, cx, cy, tall):
    for i in range(tall):
        p.put(cx, cy - i, 'W' if 0 < i < tall - 1 else 'S', outline=False)
    if tall >= 3:
        p.put(cx - 1, cy - tall // 2, 's', outline=False)
        p.put(cx + 1, cy - tall // 2, 's', outline=False)


F = os.path.join('terrain', 'features.spr')


@group('features', F, 7)
def conifer_a(p): pine_px(p, 6, 14, [(4, 2), (5, 4), (5, 6)])
@group('features', F, 7)
def conifer_b(p): pine_px(p, 5, 8, [(4, 3), (4, 5)])
@group('features', F, 7)
def conifer_c(p): pine_px(p, 5, 9, [(4, 3), (5, 5)], keys='OomQ' if False else 'Oomq')
@group('features', F, 7)
def conifer_d(p): pine_px(p, 6, 14, [(3, 2), (4, 3), (4, 4), (5, 6)])
@group('features', F, 7)
def conifer_e(p): pine_px(p, 5, 7, [(3, 2), (4, 5)])
@group('features', F, 7)
def conifer_f(p): pine_px(p, 5, 11, [(3, 2), (4, 3), (5, 5)])
@group('features', F, 7)
def conifer_g(p): pine_px(p, 5, 10, [(3, 2), (4, 4), (4, 5)], keys='OTtk')
@group('features', F, 7)
def sapling_a(p): pine_px(p, 3, 5, [(2, 1), (3, 3)])
@group('features', F, 7)
def sapling_b(p): pine_px(p, 3, 6, [(3, 2), (3, 3)], keys='Oomq')
@group('features', F, 7)
def sapling_c(p): pine_px(p, 3, 6, [(3, 2), (3, 3)], keys='OTtk')
@group('features', F, 7)
def ancient_a(p): broadleaf(p, 8.5, 20, 19, 15, keys='kktT', trunk=3.0, roots=True)
@group('features', F, 7)
def ancient_b(p): broadleaf(p, 7.5, 19, 18, 13, keys='kktT', trunk=2.6, roots=True)
@group('features', F, 7)
def ancient_c(p): broadleaf(p, 6.5, 17, 16, 11, keys='kktT', trunk=2.4, roots=True)
@group('features', F, 7)
def jungle_a(p): palm_px(p, 5, 14, 10, lean=1, reach=6)
@group('features', F, 7)
def jungle_b(p): broadleaf(p, 7.5, 12, 12, 13, keys='qmoO', trunk=1.6)
@group('features', F, 7)
def jungle_c(p):
    for (cx, r) in ((3.5, 3.4), (7.5, 4), (11.5, 3.4)):
        p.add(lambda s, cx=cx, r=r: s.ellipse(cx, 6.2, r, 2.8), 'qmoO', sigma=1.6, gloss='O')
@group('features', F, 7)
def jungle_d(p): palm_px(p, 6, 14, 11, lean=-1, crown='omw', reach=5)
@group('features', F, 6)
def deadtree_a(p): deadtree_px(p, 5, 9, 9)
@group('features', F, 6)
def deadtree_b(p): deadtree_px(p, 3, 7, 7)
@group('features', F, 6)
def deadtree_c(p): deadtree_px(p, 4, 6, 6)
@group('features', F, 6)
def deadtree_d(p): deadtree_px(p, 2, 4, 4)


def floodplain(p, seed):
    import random
    rnd = random.Random(seed)
    for _ in range(26):
        x, y = rnd.uniform(1, 29), rnd.uniform(2, 17)
        p.add(lambda s, x=x, y=y: s.ellipse(x, y, rnd.uniform(1.4, 2.8), 0.8), rnd.choice(['oO', 'oO', 'jQ', 'mO']), sigma=0.5, sep=False)
    for _ in range(6):
        x, y = rnd.uniform(3, 27), rnd.uniform(3, 16)
        p.solid(lambda s, x=x, y=y: s.ellipse(x, y, 1.5, 0.75), 'g')


@group('features', F, 6, outline=False)
def floodplain_a(p): floodplain(p, 3)
@group('features', F, 6, outline=False)
def floodplain_b(p): floodplain(p, 11)


@group('features', F, 8)
def oasis(p):
    p.add(lambda s: s.ellipse(9.5, 15.5, 7, 2.6), 'dnNg', sigma=1.5, gloss='F')
    p.add(lambda s: s.ellipse(9.5, 15.2, 7.6, 3.1, fill=255), 'mo', sigma=0.5)
    p.add(lambda s: s.ellipse(9.5, 15.5, 6.6, 2.3), 'dnNg', sigma=1.4, gloss='F')
    palm(p, 13.5, 15, 14, lean=-0.25, fronds=6)


def floes(p, seed):
    import random
    rnd = random.Random(seed)
    spots = [(5, 5), (13, 4), (23, 5), (8, 11), (18, 10), (27, 11), (4, 17), (14, 17), (24, 18), (10, 22), (21, 22)]
    for (x, y) in spots:
        x += rnd.uniform(-1, 1); y += rnd.uniform(-0.6, 0.6)
        w, h = rnd.uniform(3.2, 4.6), rnd.uniform(1.4, 2.0)
        p.add(lambda s, x=x, y=y, w=w, h=h: s.ellipse(x, y + 0.8, w, h), 'Ng', sigma=0.6)      # frost body under
        p.add(lambda s, x=x, y=y, w=w, h=h: s.ellipse(x, y, w, h), 'gfF8', sigma=1.2, gloss='8')


@group('features', F, 6)
def ice_a(p): floes(p, 1)
@group('features', F, 6)
def ice_b(p): floes(p, 7)


@group('features', F, 5)
def scrub_a(p): bush_px(p, 0, 1, 3, 2); bush_px(p, 2, 0, 3, 3)
@group('features', F, 5)
def scrub_b(p): bush_px(p, 0, 0, 4, 2)
@group('features', F, 5)
def scrub_c(p): bush_px(p, 0, 1, 3, 2, keys='Yjr'); bush_px(p, 3, 0, 3, 3, keys='Yjr')


def fires(p, phase):
    for i, (x, h) in enumerate(((6, 7), (16, 8.5), (26, 7))):
        hh = h * (1.0 if (i + phase) % 2 == 0 else 0.85)
        p.add(lambda s, x=x, hh=hh: s.stroke([(x, 9.0), (x + 0.4, 9.0 - hh * 0.5), (x - 0.3 + phase * 0.6, 9.0 - hh)], 3.0, 0.4), 'eElL', sigma=1.1, okey='e')
        p.add(lambda s, x=x, hh=hh: s.stroke([(x, 9.5), (x, 9.5 - hh * 0.5)], 1.0, 0.3), 'lLL', sigma=0.6)
        p.add(lambda s, x=x: s.ellipse(x, 9.4, 2.6, 0.7), 'h0', sigma=0.5)


@group('features', F, 6)
def flames_f1(p): fires(p, 0)
@group('features', F, 6)
def flames_f2(p): fires(p, 1)


@group('features', F, 4)
def shard_a(p): p.add(lambda s: s.poly([(0.4, 6), (1.4, 0.2), (2.6, 6)]), '0vpP', sigma=0.5, gloss='P')
@group('features', F, 4)
def shard_b(p): p.add(lambda s: s.poly([(0.3, 4), (1.6, 0.3), (2.7, 4)]), '0vpP', sigma=0.5, gloss='P')
@group('features', F, 4)
def shard_c(p): p.add(lambda s: s.poly([(0.3, 5), (1.2, 1.0), (2.3, 0.3), (3.7, 5)]), '0vpP', sigma=0.6, gloss='P')


@group('features', F, 8)
def volcano(p):
    p.add(lambda s: s.poly([(1, 40), (11, 14), (19, 14), (29, 40)]), '2345', sigma=5, gloss='6')
    p.add(lambda s: (s.stroke([(13.5, 15), (12, 22), (13, 28)], 1.0, 0.6), s.stroke([(17, 15), (18.5, 21), (17.5, 26)], 0.9, 0.5)), 'eEl', sigma=0.6)
    p.add(lambda s: s.ellipse(15, 14.3, 4.4, 1.6), 'ceEl', sigma=1.0)
    for (x, y, r) in ((16, 10, 2.4), (18.5, 7, 2.8), (21.5, 3.6, 3.0)):
        p.add(lambda s, x=x, y=y, r=r: s.ellipse(x, y, r, r * 0.85), '456', sigma=1.4)


def blizzard(p, phase):
    import random
    rnd = random.Random(5 + phase)
    for _ in range(22):
        x, y = rnd.uniform(1, 31), rnd.uniform(1, 29)
        L = rnd.uniform(1.5, 3.5)
        p.solid(lambda s, x=x, y=y, L=L: s.stroke([(x, y), (x + L, y + L * 0.5)], 0.35, 0.35, smooth=False), rnd.choice('fF8'))


@group('features', F, 4, outline=False)
def blizzard_f1(p): blizzard(p, 0)
@group('features', F, 4, outline=False)
def blizzard_f2(p): blizzard(p, 1)


@group('features', F, 7)
def fallout(p):
    import random
    rnd = random.Random(9)
    for _ in range(7):
        x, y = rnd.uniform(4, 24), rnd.uniform(6, 17)
        p.add(lambda s, x=x, y=y: s.ellipse(x, y, rnd.uniform(2.5, 4), rnd.uniform(1.0, 1.6)), 'sSW', sigma=1.0)
    for (x, y) in ((6, 9), (20, 14), (13, 17)):
        deadtree(p, x, y, 5)


@group('features', F, 4)
def wisp_a(p): mote_px(p, 1, 5, 5)
@group('features', F, 4)
def wisp_b(p): mote_px(p, 2, 4, 4)
@group('features', F, 4)
def wisp_c(p): mote_px(p, 1, 3, 3)


def door(p, open_):
    p.add(lambda s: s.poly([(0.5, 8), (0.5, 2.5), (5, 0.3), (9.5, 2.5), (9.5, 8)]), '3456', sigma=1.5)
    if open_:
        p.solid(lambda s: s.poly([(2.5, 8), (2.5, 3.4), (5, 2.0), (7.5, 3.4), (7.5, 8)]), '0')
    else:
        p.add(lambda s: s.poly([(2.5, 8), (2.5, 3.4), (5, 2.0), (7.5, 3.4), (7.5, 8)]), 'hriu', sigma=1.0)
        p.solid(lambda s: s.ellipse(6.4, 5.6, 0.45, 0.45), 'K')


@group('features', F, 6)
def door_west_closed(p): door(p, False)
@group('features', F, 6)
def door_west_open(p): door(p, True)
@group('features', F, 6)
def door_north_closed(p): door(p, False)
@group('features', F, 6)
def door_north_open(p): door(p, True)


@group('features', F, 6)
def walls(p):
    p.add(lambda s: s.rect(0, 1.6, 28, 5), '3456', sigma=1.2)
    for i in range(7):
        x = 0.5 + i * 4
        p.add(lambda s, x=x: s.rect(x, 0, x + 2.2, 2.2), '3456', sigma=0.8)


@group('features', F, 6)
def _mark(p):
    p.add(lambda s: s.rect(3.8, 3, 5.0, 11), 'hri', sigma=0.5)
    p.add(lambda s: s.poly([(5, 0.5), (8.6, 2), (5, 3.6)]), 'cCR', sigma=0.8)



# ================================================================== relief: peaks
R_ = os.path.join('terrain', 'relief.spr')


def mountain(p, sx, sy, lx, rx, by, snow=0.3, rock='3456', lit_rock='5677'.replace('77', '7'), caps=('gfF', 'fF8')):
    """A faceted peak: a lit west face and a shaded east face split by the ridge, a snow cap of
    `snow` of its height (frost in shadow, rime in light), jagged at the snow line."""
    fx = sx + (rx - sx) * 0.3
    p.add(lambda s: s.poly([(sx, sy), (lx, by), (fx, by)]), '4567', sigma=3, light=(-0.9, -0.4, 0.4))
    p.add(lambda s: s.poly([(sx, sy), (fx, by), (rx, by)]), '2345', sigma=3, light=(-0.9, -0.4, 0.4), sep=False)
    # ledges: a few dark strokes down the faces
    p.add(lambda s: (s.stroke([(sx - (sx - lx) * 0.35, sy + (by - sy) * 0.45), (sx - (sx - lx) * 0.2, sy + (by - sy) * 0.6)], 0.35, 0.3, smooth=False),
                     s.stroke([(sx + (rx - sx) * 0.45, sy + (by - sy) * 0.5), (sx + (rx - sx) * 0.55, sy + (by - sy) * 0.7)], 0.35, 0.3, smooth=False)),
          '33', flat=0, sep=False, outline=False)
    if snow:
        h = (by - sy) * snow
        yl = sy + h
        def cap(s, side):
            xl, xr = sx - (sx - lx) * snow, sx + (rx - sx) * snow
            xm = sx + (fx - sx) * snow
            jag = [(xl, yl), (xl + (xm - xl) * 0.33, yl - h * 0.25), (xl + (xm - xl) * 0.66, yl + h * 0.05), (xm, yl - h * 0.2)]
            jagr = [(xm, yl - h * 0.2), (xm + (xr - xm) * 0.4, yl + h * 0.1), (xm + (xr - xm) * 0.7, yl - h * 0.2), (xr, yl)]
            if side == 'lit':
                s.poly([(sx, sy)] + jag)
            else:
                s.poly([(sx, sy)] + jagr)
        p.add(lambda s: cap(s, 'lit'), '7fF8', sigma=1.5, sep=False)
        p.add(lambda s: cap(s, 'shade'), 'gff', sigma=1.5, sep=False)


def massif(p, peaks, snow):
    for (sx, sy, lx, rx, by, k) in peaks:
        mountain(p, sx, sy, lx, rx, by, snow=snow * k)


PEAKS = {
    'a': (32, 35, [(9, 14, 0.5, 18, 35, 0.6), (24, 12, 15, 31.5, 35, 0.6), (15.5, 0.5, 2, 29, 35, 1.0)]),
    'b': (32, 29, [(23, 8, 13, 31.5, 29, 0.7), (12, 1, 0.5, 25, 29, 1.0)]),
    'c': (29, 31, [(7, 12, 0.5, 15, 31, 0.6), (17, 1, 5, 28.5, 31, 1.0)]),
    'd': (29, 25, [(20, 6, 11, 28.5, 25, 0.7), (11, 0.5, 0.5, 22, 25, 1.0)]),
}
for _v, (_w, _h, _pk) in PEAKS.items():
    GROUPS.setdefault('relief', dict(path=R_, recipes={}))
    GROUPS['relief']['recipes'][f'peak__rock_{_v}'] = ((lambda pk=_pk: (lambda p: massif(p, pk, 0.22)))(), 8, True)
    GROUPS['relief']['recipes'][f'peak__snow_{_v}'] = ((lambda pk=_pk: (lambda p: massif(p, pk, 0.55)))(), 8, True)



# ================================================================== building blocks
RUNES = {}
def _load_runes():
    import re
    here = os.path.dirname(os.path.abspath(__file__))
    lines = open(os.path.join(here, 'glyphs', 'runes.txt')).read().split('\n')
    i = 0
    while i < len(lines):
        m = re.match(r'^= res/(mana_\w+) 7x7', lines[i])
        if m:
            i += 1
            while lines[i].startswith(':'):
                i += 1
            RUNES[m.group(1)] = lines[i:i + 7]
            i += 7
        else:
            i += 1
_load_runes()


def stamp(p, rows, x0, y0, outline=False):
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c != '.':
                p.put(x0 + x, y0 + y, c, outline=outline)


WALL = 'jQY7'      # plastered walls, lit cream
THATCH = 'riyY'    # straw roofs
TILE = 'cICR'      # red tile roofs
STONE = '3456'
WOOD = 'hriu'


def hut(p, x, base, w, h, roof=THATCH, wall=WALL, door=True, lit=False, roof_h=None):
    """A cottage: walls (x..x+w, rising h from base) under a gabled roof."""
    rh = roof_h or max(2.5, w * 0.45)
    p.add(lambda s: s.rect(x + 0.5, base - h, x + w - 0.5, base), wall, sigma=1.0)
    p.add(lambda s: s.poly([(x - 0.4, base - h + 0.6), (x + w / 2, base - h - rh), (x + w + 0.4, base - h + 0.6)]), roof, sigma=1.2)
    if door:
        p.solid(lambda s: s.rect(x + w / 2 - 0.8, base - min(h - 0.5, 2.4), x + w / 2 + 0.8, base), 'h')
    if lit:
        p.solid(lambda s: s.rect(x + w - 2.4, base - h + 1.2, x + w - 1.2, base - h + 2.4), 'L')


def keep(p, x, base, w, h, stone=STONE, merlons=True, door=True, window=True):
    p.add(lambda s: s.rect(x, base - h, x + w, base), stone, sigma=1.4)
    if merlons:
        n = max(2, int(w // 2))
        for i in range(n):
            mx = x + i * (w / n)
            p.add(lambda s, mx=mx: s.rect(mx + 0.1, base - h - 1.4, mx + w / n * 0.6, base - h + 0.4), stone, sigma=0.6, sep=False)
    if door:
        p.solid(lambda s: s.poly([(x + w / 2 - 1, base), (x + w / 2 - 1, base - 2.2), (x + w / 2, base - 2.9), (x + w / 2 + 1, base - 2.2), (x + w / 2 + 1, base)]), '0')
    if window and h > 6:
        p.solid(lambda s: s.rect(x + w / 2 - 0.5, base - h + 1.6, x + w / 2 + 0.5, base - h + 3.0), '0')


def menhir(p, x, base, w, h, keys='3456', lean=0):
    p.add(lambda s: s.poly([(x, base), (x + lean * 0.5, base - h * 0.8), (x + w / 2 + lean, base - h), (x + w + lean * 0.5, base - h * 0.8), (x + w, base)]), keys, sigma=1.0)


def mound(p, cx, base, rx, ry, keys='uijQ'):
    p.add(lambda s: s.ellipse(cx, base, rx, ry), keys, sigma=max(1.0, ry * 0.7))


def cave(p, cx, base, rx, ry, keys='3456', mouth='0', bone=True):
    mound(p, cx, base, rx, ry, keys)
    p.solid(lambda s: s.ellipse(cx, base, rx * 0.42, ry * 0.62), mouth)
    if bone:
        p.add(lambda s: s.stroke([(cx + rx * 0.5, base - 0.6), (cx + rx * 0.8, base - 1.2)], 0.45, 0.45), '78', flat=1, sep=False, outline=False)


def flame(p, x, base, h, w=None):
    w = w or h * 0.32
    p.add(lambda s: s.stroke([(x, base), (x + 0.3, base - h * 0.5), (x - 0.2, base - h)], w, 0.3), 'eElL', sigma=max(0.8, w * 0.6), okey='e', light=(0, -0.3, 0.95))
    p.add(lambda s: s.stroke([(x, base - 0.2), (x, base - h * 0.45)], w * 0.45, 0.2), 'lLL', sigma=0.6, sep=False, outline=False)


def boat(p, cx, waterline, L, sail=True, sail_keys='G78', hull=WOOD):
    p.add(lambda s: s.poly([(cx - L / 2, waterline - 2), (cx + L / 2, waterline - 2.4), (cx + L / 2 - 1.6, waterline), (cx - L / 2 + 1.2, waterline)]), hull, sigma=1.0)
    if sail:
        p.add(lambda s: s.rect(cx - 0.4, waterline - 2 - L * 0.55, cx + 0.4, waterline - 2), 'hr', flat=1, sep=False)
        p.add(lambda s: s.poly([(cx + 0.5, waterline - 2.4 - L * 0.5), (cx + L * 0.38, waterline - 3), (cx + 0.5, waterline - 3)]), sail_keys, sigma=1.0)


def ripples(p, pts):
    for (x, y, w) in pts:
        p.solid(lambda s, x=x, y=y, w=w: s.ellipse(x, y, w, 0.5), 'f')


def obelisk(p, w, h, rune=None, stone='2345', glow=True):
    cx = w / 2
    p.add(lambda s: s.poly([(cx - 3.6, h), (cx - 3.0, h - 2), (cx - 2.6, 3.4), (cx, 0.4), (cx + 2.6, 3.4), (cx + 3.0, h - 2), (cx + 3.6, h)]), stone, sigma=1.6)
    if rune:
        stamp(p, RUNES[rune], int(cx - 3.5), int(h * 0.42))


def fence(p, x0, x1, y, keys='ijQ', post=2.0):
    p.add(lambda s: s.rect(x0, y - 1.4, x1, y - 0.8), keys, flat=1, sep=False)
    x = x0
    while x <= x1 + 0.01:
        p.add(lambda s, x=x: s.rect(x - 0.45, y - 2.6, x + 0.45, y), keys, sigma=0.5, sep=False)
        x += post


def beast(p, x, base, keys='78G', head='right'):
    """A small grazing beast (sheep-like): a woolly body, a dark head, two legs."""
    p.add(lambda s: s.ellipse(x, base - 2.0, 2.4, 1.5), keys, sigma=1.0)
    hx = x + (2.4 if head == 'right' else -2.4)
    p.add(lambda s: s.ellipse(hx, base - 2.4, 0.9, 0.8), '0BD', sigma=0.6)
    p.solid(lambda s: (s.rect(x - 1.4, base - 1, x - 0.7, base), s.rect(x + 0.7, base - 1, x + 1.4, base)), '0')


# ================================================================== improvements
I = os.path.join('cities', 'improvements.spr')


@group('improvements', I, 6, outline=False)
def farm(p):
    for r in range(5):
        y = 1.6 + r * 2.1
        off = (r % 2) * 1.2
        p.add(lambda s, y=y: s.rect(0.5, y + 0.5, 23.5, y + 1.2), 'qm', flat=0, sep=False)
        x = 1 + off
        while x < 22.5:
            p.add(lambda s, x=x, y=y: s.ellipse(x + 0.9, y + 0.1, 0.95, 0.9), 'ylKY', sigma=0.7, sep=False)
            x += 2.3


@group('improvements', I, 7)
def cottage(p): hut(p, 1, 8, 6, 3.4, roof_h=3.2)
@group('improvements', I, 8)
def hamlet(p):
    hut(p, 0.6, 12, 7, 4, roof=THATCH)
    hut(p, 7.2, 12, 7, 5, roof=TILE, lit=True)
@group('improvements', I, 8)
def village(p):
    hut(p, 0.6, 11, 6, 3.6)
    hut(p, 12.4, 11, 6, 3.6, roof=TILE, lit=True)
    hut(p, 6, 12, 7, 4.6, roof=TILE, lit=True)
@group('improvements', I, 8)
def town(p):
    hut(p, 0.5, 12, 5, 3.4)
    hut(p, 17.5, 12, 5, 3.4, roof=TILE, lit=True)
    p.add(lambda s: s.rect(10, 4, 13, 13), WALL, sigma=1.0)
    p.add(lambda s: s.poly([(9.5, 4.6), (11.5, 0.2), (13.5, 4.6)]), TILE, sigma=0.8)
    hut(p, 4.5, 13.5, 6, 4.4, roof=TILE, lit=True)
    hut(p, 12.5, 13.5, 6, 4.4, lit=True)


@group('improvements', I, 7)
def mine(p):
    mound(p, 9, 11, 8.6, 6.5, keys='uijQ')
    p.add(lambda s: s.rect(6, 4.5, 12, 11), WOOD, sigma=0.8)
    p.solid(lambda s: s.rect(7.2, 5.8, 10.8, 11), '0')
    p.add(lambda s: s.ellipse(14.8, 9.6, 2.6, 1.6), '456', sigma=1.0)
    p.add(lambda s: s.ellipse(16, 8.6, 1.0, 0.8), 'yK', sigma=0.6)


@group('improvements', I, 6)
def _mark(p):
    p.add(lambda s: s.rect(4.4, 2, 5.6, 9), WOOD, sigma=0.5)
    p.add(lambda s: s.poly([(5.6, 1.2), (9.4, 2.6), (5.6, 4.4)]), 'cCR', sigma=0.8)


@group('improvements', I, 6)
def _lair(p): cave(p, 9, 11, 8.6, 6.5, keys='3456')
@group('improvements', I, 7)
def _landmark(p):
    p.add(lambda s: s.ellipse(13, 11.5, 9, 2.2), 'sS', sigma=1.0, sep=False, outline=False)
    for (x, h) in ((3, 7), (9, 9), (15, 8.5), (21, 7)):
        menhir(p, x, 12.5, 3, h)


@group('improvements', I, 7)
def _node(p):
    obelisk(p, 12, 14)


@group('improvements', I, 7)
def _cairn(p):
    for (x, y, r) in ((4, 9, 2.6), (10, 9, 2.6), (7, 7, 2.6), (7, 4.6, 2.0)):
        p.add(lambda s, x=x, y=y, r=r: s.ellipse(x, y, r, r * 0.8), '3456', sigma=1.0)
    flame(p, 7, 3.2, 3.0, 0.7)


@group('improvements', I, 7)
def _boat(p):
    boat(p, 8, 7.6, 12, sail=False)
    for x in (2, 4, 6):
        p.solid(lambda s, x=x: s.ellipse(x, 8.4, 0.5, 0.4), 'Y')


def stilt_shack(p, x, base, w, h, flag=False, lit=False):
    for px_ in (x + 0.8, x + w - 0.8):
        p.add(lambda s, px_=px_: s.rect(px_ - 0.4, base - 3, px_ + 0.4, base), 'hr', flat=1, sep=False)
    hut(p, x, base - 3, w, h, roof=WOOD, wall='rijQ', door=True, lit=lit)
    if flag:
        p.add(lambda s: s.rect(x + w - 0.6, base - 3 - h - 6, x + w + 0.2, base - 3 - h + 1), 'hr', flat=1, sep=False)
        p.add(lambda s: s.poly([(x + w + 0.2, base - 3 - h - 6), (x + w + 4, base - 3 - h - 5), (x + w + 0.2, base - 3 - h - 3.6)]), '012', sigma=0.8)


@group('improvements', I, 7)
def pirate_cove(p): stilt_shack(p, 0.6, 13, 6, 3.4, flag=True)
@group('improvements', I, 8)
def pirate_harbor(p):
    stilt_shack(p, 0.5, 16, 6, 3.6)
    stilt_shack(p, 7.5, 16, 6, 4, flag=True, lit=True)
@group('improvements', I, 8)
def pirate_port(p):
    stilt_shack(p, 0.5, 17, 6, 3.6, lit=True)
    stilt_shack(p, 7.5, 17, 6, 4.4, flag=True)
    stilt_shack(p, 14, 17, 6, 3.6, lit=True)


@group('improvements', I, 7)
def castle(p):
    keep(p, 0.6, 15, 10.8, 5.0)
    keep(p, 2.8, 10.4, 6.4, 6.0, door=False)
    p.add(lambda s: s.rect(5.6, 0.2, 6.2, 4.4), 'hr', flat=1, sep=False)
    p.add(lambda s: s.poly([(6.2, 0.4), (9.4, 1.4), (6.2, 2.6)]), '78', sigma=0.6)
@group('improvements', I, 7)
def citadel(p):
    keep(p, 1.5, 17, 15, 5.0)
    keep(p, 0.4, 12.5, 4.6, 9.5, door=False)
    keep(p, 13, 12.5, 4.6, 9.5, door=False)
    keep(p, 6, 12.5, 6, 9, door=False)


@group('improvements', I, 6)
def fort(p):
    x = 0.8
    while x < 17.2:
        p.add(lambda s, x=x: s.poly([(x, 10), (x, 3.4), (x + 0.75, 2.4), (x + 1.5, 3.4), (x + 1.5, 10)]), WOOD, sigma=0.6, sep=False)
        x += 1.6


@group('improvements', I, 6)
def tower(p):
    p.add(lambda s: (s.stroke([(0.8, 16), (2.6, 4)], 0.5, 0.5, smooth=False), s.stroke([(6.2, 16), (4.4, 4)], 0.5, 0.5, smooth=False),
                     s.stroke([(1.4, 12), (5.6, 8)], 0.4, 0.4, smooth=False), s.stroke([(5.6, 12), (1.4, 8)], 0.4, 0.4, smooth=False)), WOOD, flat=1, sep=False)
    p.add(lambda s: s.rect(1.2, 2.2, 5.8, 4.6), WOOD, sigma=0.6)
    p.add(lambda s: s.poly([(0.8, 2.4), (3.5, 0.0), (6.2, 2.4)]), TILE, sigma=0.6)
    p.solid(lambda s: s.rect(3, 3, 4, 4), 'L')


@group('improvements', I, 7)
def pasture(p):
    p.add(lambda s: (s.rect(0.5, 8.4, 21.5, 9.2), s.rect(0.5, 5.4, 21.5, 6.1)), 'ijQ', flat=1, sep=False, outline=False)
    for x in (1, 7, 13.5, 20.5):
        p.add(lambda s, x=x: s.rect(x, 4.4, x + 1, 11), 'rij', sigma=0.4, sep=False)
    for (x, d) in ((5.5, 'right'), (15.5, 'left')):
        p.add(lambda s, x=x: (s.ellipse(x, 7.6, 3.0, 2.0), s.ellipse(x - 1.2, 6.8, 1.6, 1.4), s.ellipse(x + 1.2, 6.8, 1.6, 1.4)), '678', sigma=1.0)
        hx = x + (3.0 if d == 'right' else -3.0)
        p.add(lambda s, hx=hx: s.ellipse(hx, 6.8, 1.1, 1.0), '0BD', sigma=0.5)
        p.solid(lambda s, x=x: (s.rect(x - 1.6, 9.2, x - 0.8, 10.6), s.rect(x + 0.8, 9.2, x + 1.6, 10.6)), '0')


@group('improvements', I, 6, outline=False)
def plantation(p):
    for r in range(3):
        y = 2.2 + r * 2.8
        x = 1.2 + (r % 2) * 1.4
        while x < 21:
            p.add(lambda s, x=x, y=y: s.ellipse(x, y, 1.2, 1.1), 'qmoO', sigma=0.8, sep=False)
            p.solid(lambda s, x=x, y=y: s.ellipse(x + 0.3, y + 0.2, 0.35, 0.35), 'G')
            x += 2.8


@group('improvements', I, 6, outline=False)
def winery(p):
    for r in range(3):
        y = 2.4 + r * 2.8
        p.add(lambda s, y=y: s.rect(0.6, y, 21.4, y + 0.55), 'ri', flat=0, sep=False)
        x = 1.5
        while x < 21:
            p.add(lambda s, x=x, y=y: s.ellipse(x, y - 0.4, 1.0, 0.9), 'qmo', sigma=0.6, sep=False)
            if int(x) % 4 == 1:
                p.solid(lambda s, x=x, y=y: s.ellipse(x + 0.4, y + 0.5, 0.45, 0.5), 'p')
            x += 2.0


@group('improvements', I, 8)
def windmill(p):
    p.add(lambda s: s.poly([(4, 20.5), (5, 9), (10, 9), (11, 20.5)]), WALL, sigma=1.4)
    p.add(lambda s: s.poly([(4.4, 9.4), (7.5, 5.6), (10.6, 9.4)]), TILE, sigma=0.8)
    p.solid(lambda s: s.rect(6.6, 17.5, 8.4, 20.5), 'h')
    p.solid(lambda s: s.rect(7, 12, 8, 13.2), 'L')
    for a in (40, 130, 220, 310):
        ar = math.radians(a)
        ex, ey = 7.5 + math.cos(ar) * 7, 7.6 + math.sin(ar) * 7
        p.add(lambda s, ex=ex, ey=ey: s.stroke([(7.5, 7.6), (ex, ey)], 0.9, 0.9, smooth=False), '7G8', flat=1, sep=False)
    p.add(lambda s: s.ellipse(7.5, 7.6, 1.0, 1.0), 'hr', sigma=0.5)


def _mana_node(rune):
    return lambda p: obelisk(p, 9, 17, rune)
for _r in ('air', 'body', 'chaos', 'death', 'earth', 'enchantment', 'entropy', 'fire', 'law', 'life', 'metamagic', 'mind', 'nature', 'shadow', 'spirit', 'sun', 'water'):
    GROUPS['improvements']['recipes']['mana_' + _r] = (_mana_node('mana_' + _r), 8, True)


@group('improvements', I, 6)
def graveyard(p):
    for (x, h) in ((1.5, 4), (5.5, 4.6), (9.5, 4)):
        p.add(lambda s, x=x, h=h: s.poly([(x, 8.5), (x, 8.5 - h + 1), (x + 1.2, 8.5 - h), (x + 2.4, 8.5 - h + 1), (x + 2.4, 8.5)]), '4567', sigma=0.8)
    p.add(lambda s: (s.rect(13.2, 1.5, 14.2, 8.5), s.rect(12, 3, 15.4, 4)), '78', sigma=0.5)


@group('improvements', I, 7)
def workshop(p):
    hut(p, 1, 11.5, 10, 5.4, roof=STONE, wall='rijQ', door=True)
    p.add(lambda s: s.rect(7.6, 1.6, 9, 5), STONE, sigma=0.5, sep=False)
    p.add(lambda s: (s.ellipse(8.4, 0.9, 1.2, 0.9)), '56', sigma=0.6, sep=False, outline=False)
    p.solid(lambda s: s.rect(5.6, 9.5, 6.4, 10.5), 'E')


@group('improvements', I, 8)
def enclave(p):
    p.add(lambda s: s.rect(0.5, 4, 21.5, 12.5), STONE, sigma=1.2)
    p.add(lambda s: s.rect(2, 5, 20, 11.5), 'qmo', sigma=1.5, sep=False)
    for x in (5, 11, 17):
        p.add(lambda s, x=x: s.ellipse(x, 7, 2.2, 2.4), 'qmoO', sigma=1.2)
    p.solid(lambda s: s.rect(10, 11, 12, 12.5), 'h')


@group('improvements', I, 7)
def jungle_altar(p):
    for i, (w, y0) in enumerate(((14, 7), (10, 4.4), (6, 2))):
        x = (14 - w) / 2
        p.add(lambda s, x=x, w=w, y0=y0: s.rect(x, y0, x + w, y0 + 3), STONE if i else '345D', sigma=0.8)
    p.add(lambda s: s.rect(4, 1.4, 10, 2.4), 'cC', flat=1, sep=False)


@group('improvements', I, 8)
def portal(p):
    p.add(lambda s: (s.ellipse(7.5, 8, 7, 7.5), s.ellipse(7.5, 8, 4.4, 5, fill=0)), STONE, sigma=1.2)
    p.add(lambda s: s.ellipse(7.5, 8, 4.4, 5), 'vpPW', sigma=2.0, light=(0, 0, 1), sep=False)
    p.add(lambda s: s.stroke([(7.5, 8), (9.5, 6), (8, 4.4), (5.6, 6), (6, 9.6), (9, 10.6)], 0.5, 0.5), 'WW', flat=0, sep=False, outline=False)
    p.add(lambda s: s.rect(2, 15.2, 13, 17), STONE, sigma=0.8)


@group('improvements', I, 6)
def barrow(p):
    mound(p, 10, 9, 9.6, 6.5, keys='qmoO')
    p.add(lambda s: s.rect(8, 4.6, 12, 9), STONE, sigma=0.6)
    p.solid(lambda s: s.rect(9, 5.6, 11, 9), '0')
    p.solid(lambda s: s.rect(9.6, 7, 10.4, 7.8), 'S')


@group('improvements', I, 6)
def bear_den(p): cave(p, 10, 13, 9.6, 8, keys='uijQ')
@group('improvements', I, 6)
def lion_den(p): cave(p, 10, 13, 9.6, 8, keys='xyY7')


@group('improvements', I, 8)
def goblin_fort(p):
    hut(p, 7, 10, 7, 3.4, roof=WOOD, wall='rij')
    flame(p, 15.5, 12.5, 4)
    x = 0.6
    while x < 20.5:
        y = 14.6 if (x < 4 or x > 17) else 15
        p.add(lambda s, x=x: s.poly([(x, 15), (x, 10.6), (x + 0.6, 9.4), (x + 1.2, 10.6), (x + 1.2, 15)]), WOOD, sigma=0.5, sep=False)
        x += 1.5 if (x < 4 or x > 16) else 7.5


@group('improvements', I, 7)
def ruins(p):
    for (x, h) in ((1.5, 9), (6.5, 12), (12, 6)):
        p.add(lambda s, x=x, h=h: s.rect(x, 13 - h, x + 2.6, 13), '567', sigma=0.8)
        p.add(lambda s, x=x, h=h: s.rect(x - 0.4, 13 - h - 0.8, x + 3, 13 - h + 0.2), '567', sigma=0.5, sep=False)
    p.add(lambda s: s.ellipse(11.5, 12, 3.2, 1.2), '567', sigma=0.8)
    p.add(lambda s: (s.stroke([(2, 12), (3.4, 9), (2.2, 6)], 0.45, 0.4), s.stroke([(7, 12), (8.6, 8), (7.2, 4)], 0.45, 0.4)), 'qm', flat=1, sep=False, outline=False)


@group('improvements', I, 7)
def snake_pillar(p):
    p.add(lambda s: s.rect(2, 2, 6, 20), '4567', sigma=1.0)
    p.add(lambda s: s.rect(1.2, 0.6, 6.8, 2.6), '4567', sigma=0.6, sep=False)
    p.add(lambda s: s.stroke([(1.6, 18), (6.4, 15), (1.6, 12), (6.4, 9), (1.6, 6), (5, 3.4)], 0.8, 0.6), 'sTo', sigma=0.6, sep=False)
    p.solid(lambda s: s.ellipse(5.4, 3.2, 0.5, 0.5), 'C')


@group('improvements', I, 6)
def monster_skeleton(p):
    for i in range(5):
        x = 4 + i * 4
        p.add(lambda s, x=x: s.stroke([(x, 13), (x - 1, 6), (x + 2.6, 1.6 + abs(i - 2) * 0.8)], 0.7, 0.45), '678', sigma=0.5, sep=False)
    p.add(lambda s: s.stroke([(2, 12.4), (22, 12.4)], 0.6, 0.6, smooth=False), '67', flat=1, sep=False)


@group('improvements', I, 7)
def dungeon(p):
    p.add(lambda s: s.poly([(0.5, 12), (0.5, 4), (6.5, 0.4), (12.5, 4), (12.5, 12)]), STONE, sigma=1.4)
    p.solid(lambda s: s.poly([(3.4, 12), (3.4, 5), (6.5, 3), (9.6, 5), (9.6, 12)]), '0')
    for i in range(3):
        p.add(lambda s, i=i: s.rect(4 + i * 0.4, 8.5 + i * 1.2, 9 - i * 0.4, 9.2 + i * 1.2), '34', flat=1, sep=False, outline=False)
    p.solid(lambda s: s.ellipse(6.5, 11.4, 1.6, 0.5), 'e')


@group('improvements', I, 6)
def city_ruins(p):
    p.add(lambda s: s.poly([(0.5, 10), (0.5, 5), (3, 5), (3.6, 6.4), (6, 6), (6, 10)]), '456', sigma=0.8)
    p.add(lambda s: s.poly([(8, 10), (8, 1.2), (10, 0.4), (12, 2), (12, 10)]), '456', sigma=0.8)
    p.add(lambda s: s.poly([(13.5, 10), (13.5, 6.6), (17, 5.4), (19.5, 7), (19.5, 10)]), '456', sigma=0.8)


@group('improvements', I, 7)
def camp(p):
    p.add(lambda s: s.poly([(0.6, 9.5), (5, 1.0), (9.4, 9.5)]), 'rijQ', sigma=1.2)
    p.solid(lambda s: s.poly([(4, 9.5), (5, 5), (6, 9.5)]), '0')
    flame(p, 12, 9.4, 4.2)


@group('improvements', I, 7)
def fishing_boats(p):
    boat(p, 9, 7.6, 12)
    for x in (1.5, 3.2):
        p.solid(lambda s, x=x: s.ellipse(x, 8.0, 0.5, 0.45), 'Y')


@group('improvements', I, 7)
def whaling_boats(p):
    boat(p, 6, 7.6, 10, sail=False)
    p.add(lambda s: s.stroke([(10.5, 5.4), (14, 6.8)], 0.3, 0.3, smooth=False), '88', flat=0, sep=False, outline=False)
    p.add(lambda s: s.poly([(14.6, 7.4), (16, 4.4), (17, 6.0), (18.6, 3.8), (18.2, 7.4)]), '2345', sigma=0.8)


@group('improvements', I, 6)
def penguins(p):
    for x in (2.5, 7, 11.5, 16):
        p.add(lambda s, x=x: s.ellipse(x, 4.4, 1.7, 2.8), '012', sigma=0.8)
        p.add(lambda s, x=x: s.ellipse(x + 0.4, 4.8, 0.9, 2.0), '78', sigma=0.6, sep=False)
        p.solid(lambda s, x=x: s.rect(x + 0.6, 2.6, x + 1.6, 3.2), 'l')


@group('improvements', I, 6)
def mushrooms(p):
    for (x, h, r, k) in ((3, 4, 2.6, 'cCR8'), (9, 5, 3.2, 'xyY8'), (14.5, 3.6, 2.4, 'cCR8')):
        p.add(lambda s, x=x, h=h: s.rect(x - 0.6, 7.6 - h + 1, x + 0.6, 7.6), '78', sigma=0.4)
        p.add(lambda s, x=x, h=h, r=r: s.ellipse(x, 7.6 - h + 0.6, r, r * 0.62), k, sigma=1.0)
        p.solid(lambda s, x=x, h=h: s.ellipse(x - 0.8, 7.6 - h + 0.2, 0.45, 0.4), '8')


@group('improvements', I, 7)
def quarry(p):
    mound(p, 7, 10, 7, 7, keys='4567')
    p.add(lambda s: s.poly([(4, 10), (4, 6), (7, 6), (7, 4), (10, 4), (10, 10)]), '345', sigma=0.8, sep=False)
    for (x, y) in ((13, 9.4), (16, 9.4), (14.5, 7.4)):
        p.add(lambda s, x=x, y=y: s.rect(x - 1.4, y - 1.6, x + 1.4, y + 0.4), '4567', sigma=0.6)


@group('improvements', I, 7)
def lumbermill(p):
    for i, y in enumerate((9.6, 7.6)):
        for j in range(3 - i):
            x = 2.4 + j * 2.6 + i * 1.3
            p.add(lambda s, x=x, y=y: s.ellipse(x, y, 1.4, 1.2), 'riY', sigma=0.6)
    p.add(lambda s: (s.rect(11, 3.4, 11.8, 10.5), s.rect(16, 3.4, 16.8, 10.5), s.rect(11, 3.4, 16.8, 4.2)), WOOD, flat=1, sep=False)
    p.add(lambda s: (s.ellipse(13.9, 6.6, 3.0, 3.0), s.ellipse(13.9, 6.6, 1.6, 1.6, fill=0)), '456', sigma=0.8)


@group('improvements', I, 6)
def necrototem(p):
    p.add(lambda s: s.rect(2.6, 4, 4.4, 16), '678', sigma=0.6)
    p.add(lambda s: s.ellipse(3.5, 3, 2.6, 2.4), '678', sigma=1.0)
    p.solid(lambda s: (s.ellipse(2.6, 3, 0.55, 0.55), s.ellipse(4.4, 3, 0.55, 0.55)), '0')
    p.solid(lambda s: s.ellipse(4.4, 3, 0.4, 0.4), 'S')


@group('improvements', I, 6)
def warning_post(p):
    p.add(lambda s: s.rect(2.8, 3, 4, 12), WOOD, sigma=0.5)
    p.add(lambda s: s.ellipse(3.4, 2.4, 2.0, 1.9), '678', sigma=0.8)
    p.solid(lambda s: (s.ellipse(2.8, 2.4, 0.5, 0.5), s.ellipse(4.1, 2.4, 0.5, 0.5)), '0')
    p.add(lambda s: s.poly([(4, 5), (6.6, 5.6), (5.4, 8.6), (4, 7.4)]), 'cC', sigma=0.6)


@group('improvements', I, 6)
def cage(p):
    p.add(lambda s: s.rect(0.5, 8, 8.5, 10), '345', sigma=0.6)
    for x in (1, 3, 5, 7):
        p.add(lambda s, x=x: s.rect(x, 1.6, x + 0.8, 8.2), '345', flat=1, sep=False)
    p.add(lambda s: s.rect(0.6, 0.6, 8.4, 1.8), '3456', sigma=0.5)


@group('improvements', I, 6)
def smoke(p):
    for (x, y, r) in ((3.4, 13, 1.6), (3, 10.4, 1.9), (3.8, 7.6, 2.2), (3, 4.6, 2.4), (3.8, 1.8, 2.2)):
        p.add(lambda s, x=x, y=y, r=r: s.ellipse(x, y, r, r * 0.8), '456', sigma=1.0, outline=False)
    p.solid(lambda s: s.ellipse(3.5, 14.3, 2.6, 0.7), 'e')


@group('improvements', I, 7)
def hellfire(p):
    p.add(lambda s: s.ellipse(7.5, 7, 7, 2.2), '0123', sigma=1.0)
    flame(p, 5, 7, 5.5, 1.4)
    flame(p, 9.5, 7, 6.6, 1.6)


@group('improvements', I, 7)
def ship_wreck(p):
    p.add(lambda s: s.poly([(0.5, 8), (2, 4), (12, 3), (14.5, 6), (12, 9.5), (3, 9.5)]), WOOD, sigma=1.0)
    p.solid(lambda s: (s.rect(4, 5.2, 5.2, 6.4), s.rect(8, 4.8, 9.2, 6)), '0')
    p.add(lambda s: s.stroke([(7, 3.6), (9, 0.4)], 0.45, 0.4, smooth=False), 'hr', flat=1, sep=False)


def goody(p, phase):
    for (x, y, r) in ((3, 9, 2.2), (8, 9, 2.2), (5.5, 7, 2.2), (5.5, 4.8, 1.7)):
        p.add(lambda s, x=x, y=y, r=r: s.ellipse(x, y, r, r * 0.8), '3456', sigma=0.8)
    flame(p, 5.5, 3.4, 2.6 + phase * 0.6, 0.6)


@group('improvements', I, 7)
def goody_hut_f1(p): goody(p, 0)
@group('improvements', I, 7)
def goody_hut_f2(p): goody(p, 1)


# ================================================================== landmarks
LM = os.path.join('terrain', 'landmarks.spr')


@group('landmarks', LM, 8)
def landmark_standing_stones(p):
    p.add(lambda s: (s.ellipse(17, 22, 13, 4), s.ellipse(17, 22, 9, 2.6, fill=0)), 'sSW', sigma=1.0, sep=False, outline=False)
    for (x, h, b) in ((3.4, 11, 22), (9.6, 13, 19.5), (20, 13, 19.5), (26.4, 11, 22), (14.6, 14, 26.5)):
        menhir(p, x, b, 5, h)


@group('landmarks', LM, 8)
def landmark_pool_of_tears(p):
    for (x, y) in ((2, 6), (5, 2.4), (11, 1.2), (17, 1.2), (23, 2.4), (26, 6), (23, 9.4), (5, 9.4), (14, 10.2)):
        p.add(lambda s, x=x, y=y: s.ellipse(x, y, 1.8, 1.3), STONE, sigma=0.8)
    p.add(lambda s: s.ellipse(14, 5.8, 10, 3.6), 'dnNg', sigma=1.6, gloss='F')
    p.add(lambda s: (s.ellipse(14, 5.8, 5, 1.6), s.ellipse(14, 5.8, 3.6, 1.0, fill=0)), 'SS', flat=0, sep=False, outline=False)


@group('landmarks', LM, 8)
def landmark_seven_pines(p):
    for (cx, foot, tiers) in ((4, 26, [(4, 2), (5, 3), (5, 4)]), (29, 26, [(4, 2), (5, 3), (5, 4)]), (10, 24, [(4, 2), (5, 4), (6, 5)]),
                              (23, 24, [(4, 2), (5, 4), (6, 5)]), (16, 22, [(5, 2), (6, 4), (6, 6)]), (7, 28, [(3, 2), (4, 3)]), (26, 28, [(3, 2), (4, 3)])):
        pine_px(p, cx, foot, tiers, keys='mTtk')
    for (x, y) in ((16, 3), (10, 8), (23, 8)):
        p.put(x, y, 'W', outline=False)


@group('landmarks', LM, 8)
def landmark_yggdrasil(p):
    p.add(lambda s: s.poly([(11, 34), (14, 20), (13, 14), (21, 14), (20, 20), (23, 34)]), 'hriu', sigma=2.0)
    p.add(lambda s: (s.stroke([(14, 31), (8, 33.6)], 1.2, 0.6), s.stroke([(20, 31), (26, 33.6)], 1.2, 0.6)), 'hri', sigma=0.8, sep=False)
    for (dx, dy, r) in ((-10, 12, 6), (10, 12, 6), (-5, 6, 7), (6, 6, 7), (0, 2.5, 7), (-14, 9, 4), (14, 9, 4)):
        p.add(lambda s, dx=dx, dy=dy, r=r: s.ellipse(17 + dx, dy + 5, r, r * 0.8), 'ktTo', sigma=r * 0.6, gloss='O')
    for (x, y) in ((9, 9), (17, 5), (24, 10), (13, 14), (21, 13)):
        p.put(x, y, 'W', outline=False)


@group('landmarks', LM, 8)
def landmark_bradelines_well(p):
    p.add(lambda s: s.ellipse(9.5, 9, 8.4, 4.4), STONE, sigma=1.6)
    p.add(lambda s: s.ellipse(9.5, 8, 5.6, 2.4), 'sSW', sigma=1.0, light=(0, 0, 1), sep=False)
    p.add(lambda s: (s.rect(2.6, 1.6, 3.6, 8), s.rect(15.4, 1.6, 16.4, 8), s.rect(2, 0.6, 17, 2)), WOOD, flat=1, sep=False)


@group('landmarks', LM, 8)
def landmark_tomb_of_sucellus(p):
    p.add(lambda s: s.rect(1.5, 6, 21.5, 14), '4567', sigma=1.6)
    p.add(lambda s: s.poly([(0.5, 6.6), (11.5, 0.6), (22.5, 6.6)]), '3456', sigma=1.2)
    p.add(lambda s: s.rect(9.5, 8, 13.5, 14), 'sSW', sigma=0.8, light=(0, 0, 1))


@group('landmarks', LM, 8)
def landmark_aifon_isle(p):
    p.add(lambda s: s.poly([(1, 23), (4, 15), (10, 12), (17, 10), (24, 13), (29, 18), (29, 23)]), '3456', sigma=3)
    p.add(lambda s: s.poly([(3, 23), (6, 17), (14, 15), (22, 16), (27, 23)]), 'qmo', sigma=2, sep=False)
    pine_px(p, 18, 15, [(4, 2), (5, 3), (5, 4)], keys='Ttkk')
    p.put(11, 13, 'W', outline=False)
    ripples(p, [(4, 23.4, 3), (25, 23.2, 3)])


@group('landmarks', LM, 8)
def landmark_mirror_of_heaven(p):
    p.add(lambda s: s.ellipse(12, 4.6, 11, 4), '5678', sigma=1.4)
    p.add(lambda s: s.ellipse(12, 4.4, 8, 2.6), 'gfF8', sigma=1.6, light=(0, 0, 1), sep=False)
    p.put(13, 4, '8', outline=False)


@group('landmarks', LM, 8)
def landmark_pyre_of_the_seraphic(p):
    p.add(lambda s: (s.stroke([(1, 15), (17, 11)], 1.1, 1.1, smooth=False), s.stroke([(1, 11), (17, 15)], 1.1, 1.1, smooth=False)), WOOD, sigma=0.6)
    flame(p, 9, 13, 12.5, 3.2)


@group('landmarks', LM, 8)
def landmark_broken_sepulcher(p):
    p.add(lambda s: s.rect(2, 7, 18, 13.5), '4567', sigma=1.4)
    p.solid(lambda s: s.poly([(9, 7), (10.5, 10), (9.5, 13.5), (11, 13.5), (12, 10), (11, 7)]), 'C')
    p.add(lambda s: s.poly([(12, 6.4), (21.5, 2.4), (21.5, 5.4), (13, 8.4)]), '4567', sigma=0.8)


@group('landmarks', LM, 8)
def landmark_ring_of_carcer(p):
    p.add(lambda s: s.ellipse(14.5, 11, 6, 2.6), 'cIC', sigma=1.2, sep=False, outline=False)
    for (x, h, b) in ((1.5, 7, 13.6), (6, 9, 9.6), (12.5, 9.6, 7.6), (19, 9, 9.6), (24, 7, 13.6), (12.5, 6, 15.8)):
        menhir(p, x, b, 3.4, h, keys='0123')


@group('landmarks', LM, 8)
def landmark_odios_prison(p):
    p.add(lambda s: s.rect(1, 18, 16, 24), STONE, sigma=1.2)
    for x in (2.4, 5.4, 8.4, 11.4, 14.4):
        p.add(lambda s, x=x: s.rect(x - 0.5, 3, x + 0.5, 18), '345', flat=1, sep=False)
    p.add(lambda s: s.rect(1.4, 1.4, 15.6, 3.4), '3456', sigma=0.6)
    p.add(lambda s: s.ellipse(8.5, 12, 3.2, 4.6), '012', sigma=1.2, sep=False, outline=False)
    p.solid(lambda s: (s.ellipse(7.4, 10.6, 0.5, 0.5), s.ellipse(9.6, 10.6, 0.5, 0.5)), 'R')


@group('landmarks', LM, 8)
def landmark_letum_frigus(p):
    for (x0, y0, x1, y1) in ((1, 13, 9, 22), (11, 13, 20, 22), (5, 5, 15, 14), (8, 0.6, 13, 6)):
        p.add(lambda s, a=(x0, y0, x1, y1): s.poly([(a[0], a[3]), (a[0] + 1, a[1]), (a[2] - 1, a[1] - 0.6), (a[2], a[3])]), 'NgfF', sigma=1.6, gloss='8')


@group('landmarks', LM, 8)
def landmark_maelstrom(p):
    p.add(lambda s: s.ellipse(13.5, 6.5, 13, 5.6), 'dnN', sigma=2, sep=False, outline=False)
    p.add(lambda s: s.stroke([(13.5, 6.5), (16, 5.4), (17.5, 7.4), (13.5, 9.6), (8.5, 7.4), (11, 3.2), (18, 2.4), (23, 6), (19, 11), (8, 11.4), (3, 7)], 0.55, 0.45),
          'gF', flat=1, sep=False, outline=False)
    p.solid(lambda s: s.ellipse(13.5, 6.5, 1.6, 0.9), '0')


@group('landmarks', LM, 8)
def landmark_guardian(p):
    p.add(lambda s: s.rect(1, 22, 14, 27), STONE, sigma=1.0)
    p.add(lambda s: s.poly([(3, 22), (4.6, 9), (10.4, 9), (12, 22)]), '4567', sigma=2.0)
    p.add(lambda s: s.ellipse(7.5, 6, 3.2, 3.4), '4567', sigma=1.2)
    p.add(lambda s: s.poly([(4, 7), (7.5, 1.0), (11, 7)]), '3456', sigma=1.0, sep=False)
    p.solid(lambda s: (s.ellipse(6.4, 6.4, 0.5, 0.5), s.ellipse(8.6, 6.4, 0.5, 0.5)), 'S')


@group('landmarks', LM, 7)
def landmark_dragon_bones(p):
    p.add(lambda s: s.stroke([(4, 10), (10, 8.6), (17, 8.6), (24, 10.4), (27, 12)], 0.8, 0.5), '678', sigma=0.6)
    for x in (9, 12, 15, 18):
        p.add(lambda s, x=x: s.stroke([(x, 8.8), (x - 1, 4.6), (x + 1.6, 2.4)], 0.55, 0.4), '678', sigma=0.5, sep=False)
    p.add(lambda s: s.poly([(0.5, 10.4), (2, 7), (6, 7.4), (6.6, 10.4)]), '678', sigma=0.8)
    p.solid(lambda s: s.ellipse(3, 8.6, 0.6, 0.6), '0')


@group('landmarks', LM, 6)
def landmark_remnants_of_patria(p):
    p.add(lambda s: s.poly([(0.5, 15), (0.5, 7), (4, 6), (5, 8.4), (8, 8), (8, 15)]), '4567', sigma=1.0)
    p.add(lambda s: s.rect(11, 2, 14, 15), '567', sigma=0.8)
    p.add(lambda s: s.rect(10.4, 1.0, 14.6, 2.6), '567', sigma=0.5, sep=False)
    p.add(lambda s: s.ellipse(21, 13.6, 7, 1.4), '567', sigma=0.8)
    p.add(lambda s: s.poly([(17, 15), (17, 9), (21, 8), (24, 10), (24, 15)]), '4567', sigma=1.0)



# ================================================================== resources: small objects on the tile
RS = os.path.join('cities', 'resources.spr')
RW = RH = 13


def res(fn):
    """Register a resource object (13x13, feet at 6,12)."""
    GROUPS.setdefault('resources', dict(path=RS, recipes={}))['recipes'][fn.__name__] = (fn, 8, True)
    return fn


def ingots(p, keys):
    for (x, y) in ((3.6, 12.4), (9.4, 12.4), (6.5, 9.4)):
        p.add(lambda s, x=x, y=y: s.poly([(x - 3, y), (x - 2.2, y - 3), (x + 2.2, y - 3), (x + 3, y)]), keys, sigma=0.9, gloss=keys[-1])


def rock_pile(p, ore, flecks):
    for (x, y, r) in ((4, 11, 3.2), (9, 11.2, 3.0), (6.5, 7.6, 3.0)):
        p.add(lambda s, x=x, y=y, r=r: s.ellipse(x, y, r, r * 0.8), ore, sigma=1.0)
    for (x, y) in ((4, 10), (9.6, 10.6), (6.8, 6.8)):
        p.put(x, y, flecks, outline=False)


def gem(p, cx, cy, r, keys):
    p.add(lambda s: s.poly([(cx - r, cy - r * 0.3), (cx - r * 0.5, cy - r), (cx + r * 0.5, cy - r), (cx + r, cy - r * 0.3), (cx, cy + r)]), keys, sigma=r * 0.5, gloss=keys[-1])


def quad(p, x, base, body, head, keys, legs='0', horns=None, mane=None):
    """A small beast in profile facing right: body, neck and head, legs."""
    p.add(lambda s: (s.rect(x - 3.6, base - 2.6, x - 2.6, base), s.rect(x + 1.6, base - 2.6, x + 2.6, base)), legs + legs, flat=0, sep=False)
    p.add(lambda s: s.ellipse(x, base - 4.4, body[0], body[1]), keys, sigma=1.2)
    hx, hy = x + body[0] + 0.6, base - 4.4 - body[1] - 0.2
    p.add(lambda s: s.stroke([(x + body[0] - 1, base - 4.6), (hx, hy)], 1.3, 1.0), keys, sigma=0.7)
    p.add(lambda s: s.ellipse(hx + 0.6, hy, head[0], head[1]), keys, sigma=0.7)
    if horns:
        p.add(lambda s: s.stroke([(hx, hy - head[1]), (hx - 0.6, hy - head[1] - 2), (hx + 0.8, hy - head[1] - 3.6)], 0.45, 0.35), horns, flat=1, sep=False, outline=False)
    if mane:
        p.add(lambda s: s.stroke([(x + body[0] - 1.4, base - 5.6), (hx - 0.4, hy - 0.8)], 0.7, 0.6), mane, flat=0, sep=False)


# Hand-drawn objects where the painted recipe lost the subject at 13x13 (the art review: corn read as
# a gem, wheat as a goblet, cotton as a lollipop, the pearl and the toad as blobs). Fills only: the
# void outline is added around them.
HAND_RES = {
    # three ears fanned over a leather tie, the stalks splayed below
    'wheat': ['.............',
              '...L..L..L...',
              '..LK.LKy.Ky..',
              '..Ky.KKy.yy..',
              '...KyKyyKy...',
              '....KyKyy....',
              '.....iii.....',
              '.....ijj.....',
              '....yjyjy....',
              '...yj.y.jy...',
              '..yj..y..jy..',
              '..j...j...j..',
              '.............'],
    # a cob of gold kernels in two green husk leaves peeling outward
    'corn': ['.............',
             '.....LKy.....',
             '.....KyK.....',
             '.....yKy.....',
             '.....KyK.....',
             '..O..yKy..O..',
             '..mO.KyK.Om..',
             '...mOyKyOm...',
             '....mOyOm....',
             '.....mOm.....',
             '......m......',
             '......q......',
             '.............'],
    # three white bolls on a green shrub
    'cotton': ['.............',
               '.....887.....',
               '....88776....',
               '.887.776.887.',
               '8877687688776',
               '.776.r6r.776.',
               '..r.mrmrm.r..',
               '..mmOmmmOmm..',
               '...mmOqmmq...',
               '....mmqqm....',
               '......r......',
               '......r......',
               '.............'],
    # a white pearl in an open scallop: the ribbed fan of the upper shell behind, the bowl in front
    'pearl': ['.............',
              '...PPpPpPp...',
              '..PWpPpPpPv..',
              '..PpPpPpPpv..',
              '...PpPpPpv...',
              '....pF87v....',
              '....F8876....',
              '.PP.88766.pv.',
              '.PPP87766ppv.',
              '.PPPp766ppvv.',
              '..PPPpppppv..',
              '...pppppvv...',
              '.............'],
    # a squat frog: eyes on top, a dark mouth line, a pale belly, splayed feet
    'toad': ['.............',
             '.............',
             '..OO.....OO..',
             '.OW0O...OW0O.',
             '.OOAOAAAOAAz.',
             '.OAAAAAAAAAz.',
             'OAA0000000Azz',
             'OAAAYYYYAAAzz',
             '.AAYYYYYYAzz.',
             '..AAYYYYAzz..',
             '.OAzAzzzAzzz.',
             'OA.z.....z.zz',
             '.............'],
}


def hand_res(p, fill):
    """A hand-drawn 13x13 fill with a void outline (4-neighbour) around it."""
    H, W = len(fill), len(fill[0])
    op = lambda x, y: 0 <= y < H and 0 <= x < W and fill[y][x] != '.'
    rows = [''.join('0' if c == '.' and any(op(x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))) else c
                    for x, c in enumerate(r)) for y, r in enumerate(fill)]
    stamp(p, rows, 0, 0)


@res
def _food(p): hand_res(p, HAND_RES['wheat'])
@res
def _strategic(p): rock_pile(p, '3456', 'K')
@res
def _luxury(p): gem(p, 6.5, 7.6, 4.4, 'nNgF')
@res
def _mana(p):
    p.add(lambda s: s.poly([(4, 12.4), (5, 3), (6.5, 0.8), (8, 3), (9, 12.4)]), 'vpPW', sigma=1.2, gloss='W')


@res
def corn(p): hand_res(p, HAND_RES['corn'])
@res
def rice(p):
    p.add(lambda s: s.ellipse(6.5, 7.4, 5, 3.4), 'G78', sigma=1.4)
    p.add(lambda s: s.poly([(1.2, 8), (11.8, 8), (10, 12.4), (3, 12.4)]), 'hriu', sigma=1.0)
@res
def wheat(p): hand_res(p, HAND_RES['wheat'])
@res
def cow(p): quad(p, 5.8, 12.6, (3.8, 2.4), (1.4, 1.2), 'riuj', horns='78')
@res
def pig(p):
    quad(p, 5.6, 12.6, (4.0, 2.6), (1.6, 1.4), 'baaa')
    p.solid(lambda s: s.ellipse(12.0, 5.8, 0.7, 0.7), 'R')
@res
def sheep(p):
    p.add(lambda s: (s.rect(3, 9.6, 4, 12.6), s.rect(8, 9.6, 9, 12.6)), '00', flat=0, sep=False)
    p.add(lambda s: (s.ellipse(6, 7.6, 4.2, 2.8), s.ellipse(4, 6.4, 2, 1.8), s.ellipse(7.6, 6.2, 2.2, 1.8)), '678', sigma=1.2)
    p.add(lambda s: s.ellipse(10.8, 6.6, 1.5, 1.4), '0BD', sigma=0.6)
@res
def toad(p): hand_res(p, HAND_RES['toad'])
@res
def banana(p):
    for dx in (-2.2, 0, 2.2):
        p.add(lambda s, dx=dx: s.stroke([(6.5 + dx * 0.3, 2.6), (6.5 + dx * 0.8 - 2.4, 7), (6.5 + dx - 0.6, 11.6)], 1.2, 0.8), 'ylKL', sigma=0.7)
    p.add(lambda s: s.rect(5.8, 1.2, 7.2, 3.2), 'qm', flat=0, sep=False)
@res
def deer(p): quad(p, 5.6, 12.6, (3.6, 2.2), (1.3, 1.1), 'riyY', horns='hri')
@res
def fish(p):
    p.add(lambda s: s.ellipse(6, 7.6, 4.6, 2.6), 'nNgF', sigma=1.2, gloss='F')
    p.add(lambda s: s.poly([(1.6, 7.6), (0.2, 4.8), (0.2, 10.4)]), 'nNg', sigma=0.6, sep=False)
    p.solid(lambda s: s.ellipse(8.8, 6.8, 0.5, 0.5), '0')
@res
def clam(p):
    p.add(lambda s: s.poly([(1, 11), (3, 4), (6.5, 2.4), (10, 4), (12, 11), (6.5, 12.4)]), 'jQY7', sigma=1.4)
    for dx in (-3, -1, 1, 3):
        p.add(lambda s, dx=dx: s.stroke([(6.5, 12), (6.5 + dx * 1.4, 3.6)], 0.3, 0.3, smooth=False), 'jj', flat=0, sep=False, outline=False)
@res
def crab(p):
    p.add(lambda s: s.ellipse(6.5, 9, 4.2, 2.6), 'cCR', sigma=1.2)
    p.add(lambda s: (s.ellipse(2, 5, 1.6, 1.4), s.ellipse(11, 5, 1.6, 1.4)), 'cCR', sigma=0.6)
    p.add(lambda s: (s.stroke([(3.4, 7.6), (2.2, 6)], 0.5, 0.5), s.stroke([(9.6, 7.6), (10.8, 6)], 0.5, 0.5)), 'cC', flat=1, sep=False)
@res
def whale(p):
    p.add(lambda s: s.ellipse(6.5, 10, 6, 2.8), 'dnNg', sigma=1.4)
    p.add(lambda s: (s.stroke([(6.5, 7), (5.6, 3.4), (4, 1.6)], 0.5, 0.3), s.stroke([(6.5, 7), (7.4, 3.4), (9, 1.6)], 0.5, 0.3)), 'fF', flat=1, sep=False, outline=False)
@res
def copper(p): ingots(p, 'exyl')
@res
def iron(p): ingots(p, '3456')
@res
def mithril(p): ingots(p, 'NgfF')
@res
def horse(p): quad(p, 5.4, 12.6, (3.6, 2.3), (1.4, 1.1), 'riyY', mane='h')
@res
def nightmare(p):
    quad(p, 5.4, 12.6, (3.6, 2.3), (1.4, 1.1), '0123', mane='c')
    p.solid(lambda s: s.ellipse(11.0, 4.6, 0.5, 0.5), 'R')
@res
def gunpowder(p):
    p.add(lambda s: s.ellipse(6.5, 8.6, 4, 4.2), 'hriu', sigma=1.4)
    p.add(lambda s: (s.rect(2.6, 6.2, 10.4, 6.9), s.rect(2.6, 10.4, 10.4, 11.1)), '34', flat=1, sep=False, outline=False)
    p.add(lambda s: s.stroke([(6.5, 4.4), (8, 2.4), (9.6, 1.6)], 0.35, 0.35), '45', flat=0, sep=False, outline=False)
    p.put(10, 1, 'L', outline=False)
@res
def reagents(p):
    p.add(lambda s: s.ellipse(6.5, 9, 3.6, 3.4), 'vpPW', sigma=1.2, gloss='W')
    p.add(lambda s: s.rect(5.4, 2.8, 7.6, 6.2), 'gfF', sigma=0.6)
    p.add(lambda s: s.rect(5, 1.4, 8, 3), 'hri', sigma=0.4)
@res
def sheut_stone(p):
    p.add(lambda s: s.poly([(1.6, 12.4), (2.6, 6), (6.5, 3), (10.6, 5.4), (11.6, 12.4)]), 'v012', sigma=1.4)
    p.add(lambda s: s.stroke([(4, 11), (6, 8), (5.6, 6), (8, 4.6)], 0.5, 0.4), 'CR', flat=1, sep=False, outline=False)
@res
def marble(p):
    p.add(lambda s: s.poly([(1, 12.4), (1, 6), (4, 3.6), (12, 3.6), (12, 10.4), (9, 12.4)]), 'G78', sigma=1.4)
    p.add(lambda s: s.stroke([(3, 7), (6, 9), (9, 6.4)], 0.3, 0.3), '55', flat=0, sep=False, outline=False)
@res
def dye(p):
    p.add(lambda s: s.poly([(1.6, 7), (11.4, 7), (10, 12.4), (3, 12.4)]), 'hriu', sigma=1.0)
    p.add(lambda s: s.ellipse(6.5, 7, 4.8, 1.4), 'vpP', sigma=0.6, sep=False)
    p.add(lambda s: s.stroke([(6.5, 1.0), (6.5, 4.4)], 0.4, 1.2), 'cCR', sigma=0.6, sep=False)
@res
def fur(p):
    p.add(lambda s: s.poly([(1, 4), (3.4, 2.4), (6.5, 3.4), (9.6, 2.4), (12, 4), (11, 8), (12, 12), (8, 11.4), (6.5, 12.6), (5, 11.4), (1, 12), (2, 8)]), 'riuj', sigma=1.2)
@res
def gems(p):
    gem(p, 4.2, 8.6, 3.2, 'nNgF')
    gem(p, 9.2, 9.4, 2.8, 'cCRW')
    gem(p, 7.0, 4.4, 2.6, 'sSWW')
@res
def gold(p):
    for (x, y, r) in ((3.8, 11, 2.4), (9.2, 11.2, 2.4), (6.5, 8, 2.6), (6.5, 11.6, 1.8)):
        p.add(lambda s, x=x, y=y, r=r: s.ellipse(x, y, r, r * 0.8), 'ylKL', sigma=0.9, gloss='L')
@res
def incense(p):
    p.add(lambda s: s.ellipse(6.5, 10.6, 3.4, 2), 'iyK', sigma=1.0)
    p.add(lambda s: s.rect(5.6, 11.6, 7.4, 12.6), 'iy', flat=0, sep=False)
    p.add(lambda s: s.stroke([(6.5, 8.6), (5, 6), (8, 3.6), (6, 1.0)], 0.6, 0.4), '67', flat=1, sep=False, outline=False)
@res
def ivory(p):
    p.add(lambda s: s.stroke([(2, 12), (4, 6), (8, 3), (11.6, 2.4)], 1.6, 0.5), 'G78', sigma=0.8)
@res
def silk(p):
    p.add(lambda s: s.ellipse(6.5, 8, 5.4, 3.4), 'cCRW'.replace('W', 'R'), sigma=1.4)
    p.add(lambda s: s.ellipse(2.4, 8, 1.4, 3.2), 'IC', sigma=0.6, sep=False)
    p.add(lambda s: s.poly([(10, 9.6), (12.6, 11.4), (11, 12.6), (8.6, 11)]), 'cCR', sigma=0.6)
@res
def sugar(p):
    for x in (3.6, 6.5, 9.4):
        p.add(lambda s, x=x: s.rect(x - 0.8, 1.6, x + 0.8, 12.6), 'qmoO', sigma=0.5)
        for y in (4.6, 8.0):
            p.put(x - 0.5, y, 'q', outline=False); p.put(x + 0.5, y, 'q', outline=False)
@res
def wine(p):
    for (x, y) in ((4.2, 5), (6.5, 5), (8.8, 5), (5.4, 7.2), (7.6, 7.2), (6.5, 9.4), (4.4, 7.4), (8.6, 7.4)):
        p.add(lambda s, x=x, y=y: s.ellipse(x, y, 1.3, 1.3), 'vpPW', sigma=0.6, gloss='W')
    p.add(lambda s: s.poly([(6.5, 3.4), (9, 0.6), (11.6, 2.4), (9.6, 3.6)]), 'qmo', sigma=0.6)
@res
def pearl(p): hand_res(p, HAND_RES['pearl'])
@res
def cotton(p): hand_res(p, HAND_RES['cotton'])
@res
def ale(p):
    p.add(lambda s: s.rect(2.6, 4.4, 9, 12.6), 'hriu', sigma=1.0)
    p.add(lambda s: s.ellipse(5.8, 3.8, 3.8, 1.8), '78', sigma=0.6, sep=False)
    p.add(lambda s: (s.ellipse(10.4, 8, 1.8, 2.4), s.ellipse(10.4, 8, 0.8, 1.4, fill=0)), 'hri', sigma=0.5)
@res
def fine_clothes(p):
    for i, y in enumerate((11.6, 8.6, 5.6)):
        p.add(lambda s, y=y, i=i: s.rect(1.6 + i * 0.6, y - 2.6, 11.4 - i * 0.6, y), ('dnJN', 'cICR', 'tTmo')[i], sigma=0.8)
        p.add(lambda s, y=y, i=i: s.rect(1.6 + i * 0.6, y - 0.8, 11.4 - i * 0.6, y), 'yK', flat=1, sep=False, outline=False)
@res
def jewels(p):
    p.add(lambda s: (s.ellipse(6.5, 8.6, 4.2, 3.6), s.ellipse(6.5, 8.6, 2.6, 2.2, fill=0)), 'ylKL', sigma=0.8)
    gem(p, 6.5, 4.4, 2.4, 'cCRW')
@res
def razorweed(p):
    for (x, lean) in ((4.4, -1.6), (6.5, 0.4), (8.8, 1.8)):
        p.add(lambda s, x=x, lean=lean: s.stroke([(x, 12.6), (x + lean * 0.5, 7), (x + lean, 1.4)], 0.7, 0.4), 'wqm', flat=1, sep=False)
        for y in (4, 7, 10):
            p.put(x + lean * (12.6 - y) / 11 + 1, y, 'C', outline=False)
@res
def gulagarm(p):
    p.add(lambda s: s.ellipse(6.5, 8.6, 4.4, 4), 'hriu', sigma=1.2)
    p.add(lambda s: s.rect(4.6, 2.8, 8.4, 5.4), 'riu', sigma=0.5)
    p.add(lambda s: s.ellipse(6.5, 2.8, 2.4, 1.0), 'cC', sigma=0.4, sep=False)
@res
def fruit_of_yggdrasil(p):
    p.add(lambda s: s.ellipse(6.5, 8.4, 4, 4), 'ylKL', sigma=1.4, gloss='8')
    p.add(lambda s: s.poly([(6.5, 4.4), (9.6, 1.0), (11.6, 2.6), (9, 4.6)]), 'qmo', sigma=0.6)
    p.put(3, 3, 'W', outline=False)


def mana_stone(rune):
    def f(p):
        p.add(lambda s: s.poly([(1.6, 12.6), (2.2, 3.2), (6.5, 0.4), (10.8, 3.2), (11.4, 12.6)]), '3456', sigma=1.4)
        stamp(p, RUNES[rune], 3, 3)
    return f
for _r in sorted(RUNES):
    GROUPS.setdefault('resources', dict(path=RS, recipes={}))['recipes'][_r] = (mana_stone(_r), 8, True)


NOTES = [
    ('feature/conifer_', 'pine by pixel rules: tiers lit on the west edge, shaded east, dark undersides, drooping tips'),
    ('feature/sapling_', 'new forest: a young pine by the same rules'),
    ('feature/ancient_', 'ancient forest: a broad canopy of lit domes over a thick rooted trunk'),
    ('feature/jungle_a', 'jungle: a palm, ringed trunk and two-row fronds'), ('feature/jungle_d', 'jungle: a palm, ringed trunk and two-row fronds'),
    ('feature/jungle_b', 'jungle: a bright round canopy'), ('feature/jungle_c', 'jungle: a low row of bushes'),
    ('feature/deadtree_', 'burnt forest: a charred trunk and bare branches'),
    ('feature/floodplain_', 'flood plains: lush tufts, silt and water glints; a flat ground mark'),
    ('feature/oasis', 'oasis: a blue pool in a green bank under a leaning palm'),
    ('feature/ice_', 'ice: floes of rime and frost on the water, glint shade beneath'),
    ('feature/scrub_', 'scrub: small dry bushes'), ('feature/flames_', 'burning sands: three fires (2-frame flicker), FIRE pools'),
    ('feature/shard_', 'obsidian plains: black glass shards with a lilac glint'),
    ('feature/volcano', 'volcano: a faceted cone, lava runs from a glowing crater, a smoke plume; BLOOD pool'),
    ('feature/blizzard_', 'blizzard: driven snow streaks across the tile (2 frames)'),
    ('feature/fallout', 'fallout: glowing pools among dead trees'), ('feature/wisp_', 'wisps: pale motes'),
    ('feature/door_', 'a door in a stone arch'), ('feature/walls', 'a crenellated stone wall'), ('feature/_mark', 'fallback: a marker post'),
    ('peak/rock_', 'massif: faceted peaks, lit west faces and shaded east, small snow caps'),
    ('peak/snow_', 'snowy massif: the same peaks under deep snow caps (frost in shadow)'),
    ('improvement/mana_', 'mana node: a slate obelisk carved with its sphere rune'),
    ('res/mana_', 'mana: a standing stone carved with its sphere rune'),
]


def note_for(full, fn):
    for pre, text in NOTES:
        if full.startswith(pre):
            return text
    doc = (fn.__doc__ or '').strip().split('\n')[0]
    if doc:
        return doc
    name = full.split('/', 1)[1].replace('landmark_', '').replace('_', ' ').strip()
    kind = {'res': 'resource', 'improvement': 'improvement'}.get(full.split('/')[0], full.split('/')[0])
    return f'{kind}: {name}, painted in the atlas look (tools/atlas/compose_map.py)'


# ================================================================== run
if __name__ == '__main__':
    for gname, g in GROUPS.items():
        if ONLY and gname not in ONLY:
            continue
        path = os.path.join(ART, g['path'])
        fr = frame_of(path)
        new, shots = {}, []
        for rname, (fn, budget, outline) in g['recipes'].items():
            name = f"{g['path'].split(os.sep)[-1].split('.')[0] == 'features' and 'feature' or ''}/{rname}" if False else None
            # the sprite's full name: namespace by file
            ns = {'features.spr': 'feature', 'relief.spr': None, 'landmarks.spr': 'improvement', 'improvements.spr': 'improvement',
                  'resources.spr': 'res'}[os.path.basename(path)]
            full = f'{ns}/{rname}' if ns else rname.replace('__', '/')
            if full not in fr:
                print('no sprite', full)
                continue
            sp = fr[full]
            frame = (13, 13, 6, 12) if gname == 'resources' else (sp['w'], sp['h'], sp['ax'], sp['ay'])
            p = Px(*frame)
            fn(p)
            rows = p.rows(budget=budget, outline=outline)
            new[full] = (rows, {'ax': frame[2], 'ay': frame[3], 'note': note_for(full, fn)})
            shots.append((full, rows))
        rewrite(path, new)
        if PREV:
            preview(shots, os.path.join(PREV, f'map_{gname}.png'))
        print(gname, len(new), 'sprites redrawn')
