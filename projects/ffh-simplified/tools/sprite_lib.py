"""Shared drawing library for composing .sprite pixel-art definitions.

A Canvas is a grid of palette characters ('.' = transparent). Compose
scripts (tools/compose/*.py) build canvases with these helpers and emit
.sprite files that tools/generate_sprites.py turns into PNGs.

Style conventions (match the hand-authored fighter/archer/cleric/mage):
- 32x32, subject roughly centered, occupying rows ~4..22
- 'x' (#000000) outline around every filled region
- small flat-color palettes, one accent color per sprite
"""

from pathlib import Path

TRANSPARENT = '.'
OUTLINE = 'x'

BASE_PALETTE = {
    TRANSPARENT: 'transparent',
    OUTLINE: '#000000',
}


class Canvas:
    def __init__(self, width: int = 32, height: int = 32):
        self.width = width
        self.height = height
        self.grid = [[TRANSPARENT] * width for _ in range(height)]

    # -- primitives -------------------------------------------------------

    def px(self, x: int, y: int, c: str) -> None:
        if 0 <= x < self.width and 0 <= y < self.height:
            self.grid[y][x] = c

    def get(self, x: int, y: int) -> str:
        if 0 <= x < self.width and 0 <= y < self.height:
            return self.grid[y][x]
        return TRANSPARENT

    def rect(self, x: int, y: int, w: int, h: int, c: str) -> None:
        for yy in range(y, y + h):
            for xx in range(x, x + w):
                self.px(xx, yy, c)

    def box(self, x: int, y: int, w: int, h: int, c: str) -> None:
        self.hline(x, x + w - 1, y, c)
        self.hline(x, x + w - 1, y + h - 1, c)
        self.vline(x, y, y + h - 1, c)
        self.vline(x + w - 1, y, y + h - 1, c)

    def hline(self, x0: int, x1: int, y: int, c: str) -> None:
        for x in range(min(x0, x1), max(x0, x1) + 1):
            self.px(x, y, c)

    def vline(self, x: int, y0: int, y1: int, c: str) -> None:
        for y in range(min(y0, y1), max(y0, y1) + 1):
            self.px(x, y, c)

    def line(self, x0: int, y0: int, x1: int, y1: int, c: str) -> None:
        dx = abs(x1 - x0)
        dy = -abs(y1 - y0)
        sx = 1 if x0 < x1 else -1
        sy = 1 if y0 < y1 else -1
        err = dx + dy
        while True:
            self.px(x0, y0, c)
            if x0 == x1 and y0 == y1:
                break
            e2 = 2 * err
            if e2 >= dy:
                err += dy
                x0 += sx
            if e2 <= dx:
                err += dx
                y0 += sy

    def disc(self, cx: int, cy: int, r: int, c: str) -> None:
        for y in range(cy - r, cy + r + 1):
            for x in range(cx - r, cx + r + 1):
                if (x - cx) ** 2 + (y - cy) ** 2 <= r * r + r // 2:
                    self.px(x, y, c)

    def ring(self, cx: int, cy: int, r: int, c: str) -> None:
        for y in range(cy - r - 1, cy + r + 2):
            for x in range(cx - r - 1, cx + r + 2):
                d = (x - cx) ** 2 + (y - cy) ** 2
                if r * r - r <= d <= r * r + r:
                    self.px(x, y, c)

    def triangle(self, apex_x: int, apex_y: int, base_y: int, half_width: int, c: str) -> None:
        """Filled isoceles triangle, apex up (or down if base_y < apex_y)."""
        height = abs(base_y - apex_y)
        if height == 0:
            self.hline(apex_x - half_width, apex_x + half_width, apex_y, c)
            return
        step = 1 if base_y > apex_y else -1
        for i in range(height + 1):
            y = apex_y + i * step
            hw = round(half_width * i / height)
            self.hline(apex_x - hw, apex_x + hw, y, c)

    def paste(self, rows: list, x: int, y: int) -> None:
        """Paste ASCII patch rows at (x, y); '.' in the patch is skipped."""
        for dy, row in enumerate(rows):
            for dx, ch in enumerate(row):
                if ch != TRANSPARENT:
                    self.px(x + dx, y + dy, ch)

    def mirror_left_to_right(self, axis: int = None) -> None:
        """Mirror the left half onto the right half around vertical axis."""
        axis = self.width // 2 if axis is None else axis
        for y in range(self.height):
            for x in range(axis):
                mx = 2 * axis - 1 - x
                if 0 <= mx < self.width and self.grid[y][x] != TRANSPARENT:
                    self.grid[y][mx] = self.grid[y][x]

    def outline(self, c: str = OUTLINE) -> None:
        """Draw c around every filled pixel (4-neighborhood halo)."""
        halo = set()
        for y in range(self.height):
            for x in range(self.width):
                if self.grid[y][x] not in (TRANSPARENT, c):
                    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                        nx, ny = x + dx, y + dy
                        if self.get(nx, ny) == TRANSPARENT:
                            halo.add((nx, ny))
        for x, y in halo:
            self.px(x, y, c)

    def rows(self) -> list:
        return [''.join(r) for r in self.grid]


# -- shared figure templates ----------------------------------------------

def humanoid(c: Canvas, skin: str, torso: str, legs: str = None,
             head_y: int = 4, cx: int = 15) -> None:
    """Standard soldier silhouette matching the hand-authored fighter:
    round head (rows head_y..head_y+4), broad torso, two legs. Caller adds
    gear/weapons then calls c.outline()."""
    legs = legs or torso
    # head
    c.disc(cx, head_y + 2, 2, skin)
    c.px(cx - 2, head_y + 1, skin)
    c.px(cx + 2, head_y + 1, skin)
    # torso (rows head_y+5 .. head_y+11)
    ty = head_y + 5
    c.rect(cx - 2, ty, 5, 2, torso)
    c.rect(cx - 3, ty + 1, 7, 4, torso)
    c.rect(cx - 2, ty + 5, 5, 2, torso)
    # legs (rows ty+7 .. ty+12)
    ly = ty + 7
    c.rect(cx - 3, ly, 2, 4, legs)
    c.rect(cx + 2, ly, 2, 4, legs)
    c.rect(cx - 4, ly + 3, 3, 2, legs)
    c.rect(cx + 2, ly + 3, 3, 2, legs)


def shadow(c: Canvas, cx: int = 15, y: int = 27, half_width: int = 8,
           char: str = 'z') -> None:
    """Soft ground shadow ellipse (palette should map char to #00000060)."""
    for dx in range(-half_width, half_width + 1):
        c.px(cx + dx, y, char)
        if abs(dx) <= half_width - 3:
            c.px(cx + dx, y + 1, char)


# -- emitter ----------------------------------------------------------------

def write_sprite(path, name: str, canvas: Canvas, palette: dict) -> None:
    """Write a .sprite file. Validates dimensions and palette coverage."""
    full_palette = dict(BASE_PALETTE)
    full_palette.update(palette)

    rows = canvas.rows()
    assert len(rows) == canvas.height, f"{name}: row count {len(rows)}"
    used = set()
    for i, row in enumerate(rows):
        assert len(row) == canvas.width, \
            f"{name}: row {i} has {len(row)} chars, expected {canvas.width}"
        used.update(row)
    missing = used - set(full_palette)
    assert not missing, f"{name}: chars {missing} not in palette"

    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [f"name: {name}", f"size: {canvas.width}x{canvas.height}", "palette:"]
    for ch, color in full_palette.items():
        if ch in used or ch in (TRANSPARENT, OUTLINE):
            lines.append(f"  {ch}: {color}")
    lines.append("pixels: |")
    for row in rows:
        lines.append(f"  {row}")
    path.write_text('\n'.join(lines) + '\n')
    print(f"wrote {path}")
