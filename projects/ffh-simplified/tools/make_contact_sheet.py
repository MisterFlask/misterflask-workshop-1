#!/usr/bin/env python3
"""Render an upscaled, labeled contact sheet of generated sprite PNGs
for visual review.

Usage:
    python3 tools/make_contact_sheet.py [subdir] [--out PATH]

subdir limits the sheet to assets/sprites/<subdir> (e.g. soldiers).
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw

SCALE = 6
LABEL_H = 14
COLS = 8
CHECKER = (58, 58, 70), (74, 74, 88)


def main():
    argv = sys.argv[1:]
    out = None
    if '--out' in argv:
        i = argv.index('--out')
        out = Path(argv[i + 1])
        argv = argv[:i] + argv[i + 2:]
    args = [a for a in argv if not a.startswith('--')]

    root = Path(__file__).parent.parent / 'assets' / 'sprites'
    if args:
        root = root / args[0]
    files = sorted(root.rglob('*.png'))
    if not files:
        print(f"no PNGs under {root}")
        sys.exit(1)

    cell_w = 32 * SCALE + 8
    cell_h = 32 * SCALE + 8 + LABEL_H
    cols = min(COLS, len(files))
    rows = (len(files) + cols - 1) // cols
    sheet = Image.new('RGB', (cols * cell_w, rows * cell_h), (30, 30, 38))
    draw = ImageDraw.Draw(sheet)

    for i, f in enumerate(files):
        cx = (i % cols) * cell_w + 4
        cy = (i // cols) * cell_h + 4
        # checkerboard behind transparency
        for ty in range(8):
            for tx in range(8):
                color = CHECKER[(tx + ty) % 2]
                draw.rectangle([cx + tx * 24, cy + ty * 24,
                                cx + tx * 24 + 23, cy + ty * 24 + 23], fill=color)
        img = Image.open(f).convert('RGBA')
        img = img.resize((img.width * SCALE, img.height * SCALE), Image.NEAREST)
        sheet.paste(img, (cx, cy), img)
        draw.text((cx, cy + 32 * SCALE + 2), f.stem, fill=(220, 220, 220))

    out = out or Path('/tmp') / 'sprite_contact_sheet.png'
    out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out)
    print(f"contact sheet ({len(files)} sprites) -> {out}")


if __name__ == '__main__':
    main()
