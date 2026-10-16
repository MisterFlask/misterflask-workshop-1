"""Cut figures the automatic extraction missed (they touched their card frames) out of an atlas
sheet by hand-given boxes: python3 cut_missed.py WORK. Writes WORK/crops/<atlas>_<name>.png."""
import os, sys
import numpy as np
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from colorlib import rgb_to_lab
from composelib import opening

WORK = sys.argv[1]
BOXES = {
    # atlas3, beasts row: war dog, dire wolf, sabretooth, elephant
    'atlas3_wardog': ('atlas3', (28, 958, 108, 1022)),
    'atlas3_direwolf': ('atlas3', (116, 944, 212, 1022)),
    'atlas3_sabretooth': ('atlas3', (328, 952, 422, 1022)),
    'atlas3_elephant': ('atlas3', (556, 928, 662, 1022)),
}


def components(m):
    from collections import deque
    H, W = m.shape
    lab = np.zeros((H, W), np.int32)
    n = 0
    sizes = [0]
    for y in range(H):
        for x in range(W):
            if m[y, x] and not lab[y, x]:
                n += 1
                q = deque([(y, x)]); lab[y, x] = n; c = 0
                while q:
                    cy, cx = q.popleft(); c += 1
                    for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                        yy, xx = cy + dy, cx + dx
                        if 0 <= yy < H and 0 <= xx < W and m[yy, xx] and not lab[yy, xx]:
                            lab[yy, xx] = n; q.append((yy, xx))
                sizes.append(c)
    return lab, sizes


for name, (sheet, box) in BOXES.items():
    im = np.asarray(Image.open(f'{WORK}/ref/{sheet}.png').convert('RGB').crop(box), np.float64)
    lab = rgb_to_lab(im)
    L, a, b = lab[..., 0], lab[..., 1], lab[..., 2]
    C = np.hypot(a, b)
    H = (np.degrees(np.arctan2(b, a)) + 360) % 360
    navy = (b < -4) & (L < 48) & (C > 6)
    grass = (H > 80) & (H < 160) & (C > 16)
    rim = (H > 55) & (H < 100) & (C > 22) & (L > 50)
    h = im.shape[0]
    base = np.zeros_like(navy)
    base[int(h * 0.55):] = True
    fg = ~navy & ~((grass | rim) & base)
    fg = opening(fg, 1)
    lab_, sizes = components(fg)
    keep = lab_ == int(np.argmax(sizes))
    ys, xs = np.nonzero(keep)
    out = np.zeros(im.shape[:2] + (4,), np.uint8)
    out[..., :3] = im.astype(np.uint8)
    out[..., 3] = keep * 255
    out = out[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
    Image.fromarray(out, 'RGBA').save(f'{WORK}/crops/{name}.png')
    print(name, out.shape[1], 'x', out.shape[0])
