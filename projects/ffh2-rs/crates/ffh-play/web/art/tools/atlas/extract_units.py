"""Cut the units out of the atlas images: background = connected navy panel regions, then strip
each unit's hex base. Writes crops/<atlas>_<k>.png (RGBA, full res) + <atlas>_units.json +
<atlas>_sheet.png (numbered contact sheet)."""
import json, os, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from colorlib import rgb_to_lab
from cc import label, components

R = sys.argv[1]
os.makedirs(f'{R}/crops', exist_ok=True)
font = ImageFont.load_default()


def _shift_or(m, r):
    out = m.copy()
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            if dx or dy:
                out |= np.roll(np.roll(m, dy, 0), dx, 1)
    return out


def dilate(m, r):
    return _shift_or(m, r)


def erode(m, r):
    return ~_shift_or(~m, r)


def split_touching(fg, r=3, min_seed=400):
    """Separate blobs joined by narrow necks (touching hex bases): seeds = components of fg eroded
    by r, then labels grow back over fg (geodesic Voronoi), so thin parts stay with their body."""
    seeds = label(erode(fg, r))
    cnt = np.bincount(seeds.ravel())
    small = cnt < min_seed
    small[0] = True
    seeds[small[seeds]] = 0
    lab = seeds.copy()
    for _ in range(400):
        un = fg & (lab == 0)
        if not un.any():
            break
        nb = np.maximum.reduce([np.roll(lab, 1, 0), np.roll(lab, -1, 0), np.roll(lab, 1, 1), np.roll(lab, -1, 1)])
        grow = un & (nb > 0)
        if not grow.any():
            break
        lab[grow] = nb[grow]
    # fg pixels never reached (isolated bits): give them their own components
    rest = fg & (lab == 0)
    if rest.any():
        rl = label(rest)
        lab[rest] = rl[rest] + lab.max()
    return lab


def long_runs(m, n):
    """Pixels of m lying in a horizontal run of at least n."""
    out = np.zeros_like(m)
    for y in range(m.shape[0]):
        d = np.diff(np.concatenate([[0], m[y].astype(np.int8), [0]]))
        for s, e in zip(np.nonzero(d == 1)[0], np.nonzero(d == -1)[0]):
            if e - s >= n:
                out[y, s:e] = True
    return out


def process(name):
    im = Image.open(f'{R}/ref/{name}.png').convert('RGB')
    a = np.asarray(im)
    H, W, _ = a.shape
    lab = rgb_to_lab(a.astype(np.float64))
    L, A, B = lab[..., 0], lab[..., 1], lab[..., 2]
    C = np.hypot(A, B)
    # the panel: a dark, low-chroma navy (measured: L 4-18, a 0-5, b ~-21, C ~20-24). Blue cloth
    # is more saturated, purple cloaks redder, steel lighter, outlines neutral -> all excluded.
    panel = (L < 26) & (A > -7) & (A < 7.5) & (B < -6) & (C < 31)
    # opening: erode, keep the big/edge regions, grow back a little inside the class, so the flood
    # cannot leak through 1-4 px gaps in a unit's outline.
    core = erode(panel, 2)
    nl = label(core)
    comp = components(nl)
    border_ids = set(np.unique(np.concatenate([nl[0], nl[-1], nl[:, 0], nl[:, -1]]))) - {0}
    keep_ids = [i for i, (n, *_r) in comp.items() if n > 4000 or i in border_ids]
    bg = np.isin(nl, keep_ids)
    bg = dilate(bg, 3) & panel
    fg = ~bg
    # cut the frame network: section rules and the lighter panel glows run far longer than any
    # unit or base (< ~170 px), so drop every horizontal or vertical fg run of 200+ px.
    fg &= ~dilate(long_runs(fg, 200) | long_runs(fg.T, 200).T, 1)
    fl = split_touching(fg)
    fcomp = components(fl)
    grass = (L > 30) & (L < 74) & (A > -38) & (A < -3) & (B > 16) & (B < 64)
    water = (L > 30) & (L < 78) & (B < -20) & (A < 16)
    units = []
    for i, (n, x0, y0, x1, y1) in fcomp.items():
        w, h = x1 - x0 + 1, y1 - y0 + 1
        if n < 1400 or w < 34 or h < 44 or w > 260 or h > 230:
            continue
        sub = fl[y0:y1 + 1, x0:x1 + 1] == i
        g = (grass[y0:y1 + 1, x0:x1 + 1] | water[y0:y1 + 1, x0:x1 + 1]) & sub
        rows = g.sum(1)
        # base must exist in the lower half
        half = h // 2
        cand = [y for y in range(half, h) if rows[y] >= 0.30 * w]
        if not cand:
            continue
        ybt = cand[0]
        lower = [y for y in range(ybt, h) if rows[y] >= 0.22 * w]
        yside = lower[-1] if lower else ybt
        # the unit stands on the middle of the hex's top surface: nothing below that is the unit
        yfeet = ybt + int(0.5 * (yside - ybt)) + 2
        keep = sub.copy()
        keep[min(yside, yfeet) + 1:, :] = False
        band = np.zeros_like(keep)
        band[ybt:yside + 1, :] = True
        rgb = a[y0:y1 + 1, x0:x1 + 1].astype(np.int32)
        sl = lab[y0:y1 + 1, x0:x1 + 1]
        # in the top-surface band drop grass/water and the hex rim (olive/brown edge) colours
        rim = (sl[..., 0] < 55) & (sl[..., 2] > 8) & (sl[..., 1] > -25) & (sl[..., 1] < 18)
        keep &= ~(band & (g | (rim & (np.arange(h)[:, None] > ybt + 0.55 * (yside - ybt)))))
        # largest component + sizeable others that sit above the base top
        kl = label(keep)
        kc = components(kl)
        if not kc:
            continue
        big = max(kc.items(), key=lambda t: t[1][0])
        mask = kl == big[0]
        for j, (cn, cx0, cy0, cx1, cy1) in kc.items():
            if j != big[0] and cn > 25 and cy1 <= ybt + 4:
                mask |= kl == j
        ys, xs = np.nonzero(mask)
        ux0, ux1, uy0, uy1 = xs.min(), xs.max(), ys.min(), ys.max()
        crop = np.zeros((uy1 - uy0 + 1, ux1 - ux0 + 1, 4), np.uint8)
        crop[..., :3] = rgb[uy0:uy1 + 1, ux0:ux1 + 1]
        crop[..., 3] = mask[uy0:uy1 + 1, ux0:ux1 + 1] * 255
        k = len(units)
        Image.fromarray(crop).save(f'{R}/crops/{name}_{k:02d}.png')
        units.append(dict(k=k, x0=int(x0 + ux0), y0=int(y0 + uy0), w=int(ux1 - ux0 + 1), h=int(uy1 - uy0 + 1),
                          base_top=int(y0 + ybt), blob=[int(x0), int(y0), int(x1), int(y1)]))
    # order by row then x
    units.sort(key=lambda u: (round(u['base_top'] / 40), u['x0']))
    for nk, u in enumerate(units):
        os.rename(f"{R}/crops/{name}_{u['k']:02d}.png", f"{R}/crops/{name}_tmp{nk:02d}.png")
        u['k'] = nk
    for nk in range(len(units)):
        os.rename(f"{R}/crops/{name}_tmp{nk:02d}.png", f"{R}/crops/{name}_{nk:02d}.png")
    json.dump(units, open(f'{R}/{name}_units.json', 'w'), indent=0)
    # contact sheet: each crop on a mid-grey cell, index + atlas position
    cw, ch = 120, 132
    cols = 12
    rowsn = (len(units) + cols - 1) // cols
    sheet = Image.new('RGB', (cols * cw, rowsn * ch), (120, 120, 126))
    d = ImageDraw.Draw(sheet)
    for u in units:
        c = Image.open(f"{R}/crops/{name}_{u['k']:02d}.png")
        sc = min(1.0, 104 / c.width, 112 / c.height)
        c2 = c.resize((max(1, int(c.width * sc)), max(1, int(c.height * sc))), Image.NEAREST)
        cx = (u['k'] % cols) * cw
        cy = (u['k'] // cols) * ch
        sheet.paste(c2, (cx + (cw - c2.width) // 2, cy + 116 - c2.height), c2)
        d.text((cx + 3, cy + 2), f"{u['k']}", fill=(255, 255, 0), font=font)
        d.text((cx + 3, cy + 119), f"{u['w']}x{u['h']}", fill=(20, 20, 20), font=font)
    sheet.save(f'{R}/{name}_sheet.png')
    print(name, 'units:', len(units))


for nm in sys.argv[2:]:
    process(nm)
