#!/usr/bin/env python3
"""Compose all programmatically-defined .sprite files.

Each module in tools/compose/ exposes SPRITES: dict of
    sprite_name -> () -> (Canvas, palette dict)
Definitions are written to assets/sprite_definitions/<subdir>/.
Run tools/generate_sprites.py afterwards to produce the PNGs.

The four original hand-authored soldier sprites (fighter, archer,
cleric, mage) are not composed here and must not be overwritten.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from sprite_lib import write_sprite  # noqa: E402
from compose import soldiers, buildings, features  # noqa: E402

HAND_AUTHORED = {'fighter', 'archer', 'cleric', 'mage'}


def main():
    only = sys.argv[1] if len(sys.argv) > 1 else None
    defs = Path(__file__).parent.parent / 'assets' / 'sprite_definitions'
    total = 0
    for module, subdir in ((soldiers, 'soldiers'), (buildings, 'buildings'),
                           (features, 'features')):
        if only and subdir != only:
            continue
        for name, build in module.SPRITES.items():
            if subdir == 'soldiers' and name in HAND_AUTHORED:
                raise SystemExit(f"refusing to overwrite hand-authored {name}")
            canvas, palette = build()
            write_sprite(defs / subdir / f'{name}.sprite', name, canvas, palette)
            total += 1
    print(f"composed {total} sprite definitions")


if __name__ == '__main__':
    main()
