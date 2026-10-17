"""Build every palette artifact from pal_v3: palette.txt, the ART.md palette/remap blocks, the
style.css :root tokens and the preview CSS subset. Remap tables are derived by colour math."""
import os, sys
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from colorlib import rgb_to_lab, rgb_of
from pal_v3 import BASE, EXTRAS

R = sys.argv[1]
os.makedirs(f'{R}/out', exist_ok=True)
PAL = [(k, n, h, f) for (_, k, n, h, f) in BASE] + list(EXTRAS)
KEYS = [p[0] for p in PAL]
N = len(PAL)
LAB = rgb_to_lab(np.array([rgb_of(p[2]) for p in PAL]))
KI = {k: i for i, k in enumerate(KEYS)}

LIGHTS = set('lLWSREK')          # light sources and their cores: never a fog/pool *target* for FOG
OUTLINE = KI['0']


def nearest(target, allowed, src=None, darker=False):
    best, bi = None, src
    for i in allowed:
        if darker and src is not None and LAB[i, 0] > LAB[src, 0] + 0.5:
            continue
        d = float(((LAB[i] - target) ** 2).sum())
        if best is None or d < best:
            best, bi = d, i
    return bi


def table(fn, allowed, darker=False, keep_outline=True):
    out = []
    for i in range(N):
        if keep_outline and i == OUTLINE:
            out.append(i)
            continue
        L, a, b = LAB[i]
        out.append(nearest(np.array(fn(L, a, b)), allowed, i, darker))
    return out


allk = range(N)
nolight = [i for i in allk if KEYS[i] not in LIGHTS]
notvoid = [i for i in allk if i != OUTLINE]
TABLES = {
    # remembered, not black: dimmer and greyer (daytime fog of war)
    'FOG': table(lambda L, a, b: (L * 0.80, a * 0.55, b * 0.55), [i for i in nolight], darker=True),
    # warm light: lighter, toward amber
    'FIRE': table(lambda L, a, b: (min(96, L + 10), a * 0.6 + 14, b * 0.6 + 34), notvoid),
    # corruption: redder and a little darker
    'BLOOD': table(lambda L, a, b: (L * 0.82, a * 0.4 + 34, b * 0.4 + 12), notvoid),
    # ritual: toward the pale unnatural green-teal
    'RITE': table(lambda L, a, b: (min(95, L * 0.9 + 8), a * 0.35 - 30, b * 0.35 + 2), notvoid),
    # the arcane: toward violet
    'ARC': table(lambda L, a, b: (L * 0.9, a * 0.35 + 26, b * 0.35 - 34), notvoid),
}
# remembered water stays water: step down within the navy/frost family
for src, dst in {'d': 'd', 'n': 'd', 'N': 'n', 'g': 'N', 'f': 'g', 'F': 'f', 'J': 'd'}.items():
    TABLES['FOG'][KI[src]] = KI[dst]
# ---- the passes' tables (hand-authored, docs/ART.md 2.4): the ground pass (terrain/ground.js) and the
# light pass's rings (cities/citylight.js). Pairs `a:b` remap a to b; every other key maps to itself.
PASS = {
    # one step lighter within the family, for ground colours only (path and river verges; water, ink and
    # lights unchanged)
    'VERGE': 'q:m m:o o:O k:t t:T u:i i:j j:Q Q:Y x:y y:Y Y:7 1:2 2:3 3:4 4:5 5:6 6:7 g:f v:p w:z z:A A:O D:G G:H h:u r:x',
    # the sunlit step of a ground colour (clearings): one step lighter within its own family -- grass to
    # olive and lichen, wheat to sand, greys up the ladder; snow, water, hell purples and lights unchanged
    'MOON': 'q:m m:o o:O u:i i:j j:Q Q:Y y:Y 3:4 4:5 5:6 6:7 k:t t:T x:y w:z z:A A:O D:G G:H',
    # the sunlit grain: in the brightest clearings a share of the ground catches the sun one step further
    # (grass to lichen, wheat to sand)
    'GRAIN': 'q:o m:o o:O u:i i:j j:Y Q:Y y:Y 3:4 4:5 5:6 k:t t:T x:y w:z z:A A:O D:G G:H',
    # the shaded step (cloud shade): one step darker in the family
    'SHADE': 'm:q o:m O:o Q:j y:j Y:Q j:i i:u 5:4 6:5 4:3 t:k T:t G:D H:G A:z z:w',
}
for _n, _spec in PASS.items():
    _t = list(range(N))
    for _p in _spec.split():
        _a, _b = _p.split(':')
        _t[KI[_a]] = KI[_b]
    TABLES[_n] = _t
# remembered ground (the fog pass): FOG, but snow and ice go to pale greys rather than blue
TABLES['REMEMBER'] = list(TABLES['FOG'])
for _a, _b in (('f', '6'), ('F', 'H'), ('g', '5')):
    TABLES['REMEMBER'][KI[_a]] = KI[_b]
# the light pass's rings (cities/citylight.js RINGS): the outer, inner and core tables of each pool
# family that the derived tables above do not give; keys not listed map to themselves
RING_ROWS = """
key WARM     HEARTH   EMBER    SMOULDER GORE     AURA     HALLOW   HAZE
0   u        e        e        c        c        v        k        k
1   u        e        e        c        c        v        s        k
2   u        e        E        c        c        v        s        t
3   x        e        E        c        C        p        s        t
4   x        x        E        x        C        p        s        t
5   y        E        l        x        C        P        S        T
6   Y        E        l        C        R        P        S        S
7   8        l        L        C        R        8        W        W
8   8        l        L        C        R        8        W        W
d   d        d        d        d        d        d        d        d
n   n        n        n        n        n        n        n        n
N   N        e        E        c        C        p        s        t
g   g        E        l        x        C        P        S        T
f   Y        E        l        x        C        P        S        S
F   7        l        L        C        R        8        W        W
k   u        e        E        c        c        v        s        t
t   x        x        E        c        C        p        s        T
T   y        E        l        x        C        P        S        S
q   x        x        E        c        c        p        s        t
m   y        E        l        x        C        p        S        T
o   Y        E        l        x        C        P        S        T
O   Y        l        L        x        C        P        S        S
u   x        x        E        c        C        p        s        t
Q   y        E        l        x        C        p        S        T
x   y        E        l        C        C        P        S        T
y   Y        l        L        x        C        P        S        S
Y   7        l        L        C        R        P        W        S
v   x        x        E        c        c        p        s        t
p   x        E        l        x        C        P        S        T
P   Y        l        L        C        R        8        W        S
e   E        E        l        C        C        p        s        t
E   l        l        L        C        R        P        S        S
l   L        L        L        R        R        P        W        W
L   L        L        L        R        R        8        W        W
c   x        x        E        C        C        p        s        t
C   E        E        l        C        R        P        S        T
R   l        l        L        R        R        P        W        S
s   x        x        E        c        C        p        S        T
S   y        E        l        x        C        P        W        W
W   8        l        L        C        R        8        W        W
"""
_rows = [r.split() for r in RING_ROWS.strip().split('\n')]
for _j, _n in enumerate(_rows[0][1:]):
    _t = list(range(N))
    for _r in _rows[1:]:
        _t[KI[_r[0]]] = KI[_r[_j + 1]]
    TABLES[_n] = _t

# tinctures: name field shade charge stitch under [tick]
TINCT = [
    ('Argent',  'H', 'D', '1', 'H', 'D'),
    ('Or',      'K', 'j', '1', 'K', 'j'),
    ('Gules',   'C', 'I', '8', 'C', 'I'),
    ('Azure',   'J', 'd', '8', 'J', 'd'),
    ('Vert',    'T', 't', '8', 'T', 't'),
    ('Purpure', 'P', 'p', '8', 'P', 'p'),
    ('Tenne',   'E', 'x', '1', 'E', 'x'),
    ('Celeste', 'g', 'N', '1', 'g', 'N'),
    ('Sable',   '2', '1', '7', '2', '1', '7'),
    ('Cendree', 'D', 'B', '8', 'D', 'B'),
]

names = list(TABLES)
pal_lines = [f"{i:2d} {k} {n:8s} {h} {f}" for i, (k, n, h, f) in enumerate(PAL)]
remap_lines = [("key  " + ' '.join(f"{t:8s}" for t in names)).rstrip()]
for i, k in enumerate(KEYS):
    remap_lines.append((f"{k}    " + ' '.join(f"{KEYS[TABLES[t][i]]:8s}" for t in names)).rstrip())
tinct_lines = [' '.join([t[0].ljust(7)] + list(t[1:])) for t in TINCT]

with open(f'{R}/out/palette.txt', 'w') as f:
    f.write(f"; palette.txt: the closed {N}-colour palette of docs/ART.md section 2 (\"cheerful readability\").\n")
    f.write("; Keep in sync with the guide: tools/validate.mjs compares this file with its `palette` and `remap` blocks.\n;\n")
    f.write("; palette: index key name hex family. Keys are the characters used in sprite grids.\npalette\n")
    f.write('\n'.join(pal_lines) + '\n\n')
    f.write("; remap: index remaps (section 2.4). FOG dims and greys (remembered ground, never black) and\n")
    f.write("; puts the lights out; FIRE, BLOOD, RITE and ARC pull toward firelight, corruption, ritual and the arcane.\n")
    f.write("; The passes' own tables follow: VERGE, MOON, GRAIN, SHADE and REMEMBER (the ground pass) and the\n")
    f.write("; light pass's rings WARM, HEARTH, EMBER, SMOULDER, GORE, AURA, HALLOW and HAZE (cities/citylight.js).\n")
    f.write("remap\n" + '\n'.join(remap_lines) + '\n\n')
    f.write("; tinctures (section 7.1): name field shade charge stitch stitch-under [tick]\n")
    f.write("; A border stitches with `stitch` over `under`; Sable adds a bone tick on every dash.\n")
    f.write("tinctures\n" + '\n'.join(tinct_lines) + '\n')

with open(f'{R}/out/art_blocks.md', 'w') as f:
    f.write("```palette\n" + '\n'.join(pal_lines) + "\n```\n\n```remap\n" + '\n'.join(remap_lines) + "\n```\n")

# style.css :root tokens, 6 per line
toks = [f"--c-{n}: {h};" for (k, n, h, fam) in PAL]
css = ['  ' + ' '.join(toks[i:i + 6]) for i in range(0, len(toks), 6)]
open(f'{R}/out/css_tokens.txt', 'w').write('\n'.join(css) + '\n')
print('colours', N)
for t in names:
    print(t, ''.join(KEYS[j] for j in TABLES[t]))
print('keys ', ''.join(KEYS))
