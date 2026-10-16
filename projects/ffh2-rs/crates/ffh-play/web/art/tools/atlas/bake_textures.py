"""Bake the cheerful terrain textures (64x64, seamless, palette keys) -> out/textures.spr + preview.

Each recipe: a base, soft dithered variation, then small lit marks (tufts, flowers, dune crests,
wave crests...). Hell terrains reuse their twin's layout (same seed) with their own colours, so the
land they were stays readable."""
import os, sys, random
import numpy as np
from PIL import Image, ImageDraw, ImageFont
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from colorlib import rgb_of
from pal_v3 import BASE, EXTRAS

R = sys.argv[1]
S = 64
HEX = {k: h for (_, k, _, h, _) in BASE}
HEX.update({k: h for (k, _, h, _) in EXTRAS})


class Tex:
    def __init__(self, base):
        self.g = [[base] * S for _ in range(S)]
        self.m = [[False] * S for _ in range(S)]

    def set(self, x, y, k, mark=True):
        self.g[y % S][x % S] = k
        if mark:
            self.m[y % S][x % S] = True

    def get(self, x, y):
        return self.g[y % S][x % S]

    def free(self, x, y, r):
        return not any(self.m[(y + dy) % S][(x + dx) % S] for dy in range(-r, r + 1) for dx in range(-r, r + 1))

    def patch(self, cx, cy, rx, ry, k, pat, only=None):
        for y in range(-ry, ry + 1):
            for x in range(-rx, rx + 1):
                if (x * x) / (rx * rx + 0.01) + (y * y) / (ry * ry + 0.01) > 1:
                    continue
                X, Y = (cx + x) % S, (cy + y) % S
                if only and self.g[Y][X] not in only:
                    continue
                on = {'checker': (X + Y) % 2 == 0, 'grid': X % 2 == 0 and Y % 2 == 0,
                      'solid': True, 'dense': (X + Y) % 2 == 0 or (X % 2 == 0 and Y % 2 == 1 and (X + Y) % 4 == 1)}[pat]
                if on:
                    self.g[Y][X] = k

    def rows(self):
        return [''.join(r) for r in self.g]


def scatter(t, rnd, n, gap, fn):
    placed = 0
    for _ in range(n * 60):
        if placed >= n:
            break
        x, y = rnd.randrange(S), rnd.randrange(S)
        if t.free(x, y, gap):
            fn(t, x, y, rnd)
            placed += 1


def variation(t, rnd, light, dark, n=10, lpat='grid', dpat='grid'):
    """Soft variation: small 25% grids (or checkers) of a near tone, never solid blobs."""
    for i in range(n):
        rx, ry = rnd.randint(3, 6), rnd.randint(2, 3)
        t.patch(rnd.randrange(S), rnd.randrange(S), rx, ry, light if i % 2 == 0 else dark, lpat if i % 2 == 0 else dpat)


# ---------------------------------------------------------------- recipes
def grass(seed=11, base='m', light='o', dark='q', tip='O', flowers=('L', '8')):
    rnd = random.Random(seed)
    t = Tex(base)
    variation(t, rnd, light, dark, 10, 'checker', 'grid')

    def tuft(t, x, y, rnd):
        # a little lit tuft: tip on top, blades either side, a shadow pixel under
        t.set(x, y - 1, tip); t.set(x - 1, y, light); t.set(x + 1, y, light); t.set(x, y, light); t.set(x, y + 1, dark)
    scatter(t, rnd, 70, 2, tuft)
    if flowers:
        def flower(t, x, y, rnd):
            t.set(x, y, flowers[rnd.randrange(len(flowers))]); t.set(x, y + 1, dark)
        scatter(t, rnd, 14, 3, flower)
    return t


def plains(seed=23, base='Q', dark='j', light='Y', grass1='o', grass2='A'):
    rnd = random.Random(seed)
    t = Tex(base)
    variation(t, rnd, light, dark, 8)

    def stubble(t, x, y, rnd):
        n = rnd.randint(2, 4)
        for i in range(n):
            t.set(x + i, y, light); t.set(x + i, y + 1, dark)
    scatter(t, rnd, 46, 2, stubble)

    def tuft(t, x, y, rnd):
        t.set(x, y - 1, grass1); t.set(x - 1, y, grass2); t.set(x + 1, y, grass2); t.set(x, y, grass1)
    scatter(t, rnd, 16, 3, tuft)
    return t


def desert(seed=37, base='Y', dark='Q', under='y', crest='8', speck='j'):
    rnd = random.Random(seed)
    t = Tex(base)
    variation(t, rnd, '7', dark, 6)

    def dune(t, x, y, rnd):
        n = rnd.randint(5, 9)
        for i in range(n):
            dy = -1 if 1 <= i < n - 1 else 0
            t.set(x + i, y + dy, crest); t.set(x + i, y + dy + 1, under)
    scatter(t, rnd, 22, 3, dune)

    def sp(t, x, y, rnd):
        t.set(x, y, speck)
    scatter(t, rnd, 30, 2, sp)
    return t


def tundra(seed=41, base='G', dark='D', lichen='A', tip='O', crack='5'):
    rnd = random.Random(seed)
    t = Tex(base)
    variation(t, rnd, lichen, dark, 10, 'checker', 'grid')

    def li(t, x, y, rnd):
        t.set(x, y, lichen); t.set(x + 1, y, tip if rnd.random() < 0.5 else lichen)
    scatter(t, rnd, 60, 2, li)

    def cr(t, x, y, rnd):
        for i in range(rnd.randint(2, 4)):
            t.set(x + i, y + (i % 2), crack)
    scatter(t, rnd, 14, 3, cr)
    return t


def snow(seed=53, base='F', shadow='f', deep='g', spark='8'):
    rnd = random.Random(seed)
    t = Tex(base)
    variation(t, rnd, shadow, shadow, 6)

    def streak(t, x, y, rnd):
        n = rnd.randint(4, 8)
        for i in range(n):
            t.set(x + i, y, shadow)
        t.set(x + n // 2, y + 1, deep)
    scatter(t, rnd, 20, 3, streak)

    def sp(t, x, y, rnd):
        t.set(x, y, spark)
    scatter(t, rnd, 10, 4, sp)
    return t


def marsh(seed=61, base='q', dark='w', water='N', glint='g', reed='O'):
    rnd = random.Random(seed)
    t = Tex(base)
    variation(t, rnd, 'm', dark, 10, 'checker', 'grid')

    def slit(t, x, y, rnd):
        n = rnd.randint(3, 6)
        for i in range(n):
            t.set(x + i, y, water)
        t.set(x + rnd.randrange(n), y, glint)
    scatter(t, rnd, 22, 3, slit)

    def rd(t, x, y, rnd):
        t.set(x, y, reed); t.set(x, y + 1, 'm')
    scatter(t, rnd, 26, 2, rd)
    return t


def water(seed, base, mid, crest, glint, n_crest, n_glint):
    rnd = random.Random(seed)
    t = Tex(base)
    variation(t, rnd, mid, mid, 5)

    def wave(t, x, y, rnd):
        # a small smile-shaped crest: lit dashes, the middle a pixel lower
        n = rnd.randint(3, 6)
        for i in range(n):
            dy = 1 if 0 < i < n - 1 else 0
            t.set(x + i, y + dy, crest)
    scatter(t, rnd, n_crest, 3, wave)

    def gl(t, x, y, rnd):
        t.set(x, y, glint)
    scatter(t, rnd, n_glint, 4, gl)
    return t


def recolour(t, table):
    out = Tex('0')
    out.g = [[table.get(k, k) for k in row] for row in t.g]
    return out


TEX = {}
TEX['grass'] = (grass(), 'TERRAIN_GRASS', 'sunny meadow: lit tufts, a few yellow and white flowers')
TEX['plains'] = (plains(), 'TERRAIN_PLAINS', 'golden wheat stubble with a few green tufts')
TEX['desert'] = (desert(), 'TERRAIN_DESERT', 'warm sand with lit dune crests')
TEX['tundra'] = (tundra(), 'TERRAIN_TUNDRA', 'cool grey ground with lichen and small cracks')
TEX['snow'] = (snow(), 'TERRAIN_SNOW', 'bright snow with blue wind streaks and sparkles')
TEX['marsh'] = (marsh(), 'TERRAIN_MARSH', 'wet green with water slits and reeds')
TEX['coast'] = (water(71, 'n', 'N', 'g', 'F', 26, 8), 'TERRAIN_COAST', 'friendly blue shallows with light wave crests')
TEX['ocean'] = (water(79, 'd', 'n', 'N', 'g', 18, 6), 'TERRAIN_OCEAN', 'deep blue with sparse wave crests')
# hell twins: their twin's layout, recoloured
TEX['broken_lands'] = (recolour(grass(flowers=('R',)), {'m': 'u', 'o': 'r', 'q': 'h', 'O': 'I', 'R': 'R'}), 'TERRAIN_BROKEN_LANDS',
                       'hell grass: cracked brown earth, ember-red where the tufts were')
TEX['fields_of_perdition'] = (recolour(plains(), {'Q': 'p', 'j': 'v', 'Y': 'P', 'o': 'I', 'A': 'c'}), 'TERRAIN_FIELDS_OF_PERDITION',
                              'hell plains: purple ash fields, blood-red furrows')
TEX['burning_sands'] = (recolour(desert(), {'Y': 'x', 'Q': 'r', 'y': 'e', '8': 'l', 'j': 'E'}), 'TERRAIN_BURNING_SANDS',
                        'hell desert: smouldering orange sand, amber crests')
TEX['shallows'] = (recolour(marsh(), {'q': 'v', 'w': '1', 'm': 'p', 'N': 'I', 'g': 'C', 'O': 'D'}), 'TERRAIN_SHALLOWS',
                   'hell marsh: black-purple bog with blood slits and dead reeds')

ORDER = ['grass', 'plains', 'desert', 'tundra', 'snow', 'marsh', 'coast', 'ocean',
         'broken_lands', 'fields_of_perdition', 'burning_sands', 'shallows']
os.makedirs(f'{R}/out', exist_ok=True)
with open(f'{R}/out/textures.spr', 'w') as f:
    f.write('; terrain textures (docs/ART.md 5.1-5.2): seamless 64x64, sampled in map space through the domain warp.\n')
    f.write('; Bright, readable ground: a clear base, soft dithered variation, small lit marks (tufts, flowers, dune and\n')
    f.write('; wave crests). Hell terrains are their twin\'s layout recoloured. Baked by the revamp\'s texture recipes.\n\n')
    for n in ORDER:
        t, ty, note = TEX[n]
        f.write(f'= terrain/{n} 64x64 0,0\n: types {ty}\n: note {note}\n')
        f.write('\n'.join(t.rows()) + '\n\n')

# preview: each texture tiled 2x2 at 3x
SC = 3
cell = S * 2 * SC
im = Image.new('RGB', (4 * (cell + 10) + 10, 3 * (cell + 24) + 10), (30, 30, 36))
d = ImageDraw.Draw(im)
font = ImageFont.load_default()
for i, n in enumerate(ORDER):
    t = TEX[n][0]
    tile = Image.new('RGB', (S, S))
    px = tile.load()
    for y in range(S):
        for x in range(S):
            px[x, y] = tuple(int(v) for v in rgb_of(HEX[t.g[y][x]]))
    big = Image.new('RGB', (S * 2, S * 2))
    for oy in (0, S):
        for ox in (0, S):
            big.paste(tile, (ox, oy))
    big = big.resize((cell, cell), Image.NEAREST)
    cx, cy = 10 + (i % 4) * (cell + 10), 10 + (i // 4) * (cell + 24)
    im.paste(big, (cx, cy))
    used = sorted({k for row in t.g for k in row})
    d.text((cx, cy + cell + 4), f'{n}  [{"".join(used)}]', fill=(230, 230, 230), font=font)
im.save(f'{R}/textures_preview.png')
print('ok')
