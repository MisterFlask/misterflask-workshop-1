"""Compose the units the atlas sheets lack (docs/ART.md 10.2): python3 compose_units.py WORK [NAME...]

Each recipe draws one figure at atlas 2's crop scale into WORK/crops/comp/NAME.png; quantize.py
then makes sprites of them with composed_mapping.json:

    python3 compose_units.py WORK && python3 quantize.py WORK composed_mapping.json units_composed

Recipes start from an atlas crop where one fits (recoloured, patched, combined) and paint the rest
(composelib.paint: lit domes posterized to palette ramps). Coordinates of painted parts are
relative to the figure's feet: x right, y down (a standing figure spans y = -85..0)."""
import math, os, sys
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from composelib import Canvas, Crops, Layer, Shape, blank, blur, col, paint, part

WORK = sys.argv[1]
ONLY = set(sys.argv[2:])
X = Crops(WORK)
RECIPES = {}

SKIN = dict(hue=(30, 80), cmin=14, cmax=62, lmin=40)


def recipe(fn):
    RECIPES[fn.__name__] = fn
    return fn


# ------------------------------------------------------------------ painting helpers
PW, PH, PX, PY = 260, 240, 130, 200   # painted layer size and where the feet are on it


def P(draw, keys, **kw):
    """A painted part: draw(shape, fx, fy) with (fx, fy) the feet; returns a layer that knows its anchor."""
    L = part(PW, PH, lambda s: draw(s, PX, PY), keys, **kw)
    L.anc = (PX, PY)
    return L


def put(cv, layer, dx=0, dy=0):
    """Place a P() layer (or a crop by its feet) on the canvas, offset by (dx, dy)."""
    if hasattr(layer, 'anc'):
        return cv.put(layer, dx, dy, anchor=layer.anc)
    return cv.put(layer, dx, dy)


def crop(src, debase=0.16):
    L = X.get(src)
    if debase:
        X.debase(L, debase)
    return L


def feather_wing(cv, x, y, d, span=36, keys='GH8F', n=5, up=0.0, rim='0', under=True):
    """A feathered wing from the shoulder (x, y), d=-1 left / +1 right; primaries fan from steep
    up to outward. Drawn under the current figure when under=True."""
    def draw(s, fx, fy):
        sx, sy = fx + x, fy + y
        for i in range(n):
            t = i / max(1, n - 1)
            ang = math.radians(100 + 70 * t - up * 30)          # from up to outward
            L = span * (1.0 - 0.38 * t)
            ex, ey = sx + d * math.cos(ang - math.pi / 2 + math.pi / 2) * 0, sy
            dx, dy = -math.cos(ang) * d, -math.sin(ang)
            cx, cy = sx + d * abs(dx) * L * 0.5 * (1 if d > 0 else 1) * (1 if d > 0 else 1), sy + dy * L * 0.5
            cx = sx + (abs(dx) * d) * L * 0.5
            s.ellipse(cx, cy, L * 0.5, 4.2 + 1.2 * (1 - t), rot=math.degrees(math.atan2(dy, abs(dx) * d)))
        s.ellipse(sx + d * 6, sy - 3, 9, 7)
    L = P(draw, keys, sigma=3.5, rim=rim, amb=0.35)
    return cv.under(L, 0, 0, anchor=L.anc) if under else put(cv, L)


def bat_wing(cv, x, y, d, span=40, keys='cIC', bone='0', under=True):
    def mem(s, fx, fy):
        sx, sy = fx + x, fy + y
        s.poly([(sx, sy), (sx + d * span * 0.55, sy - span * 0.62), (sx + d * span * 1.0, sy - span * 0.42),
                (sx + d * span * 0.86, sy - span * 0.12), (sx + d * span * 0.72, sy + span * 0.12),
                (sx + d * span * 0.46, sy + span * 0.02), (sx + d * span * 0.2, sy + span * 0.2)])
    def ribs(s, fx, fy):
        sx, sy = fx + x, fy + y
        s.stroke([(sx, sy), (sx + d * span * 0.55, sy - span * 0.62)], 2.2, 1.4, smooth=False)
        for (a, b) in ((1.0, -0.42), (0.86, -0.12), (0.72, 0.12)):
            s.stroke([(sx + d * span * 0.55, sy - span * 0.62), (sx + d * span * a, sy + span * b)], 1.3, 0.8, smooth=False)
    W = P(mem, keys, sigma=5, amb=0.4)
    B = P(ribs, bone + bone, rim='', flat=0)
    if under:
        cv.under(B, 0, 0, anchor=B.anc)
        return cv.under(W, 0, 0, anchor=W.anc)
    put(cv, W)
    return put(cv, B)


def halo(cv, x, y, r=9, keys='yKL'):
    return put(cv, P(lambda s, fx, fy: (s.ellipse(fx + x, fy + y, r, r * 0.42), s.ellipse(fx + x, fy + y, r - 2.6, r * 0.42 - 2.2, fill=0)),
                     keys, sigma=1.5, rim='', amb=0.6))


def horns(cv, x, y, size=10, keys='0BDG', spread=1.0, curl=0.5):
    def draw(s, fx, fy):
        for d in (-1, 1):
            bx, by = fx + x + d * size * 0.45 * spread, fy + y
            s.stroke([(bx, by), (bx + d * size * 0.55, by - size * 0.5), (bx + d * size * (0.55 - curl * 0.5), by - size * 1.1)],
                     size * 0.24, 0.7)
    return put(cv, P(draw, keys, sigma=2.5, amb=0.35))


def flames(cv, pts, keys='eElL', under=False):
    """Tongues of flame: pts = [(x, y, size), ...] (base of each tongue)."""
    def draw(s, fx, fy):
        for (x, y, z) in pts:
            s.stroke([(fx + x, fy + y), (fx + x + z * 0.15, fy + y - z * 0.9), (fx + x - z * 0.1, fy + y - z * 1.7)], z * 0.55, 0.5)
    L = P(draw, keys, sigma=3, rim='e', rimw=1.2, amb=0.45, light=np.array([0.0, -0.3, 0.95]))
    return cv.under(L, 0, 0, anchor=L.anc) if under else put(cv, L)


def staff(cv, x0, y0, x1, y1, keys='hriu', r=1.7, orb=None, orb_r=4.5):
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + x0, fy + y0), (fx + x1, fy + y1)], r, r * 0.9, smooth=False), keys, sigma=1.5))
    if orb:
        put(cv, P(lambda s, fx, fy: s.ellipse(fx + x1, fy + y1, orb_r, orb_r), orb, sigma=orb_r, gloss=orb[-1]))
    return cv


def eyes(cv, pts, keys='L', r=1.5):
    return put(cv, P(lambda s, fx, fy: [s.ellipse(fx + x, fy + y, r, r) for (x, y) in pts], keys + keys, rim='', flat=0))


# ================================================================== elementals and spirits
@recipe
def fire_elemental():
    L = crop('atlas2_30')
    L.recolor(L.sel(lmin=38), 'eEElLL', lo=2, hi=99)
    L.recolor(L.sel(lmax=38), '0ce', lo=0, hi=100)
    cv = Canvas()
    flames(cv, [(-16, -42, 9), (14, -46, 10), (0, -70, 11), (-6, -60, 8), (8, -64, 8)], under=True)
    cv.put(L)
    return cv


@recipe
def air_elemental():
    cv = Canvas()
    # a whirlwind: stacked swirls, widest at the top, with a face in the cloud
    rings = [(0, -6, 7, 3.5), (2, -14, 9, 4), (-1, -23, 12, 5), (2, -33, 15, 6), (0, -45, 19, 7), (-2, -58, 22, 8), (1, -70, 20, 8)]
    for (x, y, rx, ry) in rings:
        put(cv, P(lambda s, fx, fy, x=x, y=y, rx=rx, ry=ry: s.ellipse(fx + x, fy + y, rx, ry), '5gfF8', sigma=4, rim='4', rimw=1.4, amb=0.4))
    # swirl streaks
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 18, fy - 52), (fx - 6, fy - 47), (fx + 10, fy - 50), (fx + 20, fy - 57)], 1.6, 1.0),
                                 s.stroke([(fx + 16, fy - 30), (fx + 4, fy - 27), (fx - 10, fy - 30)], 1.4, 0.9),
                                 s.stroke([(fx - 14, fy - 72), (fx, fy - 76), (fx + 16, fy - 72)], 1.5, 0.9)), 'FF', rim='', flat=0))
    eyes(cv, [(-6, -66), (6, -66)], 'n', r=2.4)
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 58, 4, 2.2), '33', rim='', flat=0))
    return cv


@recipe
def water_elemental():
    cv = Canvas()
    body = lambda s, fx, fy: (s.poly([(fx - 15, fy), (fx - 13, fy - 30), (fx - 17, fy - 48), (fx - 10, fy - 62), (fx + 10, fy - 62),
                                      (fx + 17, fy - 48), (fx + 13, fy - 30), (fx + 16, fy)]), s.ellipse(fx, fy - 66, 12, 11))
    arms = lambda s, fx, fy: (s.stroke([(fx - 14, fy - 52), (fx - 26, fy - 44), (fx - 30, fy - 30), (fx - 24, fy - 22)], 4.5, 2.5),
                              s.stroke([(fx + 14, fy - 52), (fx + 25, fy - 58), (fx + 31, fy - 70), (fx + 27, fy - 78)], 4.5, 2.5))
    put(cv, P(arms, 'dnNg', sigma=3.5, gloss='F'))
    put(cv, P(body, 'dnNgf', sigma=8, gloss='F'))
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 9, fy - 76), (fx, fy - 80), (fx + 9, fy - 76)], 2.6, 1.6),
                                 s.stroke([(fx - 12, fy - 40), (fx - 4, fy - 36), (fx + 6, fy - 40)], 1.6, 1.0)), 'fFF', rim='', amb=0.6))
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 20, fy - 1, 9, 3.5), s.ellipse(fx + 18, fy - 1, 10, 3.5), s.ellipse(fx, fy + 1, 12, 3)),
              'gfF', rim='N', rimw=1.2, amb=0.5))
    eyes(cv, [(-5, -67), (5, -67)], 'F', r=2.2)
    eyes(cv, [(-5, -67), (5, -67)], 'd', r=1.0)
    return cv


@recipe
def earth_elemental():
    cv = Canvas()
    rock = 'u ij Q'.replace(' ', '')
    for (x0, y0, x1, y1, r) in ((-9, -30, -12, -4, 7), (9, -30, 12, -4, 7)):          # legs
        put(cv, P(lambda s, fx, fy, a=(x0, y0, x1, y1, r): s.stroke([(fx + a[0], fy + a[1]), (fx + a[2], fy + a[3])], a[4], a[4] * 0.9), rock, sigma=5))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 44, 21, 19), rock, sigma=10, gloss='Y'))               # torso boulder
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 25, fy - 50, 9, 9), s.ellipse(fx - 29, fy - 34, 8, 9),
                                 s.ellipse(fx + 25, fy - 50, 9, 9), s.ellipse(fx + 30, fy - 34, 8, 9)), rock, sigma=5))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 1, fy - 68, 11, 9), rock, sigma=6, gloss='Y'))             # head
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 14, fy - 60, 7, 3.5), s.ellipse(fx + 18, fy - 58, 6, 3), s.ellipse(fx - 4, fy - 76, 6, 3)),
              'qmo', sigma=2, rim='w', rimw=1.0))                                                              # moss
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 10, fy - 46), (fx - 3, fy - 40), (fx - 6, fy - 32)], 1.1, 0.7),
                                 s.stroke([(fx + 8, fy - 52), (fx + 12, fy - 44)], 1.0, 0.7)), 'hh', rim='', flat=0))  # cracks
    eyes(cv, [(-4, -69), (5, -69)], 'l', r=2.0)
    return cv


@recipe
def treant():
    cv = Canvas()
    bark = 'hriu'
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 6, fy - 20), (fx - 14, fy - 6), (fx - 22, fy)], 4, 2.5), s.stroke([(fx + 6, fy - 20), (fx + 13, fy - 6), (fx + 22, fy)], 4, 2.5),
                                 s.stroke([(fx, fy - 18), (fx + 2, fy)], 3.5, 2.5)), bark, sigma=3))                     # roots
    put(cv, P(lambda s, fx, fy: s.poly([(fx - 12, fy - 16), (fx - 10, fy - 56), (fx + 11, fy - 56), (fx + 13, fy - 16)]), bark, sigma=8))
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 10, fy - 50), (fx - 24, fy - 46), (fx - 30, fy - 34), (fx - 28, fy - 24)], 3.6, 2.0),
                                 s.stroke([(fx + 10, fy - 50), (fx + 24, fy - 56), (fx + 32, fy - 66)], 3.6, 2.0)), bark, sigma=3))  # arms
    for (x, y, r) in ((-14, -66, 12), (12, -68, 13), (0, -80, 13), (-22, -78, 9), (24, -80, 9), (-30, -36, 6), (32, -70, 6)):
        put(cv, P(lambda s, fx, fy, a=(x, y, r): s.ellipse(fx + a[0], fy + a[1], a[2], a[2] * 0.85), 'tTmo', sigma=a_sig(r), gloss='O'))
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 5, fy - 44, 3.4, 2.2), s.ellipse(fx + 5, fy - 44, 3.4, 2.2), s.ellipse(fx, fy - 33, 4, 2.5)), '00', rim='', flat=0))
    eyes(cv, [(-5, -44), (5, -44)], 'L', r=1.4)
    return cv


def a_sig(r):
    return max(3, r * 0.6)


@recipe
def golem():
    """The golem body in iron greys (2 3 4 5 6) with a flame core (E l); manifest remaps make the materials."""
    cv = Canvas()
    metal = '23456'
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 10, fy - 28), (fx - 11, fy - 4)], 6.5, 6), s.stroke([(fx + 10, fy - 28), (fx + 11, fy - 4)], 6.5, 6)), metal, sigma=4))
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 12, fy - 2, 8, 4), s.ellipse(fx + 12, fy - 2, 8, 4)), metal, sigma=3))
    put(cv, P(lambda s, fx, fy: s.poly([(fx - 20, fy - 60), (fx + 20, fy - 60), (fx + 16, fy - 26), (fx - 16, fy - 26)]), metal, sigma=8, gloss='8'))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 46, 6.5, 6.5), 'eElL', sigma=4, rim='2'))                        # core
    for d in (-1, 1):
        put(cv, P(lambda s, fx, fy, d=d: s.ellipse(fx + d * 22, fy - 58, 9, 8), metal, sigma=5, gloss='8'))       # pauldrons
        put(cv, P(lambda s, fx, fy, d=d: s.stroke([(fx + d * 24, fy - 52), (fx + d * 27, fy - 38), (fx + d * 26, fy - 26)], 5.5, 5), metal, sigma=4))
        put(cv, P(lambda s, fx, fy, d=d: s.ellipse(fx + d * 26, fy - 22, 7, 6.5), metal, sigma=4))               # fists
    put(cv, P(lambda s, fx, fy: s.rect(fx - 8, fy - 76, fx + 8, fy - 60), metal, sigma=5, gloss='8'))              # head
    put(cv, P(lambda s, fx, fy: s.rect(fx - 6, fy - 71, fx + 6, fy - 67), 'EE', rim='', flat=0))                   # visor glow
    return cv


@recipe
def wraith():
    cv = Canvas()
    cloak = '1v2p'
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 17, fy - 6), (fx + 21, fy - 82)], 1.8, 1.6, smooth=False), 'hriu', sigma=1.5))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 21, fy - 82), (fx + 6, fy - 87), (fx - 9, fy - 83), (fx - 15, fy - 74)], 3.0, 0.6), '3456', sigma=2, gloss='8'))
    put(cv, P(lambda s, fx, fy: s.poly([(fx - 9, fy - 62), (fx + 9, fy - 62), (fx + 17, fy - 30), (fx + 20, fy - 2), (fx + 13, fy - 8), (fx + 8, fy - 1),
                                        (fx + 2, fy - 9), (fx - 4, fy - 1), (fx - 9, fy - 9), (fx - 15, fy - 2), (fx - 20, fy - 6), (fx - 17, fy - 30)]),
              cloak, sigma=7))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 10, fy - 52), (fx - 18, fy - 40), (fx - 14, fy - 30)], 4, 3), cloak, sigma=3))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 10, fy - 52), (fx + 16, fy - 44), (fx + 19, fy - 38)], 4, 3), cloak, sigma=3))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 66, 11, 12), cloak, sigma=6, gloss='P'))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 1, fy - 63, 7, 7.5), '00', rim='', flat=0))
    eyes(cv, [(-2, -64), (4, -64)], 'S', r=1.7)
    return cv


# ================================================================== undead
def zombie(skin, cloth, flame=False):
    L = crop('atlas3_08')
    L.recolor(L.sel(lmin=0), skin, lo=2, hi=99)
    return L


@recipe
def diseased_corpse():
    L = crop('atlas3_08', 0.42)
    face = L.region(lambda s: s.ellipse(L.w * 0.55, L.h * 0.3, L.w * 0.2, L.h * 0.16))
    L.recolor(L.sel(lmin=0) & ~face, 'wzAO', lo=2, hi=99)
    L.recolor(face & L.sel(lmin=0), 'zAOO', lo=2, hi=99)
    cv = Canvas()
    cv.put(L)
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 14, fy - 50, 4, 3), s.ellipse(fx + 12, fy - 30, 4, 3), s.ellipse(fx - 6, fy - 22, 3.5, 2.5)), 'pP', rim='v', rimw=1.2))  # sores
    return cv


@recipe
def pyre_zombie():
    L = crop('atlas3_08', 0.42)
    L.recolor(L.sel(lmin=0), '0hBDx', lo=2, hi=99)
    cv = Canvas()
    flames(cv, [(-12, -56, 9), (10, -58, 10), (0, -74, 11), (-20, -34, 7), (20, -36, 7)], under=True)
    cv.put(L)
    flames(cv, [(-6, -66, 6), (8, -48, 6), (-14, -30, 5)])
    eyes(cv, [(-1, -60), (7, -60)], 'L', r=1.6)
    return cv


@recipe
def drown():
    L = crop('atlas3_08', 0.42)
    L.recolor(L.sel(lmin=0), 'ksTSW', lo=2, hi=99)
    cv = Canvas()
    cv.put(L)
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 14, fy - 58), (fx - 18, fy - 44), (fx - 13, fy - 32)], 1.6, 0.8), s.stroke([(fx + 12, fy - 54), (fx + 17, fy - 40)], 1.6, 0.8),
                                 s.stroke([(fx - 2, fy - 72), (fx - 8, fy - 64)], 1.4, 0.7)), 'wzq', sigma=1.5))   # weed
    eyes(cv, [(-1, -60), (7, -60)], 'F', r=1.5)
    return cv


@recipe
def einherjar():
    L = crop('atlas1_11', 0.3)
    L.recolor(L.sel(lmin=0), 'dnNgfF', lo=2, hi=99)
    cv = Canvas()
    cv.put(L)
    return cv


@recipe
def lich():
    L = crop('atlas2_19')
    robe = L.sel(hue=(200, 300), cmin=15)
    L.recolor(robe, 'vpP', lo=2, hi=99)
    L.recolor(L.sel(**SKIN), 'DG7')
    cv = Canvas()
    cv.put(L)
    fx, fy = L.feet()
    # a skull face in the hood
    put(cv, P(lambda s, x, y: s.ellipse(x - 1, y - 62, 6.5, 6), 'G78', sigma=3, rim='0', rimw=1.2))
    put(cv, P(lambda s, x, y: (s.ellipse(x - 4, y - 62, 2, 2.2), s.ellipse(x + 2, y - 62, 2, 2.2)), '00', rim='', flat=0))
    eyes(cv, [(-4, -62), (2, -62)], 'S', r=0.9)
    staff(cv, -22, -6, -24, -78, keys='0BD', orb='sSW')
    return cv


@recipe
def vampire():
    L = crop('atlas2_31', 0.3)
    cape = L.sel(hue=(320, 30), cmin=18)
    L.recolor(cape, 'cICR', lo=2, hi=99)
    dark = L.sel(cmax=18, lmax=45)
    L.recolor(dark, '01v2', lo=2, hi=99)
    L.recolor(L.sel(**SKIN), 'GHF')
    cv = Canvas()
    cv.put(L)
    return cv


# ================================================================== demons
@recipe
def imp():
    L = crop('atlas2_24')
    L.erase(L.rect(0, 0.45, 0.32, 1.0) & L.sel(hue=(150, 300), cmin=8))   # the satyr's blue cudgel
    L.recolor(L.sel(hue=(15, 85), cmin=10, lmin=18), 'cICR', lo=3, hi=98)
    L.recolor(L.sel(lmin=58, cmax=26), 'BDG')
    cv = Canvas()
    for d in (-1, 1):
        bat_wing(cv, d * 8, -54, d, span=30, keys='cIC')
    cv.put(L)
    return cv


@recipe
def manes():
    cv = Canvas()
    flesh = 'vpPP'
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 10, fy - 14), (fx - 13, fy - 2)], 4.5, 3.5), s.stroke([(fx + 10, fy - 14), (fx + 13, fy - 2)], 4.5, 3.5)), flesh, sigma=3))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 26, 18, 16), flesh, sigma=9, gloss='P'))
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 15, fy - 32), (fx - 24, fy - 22), (fx - 22, fy - 12)], 3.5, 2.5), s.stroke([(fx + 15, fy - 32), (fx + 24, fy - 22), (fx + 22, fy - 12)], 3.5, 2.5)), flesh, sigma=3))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 1, fy - 46, 10, 9), flesh, sigma=6, gloss='P'))
    horns(cv, 1, -53, size=7, keys='0BD', spread=1.1, curl=0.2)
    eyes(cv, [(-3, -47), (5, -47)], 'l', r=1.8)
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 1, fy - 40, 4, 1.6), 'cc', rim='', flat=0))
    return cv


@recipe
def succubus():
    L = crop('atlas2_09', 0.25)
    L.erase(L.rect(0, 0, 0.36, 1.0))
    leaves = L.sel(hue=(80, 160), cmin=15)
    L.recolor(leaves, 'vpP', lo=2, hi=99)
    robe = L.sel(lmin=55, cmax=45) & ~L.sel(**SKIN)
    L.recolor(robe, 'cICR', lo=2, hi=99)
    L.recolor(L.sel(hue=(60, 100), cmin=30, lmin=45), 'ilK')          # gold trim
    cv = Canvas()
    for d in (-1, 1):
        bat_wing(cv, d * 6, -50, d, span=30, keys='vpP', bone='0')
    cv.put(L)
    horns(cv, 0, -78, size=7, keys='0BD', spread=0.9)
    return cv


@recipe
def hellhound():
    L = crop('atlas1_44')
    fur = L.sel(cmax=25)
    L.recolor(fur, '0123', lo=2, hi=99)
    L.recolor(L.sel(hue=(330, 40), cmin=20), 'eEl')
    cv = Canvas()
    flames(cv, [(-26, -30, 6), (-14, -38, 8), (-2, -42, 8), (10, -44, 7)], under=True)
    cv.put(L)
    eyes(cv, [(19, -32)], 'L', r=1.6)
    return cv


@recipe
def pit_beast():
    L = crop('atlas3_43')
    L.recolor(L.sel(cmax=30), 'cICR', lo=2, hi=99)
    cv = Canvas()
    put(cv, P(lambda s, fx, fy: [s.stroke([(fx + x, fy - 34 - h * 0.1), (fx + x + 3, fy - 40 - h)], 3, 0.6) for x, h in ((-28, 6), (-16, 9), (-4, 9), (8, 7))], '7G8', sigma=1.6))
    cv.under(L, 0, 0)
    eyes(cv, [(26, -24)], 'L', r=1.5)
    return cv


@recipe
def balor():
    L = crop('atlas1_03')
    L.recolor(L.sel(**SKIN), 'cICR', lo=3, hi=98)
    L.recolor(L.sel(lmin=55, cmax=25), 'BDG')
    L.scale(1.32)
    cv = Canvas()
    for d in (-1, 1):
        bat_wing(cv, d * 10, -82, d, span=46, keys='cIC')
    cv.put(L)
    flames(cv, [(-30, -60, 6), (34, -62, 6)])
    return cv


@recipe
def hyborem():
    L = crop('atlas2_15')
    L.recolor(L.sel(hue=(330, 40), cmin=30), 'eElL')
    L.scale(1.28)
    cv = Canvas()
    cv.put(L)
    horns(cv, 0, -100, size=13, keys='0BDG', spread=1.2)
    flames(cv, [(-8, -104, 6), (0, -108, 7), (8, -104, 6)], under=True)
    return cv


@recipe
def auric_ascended():
    L = crop('atlas2_15')
    L.recolor(L.sel(cmax=30), '2NgfF', lo=2, hi=99)
    L.recolor(L.sel(hue=(330, 40), cmin=30), 'gfF8')
    L.scale(1.28)
    cv = Canvas()
    for d in (-1, 1):
        feather_wing(cv, d * 10, -82, d, span=46, keys='NgfF')
    cv.put(L)
    put(cv, P(lambda s, fx, fy: [s.stroke([(fx + x, fy - 100), (fx + x * 1.4, fy - 112 - abs(6 - abs(x)))], 1.8, 0.6) for x in (-8, -4, 0, 4, 8)], 'gfF', sigma=1.5))
    return cv


@recipe
def wrath():
    L = crop('atlas2_30')
    L.recolor(L.sel(lmin=38), 'ceEEl', lo=2, hi=99)
    L.recolor(L.sel(lmax=38), '00c', lo=0, hi=100)
    L.scale(1.45)
    cv = Canvas()
    flames(cv, [(-24, -60, 13), (22, -66, 14), (0, -102, 14), (-10, -90, 11), (12, -92, 11)], keys='ceEl', under=True)
    cv.put(L)
    horns(cv, 0, -112, size=16, keys='0BD', spread=1.3)
    eyes(cv, [(-6, -94), (6, -94)], 'L', r=2.4)
    return cv


@recipe
def gargoyle():
    cv = Canvas()
    stone = '3456'
    for d in (-1, 1):
        bat_wing(cv, d * 8, -52, d, span=30, keys='345', bone='2', under=False)
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 10, fy - 20), (fx - 16, fy - 8), (fx - 12, fy)], 5, 3.5), s.stroke([(fx + 10, fy - 20), (fx + 16, fy - 8), (fx + 12, fy)], 5, 3.5)), stone, sigma=3.5))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 34, 15, 17), stone, sigma=8, gloss='6'))
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 13, fy - 44), (fx - 20, fy - 30), (fx - 14, fy - 18)], 3.6, 2.4), s.stroke([(fx + 13, fy - 44), (fx + 20, fy - 30), (fx + 14, fy - 18)], 3.6, 2.4)), stone, sigma=3))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 56, 10, 9), stone, sigma=6, gloss='6'))
    horns(cv, 0, -62, size=8, keys='2345', spread=1.0, curl=0.7)
    eyes(cv, [(-4, -57), (4, -57)], 'l', r=1.6)
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 4, fy - 50), (fx, fy - 48), (fx + 4, fy - 50)], 1.0, 1.0), '22', rim='', flat=0))
    return cv


# ================================================================== angels
@recipe
def angel():
    L = crop('atlas2_13')
    L.recolor(L.sel(hue=(200, 290), cmin=12), 'yKL', lo=2, hi=99)        # shield field -> gold
    L.recolor(L.sel(cmax=14, lmin=40), '5GH8', lo=2, hi=99)              # armour -> bright silver
    cv = Canvas()
    for d in (-1, 1):
        feather_wing(cv, d * 8, -54, d, span=36)
    cv.put(L)
    halo(cv, 0, -84, r=9)
    return cv


@recipe
def herald():
    L = crop('atlas3_05')
    L.scale(1.08)
    cv = Canvas()
    for d in (-1, 1):
        feather_wing(cv, d * 6, -46, d, span=32)
    cv.put(L)
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 6, fy - 52), (fx - 26, fy - 64)], 1.8, 1.4, smooth=False), s.ellipse(fx - 28, fy - 65, 3.5, 5, rot=-30)), 'iyKL', sigma=1.8, gloss='L'))
    halo(cv, 0, -70, r=8)
    return cv


@recipe
def seraph():
    L = crop('atlas1_02')
    cv = Canvas()
    for d in (-1, 1):
        feather_wing(cv, d * 8, -70, d, span=30, keys='ylKL', up=0.6)
        feather_wing(cv, d * 8, -58, d, span=40, keys='ylKL')
        feather_wing(cv, d * 8, -42, d, span=28, keys='ylKL', up=-0.6)
    cv.put(L)
    halo(cv, 0, -96, r=10, keys='lLL')
    return cv


@recipe
def kraken():
    L = crop('atlas2_29', 0.12)
    L.recolor(L.sel(lmin=0), 'cICRx', lo=3, hi=99)
    L.recolor(L.sel(lmin=70), 'lL')
    L.scale(1.15)
    cv = Canvas()
    cv.put(L, 0, 4)
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 26, fy - 1, 9, 3), s.ellipse(fx + 26, fy - 1, 9, 3), s.ellipse(fx - 6, fy + 1, 12, 3), s.ellipse(fx + 12, fy, 7, 3)),
              'NgfF', rim='N', rimw=1.0, amb=0.5), 0, 6)
    return cv


@recipe
def skeleton():
    cv = Canvas()
    bone = 'DG78'
    for dx, fx_ in ((-6, -7), (6, 8)):
        put(cv, P(lambda s, x, y, dx=dx, f=fx_: (s.stroke([(x + dx, y - 30), (x + dx * 1.2, y - 15), (x + f, y - 3)], 2.6, 2.2),
                                                 s.ellipse(x + f + (2 if f > 0 else -2), y - 1.5, 4.5, 2.5)), bone, sigma=3))
    put(cv, P(lambda s, x, y: s.ellipse(x, y - 30, 10, 5), bone, sigma=3))
    put(cv, P(lambda s, x, y: s.stroke([(x, y - 58), (x, y - 30)], 2.6), bone, sigma=3))
    for i, yy in enumerate((54, 47, 40)):
        w = 12 - i * 1.5
        put(cv, P(lambda s, x, y, yy=yy, w=w: s.stroke([(x - w, y - yy + 2), (x - w * 0.6, y - yy - 2), (x, y - yy - 3),
                                                     (x + w * 0.6, y - yy - 2), (x + w, y - yy + 2)], 2.3), bone, sigma=2.5))
    put(cv, P(lambda s, x, y: s.stroke([(x - 11, y - 56), (x - 16, y - 44), (x - 14, y - 34)], 2.5), bone, sigma=3))
    put(cv, P(lambda s, x, y: s.ellipse(x - 17, y - 38, 10, 11), 'cICR', sigma=6, gloss='R'))
    put(cv, P(lambda s, x, y: s.ellipse(x - 17, y - 38, 3, 3), 'uyK', sigma=2))
    put(cv, P(lambda s, x, y: s.stroke([(x + 18, y - 34), (x + 27, y - 70)], 2.4, 1.2), '3456', sigma=2))
    put(cv, P(lambda s, x, y: s.stroke([(x + 13, y - 39), (x + 23, y - 36)], 1.8), 'uij', sigma=2))
    put(cv, P(lambda s, x, y: s.stroke([(x + 11, y - 56), (x + 16, y - 46), (x + 19, y - 36)], 2.5), bone, sigma=3))
    put(cv, P(lambda s, x, y: s.ellipse(x, y - 61, 7, 5), bone, sigma=3))
    put(cv, P(lambda s, x, y: s.ellipse(x, y - 72, 12, 11), bone, sigma=7, gloss='8'))
    put(cv, P(lambda s, x, y: (s.ellipse(x - 4.5, y - 71, 3.6, 4), s.ellipse(x + 4.5, y - 71, 3.6, 4)), '00', rim='', flat=0))
    eyes(cv, [(-4.5, -71), (4.5, -71)], 'S', r=1.2)
    return cv


@recipe
def giant_spider():
    cv = Canvas()
    chitin = '0hBD'
    legs = [((-14, -22), (-40, -38), (-50, -2)), ((-10, -20), (-30, -44), (-38, -2)), ((-4, -20), (-14, -42), (-22, -2)),
            ((2, -20), (6, -40), (-6, -2)),
            ((10, -20), (30, -44), (38, -2)), ((16, -22), (44, -38), (54, -2)), ((6, -20), (18, -40), (14, -2)), ((20, -18), (52, -24), (62, -2))]
    leg = lambda a, b, c, r: (lambda s, x, y: s.stroke([(x + a[0], y + a[1]), (x + b[0], y + b[1]), (x + c[0], y + c[1])], r, r / 2))
    for i, (a, b, c) in enumerate(legs):
        if i in (2, 3, 6):
            put(cv, P(leg(a, b, c, 3.2), '0hB', sigma=3))
    put(cv, P(lambda s, x, y: s.ellipse(x + 20, y - 30, 22, 17), chitin, sigma=10, gloss='G'))
    put(cv, P(lambda s, x, y: s.poly([(x + 14, y - 42), (x + 26, y - 44), (x + 34, y - 36), (x + 26, y - 30), (x + 18, y - 32)]), 'IC', sigma=3, rim=''))
    put(cv, P(lambda s, x, y: s.ellipse(x - 12, y - 24, 13, 11), chitin, sigma=7, gloss='G'))
    for i, (a, b, c) in enumerate(legs):
        if i not in (2, 3, 6):
            put(cv, P(leg(a, b, c, 3.6), chitin, sigma=3))
    put(cv, P(lambda s, x, y: (s.ellipse(x - 18, y - 27, 2.6, 2.6), s.ellipse(x - 11, y - 28, 2.6, 2.6), s.ellipse(x - 15, y - 22, 1.8, 1.8)), 'CR', rim='', sigma=1.5))
    put(cv, P(lambda s, x, y: (s.stroke([(x - 20, y - 18), (x - 23, y - 11)], 2, 1), s.stroke([(x - 13, y - 17), (x - 14, y - 10)], 2, 1)), 'G7', sigma=1.5))
    return cv



# ================================================================== animals and monsters
def dragon(body, belly, under, scale=1.22):
    L = crop('atlas2_66', 0.12)
    red = L.sel(hue=(320, 45), cmin=22)
    gold = L.sel(hue=(46, 100), cmin=22)
    blue = L.sel(hue=(180, 300), cmin=10)
    L.recolor(red, body, lo=2, hi=99)
    L.recolor(gold, belly, lo=2, hi=99)
    L.recolor(blue, under, lo=2, hi=99)
    L.scale(scale)
    cv = Canvas()
    cv.put(L)
    return cv


@recipe
def acheron():
    return dragon('cICR', 'ylK', 'v2')


@recipe
def abashi():
    return dragon('01234', 'BDG', 'v1')


@recipe
def drifa():
    return dragon('4NgfF', 'G78', 'n')


@recipe
def eurabatres():
    return dragon('ryKL', 'Y78', 'ux')


@recipe
def baby_spider():
    cv = giant_spider()
    L = Layer(cv.a)
    L.recolor(L.sel(lmin=8, cmax=30), 'hriu', lo=2, hi=99)
    cv.a = L.a
    return cv


def unride(L, erase, patches, keys):
    """Erase a rider (erase: shape fn in layer pixels) and paint the mount's back over the gap."""
    L.erase(L.region(erase))
    for fn in patches:
        L.over(part(L.w, L.h, fn, keys, sigma=4, rim='0', rimw=1.4), 0, 0)
    return L


@recipe
def elephant():
    cv = Canvas()
    cv.put(crop('atlas3_elephant', 0))   # hand-cut from sheet 3 (cut_missed.py)
    return cv


def patch_under(L, draw, keys, **kw):
    """Paint a shaded part under the layer: it shows only where the layer is transparent (gaps)."""
    kw.setdefault('sigma', 5)
    p = part(L.w, L.h, draw, keys, **kw)
    p.over(L, 0, 0)
    L.a = p.a
    return L._dirty()


def winged_beast():
    L = crop('atlas2_65', 0.14)
    w, h = L.w, L.h
    L.erase(L.region(lambda s: s.poly([(w * 0.41, h * 0.04), (w * 0.63, h * 0.04), (w * 0.64, h * 0.5), (w * 0.46, h * 0.58), (w * 0.39, h * 0.42)])))
    patch_under(L, lambda s: s.poly([(w * 0.47, h * 0.5), (w * 0.57, h * 0.12), (w * 0.69, h * 0.1), (w * 0.64, h * 0.5)]), 'rixy', sigma=4, rimw=1.2)
    patch_under(L, lambda s: s.ellipse(w * 0.52, h * 0.56, w * 0.14, h * 0.1), 'rijQ', sigma=7, rimw=1.4)
    return L


@recipe
def manticore():
    L = winged_beast()
    L.recolor(L.sel(hue=(20, 80), cmin=15), 'rxyY', lo=2, hi=99)
    cv = Canvas()
    cv.put(L)
    tail = lambda s, fx, fy: s.stroke([(fx - 40, fy - 30), (fx - 52, fy - 46), (fx - 48, fy - 64), (fx - 36, fy - 70)], 3.2, 2.0)
    put(cv, P(tail, 'rxy', sigma=2.5))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 36, fy - 70), (fx - 29, fy - 64)], 2.6, 0.5), '0BD', sigma=1.5))
    return cv

@recipe
def griffon():
    L = winged_beast()
    w, h = L.w, L.h
    head = L.region(lambda s: s.rect(w * 0.66, h * 0.25, w, h * 0.62))
    L.recolor(L.sel(hue=(20, 80), cmin=10) & head, 'DGH8', lo=2, hi=99)
    L.recolor(L.sel(hue=(20, 80), cmin=10) & ~head, 'ryKY', lo=2, hi=99)
    cv = Canvas()
    cv.put(L)
    put(cv, P(lambda s, x, y: s.poly([(x + 40, y - 50), (x + 50, y - 46), (x + 41, y - 41)]), 'ilK', sigma=1.5))
    return cv

@recipe
def lion_pride():
    a = crop('atlas3_42', 0.14)
    b = crop('atlas3_42', 0.14).scale(0.78).flip()
    b.recolor(b.sel(hue=(20, 90), cmin=12), 'ijQY', lo=2, hi=99)
    cv = Canvas()
    cv.put(b, -24, -10)
    cv.put(a, 8, 0)
    return cv


@recipe
def wolf_pack():
    a = crop('atlas3_14', 0.14)
    cv = Canvas()
    b = a.copy().scale(0.8).flip()
    b.recolor(b.sel(cmax=30), '2456', lo=2, hi=99)
    c = a.copy().scale(0.72)
    c.recolor(c.sel(cmax=30), 'hriQ', lo=2, hi=99)
    cv.put(c, 26, -12)
    cv.put(b, -26, -8)
    cv.put(a, 0, 0)
    return cv


def serpent(body, belly, scale):
    L = crop('atlas2_64', 0.1)
    w, h = L.w, L.h
    L.erase(L.region(lambda s: s.poly([(w * 0.34, 0), (w * 0.62, 0), (w * 0.64, h * 0.62), (w * 0.36, h * 0.66)])))
    L.over(part(w, h, lambda s: s.stroke([(w * 0.3, h * 0.72), (w * 0.45, h * 0.62), (w * 0.62, h * 0.6), (w * 0.72, h * 0.5)], h * 0.085, h * 0.075),
                'wqmo', sigma=4, rim='0', rimw=1.4), 0, 0)
    L.recolor(L.sel(hue=(60, 200), cmin=8), body, lo=2, hi=99)
    L.recolor(L.sel(hue=(30, 60), cmin=20), belly, lo=2, hi=99)
    L.scale(scale)
    cv = Canvas()
    cv.put(L.trim(100))
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 30, fy - 1, 10, 3), s.ellipse(fx + 4, fy, 12, 3), s.ellipse(fx + 32, fy - 1, 9, 3)),
              'NgfF', rim='N', rimw=1.0, amb=0.5), 0, 2)
    return cv


def coils(body, belly, fin, scale=1.0):
    cv = Canvas()
    z = scale
    # humps out of the water (back to front), then the neck and head
    for (x, w, hh) in ((-34, 12, 16), (-10, 13, 20)):
        put(cv, P(lambda s, fx, fy, x=x, w=w, hh=hh: s.stroke([(fx + x * z - w * z, fy), (fx + x * z - w * 0.6 * z, fy - hh * z), (fx + x * z + w * 0.6 * z, fy - hh * z), (fx + x * z + w * z, fy)], 5.5 * z, 5 * z),
                  body, sigma=4 * z, gloss=belly[-1]))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 8 * z, fy), (fx + 14 * z, fy - 26 * z), (fx + 18 * z, fy - 46 * z), (fx + 28 * z, fy - 56 * z)], 7 * z, 5 * z), body, sigma=5 * z, gloss=belly[-1]))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 12 * z, fy - 4 * z), (fx + 16 * z, fy - 26 * z), (fx + 19 * z, fy - 42 * z)], 3 * z, 2.4 * z), belly, sigma=2 * z, rim=''))
    put(cv, P(lambda s, fx, fy: [s.poly([(fx + (x - 3) * z, fy + y * z), (fx + (x + 3) * z, fy + (y + 2) * z), (fx + (x - 4) * z, fy + (y - 8) * z)])
                                   for (x, y) in ((10, -18), (11, -30), (13, -42), (19, -52))], fin, sigma=1.5 * z))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 32 * z, fy - 58 * z, 10 * z, 7 * z, rot=-12), body, sigma=4 * z, gloss=belly[-1]))
    put(cv, P(lambda s, fx, fy: s.poly([(fx + 26 * z, fy - 64 * z), (fx + 30 * z, fy - 76 * z), (fx + 34 * z, fy - 64 * z)]), fin, sigma=1.5 * z))
    eyes(cv, [(34 * z, -61 * z)], 'L', r=1.6 * z)
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 34 * z, fy - 54 * z), (fx + 41 * z, fy - 56 * z)], 1.0 * z, 1.0 * z), '00', rim='', flat=0))
    put(cv, P(lambda s, fx, fy: [s.ellipse(fx + x * z, fy, w * z, 3 * z) for (x, w) in ((-46, 6), (-34, 9), (-22, 6), (-10, 10), (2, 6), (12, 9))],
              'NgfF', rim='N', rimw=1.0, amb=0.5))
    return cv


@recipe
def sea_serpent():
    return coils('ksTS', 'jQY', 'Ii', 1.0)

@recipe
def leviathan():
    return coils('dnNg', 'G78', 'sS', 1.3)

def giant(skin, hair, scale=1.36):
    L = crop('atlas1_03', 0.2)
    L.recolor(L.sel(**SKIN), skin, lo=3, hi=98)
    L.scale(scale)
    cv = Canvas()
    cv.put(L)
    return cv


@recipe
def hill_giant():
    return giant('rxba', None)


@recipe
def gurid():
    return giant('34DG', None)


@recipe
def margalard():
    return giant('wzAO', None)


@recipe
def werewolf():
    cv = Canvas()
    fur = '23DG'
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 8, fy - 30), (fx - 15, fy - 16), (fx - 8, fy - 8), (fx - 12, fy - 1)], 5, 3),
                                 s.stroke([(fx + 8, fy - 30), (fx + 15, fy - 16), (fx + 9, fy - 8), (fx + 13, fy - 1)], 5, 3)), fur, sigma=3.5))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 42, 15, 17, rot=8), fur, sigma=8))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 2, fy - 40, 8, 10), 'DG7', sigma=5, rim=''))
    put(cv, P(lambda s, fx, fy: [s.poly([(fx + x, fy - 54), (fx + x + 5, fy - 66), (fx + x + 8, fy - 52)]) for x in (-16, -9, -2)], fur, sigma=2))   # mane
    for d in (-1, 1):
        put(cv, P(lambda s, fx, fy, d=d: s.stroke([(fx + d * 13, fy - 52), (fx + d * 22, fy - 38), (fx + d * 21, fy - 22)], 4.5, 3), fur, sigma=3))
        put(cv, P(lambda s, fx, fy, d=d: [s.stroke([(fx + d * 20 + k, fy - 22), (fx + d * 21 + k * 1.6, fy - 15)], 1.1, 0.5) for k in (-2, 0, 2)], '78', sigma=1))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 3, fy - 62, 10, 9), fur, sigma=5))                                 # head
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 6, fy - 60), (fx + 21, fy - 56)], 5.5, 3.2), fur, sigma=3))      # snout
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 8, fy - 54), (fx + 19, fy - 51)], 2.6, 2.0), '23', sigma=2))     # jaw
    put(cv, P(lambda s, fx, fy: (s.poly([(fx - 5, fy - 66), (fx - 3, fy - 82), (fx + 2, fy - 68)]), s.poly([(fx + 3, fy - 69), (fx + 8, fy - 84), (fx + 10, fy - 67)])), fur, sigma=2))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 22, fy - 57, 2, 2), '00', rim='', flat=0))
    eyes(cv, [(4, -64), (10, -64)], 'l', r=1.5)
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 10, fy - 54), (fx + 18, fy - 53)], 0.9, 0.9), '88', rim='', flat=0))
    return cv

@recipe
def gorilla():
    cv = Canvas()
    fur = '0123'
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 9, fy - 22), (fx - 12, fy - 2)], 5, 4), s.stroke([(fx + 9, fy - 22), (fx + 12, fy - 2)], 5, 4)), fur, sigma=3.5))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 36, 19, 18), fur, sigma=9))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 34, 10, 10), '3DG', sigma=6, rim=''))
    for d in (-1, 1):
        put(cv, P(lambda s, fx, fy, d=d: s.stroke([(fx + d * 16, fy - 48), (fx + d * 26, fy - 30), (fx + d * 24, fy - 6)], 6, 5), fur, sigma=4))
        put(cv, P(lambda s, fx, fy, d=d: s.ellipse(fx + d * 24, fy - 4, 6, 4), '3DG', sigma=3))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 56, 10, 10), fur, sigma=6))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 1, fy - 53, 7, 6), '3DG', sigma=4))
    eyes(cv, [(-2, -57), (4, -57)], '0', r=1.3)
    return cv


@recipe
def giant_tortoise():
    cv = Canvas()
    skin = 'wzAo'
    for (x, y) in ((-22, -6), (-8, -3)):
        put(cv, P(lambda s, fx, fy, x=x, y=y: s.stroke([(fx + x, fy - 16), (fx + x, fy + y)], 6, 6.5), skin, sigma=4))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 24, fy - 18), (fx + 34, fy - 30), (fx + 44, fy - 30)], 6.5, 6), skin, sigma=4))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 26, 31, 24), 'urij', sigma=13, gloss='Q'))
    put(cv, P(lambda s, fx, fy: [s.ellipse(fx + x, fy + y, 6.5, 5) for (x, y) in ((-14, -34), (2, -38), (16, -32), (-6, -22), (10, -20), (-20, -18), (22, -18))],
              'ijQ', sigma=3, rim='h', rimw=1.0))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 6, 31, 4.5), 'jQ', sigma=2, rim='0'))
    for (x, y) in ((-16, -1), (14, -1)):
        put(cv, P(lambda s, fx, fy, x=x, y=y: s.stroke([(fx + x, fy - 12), (fx + x + 2, fy + y)], 6, 6.5), skin, sigma=4))
    eyes(cv, [(44, -32)], '0', r=1.4)
    return cv

@recipe
def scorpion():
    cv = Canvas()
    shell = 'riyY'
    for k in range(3):
        put(cv, P(lambda s, fx, fy, k=k: (s.stroke([(fx - 6 + k * 8, fy - 12), (fx - 14 + k * 8, fy - 18), (fx - 20 + k * 9, fy - 2)], 1.8, 1.2),
                                          s.stroke([(fx - 2 + k * 8, fy - 12), (fx + 4 + k * 8, fy - 18), (fx + 8 + k * 9, fy - 2)], 1.8, 1.2)), 'rij', sigma=1.5))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 2, fy - 14, 18, 8), shell, sigma=5, gloss='Y'))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 14, fy - 14), (fx - 26, fy - 22), (fx - 30, fy - 40), (fx - 22, fy - 52), (fx - 10, fy - 50)], 4, 2.6), shell, sigma=3))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 10, fy - 50), (fx - 4, fy - 44)], 2.8, 0.5), '0BD', sigma=1.5))
    for d in (-1, 1):
        put(cv, P(lambda s, fx, fy, d=d: s.stroke([(fx + 16, fy - 14 + d * 3), (fx + 28, fy - 18 + d * 6), (fx + 36, fy - 14 + d * 8)], 3, 2.4), shell, sigma=2.5))
        put(cv, P(lambda s, fx, fy, d=d: s.ellipse(fx + 40, fy - 14 + d * 8, 5, 3.5), shell, sigma=2.5))
    eyes(cv, [(18, -18)], '0', r=1.2)
    return cv


@recipe
def hawk():
    cv = Canvas()
    put(cv, P(lambda s, fx, fy: s.poly([(fx - 4, fy - 40), (fx - 38, fy - 58), (fx - 30, fy - 48), (fx - 36, fy - 44), (fx - 24, fy - 38), (fx - 8, fy - 30)]), 'hriu', sigma=4))
    put(cv, P(lambda s, fx, fy: s.poly([(fx + 4, fy - 40), (fx + 38, fy - 58), (fx + 30, fy - 48), (fx + 36, fy - 44), (fx + 24, fy - 38), (fx + 8, fy - 30)]), 'hriu', sigma=4))
    put(cv, P(lambda s, fx, fy: s.poly([(fx - 5, fy - 24), (fx + 5, fy - 24), (fx + 8, fy - 14), (fx - 8, fy - 14)]), 'hri', sigma=2))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 34, 7, 11), 'j78', sigma=4))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 47, 6, 6), 'hri', sigma=3))
    put(cv, P(lambda s, fx, fy: s.poly([(fx - 2, fy - 45), (fx + 2, fy - 45), (fx, fy - 40)]), 'yK', sigma=1, rim=''))
    eyes(cv, [(-2.5, -48), (2.5, -48)], '0', r=0.9)
    return cv


@recipe
def guardian_vines():
    cv = Canvas()
    vine = 'wqmo'
    for pts, r in (([(-24, 0), (-26, -20), (-14, -36), (-20, -52), (-8, -64)], 4),
                   ([(22, 0), (26, -18), (14, -32), (22, -50), (12, -66)], 4),
                   ([(-6, 0), (-2, -16), (8, -28), (0, -44), (6, -58), (-2, -72)], 4.5),
                   ([(-14, 0), (-20, -10), (-30, -24), (-34, -40)], 3),
                   ([(12, 0), (20, -8), (34, -20), (36, -36)], 3)):
        put(cv, P(lambda s, fx, fy, pts=pts, r=r: s.stroke([(fx + x, fy + y) for x, y in pts], r, r * 0.45), vine, sigma=3, gloss='O'))
    thorns = [(-22, -28), (-12, -48), (20, -24), (16, -44), (4, -36), (-2, -60), (-30, -18), (30, -14)]
    put(cv, P(lambda s, fx, fy: [s.poly([(fx + x - 2, fy + y), (fx + x + 2, fy + y), (fx + x + 4, fy + y - 4)]) for x, y in thorns], '7G', sigma=1, rim='0', rimw=0.8))
    for (x, y) in ((-8, -64), (12, -66), (-34, -40), (36, -36)):
        put(cv, P(lambda s, fx, fy, x=x, y=y: s.ellipse(fx + x, fy + y, 4.5, 4.5), 'cCR', sigma=3, gloss='R'))
    return cv



# ================================================================== heroes, infantry, mounted
def headtop(L, band=0.34, minrun=6):
    """(dx, dy) of the top of the head relative to the feet: the first row whose central band holds
    a solid run (weapons are thin)."""
    fx, fy = L.feet()
    op = L.a[..., 3] > 127
    x0, x1 = int(fx - L.w * band / 2), int(fx + L.w * band / 2)
    for y in range(L.h):
        row = op[y, max(0, x0):x1]
        run = best = 0
        for v in row:
            run = run + 1 if v else 0
            best = max(best, run)
        if best >= minrun:
            xs = np.nonzero(row)[0]
            return (max(0, x0) + (xs.min() + xs.max()) / 2.0 - fx, y - fy)
    return (0, -L.h)


def hero(src, debase=0.36, scale=1.0, recolors=(), flip=False):
    L = crop(src, debase)
    for sel, keys in recolors:
        L.recolor(L.sel(**sel), keys, lo=2, hi=99)
    if scale != 1.0:
        L.scale(scale)
    if flip:
        L.flip()
    return L


def on(L, *extras_fns, under_fns=()):
    cv = Canvas()
    hx, hy = headtop(L)
    for fn in under_fns:
        fn(cv, hx, hy)
    cv.put(L)
    for fn in extras_fns:
        fn(cv, hx, hy)
    return cv


RED = dict(hue=(340, 30), cmin=25)
BLUE = dict(hue=(220, 290), cmin=15)
GREEN = dict(hue=(90, 170), cmin=14)
PURPLE = dict(hue=(285, 340), cmin=12)
GOLD = dict(hue=(60, 100), cmin=28, lmin=45)
STEEL = dict(cmax=14, lmin=35)
TAN = dict(hue=(40, 90), cmin=10, cmax=40, lmin=30)


def plume(keys='cCR', dx=0):
    return lambda cv, hx, hy: put(cv, P(lambda s, fx, fy: s.stroke([(fx + hx + dx, fy + hy + 3), (fx + hx + dx - 2, fy + hy - 6), (fx + hx + dx - 10, fy + hy - 9)], 3.2, 1.2), keys, sigma=2))


def crown(keys='yKL', w=8):
    return lambda cv, hx, hy: put(cv, P(lambda s, fx, fy: s.poly([(fx + hx - w, fy + hy + 4), (fx + hx - w, fy + hy - 3), (fx + hx - w / 2, fy + hy + 1), (fx + hx, fy + hy - 5),
                                                                  (fx + hx + w / 2, fy + hy + 1), (fx + hx + w, fy + hy - 3), (fx + hx + w, fy + hy + 4)]), keys, sigma=2, gloss='L'))


def hornset(keys='yKL', size=8):
    return lambda cv, hx, hy: horns(cv, hx, hy + 6, size=size, keys=keys, spread=1.3, curl=0.3)


def antlers():
    def f(cv, hx, hy):
        def draw(s, fx, fy):
            for d in (-1, 1):
                bx, by = fx + hx + d * 4, fy + hy + 4
                s.stroke([(bx, by), (bx + d * 6, by - 8), (bx + d * 10, by - 16)], 1.6, 1.0)
                s.stroke([(bx + d * 6, by - 8), (bx + d * 12, by - 9)], 1.3, 0.8)
                s.stroke([(bx + d * 8, by - 12), (bx + d * 5, by - 18)], 1.2, 0.8)
        put(cv, P(draw, 'hriQ', sigma=1.5))
    return f


def beard(keys='GH8'):
    return lambda cv, hx, hy: put(cv, P(lambda s, fx, fy: s.poly([(fx + hx - 6, fy + hy + 17), (fx + hx + 6, fy + hy + 17), (fx + hx + 1, fy + hy + 29)]), keys, sigma=2.5))


def wings(span=34, keys='GH8F', y=14):
    return lambda cv, hx, hy: [feather_wing(cv, hx + d * 6, hy + y, d, span=span, keys=keys) for d in (-1, 1)]


def halo_at(keys='yKL', r=8, y=-3):
    return lambda cv, hx, hy: halo(cv, hx, hy + y, r=r, keys=keys)


def glow_eyes(keys='R', dy=10, gap=3.5):
    return lambda cv, hx, hy: eyes(cv, [(hx - gap / 2, hy + dy), (hx + gap / 2 + 2, hy + dy)], keys, r=1.2)


@recipe
def alazkan():
    return on(hero('atlas3_12', recolors=[(PURPLE, '0123'), (GOLD, 'yK')]), glow_eyes('K', 12))


@recipe
def arthendain():
    return on(hero('atlas3_18', scale=0.94, recolors=[(TAN, 'dnNJ'), (dict(cmax=14, lmin=20), 'BDGH')]), plume('ylK'))


@recipe
def bambur():
    return on(hero('atlas1_04', recolors=[(STEEL, 'uyKL'), (BLUE, 'cICR')]), beard('yKL'))


@recipe
def barnaxus():
    cv = golem()
    L = Layer(cv.a)
    L.recolor(L.sel(cmax=12, lmin=5), 'uxyKL', lo=2, hi=99)
    L.recolor(L.sel(hue=(20, 80), cmin=50), 'nNgF')
    cv.a = L.a
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 26, fy - 22), (fx + 34, fy - 64)], 2.6, 1.6), '5GH8', sigma=2, gloss='F'))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 20, fy - 30), (fx + 33, fy - 27)], 2.0, 2.0), 'uyK', sigma=1.5))
    return cv


@recipe
def basium():
    return on(hero('atlas2_13', recolors=[(BLUE, 'ylK'), (STEEL, 'yKL8')]), halo_at('lLL', 9, -4), under_fns=[wings(32, 'GH8F', 18)])


@recipe
def chalid():
    return on(hero('atlas2_10', recolors=[(GREEN, 'ylKL')]), halo_at('yKL', 8, -2))


@recipe
def corlindale():
    cv = on(hero('atlas2_19', recolors=[(BLUE, 'vpPW')]))
    return cv


@recipe
def donal():
    return on(hero('atlas1_07'), plume('678'))


@recipe
def gilden():
    return on(hero('atlas3_09', recolors=[(GREEN, 'DGH8')]))


@recipe
def govannon():
    cv = on(hero('atlas2_19', recolors=[(BLUE, 'rxyY')]), beard('GH8'))
    put(cv, P(lambda s, fx, fy: s.rect(fx + 8, fy - 44, fx + 18, fy - 34), 'cIC', sigma=2))
    put(cv, P(lambda s, fx, fy: s.rect(fx + 9, fy - 43, fx + 17, fy - 41), '78', rim='', flat=1))
    return cv


@recipe
def guybrush():
    return on(hero('atlas3_03', recolors=[(TAN, 'nNJ')]))


@recipe
def herne():
    return on(hero('atlas1_35', recolors=[(TAN, 'wzA')]), antlers())


@recipe
def loki():
    return on(hero('atlas3_16', recolors=[(PURPLE, 'tTmo')]), hornset('yKL', 7))


@recipe
def losha():
    return on(hero('atlas1_01', recolors=[(RED, '0vpP'), (GOLD, 'BDGH')]))


@recipe
def magnadine():
    return on(hero('atlas2_52', recolors=[(RED, 'tTmO')]), plume('mO8'))


@recipe
def mardero():
    return on(hero('atlas2_10', recolors=[(GREEN, 'cICR'), (dict(lmin=50, cmax=30), '0123')]), hornset('0BD', 8), glow_eyes('L', 12))


@recipe
def maros():
    return on(hero('atlas1_04', scale=1.08, recolors=[(BLUE, 'dnJN'), (STEEL, '35GH')]), beard('rxy'))


@recipe
def orthus():
    return on(hero('atlas1_11', 0.3, recolors=[(SKIN, 'wzAO')]))


@recipe
def rantine():
    return on(hero('atlas2_03', recolors=[(SKIN, 'wzAO'), (dict(hue=(40, 90), cmin=12, lmin=25, lmax=75), 'exEl')]))


@recipe
def rathus():
    return on(hero('atlas3_13', recolors=[(dict(cmax=40, lmax=45), '0cI')]), glow_eyes('R', 12))


@recipe
def sphener():
    cv = on(hero('atlas2_10', recolors=[(GREEN, 'GH8F')]), halo_at('lLL', 8, -2), under_fns=[wings(30, 'GH8F', 16)])
    return cv


@recipe
def teutorix():
    return on(hero('atlas1_21', recolors=[(GREEN, 'hriu')]))


@recipe
def tumtum():
    return on(hero('atlas1_37', recolors=[(SKIN, 'wzAO')]))


@recipe
def valin():
    return on(hero('atlas3_33', recolors=[(STEEL, '6H8F'), (GOLD, 'ylK')]), plume('cCR'))


@recipe
def war_machine():
    return on(hero('atlas2_46', 0.14, scale=1.2))


@recipe
def wilboman():
    return on(hero('atlas1_03', 0.2, scale=1.4, recolors=[(SKIN, 'NgfF')]), beard('fF8'))


@recipe
def auric():
    return on(hero('atlas1_00', recolors=[(BLUE, 'gfF8')]), crown('yKL', 7))


@recipe
def lucian():
    return on(hero('atlas3_26', recolors=[(TAN, 'cIC')]))


@recipe
def radiant_guard():
    return on(hero('atlas3_29'), halo_at('lLL', 8, -2))


@recipe
def flagbearer():
    L = hero('atlas1_05')
    cv = on(L)
    fx, fy = L.feet()
    # the banner on the spear: owner cloth (red family -> slots)
    put(cv, P(lambda s, x, y: s.poly([(x + 18, y - 82), (x + 40, y - 78), (x + 34, y - 70), (x + 40, y - 62), (x + 18, y - 64)]), 'cICR', sigma=4))
    put(cv, P(lambda s, x, y: s.ellipse(x + 27, y - 72, 3.5, 3.5), 'yK', sigma=2))
    return cv


@recipe
def goblin():
    return on(hero('atlas1_12', recolors=[(GREEN, 'hriu'), (SKIN, 'zAOO')]))


@recipe
def javelin_thrower():
    return on(hero('atlas3_11'))


@recipe
def ogre():
    return on(hero('atlas2_03', scale=1.38, recolors=[(SKIN, 'zAOO')]))


@recipe
def camel_archer():
    L = hero('atlas3_37', recolors=[(dict(hue=(20, 70), cmin=18, lmax=60), 'ijQY')])
    cv = on(L)
    put(cv, P(lambda s, fx, fy: s.ellipse(fx - 8, fy - 38, 8, 7), 'ijQY', sigma=4), 0, 0)
    return cv


@recipe
def war_tortoise():
    t = giant_tortoise()
    T = Layer(t.a).trim(100).scale(1.25)
    cv = Canvas()
    cv.put(T)
    put(cv, P(lambda s, fx, fy: s.rect(fx - 14, fy - 66, fx + 12, fy - 48), 'hriu', sigma=4))                 # howdah
    put(cv, P(lambda s, fx, fy: s.rect(fx - 16, fy - 70, fx + 14, fy - 64), 'riuj', sigma=2))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 10, fy - 70), (fx + 10, fy - 92)], 1.2, 1.2, smooth=False), 'hr', sigma=1))
    put(cv, P(lambda s, fx, fy: s.poly([(fx + 11, fy - 92), (fx + 24, fy - 88), (fx + 11, fy - 82)]), 'cICR', sigma=2))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx - 4, fy - 74, 5, 5), 'ba', sigma=3))                               # archer's head
    return cv



# ================================================================== faiths: disciples, priests, high priests
# religion -> (trim ramp, robe ramp or None to keep the white robe)
FAITH = {
    'empyrean': ('ylKL', None),
    'leaves': ('qmoO', None),
    'overlords': ('ksTS', '1ks'),
    'kilmorph': ('rxyY', 'uijQ'),
    'veil': ('cICR', '0123'),
    'order': ('dnJN', None),
    'winter': ('NgfF', None),
}
NAMES = {
    'disciple': {'empyrean': 'disciple_empyrean', 'leaves': 'disciple_fellowship_of_leaves', 'overlords': 'disciple_octopus_overlords',
                 'kilmorph': 'disciple_runes_of_kilmorph', 'veil': 'disciple_the_ashen_veil', 'order': 'disciple_the_order'},
    'priest': {'empyrean': 'priest_of_the_empyrean', 'leaves': 'priest_of_leaves', 'overlords': 'priest_of_the_overlords',
               'kilmorph': 'priest_of_kilmorph', 'veil': 'priest_of_the_veil', 'order': 'priest_of_the_order', 'winter': 'priest_of_winter'},
    'high_priest': {f: 'high_priest_of_' + n for f, n in (('empyrean', 'the_empyrean'), ('leaves', 'leaves'), ('overlords', 'the_overlords'),
                                                          ('kilmorph', 'kilmorph'), ('veil', 'the_veil'), ('order', 'the_order'), ('winter', 'winter'))},
}
WHITE = dict(lmin=62, cmax=24)


def disciple_of(f):
    trim, robe = FAITH[f]
    L = crop('atlas3_05', 0.36)
    L.recolor(L.sel(**WHITE), robe or ('G78' if f != 'order' else 'fF8'), lo=2, hi=99)
    L.recolor(L.sel(lmin=30, lmax=62, cmax=30), trim, lo=2, hi=99)
    L.scale(1.08)
    cv = Canvas()
    cv.put(L)
    # a sash of the faith's colour
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 8, fy - 46), (fx + 7, fy - 30)], 2.4, 2.4, smooth=False), trim, sigma=1.5))
    return cv


def priest_of(f):
    trim, robe = FAITH[f]
    L = crop('atlas2_10', 0.25)
    L.recolor(L.sel(hue=(80, 170), cmin=14), trim, lo=2, hi=99)
    if robe:
        L.recolor(L.sel(**WHITE), robe, lo=2, hi=99)
    cv = Canvas()
    cv.put(L)
    return cv


def high_priest_of(f):
    trim, robe = FAITH[f]
    L = crop('atlas2_12', 0.2)
    L.recolor(L.sel(hue=(40, 100), cmin=25), trim, lo=2, hi=99)
    if robe:
        L.recolor(L.sel(**WHITE), robe, lo=2, hi=99)
    cv = Canvas()
    cv.put(L)
    return cv


for _f in FAITH:
    if _f in NAMES['disciple']:
        RECIPES[NAMES['disciple'][_f]] = (lambda f=_f: disciple_of(f))
    if _f not in ('leaves', 'winter'):                       # those two priests are atlas figures
        RECIPES[NAMES['priest'][_f]] = (lambda f=_f: priest_of(f))
    RECIPES[NAMES['high_priest'][_f]] = (lambda f=_f: high_priest_of(f))
del RECIPES['disciple_fellowship_of_leaves']                 # an atlas figure too


# ================================================================== robed: great people and the like
@recipe
def monk():
    L = crop('atlas2_10', 0.25)
    L.erase(L.rect(0, 0, 0.26, 1) | L.rect(0.78, 0, 1, 1))  # the leafy staves
    L.recolor(L.sel(hue=(80, 170), cmin=14), 'xyY', lo=2, hi=99)
    L.recolor(L.sel(**WHITE), 'riju', lo=2, hi=99)
    cv = Canvas()
    cv.put(L)
    staff(cv, 18, -2, 20, -70, keys='hriu')
    return cv


@recipe
def prophet():
    cv = on(hero('atlas2_19', recolors=[(BLUE, 'DGH8')]), beard('GH8'), halo_at('yKL', 7, 0))
    return cv


@recipe
def shaman():
    cv = on(hero('atlas3_10', recolors=[(TAN, 'riju')]))
    put(cv, P(lambda s, fx, fy: [s.stroke([(fx + x, fy - 70), (fx + x * 1.6, fy - 84)], 1.8, 1.0) for x in (-8, -4, 0, 4, 8)], 'cCRl', sigma=1.5))   # feathers
    staff(cv, -22, -2, -24, -76, keys='hri')
    put(cv, P(lambda s, fx, fy: s.ellipse(fx - 24, fy - 80, 5, 5), '7G8', sigma=3))
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 26, fy - 81, 1.3, 1.3), s.ellipse(fx - 22, fy - 81, 1.3, 1.3)), '00', rim='', flat=0))
    return cv


@recipe
def mokka():
    cv = shaman()
    L = Layer(cv.a)
    L.recolor(L.sel(**SKIN), 'gfF')
    L.recolor(L.sel(hue=(20, 80), cmin=10, cmax=60, lmax=50), 'nNg')
    cv.a = L.a
    return cv


@recipe
def merchant():
    return on(hero('atlas3_15', recolors=[(GREEN, 'vpP'), (dict(hue=(30, 70), cmin=20, lmin=40), 'yK')]))


@recipe
def engineer():
    cv = on(hero('atlas3_10', recolors=[(TAN, 'exEl')]))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 16, fy - 30), (fx + 22, fy - 56)], 1.6, 1.6, smooth=False), 'hri', sigma=1.2))
    put(cv, P(lambda s, fx, fy: s.rect(fx + 16, fy - 62, fx + 30, fy - 54), '3456', sigma=2, gloss='8'))
    return cv


@recipe
def scientist():
    cv = on(hero('atlas2_19', recolors=[(BLUE, 'ksTS')]))
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx + 18, fy - 36, 5, 5), s.rect(fx + 16.5, fy - 46, fx + 19.5, fy - 38)), 'gfFW', sigma=2.5, gloss='F'))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 18, fy - 34, 3.4, 2.5), 'sS', rim='', sigma=2))
    return cv


@recipe
def artist():
    L = crop('atlas2_09', 0.25)
    L.erase(L.rect(0, 0, 0.36, 1.0))
    L.recolor(L.sel(hue=(80, 160), cmin=15), 'riux', lo=2, hi=99)
    L.recolor(L.sel(lmin=55, cmax=45) & ~L.sel(**SKIN), 'cCR8', lo=2, hi=99)
    cv = Canvas()
    cv.put(L)
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 18, fy - 34), (fx - 22, fy - 52)], 1.8, 1.4), s.stroke([(fx - 6, fy - 34), (fx - 2, fy - 52)], 1.8, 1.4),
                                 s.stroke([(fx - 20, fy - 34), (fx - 4, fy - 34)], 2, 2, smooth=False)), 'iyK', sigma=1.5, gloss='L'))   # lyre
    put(cv, P(lambda s, fx, fy: [s.stroke([(fx - 18 + k * 4, fy - 36), (fx - 18 + k * 4, fy - 49)], 0.6, 0.6, smooth=False) for k in range(4)], '88', rim='', flat=0))
    return cv


@recipe
def supplies():
    cv = Canvas()
    put(cv, P(lambda s, fx, fy: s.rect(fx - 24, fy - 22, fx + 2, fy), 'hriu', sigma=5))
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx - 24, fy - 11), (fx + 2, fy - 11)], 0.9, 0.9, smooth=False), s.stroke([(fx - 11, fy - 22), (fx - 11, fy)], 0.9, 0.9, smooth=False)), 'hh', rim='', flat=0))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 14, fy - 12, 10, 12), 'rij', sigma=6))                            # barrel
    put(cv, P(lambda s, fx, fy: (s.stroke([(fx + 5, fy - 18), (fx + 23, fy - 18)], 1.2, 1.2, smooth=False), s.stroke([(fx + 5, fy - 6), (fx + 23, fy - 6)], 1.2, 1.2, smooth=False)), '34', rim='', flat=1))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx - 6, fy - 32, 13, 10), 'ijQY', sigma=6))                             # sack
    put(cv, P(lambda s, fx, fy: s.ellipse(fx - 6, fy - 42, 4, 3), 'ijQ', sigma=2))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 12, fy - 28, 8, 6), 'ijQY', sigma=4))
    return cv


@recipe
def disciple_of_acheron():
    return on(hero('atlas2_25', recolors=[(PURPLE, 'ceEl')]), lambda cv, hx, hy: flames(cv, [(hx + 16, hy + 2, 6)]))


@recipe
def puppet():
    cv = on(hero('atlas2_25', recolors=[(PURPLE, 'pPW')]))
    put(cv, P(lambda s, fx, fy: [s.stroke([(fx + x, fy - 104), (fx + x * 0.8, fy - y)], 0.5, 0.5, smooth=False) for x, y in ((-12, 46), (0, 82), (12, 46))], '88', rim='', flat=0))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 16, fy - 104), (fx + 16, fy - 104)], 1.8, 1.8, smooth=False), 'hri', sigma=1.2))
    return cv


# ================================================================== ships
def ship(src, recolors=(), scale=1.0):
    L = crop(src, 0)
    for sel, keys in recolors:
        L.recolor(L.sel(**sel), keys, lo=2, hi=99)
    if scale != 1:
        L.scale(scale)
    cv = Canvas()
    cv.put(L)
    return cv


SAIL = dict(lmin=62, cmax=30)


@recipe
def arcane_barge():
    cv = ship('atlas1_54', [(SAIL, 'pPW')])
    put(cv, P(lambda s, fx, fy: s.ellipse(fx - 4, fy - 82, 5, 5), 'pPW', sigma=3, gloss='W'))
    return cv


@recipe
def black_wind():
    return ship('atlas1_56', [(SAIL, '1234'), (dict(hue=(20, 70), cmin=20, lmax=60), 'h0B')])


@recipe
def galleon():
    return ship('atlas1_57', [(SAIL, 'jQY7')])


@recipe
def queen_of_the_line():
    cv = ship('atlas1_59', [(SAIL, 'G78F')], scale=1.22)
    put(cv, P(lambda s, fx, fy: s.poly([(fx + 4, fy - 128), (fx + 34, fy - 124), (fx + 4, fy - 120)]), 'cICR', sigma=2))
    return cv


@recipe
def workboat():
    cv = Canvas()
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 2, fy - 18), (fx - 2, fy - 58)], 1.3, 1.3, smooth=False), 'hr', sigma=1))
    put(cv, P(lambda s, fx, fy: s.poly([(fx, fy - 56), (fx + 22, fy - 24), (fx, fy - 22)]), 'G78F', sigma=5))
    put(cv, P(lambda s, fx, fy: s.poly([(fx - 4, fy - 52), (fx - 18, fy - 26), (fx - 4, fy - 24)]), 'cICR', sigma=4))
    put(cv, P(lambda s, fx, fy: s.poly([(fx - 30, fy - 20), (fx + 30, fy - 20), (fx + 22, fy - 6), (fx - 22, fy - 6)]), 'hriu', sigma=4))
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 30, fy - 20), (fx + 30, fy - 20)], 1.5, 1.5, smooth=False), 'ijQ', sigma=1))
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 26, fy - 3, 9, 3), s.ellipse(fx + 24, fy - 3, 9, 3), s.ellipse(fx, fy - 2, 14, 3)), 'NgfF', rim='N', rimw=1.0, amb=0.5))
    return cv


# ================================================================== spells
@recipe
def fireball():
    cv = Canvas()
    flames(cv, [(-22, -24, 8), (-14, -26, 10), (-8, -30, 9)], keys='eEl')
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 6, fy - 34, 15, 15), 'ceEl', sigma=8, rim='e', light=np.array([0.0, -0.3, 0.95])))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 8, fy - 36, 7, 7), 'lLL', sigma=4, rim=''))
    return cv


@recipe
def meteor():
    cv = Canvas()
    put(cv, P(lambda s, fx, fy: s.stroke([(fx - 36, fy - 74), (fx - 18, fy - 52), (fx - 2, fy - 36)], 2, 12), 'ceEl', sigma=4, rim='', light=np.array([0.0, -0.3, 0.95])))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 4, fy - 30, 12, 11), 'hriu', sigma=6, rim='0'))
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx + 1, fy - 33, 3, 2.5), s.ellipse(fx + 9, fy - 26, 2.5, 2)), 'hh', rim='', flat=0))
    flames(cv, [(-6, -24, 5), (-8, -36, 5)], keys='Ell')
    return cv


@recipe
def eye():
    cv = Canvas()
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 40, 17, 14), 'vpP', sigma=6, rim='0'))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx, fy - 40, 12, 9), 'G78', sigma=5, rim='v'))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 1, fy - 40, 6, 6), 'CRl', sigma=3, rim='c'))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx + 1, fy - 40, 2.6, 4), '00', rim='', flat=0))
    put(cv, P(lambda s, fx, fy: [s.stroke([(fx + x, fy - 28), (fx + x * 1.3, fy - 16), (fx + x, fy - 6)], 1.8, 0.8) for x in (-8, 0, 8)], 'vpP', sigma=1.5))
    return cv


@recipe
def severed_soul():
    cv = Canvas()
    put(cv, P(lambda s, fx, fy: s.stroke([(fx + 4, fy - 4), (fx - 8, fy - 16), (fx + 6, fy - 30), (fx - 2, fy - 46)], 2, 10), 'sSW', sigma=5, rim='s', amb=0.5))
    put(cv, P(lambda s, fx, fy: s.ellipse(fx - 2, fy - 56, 11, 11), 'sSW', sigma=6, rim='s', amb=0.5))
    put(cv, P(lambda s, fx, fy: (s.ellipse(fx - 6, fy - 57, 2.5, 3.5), s.ellipse(fx + 3, fy - 57, 2.5, 3.5), s.ellipse(fx - 1, fy - 49, 3, 2)), 'ss', rim='', flat=0))
    return cv


# ================================================================== equipment (24x24 frames)
def item(draws):
    cv = Canvas()
    for fn, keys, kw in draws:
        put(cv, P(fn, keys, **kw))
    return cv


def blade(keys='5GH8', hilt='uyK', grip='hr', length=46, tilt=18, width=3.2):
    t = math.radians(tilt)
    def b(s, fx, fy):
        x0, y0 = fx - 4, fy - 8
        x1, y1 = x0 + math.sin(t) * length, y0 - math.cos(t) * length
        s.stroke([(x0, y0), (x1, y1)], width, 0.6, smooth=False)
    def g(s, fx, fy):
        x0, y0 = fx - 4, fy - 8
        cx, cy = x0 + math.sin(t) * 4, y0 - math.cos(t) * 4
        s.stroke([(cx - math.cos(t) * 9, cy - math.sin(t) * 9), (cx + math.cos(t) * 9, cy + math.sin(t) * 9)], 1.8, 1.8, smooth=False)
    def h(s, fx, fy):
        x0, y0 = fx - 4, fy - 8
        s.stroke([(x0 - math.sin(t) * 9, y0 + math.cos(t) * 9), (x0, y0)], 1.8, 1.8, smooth=False)
        s.ellipse(x0 - math.sin(t) * 10, y0 + math.cos(t) * 10, 2.4, 2.4)
    return [(h, grip + grip[-1], dict(sigma=1.2)), (b, keys, dict(sigma=2, gloss=keys[-1])), (g, hilt, dict(sigma=1.2, gloss=hilt[-1]))]


def bottle(keys):
    return [(lambda s, fx, fy: s.ellipse(fx, fy - 12, 11, 11), keys, dict(sigma=6, gloss='F')),
            (lambda s, fx, fy: s.rect(fx - 3.5, fy - 30, fx + 3.5, fy - 20), 'gfF', dict(sigma=2)),
            (lambda s, fx, fy: s.rect(fx - 4.5, fy - 34, fx + 4.5, fy - 29), 'hri', dict(sigma=1.5))]


def chest():
    return [(lambda s, fx, fy: s.rect(fx - 18, fy - 22, fx + 18, fy), 'hriu', dict(sigma=5)),
            (lambda s, fx, fy: s.ellipse(fx, fy - 22, 18, 8), 'riuj', dict(sigma=4)),
            (lambda s, fx, fy: (s.rect(fx - 18, fy - 14, fx + 18, fy - 11), s.rect(fx - 2.5, fy - 18, fx + 2.5, fy - 8)), 'yK', dict(sigma=1.2, gloss='L'))]


def coins(n=7, spread=20):
    pts = [(-14, -4), (12, -3), (-4, -2), (4, -10), (-10, -12), (16, -10), (0, -18), (-18, -10), (8, -18)][:n]
    return [(lambda s, fx, fy, pts=pts: [s.ellipse(fx + x, fy + y, 5, 3.4) for x, y in pts], 'yKL', dict(sigma=2, gloss='L'))]


ITEMS = {
    'athame': blade('5GH8', 'cIC', length=30, width=3.6),
    'nether_blade': blade('vpPW', '0BD', length=46),
    'godslayer': blade('GH8F', 'yKL', length=50, width=3.8),
    'orthuss_axe': [(lambda s, fx, fy: s.stroke([(fx - 10, fy - 2), (fx + 10, fy - 50)], 2.6, 2.6, smooth=False), 'hri', dict(sigma=1.2)),
                    (lambda s, fx, fy: s.poly([(fx + 6, fy - 46), (fx + 22, fy - 52), (fx + 24, fy - 34), (fx + 10, fy - 34)]), '3456', dict(sigma=3, gloss='8'))],
    'golden_hammer': [(lambda s, fx, fy: s.stroke([(fx - 8, fy - 2), (fx + 6, fy - 40)], 2.6, 2.6, smooth=False), 'hri', dict(sigma=1.2)),
                      (lambda s, fx, fy: s.poly([(fx - 6, fy - 44), (fx + 12, fy - 50), (fx + 16, fy - 38), (fx - 2, fy - 32)]), 'iyKL', dict(sigma=3, gloss='L'))],
    'crown_of_akharien': [(lambda s, fx, fy: s.poly([(fx - 16, fy - 4), (fx - 18, fy - 22), (fx - 9, fy - 12), (fx, fy - 26), (fx + 9, fy - 12), (fx + 18, fy - 22), (fx + 16, fy - 4)]),
                           'GH8F', dict(sigma=4, gloss='F')), (lambda s, fx, fy: s.ellipse(fx, fy - 10, 3.5, 3.5), 'nNg', dict(sigma=2))],
    'crown_of_command': [(lambda s, fx, fy: s.poly([(fx - 16, fy - 4), (fx - 18, fy - 22), (fx - 9, fy - 12), (fx, fy - 26), (fx + 9, fy - 12), (fx + 18, fy - 22), (fx + 16, fy - 4)]),
                          'iyKL', dict(sigma=4, gloss='L')), (lambda s, fx, fy: s.ellipse(fx, fy - 10, 3.5, 3.5), 'cCR', dict(sigma=2))],
    'black_mirror': [(lambda s, fx, fy: s.ellipse(fx, fy - 22, 14, 18), 'uyK', dict(sigma=4)), (lambda s, fx, fy: s.ellipse(fx, fy - 22, 10, 14), '01v', dict(sigma=5, rim='', gloss='p')),
                     (lambda s, fx, fy: s.rect(fx - 2, fy - 4, fx + 2, fy), 'uy', dict(sigma=1))],
    'dragons_horde': chest() + coins(9),
    'treasure': chest() + coins(4),
    'empty_bier': [(lambda s, fx, fy: s.rect(fx - 22, fy - 14, fx + 22, fy - 6), 'hriu', dict(sigma=3)),
                   (lambda s, fx, fy: (s.rect(fx - 20, fy - 6, fx - 16, fy), s.rect(fx + 16, fy - 6, fx + 20, fy)), 'hri', dict(sigma=1.2)),
                   (lambda s, fx, fy: s.rect(fx - 20, fy - 18, fx + 20, fy - 13), '678', dict(sigma=2))],
    'gela': [(lambda s, fx, fy: s.stroke([(fx - 12, fy - 26), (fx, fy - 12), (fx + 12, fy - 26)], 1.2, 1.2), 'yK', dict(sigma=1)),
             (lambda s, fx, fy: s.ellipse(fx, fy - 10, 8, 8), 'sSW', dict(sigma=4, gloss='W'))],
    'jade_torc': [(lambda s, fx, fy: (s.ellipse(fx, fy - 14, 16, 12), s.ellipse(fx, fy - 14, 11, 7, fill=0)), 'ktTS', dict(sigma=3, gloss='W'))],
    'healing_salve': [(lambda s, fx, fy: s.rect(fx - 12, fy - 18, fx + 12, fy), 'G78', dict(sigma=4)), (lambda s, fx, fy: s.ellipse(fx, fy - 18, 12, 4), 'riu', dict(sigma=2)),
                      (lambda s, fx, fy: (s.rect(fx - 2, fy - 14, fx + 2, fy - 3), s.rect(fx - 6, fy - 10, fx + 6, fy - 7)), 'CC', dict(rim='', flat=0))],
    'infernal_grimoire': [(lambda s, fx, fy: s.poly([(fx - 18, fy - 4), (fx + 16, fy - 8), (fx + 18, fy - 22), (fx - 16, fy - 18)]), 'cICR', dict(sigma=3)),
                          (lambda s, fx, fy: s.ellipse(fx, fy - 13, 4, 3), 'lL', dict(sigma=2))],
    'pieces_of_barnaxus': [(lambda s, fx, fy: s.ellipse(fx - 8, fy - 8, 9, 7), 'uxyK', dict(sigma=4, gloss='L')),
                           (lambda s, fx, fy: s.rect(fx + 2, fy - 16, fx + 18, fy - 2), 'uxyK', dict(sigma=4, gloss='L')),
                           (lambda s, fx, fy: s.ellipse(fx - 2, fy - 20, 6, 6), 'nNgF', dict(sigma=3))],
    'potion_of_invisibility': bottle('nNgf'),
    'potion_of_restoration': bottle('cCRl'),
    'rod_of_winds': [(lambda s, fx, fy: s.stroke([(fx - 10, fy - 2), (fx + 8, fy - 48)], 2.6, 2.6, smooth=False), 'riyK', dict(sigma=1.2)),
                     (lambda s, fx, fy: s.stroke([(fx - 2, fy - 50), (fx + 10, fy - 56), (fx + 18, fy - 48), (fx + 10, fy - 40), (fx + 4, fy - 46)], 1.6, 1.0), 'gfF', dict(sigma=1.2))],
    'scorched_staff': [(lambda s, fx, fy: s.stroke([(fx - 10, fy - 2), (fx + 8, fy - 48)], 2.8, 2.4), 'hBD', dict(sigma=1.2)),
                       (lambda s, fx, fy: s.ellipse(fx + 8, fy - 50, 4, 4), 'eEl', dict(sigma=2))],
    'staff_of_souls': [(lambda s, fx, fy: s.stroke([(fx - 10, fy - 2), (fx + 8, fy - 46)], 2.6, 2.6, smooth=False), 'BDG', dict(sigma=1.2)),
                       (lambda s, fx, fy: s.ellipse(fx + 8, fy - 50, 6, 6), 'G78', dict(sigma=3)),
                       (lambda s, fx, fy: (s.ellipse(fx + 6, fy - 51, 1.5, 1.5), s.ellipse(fx + 10, fy - 51, 1.5, 1.5)), 'SS', dict(rim='', flat=0))],
    'sylivens_perfect_lyre': [(lambda s, fx, fy: (s.stroke([(fx - 12, fy - 4), (fx - 16, fy - 26), (fx - 10, fy - 34)], 2.2, 1.6), s.stroke([(fx + 12, fy - 4), (fx + 16, fy - 26), (fx + 10, fy - 34)], 2.2, 1.6),
                                                     s.stroke([(fx - 12, fy - 4), (fx + 12, fy - 4)], 2.4, 2.4, smooth=False)), 'iyKL', dict(sigma=1.5, gloss='L')),
                              (lambda s, fx, fy: [s.stroke([(fx + k * 4, fy - 6), (fx + k * 4, fy - 26)], 0.6, 0.6, smooth=False) for k in (-2, -1, 0, 1, 2)], '88', dict(rim='', flat=0))],
    'timor_mask': [(lambda s, fx, fy: s.ellipse(fx, fy - 16, 14, 15), 'G78', dict(sigma=6)), (lambda s, fx, fy: (s.ellipse(fx - 5, fy - 19, 3.4, 2.4), s.ellipse(fx + 5, fy - 19, 3.4, 2.4), s.ellipse(fx, fy - 9, 4, 2)), '00', dict(rim='', flat=0)),
                   (lambda s, fx, fy: s.stroke([(fx - 12, fy - 28), (fx, fy - 32), (fx + 12, fy - 28)], 1.4, 1.4), 'cC', dict(sigma=1))],
    'war': [(lambda s, fx, fy: s.stroke([(fx - 6, fy - 2), (fx - 2, fy - 46)], 2.4, 2.4, smooth=False), 'hri', dict(sigma=1)),
            (lambda s, fx, fy: s.poly([(fx - 2, fy - 46), (fx + 20, fy - 42), (fx + 14, fy - 36), (fx + 20, fy - 30), (fx - 2, fy - 30)]), 'cICR', dict(sigma=3))],
}
for _n, _d in ITEMS.items():
    RECIPES['equipment_' + _n] = (lambda d=_d: item(d))



# ================================================================== variants (each its own sprite, so no in-game remaps)
def revariant(base, recolors):
    cv = RECIPES[base]()
    L = Layer(cv.a)
    for sel, keys in recolors:
        L.recolor(L.sel(**sel), keys, lo=2, hi=99)
    cv.a = L.a
    return cv


METAL = dict(cmax=12, lmin=5)
CORE = dict(hue=(20, 80), cmin=50)
GOLEMS = {
    'iron_golem': [],
    'wood_golem': [(METAL, 'hriuj'), (CORE, 'qmO')],
    'mud_golem': [(METAL, 'uijQY'), (CORE, 'riy')],
    'bone_golem': [(METAL, 'jQY78'), (CORE, 'sSW')],
    'mithril_golem': [(METAL, 'dnNgf'), (CORE, 'pPW')],
    'nullstone_golem': [(METAL, '01v23'), (CORE, 'gfF')],
    'clockwork_golem': [(METAL, 'uxyKL'), (CORE, 'eEl')],
    'flesh_golem': [(METAL, 'crbaa'), (CORE, 'cCR')],
}
for _n, _r in GOLEMS.items():
    RECIPES[_n] = (lambda r=_r: revariant('golem', r))

FIRE = dict(hue=(10, 90), cmin=30)
RECIPES['aurealis'] = lambda: revariant('fire_elemental', [(dict(lmin=0), 'yKLL8')])
RECIPES['ice_elemental'] = lambda: revariant('water_elemental', [(dict(lmin=0), 'NgfFF')])
RECIPES['lightning_elemental'] = lambda: revariant('air_elemental', [(dict(lmin=60), 'xylL'), (dict(lmax=60), 'vp')])
RECIPES['beastman'] = lambda: revariant('werewolf', [(dict(cmax=20, lmin=15), 'hriuj')])
RECIPES['tar_demon'] = lambda: revariant('manes', [(dict(hue=(250, 340), cmin=10), '0hBD')])
RECIPES['meshabber'] = lambda: revariant('balor', [(dict(hue=(330, 40), cmin=25), 'vpPW')])
RECIPES['angel_of_death'] = lambda: revariant('angel', [(dict(cmax=16, lmin=30), '0123'), (GOLD, 'cIC')])
RECIPES['repentant_angel'] = lambda: revariant('angel', [(dict(cmax=16, lmin=30), '45DG'), (GOLD, '6H')])


if __name__ == '__main__':
    os.makedirs(f'{WORK}/crops/comp', exist_ok=True)
    for name, fn in RECIPES.items():
        if ONLY and name not in ONLY:
            continue
        cv = fn()
        out = cv.save(f'{WORK}/crops/comp/{name}.png')
        print(f'{name:24s} {out.w}x{out.h}')
