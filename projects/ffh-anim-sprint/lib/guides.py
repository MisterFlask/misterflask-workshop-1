"""Pose guides drawn from poses.json: flat-colour blockouts (the character's own colours) or grey mannequins.

  python lib/guides.py --anim strike --style colour --out work/guides_strike.png   # 3x strip of guides
  python lib/guides.py --check                                                    # frame-0 guide over the sprite

render_guide(pose, style) -> 160x128 RGBA at 1x (no outlines, near limbs last). Templates upscale it 3x NEAREST.
"""
import argparse, json, math
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
POSES = json.loads((ROOT / "poses.json").read_text())
W, H = POSES["canvas"]
HEAD_R = POSES["head_r"]

COLOUR = dict(tunic=(61, 105, 245), shorts=(50, 88, 220), sleeve=(240, 226, 184), skin=(255, 208, 140),
              hair=(121, 62, 43), boot=(160, 80, 24), legging=(222, 160, 29), belt=(161, 133, 86),
              blade=(205, 218, 232), hilt=(202, 133, 64), wood=(170, 95, 35), rim=(231, 160, 37),
              boss=(227, 235, 236), eye=(40, 24, 20))
GREY = dict(tunic=(150, 150, 150), shorts=(140, 140, 140), sleeve=(165, 165, 165), skin=(175, 175, 175),
            hair=(95, 95, 95), boot=(90, 90, 90), legging=(160, 160, 160), belt=(120, 120, 120),
            blade=(235, 235, 235), hilt=(110, 110, 110), wood=(125, 125, 125), rim=(150, 150, 150),
            boss=(220, 220, 220), eye=(40, 40, 40))
FAR_DARKEN = 0.82  # far (sword-side) limbs a shade darker so the two sides read apart


def _c(rgb, k=1.0):
    return tuple(int(max(0, min(255, v * k))) for v in rgb) + (255,)


def capsule(d, a, b, w, col):
    d.line([tuple(a), tuple(b)], fill=col, width=w)
    r = (w - 1) / 2
    for p in (a, b):
        d.ellipse([p[0] - r, p[1] - r, p[0] + r, p[1] + r], fill=col)


def rot(v, deg):
    t = math.radians(deg)
    return (v[0] * math.cos(t) - v[1] * math.sin(t), v[0] * math.sin(t) + v[1] * math.cos(t))


def render_guide(p, style="colour"):
    pal = COLOUR if style == "colour" else GREY
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    g = lambda k: tuple(p[k][:2])

    def leg(side, k):
        hip, kn, ft = g(side + "_hip"), g(side + "_kn"), g(side + "_ft")
        capsule(d, hip, kn, 8, _c(pal["legging"], k))
        capsule(d, kn, ft, 7, _c(pal["legging"], k))
        bx, by = ft[0] - 2, ft[1] + 1
        d.ellipse([bx - 6, by - 4, bx + 5, by + 3], fill=_c(pal["boot"], k))
        capsule(d, kn, ((kn[0] + ft[0]) / 2, (kn[1] + ft[1]) / 2 + 2), 7, _c(pal["boot"], k))
        capsule(d, (ft[0] + 0.0, ft[1] - 4.0), ft, 8, _c(pal["boot"], k))

    def arm(side, k):
        sh, el, ha = g(side + "_sh"), g(side + "_el"), g(side + "_ha")
        capsule(d, sh, el, 7, _c(pal["sleeve"], k))
        capsule(d, el, ha, 6, _c(pal["skin"], k))
        d.ellipse([ha[0] - 3, ha[1] - 3, ha[0] + 3, ha[1] + 3], fill=_c(pal["skin"], k))

    def torso():
        rs, ls, rh, lh, ch, pe = g("r_sh"), g("l_sh"), g("r_hip"), g("l_hip"), g("chest"), g("pelvis")
        poly = [(rs[0] - 2, rs[1] - 2), (ls[0] + 2, ls[1] - 2), (lh[0] + 4, lh[1] + 2), (rh[0] - 4, rh[1] + 2)]
        d.polygon(poly, fill=_c(pal["tunic"]))
        d.ellipse([ch[0] - 12, ch[1] - 7, ch[0] + 12, ch[1] + 9], fill=_c(pal["tunic"]))
        # shorts + belt around the pelvis
        d.ellipse([pe[0] - 11, pe[1] - 3, pe[0] + 11, pe[1] + 7], fill=_c(pal["shorts"]))
        d.line([(rh[0] - 5, pe[1] - 3), (lh[0] + 5, pe[1] - 3)], fill=_c(pal["belt"]), width=3)
        capsule(d, g("neck"), ch, 6, _c(pal["skin"]))

    def head():
        hx, hy = g("head")
        tilt = p.get("head_tilt", 0)
        d.ellipse([hx - HEAD_R, hy - HEAD_R, hx + HEAD_R, hy + HEAD_R], fill=_c(pal["skin"]))
        # hair cap over the top and back (back = upper-right for a lower-left facing figure)
        ox, oy = rot((3, -4), tilt)
        d.ellipse([hx + ox - 12, hy + oy - 11, hx + ox + 12, hy + oy + 9], fill=_c(pal["hair"]))
        fx, fy = rot((-3, 3), tilt)
        d.ellipse([hx + fx - 9, hy + fy - 7, hx + fx + 8, hy + fy + 8], fill=_c(pal["skin"]))
        ex, ey = rot((-4, 1), tilt)
        if p.get("eyes", "open") == "shut":
            d.line([(hx + ex - 2, hy + ey), (hx + ex + 1, hy + ey)], fill=_c(pal["eye"]), width=1)
        else:
            d.rectangle([hx + ex - 1, hy + ey - 1, hx + ex, hy + ey + 1], fill=_c(pal["eye"]))

    def sword():
        if "sword" in p:
            hilt, tip = p["sword"]
        else:
            hilt, tip = g("r_ha"), g("tip")
        vx, vy = tip[0] - hilt[0], tip[1] - hilt[1]
        n = math.hypot(vx, vy) or 1
        ux, uy = vx / n, vy / n
        base = (hilt[0] + ux * 3, hilt[1] + uy * 3)
        capsule(d, base, tip, 5, _c(pal["blade"]))
        px_, py_ = -uy, ux
        d.line([(base[0] + px_ * 4, base[1] + py_ * 4), (base[0] - px_ * 4, base[1] - py_ * 4)], fill=_c(pal["hilt"]), width=2)
        capsule(d, (hilt[0] - ux * 3, hilt[1] - uy * 3), base, 3, _c(pal["hilt"]))

    def shield():
        cx, cy, rx, ry = p["shield"]
        d.ellipse([cx - rx, cy - ry, cx + rx, cy + ry], fill=_c(pal["wood"]), outline=_c(pal["rim"]), width=2)
        d.ellipse([cx - 2, cy - 2, cx + 2, cy + 2], fill=_c(pal["boss"]))

    parts = {"sword": sword, "r_arm": lambda: arm("r", FAR_DARKEN), "l_arm": lambda: arm("l", 1.0),
             "r_leg": lambda: leg("r", FAR_DARKEN), "l_leg": lambda: leg("l", 1.0), "torso": torso,
             "head": head, "shield": shield}
    for part in p.get("order", POSES["default_order"]):
        parts[part]()
    return im


def strip(anim, style="colour", scale=3, bg=(255, 0, 255, 255)):
    frames = [render_guide(p, style) for p in POSES["anims"][anim]]
    out = Image.new("RGBA", (W * len(frames), H), bg)
    for i, f in enumerate(frames):
        out.alpha_composite(f, (i * W, 0))
    return out.resize((out.width * scale, out.height * scale), Image.NEAREST)


def check(out):
    sprite = Image.open(ROOT / "m7/roster/units/unit_swordsman.png").convert("RGBA")
    base = Image.new("RGBA", (W, H), (126, 170, 60, 255))
    base.alpha_composite(sprite, tuple(POSES["sprite_offset"]))
    g = render_guide(POSES["anims"]["idle"][0], "colour")
    g2 = g.copy(); g2.putalpha(g.getchannel("A").point(lambda a: a * 0.55))
    over = base.copy(); over.alpha_composite(g2)
    row = Image.new("RGBA", (W * 3, H), (126, 170, 60, 255))
    row.alpha_composite(base, (0, 0)); row.alpha_composite(over, (W, 0))
    gg = Image.new("RGBA", (W, H), (255, 0, 255, 255)); gg.alpha_composite(g); row.alpha_composite(gg, (2 * W, 0))
    row.resize((row.width * 4, row.height * 4), Image.NEAREST).save(out)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--anim"); ap.add_argument("--style", default="colour"); ap.add_argument("--out")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    if a.check:
        check(a.out or ROOT / "work/guide_check.png")
    else:
        strip(a.anim, a.style).save(a.out)
