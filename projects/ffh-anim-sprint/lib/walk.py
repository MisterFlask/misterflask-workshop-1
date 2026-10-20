"""A walk cycle made with the recipe's method (an experiment after the sprint), and a clip of the unit walking.

  python3 lib/walk.py --unit units/swordsman.json --out final_swordsman --check-guides   # walk guides, no API calls
  python3 lib/walk.py --unit units/swordsman.json --out final_swordsman                 # 1 image call (n=2), ~$0.08
  python3 lib/walk.py --unit units/swordsman.json --out final_swordsman --rebuild --pick 2
  python3 lib/walk.py --unit units/swordsman.json --out final_swordsman --clip-only

Method (ANIMATING_UNITS.md): a walk is a big action, so the image model draws its 6 frames on one guided sprite sheet
(rule 2; cell 1 = the real sprite, cells 2-7 = blockouts); it walks the way the sprite looks (rule 1) and is mirrored
for the other lower diagonal. The poses come from a small planted-foot walk rig on the unit's frame-0 skeleton: each
foot travels along the walking direction on the isometric ground (screen direction (96, 48) for lower-right), plants
for half the cycle and swings with a lift for the other half; knees by two-bone IK, a 1.5 px body bob, a slight lean
and arm counter-swing. Frames are sliced, de-fringed and slid onto their guides like the strike.

Writes <out>/walk/ (frames + manifest.json with the cycle's ground speed), <out>/sheet_walk.png, the same frames in
<out>_nofx/walk/, <out>/preview/walk_strip3x.png and <out>/walk_clip.{gif,mp4}: the unit walking two tiles along its
facing, turning (mirrored) and walking two tiles down the other diagonal, then idling.
"""
import argparse, json, math, sys
from pathlib import Path
from PIL import Image, ImageOps

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402
import finish, gen, guides, preview, recipe, sheet  # noqa: E402

ROOT = ak.ROOT
N, MS = 6, 110                          # frames per cycle, ms per frame
S, H, B, W, LEAN, SWING = 9.0, 4.0, 1.5, 3.0, 1.0, 10.0   # half stride, foot lift, bob, half track width (px), lean, arm swing (deg)
UPPER = ("head", "neck", "chest", "r_sh", "r_el", "r_ha", "tip", "l_sh", "l_el", "l_ha")


def walk_dir(facing):
    return (0.894, 0.447) if facing == "lower-right" else (-0.894, 0.447)


def _leg3d(f, lat, zf, zh, l1, l2):
    """Knee of a leg in a ground frame (forward f, sideways lat, height z; the hip is at (0, 0, zh) above the ground
    point under it, the foot at (f, lat, zf)). The leg stays straight when it can reach and otherwise bends its knee
    forward. Solving in 3D keeps a leg that points into the scene straight, foreshortened, instead of bending it."""
    v = (f, lat, zf - zh)
    d = max(1e-6, math.sqrt(sum(c * c for c in v)))
    u = tuple(c / d for c in v)
    if d >= l1 + l2:
        return tuple(c * l1 for c in u[:2]) + (zh + u[2] * l1,)
    a = (l1 * l1 - l2 * l2 + d * d) / (2 * d)
    h = math.sqrt(max(0.0, l1 * l1 - a * a))
    fwd = (1.0, 0.0, 0.0)
    dot = sum(x * y for x, y in zip(fwd, u))
    perp = tuple(x - dot * y for x, y in zip(fwd, u))
    n = math.sqrt(sum(c * c for c in perp)) or 1.0
    perp = tuple(c / n for c in perp)
    return (a * u[0] + h * perp[0], a * u[1] + h * perp[1], zh + a * u[2] + h * perp[2])


def _rot(p, c, deg):
    t = math.radians(deg)
    x, y = p[0] - c[0], p[1] - c[1]
    return (c[0] + x * math.cos(t) - y * math.sin(t), c[1] + x * math.sin(t) + y * math.cos(t))


def walk_poses(u):
    """N walk poses on the unit's frame-0 skeleton, walking toward its facing."""
    rest = recipe.unit_poses(u)["idle"][0]
    dx, dy = walk_dir(u["facing"])
    near_dir = (-dx, dy)                                    # toward the viewer, across the walking direction
    pel = rest["pelvis"]
    ground_y = (rest["r_ft"][1] + rest["l_ft"][1]) / 2      # the ground line under the hips, from the rest stance
    legs = {}
    for side in ("r", "l"):
        hip, kn, ft = rest[side + "_hip"], rest[side + "_kn"], rest[side + "_ft"]
        screen_left = hip[0] < pel[0]
        near = screen_left if u["facing"] == "lower-right" else not screen_left
        legs[side] = dict(l1=math.dist(hip, kn), l2=math.dist(kn, ft), lat=W if near else -W, near=near)
    reach = sum(legs[s]["l1"] + legs[s]["l2"] for s in legs) / 2
    for leg in legs.values():                              # same leg length both sides, each keeping its thigh:shin ratio
        k = reach / (leg["l1"] + leg["l2"])
        leg["l1"] *= k
        leg["l2"] *= k
    near_side = "r" if legs["r"]["near"] else "l"
    out = []
    for i in range(N):
        p = {k: (list(v) if isinstance(v, (list, tuple)) else v) for k, v in rest.items()}
        ph_near = (i + 0.5) / N
        bob = B * math.cos(4 * math.pi * ph_near)          # low near double support, high at passing
        for k in ("pelvis", "r_hip", "l_hip"):
            p[k] = [rest[k][0], rest[k][1] + bob]
        for k in UPPER:
            p[k] = [rest[k][0] + LEAN * dx, rest[k][1] + bob]
        if "shield" in rest:
            p["shield"] = [rest["shield"][0] + LEAN * dx, rest["shield"][1] + bob, rest["shield"][2], rest["shield"][3]]
        for side, leg in legs.items():
            ph = ph_near if side == near_side else (ph_near + 0.5) % 1.0
            f = S * math.cos(2 * math.pi * ph)
            lift = H * math.sin(2 * math.pi * (ph - 0.5)) if ph > 0.5 else 0.0
            hip = p[side + "_hip"]
            zh = ground_y - hip[1]                         # hip height above the ground line, in screen px
            knee = _leg3d(f, leg["lat"], lift, zh, leg["l1"], leg["l2"])
            # project: forward along the walking direction, sideways along near_dir, height straight up the screen
            proj = lambda q: [hip[0] + q[0] * dx + q[1] * near_dir[0], ground_y + q[0] * dy + q[1] * near_dir[1] - q[2]]
            p[side + "_ft"] = proj((f, leg["lat"], lift))
            p[side + "_kn"] = proj(knee)
        # arms counter-swing: each arm swings opposite to the leg on its side
        for side in ("r", "l"):
            ph = ph_near if side == near_side else (ph_near + 0.5) % 1.0
            swing = -SWING * math.cos(2 * math.pi * ph)        # leg forward -> arm back
            sh, ha = p[side + "_sh"], p[side + "_ha"]
            fwd = _rot(ha, sh, 1.0)
            sgn = 1 if (fwd[0] - ha[0]) * dx + (fwd[1] - ha[1]) * dy > 0 else -1
            chain = [side + "_el", side + "_ha"] + (["tip"] if side == "r" else [])
            for k in chain:
                p[k] = list(_rot(p[k], sh, sgn * swing))
            if side == "l" and "shield" in p:
                c = _rot(p["shield"][:2], sh, sgn * swing)
                p["shield"] = [c[0], c[1], p["shield"][2], p["shield"][3]]
        out.append(p)
    # the front foot of the iso stride lands lower on screen than the rest stance: lift the whole guide so every boot
    # stays inside the canvas (the slicer stands the redrawn figure on the ground row anyway)
    lift_all = min(0.0, 122.0 - max(max(p["r_ft"][1], p["l_ft"][1]) for p in out))
    for p in out:
        for k, v in list(p.items()):
            if k in ("shield",) or (isinstance(v, list) and len(v) == 2 and all(isinstance(t, (int, float)) for t in v)):
                p[k] = [v[0], v[1] + lift_all] + list(v[2:])
    return out


def walk_texts(u):
    near, far = ("screen-left", "screen-right") if u["facing"] == "lower-right" else ("screen-right", "screen-left")
    fwd = u["facing"]
    back = "upper-left" if fwd == "lower-right" else "upper-right"
    lead = f"walking toward the {fwd} at a steady pace, the weapon still held as in cell 1"
    return [
        f"{lead}: the {near} leg is forward, its heel planted toward the {fwd}; the {far} leg is behind toward the "
        f"{back}, its heel lifting; the arms swing opposite to the legs",
        f"passing: the weight is on the {near} leg, straight under the body, which is at its highest; the {far} leg "
        f"swings forward past it with the foot lifted",
        f"the {near} leg is now behind, toward the {back}, pushing off from its toes; the {far} leg reaches forward "
        f"toward the {fwd}, heel about to touch down; the body dips",
        f"the {far} leg is forward, its heel planted toward the {fwd}; the {near} leg is behind toward the {back}, its "
        f"heel lifting; the arms swing the other way",
        f"passing: the weight is on the {far} leg, the body at its highest; the {near} leg swings forward past it with "
        f"the foot lifted",
        f"the {far} leg is behind, pushing off from its toes; the {near} leg reaches forward toward the {fwd}, heel about "
        f"to touch down; the body dips",
    ]


def register(u):
    """Make the recipe's sheet helpers (prepare, review_sheet) know the walk sheet."""
    recipe.SHEETS["walk"] = [("walk", i) for i in range(N)]
    recipe.LABEL["walk"] = "a WALK cycle that loops (the frame after cell 7 is cell 2 again)"
    recipe.STANDING |= {("walk", i) for i in range(N)}
    poses = recipe.unit_poses(u)
    poses["walk"] = walk_poses(u)
    return poses


def check_guides(u, out, poses, style):
    wdir = ROOT / "work" / out
    wdir.mkdir(parents=True, exist_ok=True)
    base = recipe.base_canvas(u)
    cells = [base] + [guides.render_guide(p, style) for p in poses["walk"]]
    row = Image.new("RGBA", (ak.W * len(cells), ak.H), (255, 0, 255, 255))
    for i, c in enumerate(cells):
        row.alpha_composite(c, (i * ak.W, 0))
    row.resize((row.width * 2, row.height * 2), Image.NEAREST).save(wdir / "walk_guides.png")
    return wdir / "walk_guides.png"


def generate(u, out, poses, style):
    wdir = ROOT / "work" / out
    wdir.mkdir(parents=True, exist_ok=True)
    base = recipe.base_canvas(u)
    cells = [base] + [guides.render_guide(p, style) for p in poses["walk"]]
    sheet.make_template(cells, out=wdir / "template_walk.png")
    texts = [(k + 2, t) for k, t in enumerate(walk_texts(u))]
    prompt = recipe.prompt(u, recipe.LABEL["walk"], texts, style)
    res = gen.parallel([dict(tag=u.get("tag", "anim/final"), images=[wdir / "template_walk.png", gen.ROSTER],
                             prompt=prompt, out=wdir / "raw_walk.png", n=2)], workers=1)
    return [{k: r.get(k) for k in ("ok", "files", "usd", "error")} for r in res]


def build(u, out, poses, style, pick=None):
    wdir = ROOT / "work" / out
    base = recipe.base_canvas(u)
    geom = json.loads((wdir / "template_walk.geom.json").read_text())
    raws = recipe.raw_files(wdir, "walk")
    recipe.review_sheet(wdir, "walk", base, u, poses, style)
    k = pick or 1
    fr, diag = sheet.slice_sheet(raws[k - 1], geom, n=N + 1, ref_canvas=base, ground_snap=recipe.ground_limit(u),
                                 ground_row=recipe.ground_row(u, base))
    fr, moved = recipe.prepare(u, "walk", fr, poses, style, base)
    frames = fr[1:]
    for suffix in ("_nofx", ""):
        d = ROOT / (out + suffix) / "walk"
        d.mkdir(parents=True, exist_ok=True)
        for old in d.glob("frame_*.png"):
            old.unlink()
        man = {"unit": u["name"], "facing": u["facing"], "anim": "walk", "canvas": [ak.W, ak.H], "anchor": list(ak.ANCHOR),
               "loop": True, "hold_last": False,
               "ground_speed_px_per_s": round(4 * S / (N * MS / 1000), 1),
               "ground_direction": list(walk_dir(u["facing"])),
               "note": "move the unit along ground_direction (screen px) at ground_speed while this loops; mirror the "
                       "frames and the x of the direction for the other lower diagonal",
               "frames": []}
        for i, f in enumerate(frames):
            f.save(d / f"frame_{i:02d}.png")
            man["frames"].append({"file": f"frame_{i:02d}.png", "ms": MS, "events": []})
        (d / "manifest.json").write_text(json.dumps(man, indent=1))
        sh = Image.new("RGBA", (ak.W * N, ak.H), (0, 0, 0, 0))
        for i, f in enumerate(frames):
            sh.alpha_composite(f, (i * ak.W, 0))
        sh.save(ROOT / (out + suffix) / "sheet_walk.png")
    pv = ROOT / out / "preview"
    pv.mkdir(parents=True, exist_ok=True)
    preview.strip3x(frames, man, f"{out} / walk").save(pv / "walk_strip3x.png")
    return {"picked": k, "x_snap": moved, "ground_snap": diag.get("ground_snap"), "k": diag["k"]}


def clip(u, out, tiles=2, idle_ms=1500, tick=40, scale=2):
    """The unit walks `tiles` tiles along its facing, turns (mirrored) and walks `tiles` down the other diagonal, then
    idles. Walk frames come from <out>/walk, idle from <out>/idle."""
    d = ROOT / out
    preview.set_unit(d)
    walk = [Image.open(d / "walk" / f"frame_{i:02d}.png").convert("RGBA") for i in range(N)]
    idle_man = json.loads((d / "idle" / "manifest.json").read_text())
    idle = [(Image.open(d / "idle" / f["file"]).convert("RGBA"), f["ms"]) for f in idle_man["frames"]]
    speed = 4 * S / (N * MS / 1000)                        # screen px per second along the walking direction
    g = preview.grass()
    rows = cols = 3
    X0, Y0 = 40 + (rows - 1) * 96, 110
    Wd, Ht = rows * 96 + cols * 96 + 80, Y0 + (rows + cols - 2) * 48 + g.height + 40

    def tile_anchor(r, c):
        return X0 + (c - r) * 96 + 96, Y0 + (c + r) * 48 + 54

    first = (1, 0) if u["facing"] == "lower-left" else (0, 1)   # tile step (dr, dc) along the facing
    second = (0, 1) if first == (1, 0) else (1, 0)              # the other lower diagonal, mirrored frames
    start = (0, 0)
    mid = (start[0] + first[0] * tiles, start[1] + first[1] * tiles)
    end = (mid[0] + second[0] * tiles, mid[1] + second[1] * tiles)
    legs = [(tile_anchor(*start), tile_anchor(*mid), False), (tile_anchor(*mid), tile_anchor(*end), True)]
    order = sorted(((r, c) for r in range(rows) for c in range(cols)), key=lambda t: (t[0] + t[1], t[0]))
    ground = Image.new("RGBA", (Wd, Ht), (54, 92, 150, 255))
    for r, c in order:
        ground.alpha_composite(g, (X0 + (c - r) * 96, Y0 + (c + r) * 48))

    def scene(frame, ax, ay, mirrored):
        sc = ground.copy()
        preview.unit_on(sc, ImageOps.mirror(frame) if mirrored else frame, ax, ay, "blue", mirrored=mirrored)
        return sc

    frames, t_walk = [], 0.0
    for (a, b, mirrored) in legs:
        dist = math.dist(a, b)
        n = int(dist / speed * 1000 / tick)
        for j in range(n + 1):
            f = j / n
            ax, ay = a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f
            frames.append((scene(walk[int(t_walk // MS) % N], round(ax), round(ay), mirrored), tick))
            t_walk += tick
    ax, ay = legs[-1][1]
    t, k = 0, 0
    while t < idle_ms:
        im, ms = idle[k % len(idle)]
        frames.append((scene(im, ax, ay, True), ms))
        t += ms
        k += 1
    up = [(fr.resize((fr.width * scale, fr.height * scale), Image.NEAREST), ms) for fr, ms in frames]
    preview.save_mp4(up, d / "walk_clip.mp4")
    preview.save_gif(frames, d / "walk_clip.gif")
    return {"seconds": round(sum(ms for _, ms in frames) / 1000, 1), "speed_px_per_s": round(speed, 1),
            "frames": len(frames)}


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--unit", required=True); ap.add_argument("--out", required=True)
    ap.add_argument("--check-guides", action="store_true"); ap.add_argument("--rebuild", action="store_true")
    ap.add_argument("--pick", type=int); ap.add_argument("--clip-only", action="store_true")
    a = ap.parse_args()
    u = recipe.load_unit(a.unit)
    style = u.get("guide_style", "grey")
    poses = register(u)
    if a.check_guides:
        print(check_guides(u, a.out, poses, style))
        sys.exit(0)
    if not a.clip_only:
        if not a.rebuild:
            print(json.dumps(generate(u, a.out, poses, style)))
        print(json.dumps(build(u, a.out, poses, style, a.pick)))
    print(json.dumps(clip(u, a.out)))
