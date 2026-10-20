"""R - a cut-out rig built from the static sprite's own pixels (no image model in the strike).

The static sprite is segmented into part layers with polygon masks (head+hair, torso+belt+shorts top, far sword arm
= sleeve / forearm+fist / sword, near arm = sleeve / shield+hidden forearm, each leg = thigh / shin+boot). Pixels
hidden behind other parts are painted in code (nearest-pixel clone into a target silhouette, then the outline is
redrawn). Each strike frame poses the parts to the poses.json joints: limbs are rotated (and stretched a little along
the bone for foreshortening) about their pivots with a 4x-supersampled area-sampled affine, the torso is bent with the
rigid MLS warp (lean, squash), the head turns about the neck with its hair lagging (MLS on the head layer), the
shield lags the arm by a frame, and parts are composited in each frame's depth order.

  python arms/R/make_r.py            # rebuild parts, rig.json and the strike
  python arms/R/make_r.py --debug    # also write work/R/ part overlays and per-frame part views
"""
import json, sys, time
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import rig  # noqa: E402

ARM = "R"
HERE = ROOT / "arms" / ARM
WORK = ROOT / "work" / ARM
P0 = ak.POSES["anims"]["strike"][0]  # rest pose = the static sprite (idle frame 0 has the same joints)
OUTLINE = (33, 22, 30, 255)

# ---------------------------------------------------------------- segmentation (canvas coordinates)
# Pixels are assigned to parts in this priority order (a later part claims the pixels it covers).
POLYS = {
    "torso":   [[(66, 62), (98, 62), (97, 80), (95, 96), (66, 97), (69, 80)]],
    "r_thigh": [[(65, 95), (81, 95), (81, 101), (78, 105), (78, 111), (68, 111), (65, 102)]],
    "l_thigh": [[(82, 95), (95, 95), (96, 103), (97, 111), (88, 112), (85, 104), (82, 100)]],
    "r_shin":  [[(64, 108), (79, 107), (78, 126), (56, 127), (56, 116), (64, 113)]],
    "l_shin":  [[(86, 108), (99, 106), (104, 112), (109, 120), (108, 127), (88, 127), (88, 116)]],
    "head":    [[(64, 46), (76, 38), (98, 39), (104, 48), (102, 60), (95, 65), (88, 66), (86, 67), (79, 67),
                 (74, 66), (72, 66), (66, 62), (64, 56)]],
    "r_upper": [[(63, 70), (66, 67), (70, 67), (70, 79), (69, 81), (65, 80), (63, 75)]],
    "r_fore":  [[(55, 81), (58, 79), (62, 78), (65, 76), (70, 78), (70, 84), (66, 86), (63, 88), (58, 88), (55, 85)]],
    "sword":   [[(42, 46), (52, 46), (60, 66), (61, 73), (65, 73), (65, 77), (58, 81), (51, 84), (49, 80),
                 (51, 77), (49, 72), (41, 52)],
                [(57, 87), (65, 87), (65, 93), (57, 93)]],
    "l_upper": [[(89, 67), (95, 65), (102, 69), (102, 76), (98, 80), (91, 80), (89, 74)]],
    "shield":  [[(93, 77), (100, 75), (109, 75), (114, 82), (115, 98), (111, 106), (100, 107), (94, 103), (92, 92)]],
}
PRIORITY = list(POLYS)

# Target silhouettes for hidden-pixel fill: the part as it would look with nothing in front of it.
FILL = {
    # tunic continues under the far sleeve, the hair/chin and the near sleeve/shield edge
    "torso":   [[(74, 62), (93, 62), (97, 67), (97, 74), (96, 84), (95, 96), (67, 97), (69, 90), (71, 84),
                 (71, 70)]],
    # shorts legs continue up under the belt so the hips can turn without a gap
    "r_thigh": [[(67, 90), (81, 90), (81, 101), (78, 105), (77, 110), (69, 110), (66, 102)]],
    "l_thigh": [[(82, 90), (94, 90), (95, 101), (96, 109), (89, 111), (85, 104), (82, 99)]],
    # shins continue up under the knee
    "r_shin":  [[(67, 104), (77, 104), (77, 113)]],
    "l_shin":  [[(88, 104), (97, 104), (98, 112)]],
    # sleeves continue under the tunic edge / the shield (round puffed sleeve)
    "r_upper": [[(64, 69), (67, 67), (72, 67), (74, 71), (74, 78), (70, 81), (65, 80), (63, 75)]],
    "l_upper": [[(89, 67), (95, 65), (101, 68), (102, 76), (99, 82), (92, 82), (89, 75)]],
}

# pivots (rest positions) and the bone each part follows: (pivot joint, aim joint)
BONES = {"r_upper": ("r_sh", "r_el"), "r_fore": ("r_el", "r_ha"), "sword": ("r_ha", "tip"),
         "l_upper": ("l_sh", "l_el"), "r_thigh": ("r_hip", "r_kn"), "r_shin": ("r_kn", "r_ft"),
         "l_thigh": ("l_hip", "l_kn"), "l_shin": ("l_kn", "l_ft")}
STRETCH = {"r_upper": (0.85, 1.25), "r_fore": (0.85, 1.35), "sword": (0.88, 1.03), "l_upper": (0.9, 1.15),
           "r_thigh": (0.9, 1.15), "r_shin": (0.9, 1.1), "l_thigh": (0.9, 1.12), "l_shin": (0.9, 1.1)}
GROUP = {"sword": ["sword"], "r_arm": ["r_upper", "r_fore"], "r_leg": ["r_shin", "r_thigh"], "torso": ["torso"],
         "l_leg": ["l_shin", "l_thigh"], "head": ["head"], "l_arm": ["l_upper"], "shield": ["shield"]}


def segment(base):
    a = np.asarray(base)
    op = a[..., 3] > 0
    label = np.full(op.shape, -1, int)
    for k, name in enumerate(PRIORITY):
        m = rig.polygon_mask(base.size, POLYS[name])
        label[m & op] = k
    # tunic-blue pixels caught by the head / sleeve polygons are the tunic's collar and straps: give them to the torso
    r, g, b = (a[..., i].astype(int) for i in range(3))
    blue = (b > r + 60) & (b > g + 40)
    for name in ("head", "r_upper", "l_upper"):
        k = PRIORITY.index(name)
        label[(label == k) & blue] = PRIORITY.index("torso")
    unassigned = op & (label < 0)
    if unassigned.any():  # stray edge pixels: give them to the nearest assigned part
        idx = ndimage.distance_transform_edt(label < 0, return_distances=False, return_indices=True)
        label[unassigned] = label[idx[0][unassigned], idx[1][unassigned]]
    # crumbs: polygon edges slice off a few outline pixels of a neighbouring part; when the parts move apart these
    # specks would float in the air. Components under 12 px go to the neighbouring part they touch.
    for _ in range(2):
        for k in range(len(PRIORITY)):
            lab, n = ndimage.label(label == k, structure=np.ones((3, 3)))
            if n < 2:
                continue
            sizes = ndimage.sum(np.ones_like(lab), lab, range(1, n + 1))
            for c in np.nonzero(sizes < 12)[0]:
                comp = lab == (c + 1)
                ring = ndimage.binary_dilation(comp, np.ones((3, 3))) & ~comp & (label >= 0) & (label != k)
                if ring.any():
                    label[comp] = np.bincount(label[ring]).argmax()
    return {name: rig.part_layer(base, label == k) for k, name in enumerate(PRIORITY)}, label


def fill_hidden(layer, polys, hidden, outline=True):
    """Paint the pixels inside the target silhouette that the part does not have but that a part in FRONT of it hides
    in the rest pose (`hidden`): clone the nearest opaque pixel of the part (so stripes/shading extend), then redraw
    a 1 px dark outline on the new silhouette edge. Pixels nothing hides are left alone, so the rest pose rebuilt from
    the parts stays identical to the sprite."""
    a = np.asarray(layer).copy()
    have = a[..., 3] > 200
    sil = (rig.polygon_mask(layer.size, polys) & (hidden | have)) | have
    need = sil & ~have
    if not need.any():
        return layer
    # clone from the part's interior (not its outline) so the fill continues the material, not the edge
    lum = a[..., :3].astype(int).sum(-1)
    interior = have & (lum > 160)
    src = interior if interior.sum() > 10 else have
    idx = ndimage.distance_transform_edt(~src, return_distances=False, return_indices=True)
    a[need] = a[idx[0][need], idx[1][need]]
    a[need, 3] = 255
    if outline:
        er = ndimage.binary_erosion(sil, iterations=1)
        edge = sil & ~er & need
        a[edge] = OUTLINE
    return Image.fromarray(a, "RGBA")


def paint_grip(sword):
    """The fist hides the sword grip; paint it (brown leather with a dark edge) so a turning wrist shows no gap."""
    d = ImageDraw.Draw(sword)
    d.line([(57, 80), (61, 88)], fill=OUTLINE, width=3)
    d.line([(57, 80), (61, 88)], fill=(118, 66, 38, 255), width=1)
    return sword


def build_parts(debug=False):
    base = ak.base_canvas()
    parts, label = segment(base)
    seq = [n for g in P0.get("order", ak.POSES["default_order"]) for n in GROUP[g]]
    for name, polys in FILL.items():
        front = [PRIORITY.index(n) for n in seq[seq.index(name) + 1:]]
        hidden = np.isin(label, front)
        parts[name] = fill_hidden(parts[name], polys, hidden)
    parts["sword"] = paint_grip(parts["sword"])
    (HERE / "parts").mkdir(parents=True, exist_ok=True)
    for name, im in parts.items():
        im.save(HERE / "parts" / f"{name}.png")
    rigj = {"canvas": [ak.W, ak.H], "rest_pose": {k: v for k, v in P0.items() if isinstance(v, list)},
            "priority": PRIORITY, "mask_polygons": POLYS, "fill_silhouettes": FILL, "bones": BONES,
            "stretch_limits": STRETCH, "depth_groups": GROUP,
            "pivots": {n: P0[b[0]] for n, b in BONES.items()} | {"torso": P0["pelvis"], "head": P0["neck"],
                                                                  "shield": P0["shield"][:2]},
            "notes": "parts/*.png are full 160x128 canvases in rest position; depth order per frame comes from "
                     "poses.json 'order' with depth_groups expanding each entry"}
    (HERE / "rig.json").write_text(json.dumps(rigj, indent=1))
    if debug:
        WORK.mkdir(parents=True, exist_ok=True)
        cols = [(255, 0, 0), (0, 200, 0), (0, 0, 255), (255, 200, 0), (255, 0, 255), (0, 255, 255), (255, 128, 0),
                (128, 0, 255), (255, 255, 255), (0, 128, 128), (128, 255, 0)]
        ov = np.zeros((ak.H, ak.W, 4), np.uint8)
        for k in range(len(PRIORITY)):
            ov[label == k] = cols[k % len(cols)] + (255,)
        big = Image.fromarray(ov, "RGBA").crop((36, 34, 124, 128)).resize((88 * 6, 94 * 6), Image.NEAREST)
        big.save(WORK / "labels.png")
        sheet = Image.new("RGBA", (88 * len(parts), 94), (90, 150, 60, 255))
        for i, (n, im) in enumerate(parts.items()):
            sheet.alpha_composite(im.crop((36, 34, 124, 128)), (88 * i, 0))
        sheet.resize((sheet.width * 3, sheet.height * 3), Image.NEAREST).save(WORK / "parts_sheet.png")
    return parts


# ---------------------------------------------------------------- geometry
def warp_affine(im, M, a0, a1, ss=4):
    """out = M (in - a0) + a1, sampled by 4x nearest supersampling + premultiplied box filter (area sampling): crisper
    than 1x bicubic on soft-edged pixel art, and exact (pixel-identical) for the identity."""
    M = np.asarray(M, float)
    Mi = np.linalg.inv(M)
    a0 = np.asarray(a0, float) + 0.5  # joints are pixel indices -> pixel centres
    a1 = np.asarray(a1, float) + 0.5
    # continuous coords: in = Mi (out - a1) + a0 ; at ss x: in_s = Mi out_s + ss (a0 - Mi a1)
    c = ss * (a0 - Mi @ a1)
    mat = (Mi[0, 0], Mi[0, 1], c[0], Mi[1, 0], Mi[1, 1], c[1])
    pm = ak._premul(im)
    W, H = im.size
    chans = []
    for i in range(4):
        ch = Image.fromarray(pm[..., i], "F").resize((W * ss, H * ss), Image.NEAREST)
        t = ch.transform((W * ss, H * ss), Image.AFFINE, mat, resample=Image.NEAREST)
        chans.append(np.asarray(t.resize((W, H), Image.BOX)))
    return ak._unpremul(np.dstack(chans))


def rotm(deg):
    t = np.radians(deg)  # + = counter-clockwise on screen (y down)
    return np.array([[np.cos(t), np.sin(t)], [-np.sin(t), np.cos(t)]])


def bone_matrix(s0, e0, s1, e1, lim=(1.0, 1.0)):
    """Rotation taking bone s0->e0 onto the direction s1->e1, with a stretch along the bone (clamped)."""
    v0 = np.subtract(e0, s0).astype(float)
    v1 = np.subtract(e1, s1).astype(float)
    L0, L1 = np.hypot(*v0), np.hypot(*v1)
    k = float(np.clip(L1 / L0, *lim))
    u = v0 / L0
    S = np.eye(2) + (k - 1) * np.outer(u, u)  # stretch along the rest bone direction
    ang = rig.angle_of((0, 0), v1) - rig.angle_of((0, 0), v0)
    return rotm(ang) @ S, ang, k


def lerp(a, b, t):
    return [a[i] + (b[i] - a[i]) * t for i in range(len(a))]


# ---------------------------------------------------------------- code-drawn blade
# Rotating the sprite's stair-stepped blade outline by large angles gives a serrated (saw-tooth) edge. For large
# sword turns the blade is redrawn in code instead: a straight tapered blade whose cross-section colours are averaged
# from the sprite's own blade (so it keeps its light edge / white ridge / blue-grey shade / dark outline). The sprite's
# crossguard, grip and pommel are still the raster pixels, rotated on top.
BLADE_C = np.array([52.51, 65.85])     # blade centreline (PCA of the steel pixels), continuous coords
BLADE_D = np.array([-0.3684, -0.9297])  # unit direction toward the tip
BLADE_U = (-13.0, 16.2)                 # along-axis extent (guard .. tip incl. outline)
BLADE_HW = 4.4                          # half width incl. outline
HILT_U = -10.5                          # sword pixels below this (u <) are the hilt and stay raster
_PROFILE = None


def _uv(xy, c, d):
    n = np.array([-d[1], d[0]])
    q = xy - c
    return q @ d, q @ n


def blade_profile(sword):
    global _PROFILE
    if _PROFILE is None:
        pm = ak._premul(sword)
        vs = np.arange(-BLADE_HW, BLADE_HW + 1e-6, 0.25)
        us = np.arange(-7.0, 9.0, 0.5)
        n = np.array([-BLADE_D[1], BLADE_D[0]])
        acc = np.zeros((len(vs), 4))
        for u in us:
            pts = BLADE_C + u * BLADE_D + vs[:, None] * n  # continuous coords
            coords = [pts[:, 1] - 0.5, pts[:, 0] - 0.5]
            acc += np.stack([ndimage.map_coordinates(pm[..., c], coords, order=1) for c in range(4)], 1)
        _PROFILE = (vs, acc / len(us))
    return _PROFILE


def draw_blade(sword, M, a0, a1, ss=4):
    vs, prof = blade_profile(sword)
    a0 = np.asarray(a0, float) + 0.5
    a1 = np.asarray(a1, float) + 0.5
    c1 = M @ (BLADE_C - a0) + a1
    dm = M @ BLADE_D
    su = np.hypot(*dm)
    d1 = dm / su
    W, H = ak.W, ak.H
    ys, xs = np.mgrid[0:H * ss, 0:W * ss]
    xy = np.stack([(xs + 0.5) / ss, (ys + 0.5) / ss], -1)
    u, v = _uv(xy, c1, d1)
    u = u / su
    taper = 7.0
    w = np.where(u < BLADE_U[1] - taper, BLADE_HW,
                 BLADE_HW * np.clip((BLADE_U[1] - u) / taper, 0.0, 1.0) ** 0.8 + 0.6)
    vv = v * BLADE_HW / np.maximum(w, 1e-3)
    inside = (u > BLADE_U[0]) & (u < BLADE_U[1]) & (np.abs(vv) <= BLADE_HW)
    out = np.zeros((H * ss, W * ss, 4), np.float32)
    for c in range(4):
        out[..., c] = np.interp(vv, vs, prof[:, c])
    out[~inside] = 0
    small = out.reshape(H, ss, W, ss, 4).mean((1, 3))
    return ak._unpremul(small)


def hilt_layer(sword):
    a = np.asarray(sword).copy()
    ys, xs = np.mgrid[0:ak.H, 0:ak.W]
    u, _ = _uv(np.stack([xs + 0.5, ys + 0.5], -1), BLADE_C, BLADE_D)
    a[u >= HILT_U] = 0
    return Image.fromarray(a, "RGBA")


# ---------------------------------------------------------------- posing
def pose_frame(parts, p, prev=None, squash=(1.0, 1.0), hair_lag=(0, 0), shield_lag=(0, 0), extra=None):
    extra = extra or {}
    layers = {}
    # torso: rigid MLS bend from the rest joints to the frame joints (lean / crouch), feet-side joints pinned to hips
    tj = ["pelvis", "r_hip", "l_hip", "chest", "neck", "r_sh", "l_sh"]
    src = [P0[j] for j in tj]
    dst = [list(p[j]) for j in tj]
    if squash != (1.0, 1.0):  # squash about the pelvis on the contact frame
        pc = np.array(p["pelvis"], float)
        dst = [list(pc + (np.array(d) - pc) * np.array(squash)) for d in dst]
    layers["torso"] = rig.mls_warp(parts["torso"], src + [(82, 96), (70, 96), (94, 96)],
                                   dst + [list(np.add(p["pelvis"], (0, 6))), list(np.add(p["r_hip"], (-7, 2))),
                                          list(np.add(p["l_hip"], (6, 2)))])
    neck1 = dst[4]
    # head: turn about the neck by head_tilt (pose: + = chin up/back = clockwise), hair lags via MLS
    head = parts["head"]
    if max(abs(hair_lag[0]), abs(hair_lag[1])) >= 1.0:  # sub-pixel lag would only blur the head
        hs = [(82, 66), (80, 58), (86, 58), (74, 60), (84, 42), (98, 48), (100, 60), (70, 46)]
        lag = np.array(hair_lag, float)
        hd = [hs[0], hs[1], hs[2], hs[3]] + [tuple(np.add(q, lag * w)) for q, w in
                                              zip(hs[4:], (0.8, 1.0, 0.7, 0.6))]
        head = rig.mls_warp(head, hs, hd)
    tilt = p.get("head_tilt", 0) + extra.get("head_tilt", 0)
    layers["head"] = warp_affine(head, rotm(-tilt), P0["neck"], neck1)
    # far arm chain: sleeve from the shoulder, forearm+fist from the actual elbow, sword from the actual hand
    sh1 = dst[5]
    M, _, _ = bone_matrix(P0["r_sh"], P0["r_el"], sh1, p["r_el"], STRETCH["r_upper"])
    layers["r_upper"] = warp_affine(parts["r_upper"], M, P0["r_sh"], sh1)
    el1 = np.add(sh1, M @ np.subtract(P0["r_el"], P0["r_sh"]))
    M, _, _ = bone_matrix(P0["r_el"], P0["r_ha"], el1, p["r_ha"], STRETCH["r_fore"])
    layers["r_fore"] = warp_affine(parts["r_fore"], M, P0["r_el"], el1)
    ha1 = np.add(el1, M @ np.subtract(P0["r_ha"], P0["r_el"]))
    tip = np.add(p["tip"], extra.get("tip", (0, 0)))
    M, sang, _ = bone_matrix(P0["r_ha"], P0["tip"], ha1, tip, STRETCH["sword"])
    if abs(sang) > 12:  # big turn: code-drawn blade under the rotated raster hilt (no saw-tooth edge)
        sw = draw_blade(parts["sword"], M, P0["r_ha"], ha1)
        sw.alpha_composite(warp_affine(hilt_layer(parts["sword"]), M, P0["r_ha"], ha1))
        layers["sword"] = sw
    else:
        layers["sword"] = warp_affine(parts["sword"], M, P0["r_ha"], ha1)
    # near arm: sleeve from the shoulder; shield follows the hand with a lag and turns a little with the forearm
    lsh1 = dst[6]
    M, _, _ = bone_matrix(P0["l_sh"], P0["l_el"], lsh1, p["l_el"], STRETCH["l_upper"])
    layers["l_upper"] = warp_affine(parts["l_upper"], M, P0["l_sh"], lsh1)
    _, fang, _ = bone_matrix(P0["l_el"], P0["l_ha"], p["l_el"], p["l_ha"])
    sc1 = np.add(p["shield"][:2], shield_lag)
    layers["shield"] = warp_affine(parts["shield"], rotm(0.35 * fang + extra.get("shield_rot", 0)),
                                   P0["shield"][:2], sc1)
    # legs: thigh from the hip (torso-warped hip), shin from the actual knee aimed at the planted foot
    for s in ("r", "l"):
        hip1 = dst[1] if s == "r" else dst[2]
        M, _, _ = bone_matrix(P0[s + "_hip"], P0[s + "_kn"], hip1, p[s + "_kn"], STRETCH[s + "_thigh"])
        layers[s + "_thigh"] = warp_affine(parts[s + "_thigh"], M, P0[s + "_hip"], hip1)
        kn1 = np.add(hip1, M @ np.subtract(P0[s + "_kn"], P0[s + "_hip"]))
        M, _, _ = bone_matrix(P0[s + "_kn"], P0[s + "_ft"], kn1, p[s + "_ft"], STRETCH[s + "_shin"])
        layers[s + "_shin"] = warp_affine(parts[s + "_shin"], M, P0[s + "_kn"], kn1)
    out = Image.new("RGBA", (ak.W, ak.H), (0, 0, 0, 0))
    order = extra.get("order", p.get("order", ak.POSES["default_order"]))
    seq = [n for g in order for n in GROUP[g]]
    if "sword" in seq and seq.index("sword") > seq.index("r_fore"):
        # the fist wraps the grip: draw the sword just under the fist, keeping the fist's depth slot
        seq.remove("sword")
        seq.insert(seq.index("r_fore"), "sword")
    for name in seq:
        out.alpha_composite(layers[name])
    return out, layers


def fit_ground(f):
    """Feet stay planted on the ground row: shift the frame so its lowest opaque row sits on the anchor row - 1."""
    f2, _ = ak.snap_ground(f, max_px=3)
    return f2


def strike(parts):
    S = ak.POSES["anims"]["strike"]
    # per-frame animation-principle tweaks on top of the pose joints:
    #   hair lags the head one frame (trails opposite the head's motion), the shield lags the arm (follows the
    #   previous frame's hand motion), squash on the contact frame, small stretch on the swing.
    head_xy = [np.array(q["head"], float) for q in S]
    shield_xy = [np.array(q["shield"][:2], float) for q in S]
    tweaks = {
        # frame 1: the forearm and fist go in front of the hair (poses.json puts the arm behind the head, which hid
        # the hand entirely and left the sword hilt sprouting from the hair)
        1: dict(extra=dict(order=["r_leg", "torso", "l_leg", "head", "r_arm", "sword", "l_arm", "shield"])),
        2: dict(squash=(1.0, 0.97)),
        3: dict(squash=(1.0, 1.03)),
        4: dict(squash=(1.03, 0.95)),
        5: dict(squash=(1.02, 0.97)),
        6: dict(),
        7: dict(),
    }
    # overlapping action as a one-frame spring: the hair (and the shield) chase the head (hand) position, so they
    # trail behind on the whip forward (frames 3-5) and catch up on the recovery
    hair = head_xy[0].copy()
    shl = shield_xy[0].copy()
    frames = [ak.base_canvas()]
    for i in range(1, len(S)):
        tw = tweaks.get(i, {})
        hair = hair + (head_xy[i] - hair) * 0.55
        shl = shl + (shield_xy[i] - shl) * 0.6
        lag = np.clip(hair - head_xy[i], -3, 3)
        slag = np.clip(shl - shield_xy[i], -2, 2)
        if i == 7:  # settle: almost no lag left so idle resumes without a pop
            lag, slag = lag * 0.3, slag * 0.3
        f, _ = pose_frame(parts, S[i], squash=tw.get("squash", (1.0, 1.0)), hair_lag=tuple(lag),
                          shield_lag=tuple(slag), extra=tw.get("extra"))
        frames.append(fit_ground(f))
    return frames


def main(debug=False):
    t0 = time.time()
    parts = build_parts(debug)
    # sanity: the rest pose rebuilt from the parts should match the sprite closely
    rest, _ = pose_frame(parts, P0)
    diff = np.abs(np.asarray(rest).astype(int) - np.asarray(ak.base_canvas()).astype(int))[..., 3]
    print("rest rebuild alpha diff: mean %.3f max %d" % (diff.mean(), diff.max()))
    frames = strike(parts)
    man = ak.write_anim(ARM, "strike", frames, note="cut-out rig from the sprite's own pixels (make_r.py)")
    if debug:
        WORK.mkdir(parents=True, exist_ok=True)
        rest.save(WORK / "rest_rebuild.png")
    print(json.dumps(man.get("warnings", []), indent=1))
    return t0


if __name__ == "__main__":
    main(debug="--debug" in sys.argv)
