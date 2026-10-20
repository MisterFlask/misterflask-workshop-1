"""Isometric scene compositor for the M7 art mode (reference implementation of the placement contract).

Renders a layout (terrain grid + features + buildings + units) from asset PNGs
that follow spec.md. Assets are looked up by exact filename in the given asset
dirs (priority order), then in work/placeholders/ (generated on demand).

  python lib/compose.py --assets roster --layout layouts/layout_ref.json --out scene.png
  python lib/compose.py --assets work/unit_lich roster --override unit_lich=x.png ...
Layouts may add "terrain_codes": {"D": "tile_desert"}, "feature_codes": {"j": "feat_jungle"}
and "water_keys": ["tile_water", "tile_ocean"] (tiles drawn 8 px lower, showing banks).
"""
import argparse, json, math, os, sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).resolve().parent
PLACEHOLDERS = HERE.parent / "work" / "placeholders"  # generated on demand, never shipped
TW, TH, TCANVAS_H = 192, 96, 128
HALF_W, HALF_H = TW // 2, TH // 2

TERRAIN_CODES = {"G": "tile_grass", "F": "tile_farm", "W": "tile_water", "H": "tile_hills"}
FEATURE_CODES = {"f": "feat_forest", "m": "feat_mountain"}
ALL_KEYS = ["tile_grass", "tile_farm", "tile_water", "tile_hills", "feat_forest", "feat_mountain",
            "bld_city", "bld_temple_leaves", "unit_swordsman", "unit_worker",
            "unit_priest_of_leaves", "unit_hyborem"]
BANNERS = {"blue": ((32, 78, 196), (14, 34, 104)), "red": ((176, 28, 28), (82, 10, 14)),
           "green": ((48, 140, 56), (18, 66, 26)), "purple": ((112, 52, 168), (52, 20, 84)),
           "black": ((40, 40, 48), (12, 12, 16)), "white": ((228, 228, 236), (120, 120, 132)),
           "yellow": ((226, 190, 40), (120, 90, 10)), "orange": ((220, 110, 30), (110, 50, 10))}


def font(size):
    try:
        return ImageFont.load_default(size=size)
    except TypeError:
        return ImageFont.load_default()


# ---------------------------------------------------------------- placeholders
def diamond_poly(ox=0, oy=0):
    return [(ox + 96, oy), (ox + 192, oy + 48), (ox + 96, oy + 96), (ox, oy + 48)]


def skirt_poly(ox=0, oy=0, depth=24):
    return [(ox, oy + 48), (ox + 96, oy + 96), (ox + 192, oy + 48),
            (ox + 192, oy + 48 + depth), (ox + 96, oy + 96 + depth), (ox, oy + 48 + depth)]


def make_placeholders():
    PLACEHOLDERS.mkdir(parents=True, exist_ok=True)
    f = font(14)

    def mark(d, x, y, text):
        d.text((x, y), text, fill=(255, 0, 255, 255), font=f, anchor="mm")

    tiles = {"tile_grass": (141, 179, 58), "tile_farm": (217, 180, 74),
             "tile_water": (31, 111, 209), "tile_hills": (120, 150, 70)}
    for k, col in tiles.items():
        im = Image.new("RGBA", (TW, TCANVAS_H), (0, 0, 0, 0))
        d = ImageDraw.Draw(im)
        d.polygon(skirt_poly(), fill=(107, 74, 43, 255))
        d.polygon(diamond_poly(), fill=col + (255,), outline=(60, 60, 60, 255))
        mark(d, 96, 48, "PH " + k[5:])
        im.save(PLACEHOLDERS / f"{k}.png")
    # features
    im = Image.new("RGBA", (192, 160), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    for (x, y) in [(60, 120), (96, 100), (132, 120), (78, 140), (114, 140), (96, 150)]:
        d.polygon([(x, y - 50), (x - 16, y), (x + 16, y)], fill=(30, 90, 40, 255), outline=(10, 40, 15, 255))
    mark(d, 96, 20, "PH forest"); im.save(PLACEHOLDERS / "feat_forest.png")
    im = Image.new("RGBA", (192, 190), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    d.polygon([(96, 20), (20, 160), (172, 160)], fill=(130, 130, 140, 255), outline=(50, 50, 60, 255))
    d.polygon([(96, 20), (74, 60), (118, 60)], fill=(240, 240, 250, 255))
    mark(d, 96, 175, "PH mountain"); im.save(PLACEHOLDERS / "feat_mountain.png")
    # buildings
    im = Image.new("RGBA", (384, 300), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    d.polygon([(192, 110), (360, 194), (192, 290), (24, 194)], fill=(150, 150, 155, 255), outline=(60, 60, 70, 255))
    for (x, y) in [(192, 90), (60, 180), (324, 180), (192, 270)]:
        d.rectangle([x - 14, y - 40, x + 14, y + 10], fill=(200, 200, 205, 255), outline=(60, 60, 70, 255))
        d.polygon([(x, y - 70), (x - 18, y - 40), (x + 18, y - 40)], fill=(30, 80, 200, 255))
    mark(d, 192, 200, "PH city"); im.save(PLACEHOLDERS / "bld_city.png")
    im = Image.new("RGBA", (192, 200), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    d.rectangle([84, 90, 108, 180], fill=(110, 70, 40, 255)); d.ellipse([40, 20, 152, 120], fill=(50, 140, 60, 255))
    mark(d, 96, 190, "PH temple"); im.save(PLACEHOLDERS / "bld_temple_leaves.png")
    for k in ["unit_swordsman", "unit_worker", "unit_priest_of_leaves", "unit_hyborem"]:
        im = Image.new("RGBA", (96, 120), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
        d.rounded_rectangle([34, 40, 62, 116], 12, fill=(160, 160, 160, 255), outline=(60, 60, 60, 255))
        d.ellipse([36, 18, 60, 42], fill=(200, 200, 200, 255), outline=(60, 60, 60, 255))
        mark(d, 48, 80, k[5:9]); im.save(PLACEHOLDERS / f"{k}.png")


# ---------------------------------------------------------------- asset lookup
class Assets:
    def __init__(self, dirs, overrides):
        self.dirs = [Path(d) for d in dirs]
        self.overrides = overrides
        self.cache, self.source = {}, {}

    def find(self, key):
        if not (PLACEHOLDERS / "unit_swordsman.png").exists():
            make_placeholders()
        if key in self.overrides:
            return Path(self.overrides[key])
        name = f"{key}.png"
        for d in self.dirs:
            if not d.exists():
                continue
            hits = [p for p in d.rglob(name) if "assets" in p.parts]
            hits = hits or list(d.rglob(name))
            if hits:
                hits.sort(key=lambda p: (len(p.parts), str(p)))
                return hits[0]
        if (PLACEHOLDERS / name).exists():
            return PLACEHOLDERS / name
        generic = {"unit": "unit_swordsman", "bld": "bld_temple_leaves", "feat": "feat_forest", "tile": "tile_grass"}
        return PLACEHOLDERS / f"{generic.get(key.split('_')[0], 'unit_swordsman')}.png"

    def get(self, key):
        if key not in self.cache:
            p = self.find(key)
            self.source[key] = str(p)
            self.cache[key] = Image.open(p).convert("RGBA")
        return self.cache[key]


# ---------------------------------------------------------------- drawing helpers
def draw_banner(scene, x, y, color="blue"):
    """Banner pole whose base is at (x, y)."""
    fg, dk = BANNERS.get(color, BANNERS["blue"])
    lay = Image.new("RGBA", (40, 96), (0, 0, 0, 0)); d = ImageDraw.Draw(lay)
    d.rectangle([3, 4, 6, 95], fill=(92, 58, 30, 255)); d.rectangle([3, 4, 3, 95], fill=(140, 96, 52, 255))
    d.ellipse([1, 0, 8, 7], fill=(236, 196, 72, 255), outline=(120, 80, 20, 255))
    flag = [(7, 8), (36, 8), (36, 40), (21, 48), (7, 40)]
    d.polygon(flag, fill=fg + (255,), outline=dk + (255,))
    d.line([(9, 10), (34, 10)], fill=(236, 196, 72, 255), width=2)
    # simple heraldic charge (rampant-ish lion silhouette as a chunky glyph)
    gold = (240, 200, 70, 255)
    for (px, py, w, h) in [(17, 18, 8, 3), (15, 21, 10, 6), (14, 27, 4, 6), (22, 27, 4, 6), (24, 15, 4, 4)]:
        d.rectangle([px, py, px + w - 1, py + h - 1], fill=gold)
    scene.alpha_composite(lay, (int(x - 4), int(y - 95)))


def draw_highlight(scene, tx, ty):
    lay = Image.new("RGBA", (TW, TH), (0, 0, 0, 0)); d = ImageDraw.Draw(lay)
    inset = [(96, 8), (180, 48), (96, 88), (12, 48)]
    d.polygon(inset, fill=(250, 230, 90, 60))
    d.line(inset + [inset[0]], fill=(255, 236, 110, 230), width=3)
    lay = lay.filter(ImageFilter.GaussianBlur(0.6))
    scene.alpha_composite(lay, (int(tx), int(ty)))


def draw_shadow(scene, cx, cy, w=46, h=14, a=95):
    lay = Image.new("RGBA", (w + 8, h + 8), (0, 0, 0, 0)); d = ImageDraw.Draw(lay)
    d.ellipse([4, 4, 4 + w, 4 + h], fill=(10, 20, 10, a))
    lay = lay.filter(ImageFilter.GaussianBlur(2.5))
    scene.alpha_composite(lay, (int(cx - w / 2 - 4), int(cy - h / 2 - 4)))


def background(w, h, seed=7):
    """Deep ocean with soft swell lines + cloud puffs drawn later."""
    bg = Image.new("RGBA", (w, h), (12, 52, 128, 255))
    d = ImageDraw.Draw(bg)
    for i in range(0, h, 6):
        k = (i * 37 + seed * 11) % 23
        for j in range(-40, w, 46):
            x = j + (k * 7 + i * 3) % 46
            d.line([(x, i), (x + 10, i)], fill=(30, 86, 170, 255), width=2)
    return bg


def clouds(scene, seed=3):
    w, h = scene.size
    shade = Image.new("RGBA", (w, h), (0, 0, 0, 0)); ds = ImageDraw.Draw(shade)
    lite = Image.new("RGBA", (w, h), (0, 0, 0, 0)); dl = ImageDraw.Draw(lite)
    spots = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 1.02), (-0.02, 0.5), (1.02, 0.55)]
    for i, (fx, fy) in enumerate(spots):
        cx, cy = fx * w, fy * h
        for j in range(9):
            a = (i * 9 + j) * 2.39996
            r = 60 + ((i * 31 + j * 17) % 50)
            ox, oy = math.cos(a) * (40 + j * 14), math.sin(a) * (24 + j * 8)
            ds.ellipse([cx + ox - r, cy + oy - r * 0.62, cx + ox + r, cy + oy + r * 0.72], fill=(190, 208, 232, 255))
            dl.ellipse([cx + ox - r * 0.9, cy + oy - r * 0.7, cx + ox + r * 0.88, cy + oy + r * 0.45], fill=(252, 253, 255, 255))
    scene.alpha_composite(shade.filter(ImageFilter.GaussianBlur(1.0)))
    scene.alpha_composite(lite.filter(ImageFilter.GaussianBlur(1.0)))


# ---------------------------------------------------------------- render
def render(layout, assets, margin_top=240, margin=40, decorate=True):
    tcodes = dict(TERRAIN_CODES, **layout.get("terrain_codes", {}))
    fcodes = dict(FEATURE_CODES, **layout.get("feature_codes", {}))
    rows = len(layout["terrain"]); cols = len(layout["terrain"][0])
    W = (rows + cols) * HALF_W + 2 * margin
    H = (rows + cols) * HALF_H + TCANVAS_H + margin_top + margin
    X0, Y0 = margin + rows * HALF_W - HALF_W, margin_top
    scene = background(W, H) if decorate else Image.new("RGBA", (W, H), (40, 40, 48, 255))
    water_drop = layout.get("water_drop", 8)

    def tile_xy(r, c):
        return X0 + (c - r) * HALF_W, Y0 + (c + r) * HALF_H

    # 1) terrain, back to front
    order = sorted(((r, c) for r in range(rows) for c in range(cols)), key=lambda t: (t[0] + t[1], t[0]))
    for r, c in order:
        key = tcodes[layout["terrain"][r][c]]
        im = assets.get(key)
        x, y = tile_xy(r, c)
        dy = water_drop if key in layout.get("water_keys", ["tile_water"]) else 0
        scene.alpha_composite(im, (int(x + (TW - im.width) // 2), int(y + dy)))

    # 2) ground decals: highlights
    for u in layout.get("units", []):
        if u.get("highlight"):
            x, y = tile_xy(*u["rc"]); draw_highlight(scene, x, y)

    # 3) objects sorted by depth of their front-most tile
    objs = []
    feats = layout.get("features", [])
    for r, row in enumerate(feats):
        for c, ch in enumerate(row):
            if ch in fcodes:
                objs.append(((r + c, 0, r), "feature", fcodes[ch], (r, c), {}))
    for b in layout.get("buildings", []):
        r, c = b["rc"]; fp = b.get("footprint", 1)
        fr, fc = r + fp - 1, c + fp - 1
        objs.append(((fr + fc, 1, fr), "building", "bld_" + b["type"], (fr, fc), b))
    for u in layout.get("units", []):
        r, c = u["rc"]
        objs.append(((r + c, 2, r), "unit", "unit_" + u["type"], (r, c), u))
    objs.sort(key=lambda o: o[0])
    for _, kind, key, (r, c), meta in objs:
        im = assets.get(key)
        x, y = tile_xy(r, c)
        if kind in ("feature", "building"):
            ax, ay = x + HALF_W, y + TH  # bottom vertex of front-most tile
            scene.alpha_composite(im, (int(ax - im.width / 2), int(ay - im.height)))
        else:
            ax, ay = x + HALF_W + meta.get("dx", 0), y + HALF_H + meta.get("dy", 6)
            if meta.get("banner", "blue"):  # banner stands just left of the sprite's opaque body (big units push it out)
                bb = im.getchannel("A").getbbox() or (0, 0, im.width, im.height)
                left = bb[0] - im.width / 2
                draw_banner(scene, ax + min(-34, left - 10), ay - 6, meta.get("banner", "blue"))
            draw_shadow(scene, ax, ay - 2, w=int(min(70, max(40, im.width * 0.5))))
            scene.alpha_composite(im, (int(ax - im.width / 2), int(ay - im.height)))
    if decorate and layout.get("clouds", True):
        clouds(scene)
    return scene


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--assets", nargs="*", default=[], help="asset dirs, priority order")
    ap.add_argument("--layout", default=str(HERE.parent / "layouts" / "layout_ref.json"))
    ap.add_argument("--out", required=True)
    ap.add_argument("--override", nargs="*", default=[], help="key=path")
    ap.add_argument("--no-decor", action="store_true")
    ap.add_argument("--scale", type=float, default=1.0, help="optional final resize (nearest)")
    a = ap.parse_args(argv)
    if not PLACEHOLDERS.exists() or len(list(PLACEHOLDERS.glob("*.png"))) < len(ALL_KEYS):
        make_placeholders()
    ov = dict(kv.split("=", 1) for kv in a.override)
    assets = Assets(a.assets, ov)
    layout = json.load(open(a.layout))
    scene = render(layout, assets, decorate=not a.no_decor)
    if a.scale != 1.0:
        scene = scene.resize((int(scene.width * a.scale), int(scene.height * a.scale)), Image.NEAREST)
    Path(a.out).parent.mkdir(parents=True, exist_ok=True)
    scene.convert("RGB").save(a.out)
    missing = [k for k, s in assets.source.items() if str(PLACEHOLDERS) in s]
    print(json.dumps({"out": a.out, "size": scene.size, "placeholders_used": missing,
                      "sources": assets.source}, indent=1))


if __name__ == "__main__":
    main()
