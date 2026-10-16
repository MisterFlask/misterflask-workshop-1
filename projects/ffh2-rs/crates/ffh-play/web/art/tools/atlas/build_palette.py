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
remap_lines = ["key  " + ' '.join(f"{t:5s}" for t in names)]
for i, k in enumerate(KEYS):
    remap_lines.append(f"{k}    " + ' '.join(f"{KEYS[TABLES[t][i]]:5s}" for t in names))
tinct_lines = [' '.join([t[0].ljust(7)] + list(t[1:])) for t in TINCT]

with open(f'{R}/out/palette.txt', 'w') as f:
    f.write(f"; palette.txt: the closed {N}-colour palette of docs/ART.md section 2 (\"cheerful readability\").\n")
    f.write("; Keep in sync with the guide: tools/validate.mjs compares this file with its `palette` and `remap` blocks.\n;\n")
    f.write("; palette: index key name hex family. Keys are the characters used in sprite grids.\npalette\n")
    f.write('\n'.join(pal_lines) + '\n\n')
    f.write("; remap: index remaps (section 2.4). FOG dims and greys (remembered ground, never black) and\n")
    f.write("; puts the lights out; FIRE, BLOOD, RITE and ARC pull toward firelight, corruption, ritual and the arcane.\n")
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
