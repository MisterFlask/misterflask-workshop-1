"""Greedy + constrained k-means: keep BASE fixed, add extras that best represent the unit pixels."""
import glob, json, os, sys
import numpy as np
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from colorlib import rgb_to_lab, lab_to_rgb, kmeans_lab, hexof, rgb_of, lch
from pal_v3 import BASE

R = sys.argv[1]
NMAX = int(sys.argv[2]) if len(sys.argv) > 2 else 24
px = []
SRC = sys.argv[3] if len(sys.argv) > 3 else 'small'
for f in sorted(glob.glob(f'{R}/{SRC}/*.png')):
    a = np.asarray(Image.open(f).convert('RGBA'))
    px.append(a[a[..., 3] > 0][:, :3])
px = np.concatenate(px).astype(np.float64)
_L = rgb_to_lab(px)[:, 0]
px = px[_L > 9]  # outline darks all map to void
rng = np.random.default_rng(5)
px = px[rng.choice(len(px), min(len(px), 120000), replace=False)]
X = rgb_to_lab(px)
base = rgb_to_lab(np.array([rgb_of(h) for (_, _, _, h, _) in BASE]))
# candidate pool: fine k-means of the unit pixels
cand_rgb, _ = kmeans_lab(px, 80, iters=20, seed=9)
cand = rgb_to_lab(cand_rgb)


def err(P):
    d = np.sqrt(((X[:, None, :] - P[None, :, :]) ** 2).sum(-1))
    return d.min(1), d.argmin(1)


extras = np.zeros((0, 3))
cur, _ = err(base)
print(f'base: mean dE {cur.mean():.2f}')
log = []
for step in range(NMAX):
    best, bi = None, -1
    for i, c in enumerate(cand):
        dm = np.minimum(cur, np.sqrt(((X - c) ** 2).sum(-1)))
        m = dm.mean()
        if best is None or m < best:
            best, bi = m, i
    extras = np.vstack([extras, cand[bi]])
    # refine extras only (Lloyd), base fixed
    for _ in range(6):
        P = np.vstack([base, extras])
        _, lab_ = err(P)
        for j in range(len(extras)):
            sel = lab_ == len(base) + j
            if sel.sum() > 20:
                extras[j] = X[sel].mean(0)
    cur, _ = err(np.vstack([base, extras]))
    log.append((step + 1, float(cur.mean()), float(np.percentile(cur, 90))))
    print(f'+{step+1:2d}: mean dE {cur.mean():.2f}  p90 {np.percentile(cur,90):.2f}')
erg = lab_to_rgb(extras)
out = []
for c in erg:
    L, C, H = lch(c)
    out.append(dict(hex=hexof(c), L=round(L, 1), C=round(C, 1), h=round(H)))
json.dump(dict(extras=out, log=log), open(f'{R}/extras.json', 'w'), indent=1)
for o in out:
    print(o)
