"""Turn mapped unit crops into palette sprites (text grids) + a preview sheet.

    python3 quantize.py WORK [MAPPING.json [OUT]]

out/OUT.spr        the sprites, ready for web/art/units/ (OUT defaults to units_atlas)
OUT_preview.png    each sprite at 4x on grass, owner slots filled red and blue.

A mapping entry's `src` is an atlas crop (atlasN_KK) or a composed figure (comp:NAME, written by
compose_units.py into WORK/crops/comp/ at atlas 2's scale). Options: slots, frame (24, 32, 40),
ship, size (a scale factor, for small creatures), feet (false: keep the bottom rows as drawn,
for painted figures that carry no hex base), maxc (colour budget, default 18)."""
import json, os, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from colorlib import rgb_to_lab, rgb_of, srgb_to_linear, linear_to_srgb
from pal_v3 import BASE, EXTRAS

R = sys.argv[1]
os.makedirs(f'{R}/out', exist_ok=True)
PAL = [(k, n, h, f) for (_, k, n, h, f) in BASE] + list(EXTRAS)
KEYS = [p[0] for p in PAL]
PRGB = np.array([rgb_of(p[2]) for p in PAL])
PLAB = rgb_to_lab(PRGB)
KI = {k: i for i, k in enumerate(KEYS)}
LUM = PLAB[:, 0]
MAP = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(os.path.abspath(__file__)), 'unit_mapping.json')
mapping = json.load(open(MAP))
OUT = sys.argv[3] if len(sys.argv) > 3 else 'units_atlas'
scales = json.load(open(f'{R}/scales.json'))
# the pixel-over pass (cleanup.py): opt-in with FFH_CLEAN=1 until its output is reviewed and installed
import cleanup
CLEAN = os.environ.get('FFH_CLEAN') == '1'
CP = cleanup.Pal(KEYS, PLAB, [p[3] for p in PAL])
FAM = {
    'red': ['R', 'C', 'I', 'c'],
    'blue': ['J', 'N', 'n', 'd'],
    'purple': ['P', 'p', 'v'],
    'olive': ['A', 'z', 'w'],
    'skyblue': ['g', 'N', 'n'],
}
TRANS = -1


def downsample(crop, s):
    a = crop.astype(np.float64)
    alpha = a[..., 3] / 255.0
    lin = srgb_to_linear(a[..., :3])
    w = max(1, int(round(crop.shape[1] * s)))
    h = max(1, int(round(crop.shape[0] * s)))
    def rs(ch):
        return np.asarray(Image.fromarray(ch.astype(np.float32), mode='F').resize((w, h), Image.BOX))
    A = rs(alpha)
    rgb = np.stack([rs(lin[..., c] * alpha) for c in range(3)], -1) / np.maximum(A[..., None], 1e-6)
    return np.clip(np.round(linear_to_srgb(rgb)), 0, 255), A >= 0.5


def quantize(rgb, opaque):
    lab = rgb_to_lab(rgb)
    d = ((lab[..., None, :] - PLAB[None, None, :, :]) ** 2).sum(-1)
    idx = d.argmin(-1)
    idx[lab[..., 0] < 9] = KI['0']
    idx[~opaque] = TRANS
    return idx


def neighbours_trans(idx, y, x):
    H, W = idx.shape
    for dy, dx in ((-1, 0), (1, 0), (0, -1), (0, 1)):
        yy, xx = y + dy, x + dx
        if yy < 0 or yy >= H or xx < 0 or xx >= W or idx[yy, xx] == TRANS:
            return True
    return False


def clean(idx):
    H, W = idx.shape
    # drop isolated pixels
    for y in range(H):
        for x in range(W):
            if idx[y, x] == TRANS:
                continue
            n = 0
            for dy, dx in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                yy, xx = y + dy, x + dx
                if 0 <= yy < H and 0 <= xx < W and idx[yy, xx] != TRANS:
                    n += 1
            if n == 0:
                idx[y, x] = TRANS
    return idx


def outline(idx):
    """Dark edge pixels become the outline; light edges get an outline pixel outside them."""
    H, W = idx.shape
    out = idx.copy()
    add = []
    for y in range(H):
        for x in range(W):
            v = idx[y, x]
            if v == TRANS or not neighbours_trans(idx, y, x):
                continue
            if LUM[v] < 34:
                out[y, x] = KI['0']
            else:
                for dy, dx in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                    yy, xx = y + dy, x + dx
                    if 0 <= yy < H and 0 <= xx < W and idx[yy, xx] == TRANS:
                        add.append((yy, xx))
    for yy, xx in add:
        out[yy, xx] = KI['0']
    return out


def clean_feet(idx):
    """Drop hex-base residue at the feet: rows near the bottom keep only the body's column span,
    and lose grass and pale rim colours."""
    H, W = idx.shape
    ys, xs = np.nonzero(idx != TRANS)
    if len(ys) == 0:
        return idx
    yb = ys.max()
    body = idx[max(0, yb - 9):max(1, yb - 3)]
    bx = np.nonzero((body != TRANS).any(0))[0]
    if len(bx):
        x0, x1 = bx.min() - 1, bx.max() + 1
        for y in range(max(0, yb - 3), yb + 1):
            idx[y, :max(0, x0)] = TRANS
            idx[y, x1 + 1:] = TRANS
    grass = {KI[k] for k in 'qmoOAzktT'}
    rim = {KI[k] for k in 'QjYy'}
    for y in range(max(0, yb - 2), yb + 1):
        for x in range(W):
            v = idx[y, x]
            if v in grass or (v in rim and y >= yb - 1):
                idx[y, x] = TRANS
    # keep only the main connected body (drops detached specks and stray staves)
    from collections import deque
    seen = np.zeros_like(idx, bool)
    best = []
    for sy in range(H):
        for sx in range(W):
            if idx[sy, sx] == TRANS or seen[sy, sx]:
                continue
            comp = []
            q = deque([(sy, sx)]); seen[sy, sx] = True
            while q:
                y, x = q.popleft(); comp.append((y, x))
                for dy in (-1, 0, 1):
                    for dx in (-1, 0, 1):
                        yy, xx = y + dy, x + dx
                        if 0 <= yy < H and 0 <= xx < W and not seen[yy, xx] and idx[yy, xx] != TRANS:
                            seen[yy, xx] = True; q.append((yy, xx))
            if len(comp) > len(best):
                best = comp
    keep = np.zeros_like(seen)
    for y, x in best:
        keep[y, x] = True
    idx[~keep] = TRANS
    return idx


def strip_water(idx):
    H, W = idx.shape
    water = {KI[k] for k in 'dnNgfF'}
    y0 = int(H * 0.62)
    for y in range(y0, H):
        for x in range(W):
            if idx[y, x] in water:
                idx[y, x] = TRANS
    return idx


def slots(idx, mode):
    if mode == 'none':
        return idx, None
    op = idx[idx != TRANS]
    if mode == 'auto':
        best = None
        for fam in ('red', 'blue', 'purple'):
            n = sum(int((op == KI[k]).sum()) for k in FAM[fam])
            if n >= 0.07 * len(op) and (best is None or n > best[1]):
                best = (fam, n)
        if best is None:
            n = sum(int((op == KI[k]).sum()) for k in FAM['olive'])
            if n >= 0.10 * len(op):
                best = ('olive', n)
        if best is None:
            return idx, None
        mode = best[0]
    keys = FAM[mode]
    # lighter half -> field (#), darker -> shade (%)
    lums = sorted(((LUM[KI[k]], k) for k in keys), reverse=True)
    field = {KI[k] for _, k in lums[:max(1, len(lums) // 2)]}
    shade = {KI[k] for _, k in lums[max(1, len(lums) // 2):]}
    out = idx.copy()
    out[np.isin(idx, list(field))] = -2   # '#'
    out[np.isin(idx, list(shade))] = -3   # '%'
    return out, mode


def reduce_colours(idx, maxc=18):
    """Per-sprite palette discipline: merge the least-used colour into its nearest used colour
    (Lab) until at most maxc colours remain (outline and owner slots not counted)."""
    while True:
        vals, cnt = np.unique(idx[idx >= 0], return_counts=True)
        keep = [(v, c) for v, c in zip(vals, cnt) if v != KI['0']]
        if len(keep) <= maxc:
            return idx
        v0 = min(keep, key=lambda t: t[1])[0]
        others = [v for v, _ in keep if v != v0]
        tgt = min(others, key=lambda v: float(((PLAB[v] - PLAB[v0]) ** 2).sum()))
        idx[idx == v0] = tgt


def frame_place(idx, F):
    """Place idx in an FxF frame: bottom opaque row on the anchor row, centred on the anchor col."""
    ax, ay = {24: (11, 21), 32: (15, 29), 40: (19, 37)}[F]
    ys, xs = np.nonzero(idx != TRANS)
    y1 = ys.max()
    cx = (xs.min() + xs.max()) / 2.0
    out = np.full((F, F), TRANS, np.int32)
    oy = ay - y1
    ox = int(round(ax + 0.5 - cx))
    H, W = idx.shape
    for y in range(H):
        for x in range(W):
            if idx[y, x] == TRANS:
                continue
            yy, xx = y + oy, x + ox
            if 0 <= yy < F and 0 <= xx < F:
                out[yy, xx] = idx[y, x]
    return out, ax, ay


def to_text(g):
    ch = {TRANS: '.', -2: '#', -3: '%', -4: '*'}
    return [''.join(ch.get(int(v), KEYS[int(v)] if v >= 0 else '.') for v in row) for row in g]


sprites = []
for name, opt in mapping.items():
    if name.startswith('_'):
        continue
    if opt['src'].startswith('comp:'):
        crop = np.asarray(Image.open(f"{R}/crops/comp/{opt['src'][5:]}.png").convert('RGBA'))
        s = scales['atlas2']
    else:
        atlas, k = opt['src'].rsplit('_', 1)
        crop = np.asarray(Image.open(f'{R}/crops/{atlas}_{k}.png').convert('RGBA'))
        s = scales[atlas]
    s *= opt.get('size', 1.0)
    F = opt.get('frame', 32)
    maxw, maxh = {24: (24, 22), 32: (32, 30), 40: (40, 38)}[F]
    # outline adds up to 1 px each side
    fit = min(1.0, (maxw - 2) / (crop.shape[1] * s), (maxh - 1) / (crop.shape[0] * s))
    rgb, op = downsample(crop, s * fit)
    idx = quantize(rgb, op)
    if opt.get('ship'):
        idx = strip_water(idx)
    idx = clean(idx)
    if not opt.get('ship') and opt.get('feet', True):
        idx = clean_feet(idx)
    if CLEAN and opt.get('cleanup', True):
        idx = cleanup.pre(idx, CP, opt)
    idx = outline(idx)
    idx = reduce_colours(idx, opt.get('maxc', 18))
    if CLEAN and opt.get('cleanup', True):
        idx = cleanup.post(idx, CP, opt)
    before = idx.copy()
    idx, fam = slots(idx, opt.get('slots', 'auto'))
    if CLEAN and opt.get('cleanup', True) and fam:
        idx = cleanup.flecks(idx, before, CP)
    g, ax, ay = frame_place(idx, F)
    sprites.append((name, F, ax, ay, to_text(g), fam, opt['src']))

with open(f'{R}/out/{OUT}.spr', 'w') as f:
    if OUT == 'units_atlas':
        f.write('; units_atlas.spr: units cut from the unit atlases (docs/ART.md), square-tile frames.\n')
    else:
        f.write(f'; {OUT}.spr: units composed from atlas parts and painted shapes (docs/ART.md 10.2).\n')
    f.write('; 32x32 with feet at 15,29 (the whole tile); giants 40x40 with feet at 19,37.\n')
    f.write('; Generated by the atlas pipeline -- owner cloth is #/% (field/shade).\n\n')
    for (name, F, ax, ay, rows, fam, src) in sprites:
        f.write(f'= {name} {F}x{F} {ax},{ay}\n')
        f.write(f': note from {src}' + (f', owner cloth from its {fam} cloth' if fam else '') + '\n')
        f.write('\n'.join(rows) + '\n\n')

# preview
font = ImageFont.load_default()
SC = 4
cw, ch = 40 * SC + 8, 40 * SC + 20
cols = 8
n = len(sprites)
rowsn = (n + cols - 1) // cols
sheet = Image.new('RGB', (cols * cw, rowsn * ch), (40, 40, 46))
d = ImageDraw.Draw(sheet)
grass = (111, 165, 52)
FILL = {'#': rgb_of('#c8272b'), '%': rgb_of('#8f1e1f')}
FILL2 = {'#': rgb_of('#3356b6'), '%': rgb_of('#0b4290')}
for i, (name, F, ax, ay, rows, fam, src) in enumerate(sprites):
    cx, cy = (i % cols) * cw, (i // cols) * ch
    img = Image.new('RGB', (40, 40), grass)
    px = img.load()
    fill = FILL if i % 2 == 0 else FILL2
    off = (40 - F) // 2
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c == '.':
                continue
            rgb = fill[c] if c in fill else PRGB[KI[c]]
            px[x + off, y + off] = tuple(int(v) for v in rgb)
    sheet.paste(img.resize((40 * SC, 40 * SC), Image.NEAREST), (cx + 4, cy + 2))
    d.text((cx + 4, cy + 40 * SC + 5), name.replace('unit/', '')[:26], fill=(230, 230, 230), font=font)
sheet.save(f'{R}/{OUT}_preview.png' if OUT != 'units_atlas' else f'{R}/units_preview.png')
print('sprites', len(sprites))
print(' '.join(f"{n.replace('unit/','')}:{fam}" for (n,_,_,_,_,fam,_) in sprites))
