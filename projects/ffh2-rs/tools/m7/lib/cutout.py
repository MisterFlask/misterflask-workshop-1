"""Turn a raw image (AI output, SVG render, big drawing) into a spec-conformant asset.

  python tools/cutout.py --in raw.png --out assets/unit_hyborem.png --kind unit --height 105
  python tools/cutout.py --in raw.png --out assets/feat_forest.png --kind feature --width 184
  python tools/cutout.py --in raw.png --out assets/bld_city.png --kind city
  python tools/cutout.py --in tex.png --out assets/tile_grass.png --kind tile --project --skirt earth
  python tools/cutout.py --in isotile.png --out assets/tile_water.png --kind tile --skirt water

Background removal: if the image has real alpha it is used; otherwise the background
colour (auto = median border colour, or --key '#ff00ff') is flood-removed from the border
with a soft edge (--tol). Then: despill, drop specks, trim, scale, place on the spec canvas
with the spec anchor. --pixel N gives chunky N-px pixels (box-downscale then nearest-upscale).
--crop x0,y0,x1,y1 crops the raw image first (e.g. one cell of a sprite sheet).
--outline adds a 1 px dark outline (darkened edge colour) around units, features and buildings.
--holes N also clears enclosed background-coloured pockets (inside a bow, between legs) of >= N px.
--key accepts a comma list ('#ff00ff,#ffffff'). TIP: models without real transparency
(gpt-image-2, chatgpt-image-latest) often paint a FAKE checkerboard; ask them for a flat
solid #FF00FF magenta background instead and use --key '#ff00ff' --holes 30.
kinds:
  unit     -> canvas >= 96x120, bottom-centre, figure height --height (default 88)
  feature  -> canvas 192 wide (or --width), bottom-centre, scaled to --width (default 184)
  building -> like feature (default width 184)
  city     -> canvas 384 wide, scaled to --width (default 372)
  tile     -> 192x128; content scaled to 192 wide and masked to diamond+skirt.
              --project treats input as a square top-down texture and projects it to the iso diamond.
              --skirt earth|water|none synthesises the side skirt.
"""
import argparse, json
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw, ImageFilter
from scipy import ndimage


def parse_hex(h):
    h = h.lstrip("#"); return np.array([int(h[i:i + 2], 16) for i in (0, 2, 4)], dtype=np.float32)


def remove_bg(im, key="auto", tol=48.0, holes=0, stats=None):
    """Flood-remove the background colour from the border with a soft edge.
    holes=N also clears ENCLOSED pockets of >= N px, but only pockets that really are background:
    at least 60% of their pixels must be within tol/2 of the key (or 20% for small pockets < 300 px,
    i.e. anti-aliased gaps between fence rails or fingers). Magenta-ish ARTWORK (orbs,
    portals, glows) is a gradient, not the flat key, so it survives (fix found by the M7 replication test;
    the shipped sprint assets are unaffected because their pockets were near-pure background)."""
    a = np.asarray(im.convert("RGBA")).astype(np.float32)
    alpha = a[..., 3]
    if (alpha < 250).mean() > 0.02:  # real alpha present
        return im.convert("RGBA")
    rgb = a[..., :3]
    if key == "auto":
        # dominant border colours (handles flat keys AND fake checkerboards / gradients)
        border = np.concatenate([rgb[:3].reshape(-1, 3), rgb[-3:].reshape(-1, 3),
                                 rgb[:, :3].reshape(-1, 3), rgb[:, -3:].reshape(-1, 3)])
        q = (border // 24).astype(int)
        codes = q[:, 0] * 10000 + q[:, 1] * 100 + q[:, 2]
        uniq, cnt = np.unique(codes, return_counts=True)
        order = np.argsort(-cnt)
        keys, covered = [], 0
        for i in order[:6]:
            keys.append(border[codes == uniq[i]].mean(0)); covered += cnt[i]
            if covered / len(codes) > 0.9:
                break
        ks = np.array(keys)
    else:
        ks = np.array([parse_hex(h) for h in key.split(",")])
    dist = np.min(np.stack([np.sqrt(((rgb - k) ** 2).sum(-1)) for k in ks]), axis=0)
    k = ks[0]
    near = dist < tol * 1.6
    lab, n = ndimage.label(near)
    edge_labels = set(np.unique(np.concatenate([lab[0], lab[-1], lab[:, 0], lab[:, -1]]))) - {0}
    bgmask = np.isin(lab, list(edge_labels))
    cleared, kept = 0, 0
    if holes:  # also clear enclosed background-coloured pockets (e.g. inside a bow) of >= holes px
        sizes = ndimage.sum(near, lab, range(1, n + 1))
        core = ndimage.sum(dist < tol * 0.5, lab, range(1, n + 1))
        for i, sz in enumerate(sizes, 1):
            if i not in edge_labels and sz >= holes:
                frac = core[i - 1] / sz
                if frac >= 0.6 or (frac >= 0.2 and sz < 300):  # small anti-aliased gaps (fence rails, fingers) too
                    bgmask |= lab == i; cleared += int(sz)
                else:
                    kept += int(sz)
    if stats is not None:
        stats.update(pocket_px_cleared=cleared, pocket_px_kept_as_art=kept)
    soft = np.clip((dist - tol) / (tol * 0.6), 0, 1)
    new_alpha = np.where(bgmask, soft * 255, 255.0)
    # despill: pull key colour out of semi-transparent edge pixels
    edge = (new_alpha < 255) & (new_alpha > 0)
    if edge.any():
        t = (new_alpha[edge] / 255.0)[:, None]
        rgb[edge] = np.clip((rgb[edge] - k * (1 - t)) / np.maximum(t, 0.25), 0, 255)
    out = np.dstack([rgb, new_alpha]).astype(np.uint8)
    return Image.fromarray(out, "RGBA")


def clean_specks(im, min_px=40):
    a = np.asarray(im).copy()
    solid = a[..., 3] > 40
    lab, n = ndimage.label(solid)
    if n > 1:
        sizes = ndimage.sum(solid, lab, range(1, n + 1))
        for i, s in enumerate(sizes, 1):
            if s < min_px:
                a[lab == i, 3] = 0
    return Image.fromarray(a, "RGBA")


def trim(im):
    bb = im.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
    return im.crop(bb) if bb else im


def scale_to(im, w=None, h=None):
    if w and not h:
        h = max(1, round(im.height * w / im.width))
    elif h and not w:
        w = max(1, round(im.width * h / im.height))
    # premultiplied-ish resize to avoid dark/colour fringes
    a = np.asarray(im).astype(np.float32)
    al = a[..., 3:4] / 255.0
    pm = np.dstack([a[..., :3] * al, a[..., 3:4]])
    pm_im = [Image.fromarray(pm[..., i].astype(np.float32), "F").resize((w, h), Image.LANCZOS) for i in range(4)]
    r, g, b, A = [np.asarray(c) for c in pm_im]
    A = np.clip(A, 0, 255); al2 = np.maximum(A / 255.0, 1e-4)
    rgb = np.clip(np.dstack([r, g, b]) / al2[..., None], 0, 255)
    return Image.fromarray(np.dstack([rgb, A]).astype(np.uint8), "RGBA")


def pixelate(im, n):
    if n <= 1:
        return im
    small = im.resize((max(1, im.width // n), max(1, im.height // n)), Image.BOX)
    a = np.asarray(small).copy(); a[..., 3] = np.where(a[..., 3] > 110, 255, 0)
    return Image.fromarray(a, "RGBA").resize((small.width * n, small.height * n), Image.NEAREST)


def outline(im, dark=0.35):
    """1 px outline outside the sprite, coloured as a darkened average of the touching pixels."""
    a = np.asarray(im).astype(np.float32)
    solid = a[..., 3] > 40
    ring = ndimage.binary_dilation(solid, structure=[[0, 1, 0], [1, 1, 1], [0, 1, 0]]) & ~solid
    w = solid.astype(np.float32)
    k = np.ones((3, 3), np.float32)
    cnt = ndimage.convolve(w, k, mode="constant")
    col = np.dstack([ndimage.convolve(a[..., c] * w, k, mode="constant") for c in range(3)]) / np.maximum(cnt, 1)[..., None]
    out = a.copy()
    out[ring, :3] = col[ring] * dark
    out[ring, 3] = 255
    return Image.fromarray(np.clip(out, 0, 255).astype(np.uint8), "RGBA")


def place(im, cw, ch, bottom_pad=0):
    canvas = Image.new("RGBA", (max(cw, im.width), max(ch, im.height + bottom_pad)), (0, 0, 0, 0))
    canvas.alpha_composite(im, ((canvas.width - im.width) // 2, canvas.height - im.height - bottom_pad))
    return canvas


def diamond_mask(depth=32, skirt=True):
    m = Image.new("L", (192, 128), 0); d = ImageDraw.Draw(m)
    d.polygon([(96, 0), (192, 48), (96, 96), (0, 48)], fill=255)
    if skirt:
        d.polygon([(0, 48), (96, 96), (192, 48), (192, 48 + depth), (96, 96 + depth), (0, 48 + depth)], fill=255)
    return m


def project_square(tex):
    """Square top-down texture -> 192x96 iso diamond (rotate 45, squash y by 0.5)."""
    t = tex.convert("RGBA").resize((272, 272), Image.LANCZOS)
    r = t.rotate(45, resample=Image.BICUBIC, expand=True)
    r = r.resize((192, 96), Image.LANCZOS)
    return r


def synth_skirt(top, kind, depth=24):
    """Add an earth/water side face below the diamond using the diamond edge colours."""
    out = Image.new("RGBA", (192, 128), (0, 0, 0, 0))
    d = ImageDraw.Draw(out)
    if kind == "earth":
        cols = [(122, 84, 46), (98, 66, 36), (80, 52, 28)]
    else:
        cols = [(18, 70, 150), (14, 54, 120), (10, 40, 92)]
    for i in range(depth):
        c = cols[min(2, i * 3 // depth)]
        d.line([(0, 48 + i), (96, 96 + i)], fill=c + (255,))
        cc = tuple(int(v * 0.82) for v in c)
        d.line([(96, 96 + i), (191, 48 + i)], fill=cc + (255,))
    if kind == "earth":  # grass lip + a few pebbles
        d.line([(0, 48), (96, 96)], fill=(70, 110, 30, 255), width=2)
        d.line([(96, 96), (191, 48)], fill=(60, 96, 26, 255), width=2)
        rng = np.random.default_rng(5)
        for _ in range(26):
            x = int(rng.integers(4, 188)); yb = 48 + abs(96 - x) * -0.5 + 48
            y = int(yb + rng.integers(4, depth - 2))
            d.point((x, y), fill=(140, 104, 64, 255))
    out.alpha_composite(top, (0, 0))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--in", dest="inp", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--kind", choices=["unit", "feature", "building", "city", "tile"], required=True)
    ap.add_argument("--key", default="auto"); ap.add_argument("--tol", type=float, default=48)
    ap.add_argument("--height", type=int); ap.add_argument("--width", type=int)
    ap.add_argument("--pixel", type=int, default=1)
    ap.add_argument("--crop")
    ap.add_argument("--project", action="store_true")
    ap.add_argument("--skirt", default="keep", choices=["keep", "earth", "water", "none"])
    ap.add_argument("--no-bg-removal", action="store_true")
    ap.add_argument("--outline", action="store_true", help="add a 1 px dark outline coloured from the sprite edge")
    ap.add_argument("--holes", type=int, default=0, help="also clear enclosed bg-coloured pockets >= N px (0=off)")
    a = ap.parse_args()
    im = Image.open(a.inp).convert("RGBA")
    if a.crop:
        im = im.crop(tuple(int(v) for v in a.crop.split(",")))

    if a.kind == "tile":
        if a.project:
            top = project_square(im)
        else:
            if not a.no_bg_removal:
                im = remove_bg(im, a.key, a.tol, a.holes)
            im = trim(im)
            im = scale_to(im, w=192)
            top = Image.new("RGBA", (192, 128), (0, 0, 0, 0)); top.alpha_composite(im.crop((0, 0, 192, min(128, im.height))), (0, 0))
        top = pixelate(top, a.pixel)
        if a.project or a.skirt in ("earth", "water"):
            canvas = Image.new("RGBA", (192, 128), (0, 0, 0, 0)); canvas.alpha_composite(top.crop((0, 0, 192, min(128, top.height))), (0, 0))
            dm = diamond_mask(skirt=False)
            canvas.putalpha(Image.fromarray(np.minimum(np.asarray(canvas.getchannel("A")), np.asarray(dm))))
            out = synth_skirt(canvas, a.skirt if a.skirt in ("earth", "water") else "earth") if a.skirt != "none" else canvas
        else:
            out = top
            m = diamond_mask(skirt=a.skirt != "none")
            out.putalpha(Image.fromarray(np.minimum(np.asarray(out.getchannel("A")), np.asarray(m))))
    else:
        if not a.no_bg_removal:
            im = remove_bg(im, a.key, a.tol, a.holes)
        im = trim(clean_specks(im))
        if a.kind == "unit":
            im = scale_to(im, h=a.height or 88)
            im = pixelate(im, a.pixel)
            out = place(im, 96, 120, bottom_pad=2)
        elif a.kind == "city":
            im = scale_to(im, w=a.width or 372)
            im = pixelate(im, a.pixel)
            out = place(im, 384, im.height)
        else:
            im = scale_to(im, w=a.width or 184)
            im = pixelate(im, a.pixel)
            out = place(im, 192, im.height)
    if a.outline and a.kind != "tile":
        out = outline(out)
    Path(a.out).parent.mkdir(parents=True, exist_ok=True)
    out.save(a.out)
    print(json.dumps({"out": a.out, "size": out.size}))


if __name__ == "__main__":
    main()
