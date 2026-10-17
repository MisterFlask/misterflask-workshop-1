"""Downsample the unit crops to sprite resolution (RGBA, unquantized) -> small/<atlas>_<k>.png.

One uniform scale per atlas (so relative unit sizes survive): a typical infantryman stands ~27 px.
Area averaging in linear light with alpha-weighted colour; coverage >= 0.5 is opaque."""
import glob, json, os, sys
import numpy as np
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from colorlib import srgb_to_linear, linear_to_srgb

R = sys.argv[1]
TARGET_INF = float(sys.argv[2]) if len(sys.argv) > 2 else 27.0
os.makedirs(f'{R}/small', exist_ok=True)


def box_resize(arr, w, h):
    return np.asarray(Image.fromarray(arr.astype(np.float32), mode='F').resize((w, h), Image.BOX))


scales = {}
for name in ('atlas1', 'atlas2', 'atlas3'):
    units = json.load(open(f'{R}/{name}_units.json'))
    hs = [u['h'] for u in units if u['h'] < 110 and u['w'] < 100]
    s = TARGET_INF / float(np.median(hs))
    scales[name] = s
    for u in units:
        a = np.asarray(Image.open(f"{R}/crops/{name}_{u['k']:02d}.png").convert('RGBA')).astype(np.float64)
        alpha = a[..., 3] / 255.0
        lin = srgb_to_linear(a[..., :3])
        w = max(1, int(round(u['w'] * s)))
        h = max(1, int(round(u['h'] * s)))
        A = box_resize(alpha, w, h)
        rgb = np.stack([box_resize(lin[..., c] * alpha, w, h) for c in range(3)], -1)
        rgb = rgb / np.maximum(A[..., None], 1e-6)
        out = np.zeros((h, w, 4), np.uint8)
        out[..., :3] = np.clip(np.round(linear_to_srgb(rgb)), 0, 255)
        out[..., 3] = (A >= 0.5) * 255
        Image.fromarray(out).save(f"{R}/small/{name}_{u['k']:02d}.png")
        u['sw'], u['sh'] = w, h
    json.dump(units, open(f'{R}/{name}_units.json', 'w'), indent=0)
json.dump(scales, open(f'{R}/scales.json', 'w'))
print('scales', {k: round(v, 3) for k, v in scales.items()})
