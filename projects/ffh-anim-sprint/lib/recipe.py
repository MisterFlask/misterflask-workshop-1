"""The recommended recipe as one tool: a full idle/strike/flinch/death set for an M7 unit.

  python lib/recipe.py --unit units/swordsman.json --out final            # generate + build (3 model calls, n=2)
  python lib/recipe.py --unit units/swordsman.json --out final --rebuild  # re-cut from saved raws, no API calls
  python lib/recipe.py --unit units/hyborem.json --out final_hyborem --anims strike death
  python lib/recipe.py --grid m7/roster/units/unit_ogre_embers.png --out final_ogre   # coordinates for annotating
  python lib/recipe.py --unit units/ogre.json --out final_ogre --check-guides          # guides, no API calls
  python lib/recipe.py --unit units/ogre.json --out final_ogre --show-beats            # texts the model will get

The full how-to for agents is ANIMATING_UNITS.md.

How it works (each choice comes from the sprint's results; see the report):
  idle    MLS mesh warp of the TRUE sprite toward each idle pose (sub-pixel breathing, no redraw, so no boil).
  strike  one guided sprite sheet (cell 1 = true sprite, cells 2-8 = blockout guides from the poses), sliced with one
          global scale; the last frame (settle) is the true sprite warped toward the settle pose, so the return to
          idle does not pop.
  flinch  guided sheet for the big frames 0-2; frame 3 (settle) warped from the true sprite.
  death   guided sheet for frames 1-7; frame 0 = flinch frame 0 (same pose by the beat sheet).
  clean   redrawn frames are de-fringed (the magenta keying halo becomes outline colour); warps use the sharpened
          MLS map so idle and settle frames stay crisp. Both were added after the blind strike judging.
  finish  smear + spark on the strike, hit flash on flinch/death frame 0 (FX) - written to <out>/ ; the same set
          without FX goes to <out>_nofx/ for fair comparison with round-1 arms.

A unit file (JSON) gives: name, sprite path, description (one line, what the model must keep), facing (the way the
sprite looks: "lower-left" or "lower-right"; the strike always goes that way, and the hit comes from there), guide
style ("colour" for the swordsman's own palette, "grey" for any other unit), the unit's frame-0 joints in 1x canvas
coordinates (same keys as poses.json, r_ = the weapon arm; omit "shield" if it has none) and optional beat-text
overrides. The facing and the weapon's screen side pick the source pose set (pose_set): "far" (poses.json, written for
a lower-left facer with the weapon in the far, screen-left hand) or "near" (poses_near.json, a lower-right facer with
the weapon in the near, screen-left hand), mirrored when the weapon is in the screen-right hand. Poses for other units
are retargeted bone by bone: each bone keeps the unit's own frame-0 angle plus the source bone's rotation away from
its rest angle, at the unit's own bone lengths.
"""
import argparse, json, math, sys, time
from pathlib import Path
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402
import beats, finish, gen, guides, metrics, rig, sheet  # noqa: E402

ROOT = ak.ROOT
FACINGS = ("lower-left", "lower-right")
SETS = {"far": "poses.json", "near": "poses_near.json"}
JOINTS = ("head", "neck", "chest", "pelvis", "r_sh", "r_el", "r_ha", "tip", "l_sh", "l_el", "l_ha",
          "r_hip", "r_kn", "r_ft", "l_hip", "l_kn", "l_ft")
WEAPON_CHAIN = {("r_sh", "r_el"), ("r_el", "r_ha"), ("r_ha", "tip")}
WARP_FRAMES = {("idle", 1), ("idle", 2), ("idle", 3), ("strike", 7), ("flinch", 3)}  # the true sprite, mesh-warped
STANDING = {("strike", i) for i in range(1, 7)} | {("flinch", i) for i in range(3)} | {("death", i) for i in range(1, 4)}
TREE = [("pelvis", "chest"), ("chest", "neck"), ("neck", "head"), ("chest", "r_sh"), ("r_sh", "r_el"), ("r_el", "r_ha"),
        ("r_ha", "tip"), ("chest", "l_sh"), ("l_sh", "l_el"), ("l_el", "l_ha"), ("pelvis", "r_hip"), ("r_hip", "r_kn"),
        ("r_kn", "r_ft"), ("pelvis", "l_hip"), ("l_hip", "l_kn"), ("l_kn", "l_ft")]


# ---------------------------------------------------------------- unit + poses
def load_unit(path):
    u = json.loads(Path(path).read_text())
    for k in ("name", "sprite", "description", "joints"):
        if k not in u:
            raise SystemExit(f"unit file {path} is missing '{k}' (see ANIMATING_UNITS.md, step 1)")
    if u.get("facing") not in FACINGS:  # the strike goes where the sprite looks (ANIMATING_UNITS.md, the rules)
        raise SystemExit(f"{path}: 'facing' must be one of {FACINGS}, the way the sprite looks; got {u.get('facing')!r} "
                         "(ANIMATING_UNITS.md, step 1b)")
    missing = [j for j in JOINTS if j not in u["joints"]]
    if missing:
        raise SystemExit(f"{path}: joints {missing} are missing (every joint except shield is required)")
    u["pose_set"], u["mirror"] = pose_set(u)
    u["sprite_img"] = Image.open(ROOT / u["sprite"]).convert("RGBA")
    sp = u["sprite_img"]
    if sp.height > ak.H or sp.width > ak.SAFE_X[1] - ak.SAFE_X[0]:
        raise SystemExit(f"{u['sprite']} is {sp.width}x{sp.height}; the animation canvas is {ak.W}x{ak.H} with a "
                         f"{ak.SAFE_X[1] - ak.SAFE_X[0]} px wide safe region. Big units need a bigger canvas "
                         "(ANIMATING_UNITS.md, 'Known limits').")
    u["offset"] = (int(ak.ANCHOR[0] - sp.width / 2), ak.H - sp.height)  # M7: feet 2 px above the bottom
    return u


def pose_set(u):
    """(source set, mirror) for this unit, so that the strike goes the way the sprite looks: the far set looks
    lower-left with the weapon in the far (screen-left) hand, the near set looks lower-right with the weapon in the
    near (screen-left) hand, and a mirrored set swaps both the facing and the weapon's screen side."""
    weapon_left = u["joints"]["r_sh"][0] <= u["joints"]["l_sh"][0]
    if weapon_left:
        return ("far" if u["facing"] == "lower-left" else "near"), False
    return ("near" if u["facing"] == "lower-left" else "far"), True


def smear_direction(u):
    """Which way the strike's blade turns on screen (finish.smear), from the source set, flipped when mirrored."""
    d = json.loads((ROOT / SETS[u["pose_set"]]).read_text()).get("smear", "ccw")
    return {"ccw": "cw", "cw": "ccw"}.get(d, d) if u["mirror"] else d


def base_canvas(u):
    c = Image.new("RGBA", (ak.W, ak.H), (0, 0, 0, 0))
    c.alpha_composite(u["sprite_img"], u["offset"])
    return c


def _wrap(a):
    return (a + math.pi) % (2 * math.pi) - math.pi


def retarget(u, src_rest, src_pose, mode="rel"):
    """Unit pose = unit rest skeleton driven by the source pose (lengths stay the unit's). mode:
    rel    every bone keeps the unit's rest angle plus the source bone's rotation (the sprint's retarget);
    drawn  as rel, but the weapon arm and weapon (WEAPON_CHAIN) turn toward the source's ABSOLUTE angle as the source
           bone turns further from rest (fully past 60 degrees), so a unit that holds its weapon differently (a club on
           the shoulder) still swings it where the beat sheet says; for frames the model draws;
    disp   every joint moves by the source joint's displacement from rest (x scale): small motion such as the idle
           breath, which is a stretch the angles cannot carry; for the mesh-warped frames."""
    rest = u["joints"]
    out = {k: v for k, v in src_pose.items() if k not in rest and k not in ("shield", "sword", "tip")}
    s = u.get("scale", 1.0)
    if mode == "disp":
        for j, rv in rest.items():
            if j == "shield":
                continue
            sp, sr = src_pose.get(j), src_rest.get(j)
            out[j] = tuple(rv[:2]) if sp is None or sr is None else (rv[0] + (sp[0] - sr[0]) * s, rv[1] + (sp[1] - sr[1]) * s)
        if "shield" in rest and "shield" in src_pose:
            rr, sp, sr = rest["shield"], src_pose["shield"], src_rest["shield"]
            out["shield"] = (rr[0] + (sp[0] - sr[0]) * s, rr[1] + (sp[1] - sr[1]) * s, rr[2] * sp[2] / sr[2], rr[3] * sp[3] / sr[3])
        return _pose_tail(u, src_pose, out)
    px, py = src_pose["pelvis"][:2]
    rx, ry = src_rest["pelvis"][:2]
    out["pelvis"] = (rest["pelvis"][0] + (px - rx) * s, rest["pelvis"][1] + (py - ry) * s)
    sp_tip = src_pose.get("tip") or (src_pose["sword"][1] if "sword" in src_pose else None)
    for a, b in TREE:
        if b not in rest or a not in rest:
            continue
        sb = sp_tip if b == "tip" else src_pose.get(b)
        sa = src_pose.get(a) if not (a == "r_ha" and "sword" in src_pose and b == "tip") else src_pose["sword"][0]
        if sb is None or sa is None:
            continue
        L = math.dist(rest[a][:2], rest[b][:2])
        # relative retarget: the unit's own rest angle plus the source bone's rotation away from ITS rest angle,
        # so frame 0 reproduces the unit's sprite exactly
        sra, srb = src_rest[a], (src_rest.get("tip") if b == "tip" else src_rest[b])
        d_src = math.atan2(sb[1] - sa[1], sb[0] - sa[0]) - math.atan2(srb[1] - sra[1], srb[0] - sra[0])
        ang = math.atan2(rest[b][1] - rest[a][1], rest[b][0] - rest[a][0]) + d_src
        if mode == "drawn" and (a, b) in WEAPON_CHAIN:
            w = min(1.0, max(0.0, (abs(_wrap(d_src)) - math.radians(15)) / math.radians(45)))
            ang += w * _wrap(math.atan2(sb[1] - sa[1], sb[0] - sa[0]) - ang)
        base = out[a] if a in out else rest[a]
        if b == "tip" and "sword" in src_pose:  # dropped sword: place it where the source drops it, unit-length blade
            h = (rest["pelvis"][0] + (src_pose["sword"][0][0] - rx) * s, rest["pelvis"][1] + (src_pose["sword"][0][1] - ry) * s)
            out["sword"] = (h, (h[0] + L * math.cos(ang), h[1] + L * math.sin(ang)))
            continue
        nb = (base[0] + L * math.cos(ang), base[1] + L * math.sin(ang))
        if b == "tip":
            # a long blade can leave the canvas (x 16-143, y 0-127): shorten the GUIDE blade along the same angle so
            # the guide stays inside its cell; the model still draws the unit's real sword. Big units really want a
            # bigger canvas; see the report.
            for _ in range(40):
                if 18 <= nb[0] <= 142 and 3 <= nb[1] <= 125:
                    break
                L *= 0.95
                nb = (base[0] + L * math.cos(ang), base[1] + L * math.sin(ang))
                out["_weapon_shortened"] = True
        out[b] = nb
    if "shield" in rest and "shield" in src_pose:
        dx, dy = src_pose["shield"][0] - src_pose["l_ha"][0], src_pose["shield"][1] - src_pose["l_ha"][1]
        rr = rest["shield"]
        out["shield"] = (out["l_ha"][0] + dx * s, out["l_ha"][1] + dy * s, rr[2] * src_pose["shield"][2] / src_rest["shield"][2],
                         rr[3] * src_pose["shield"][3] / src_rest["shield"][3])
    return _pose_tail(u, src_pose, out)


def _pose_tail(u, src_pose, out):
    out["head_tilt"] = src_pose.get("head_tilt", 0)
    out["eyes"] = src_pose.get("eyes", "open")
    out["order"] = [p for p in src_pose.get("order", ak.POSES["default_order"]) if p != "shield" or "shield" in u["joints"]]
    out["head_r"] = u.get("head_r", ak.POSES["head_r"])
    out["limb"] = u.get("limb", 1.0)
    if u.get("palette"):
        out["palette"] = u["palette"]
    return out


def unit_poses(u):
    src = json.loads((ROOT / u.get("poses_from", SETS[u["pose_set"]])).read_text())["anims"]
    if u["mirror"]:
        src = {a: [guides.mirror_pose(p) for p in fr] for a, fr in src.items()}
    if u.get("same_as_source"):
        return src

    def mode(anim, i):
        if u.get("legacy") or i == 0 and anim in ("idle", "strike"):
            return "rel"
        return "disp" if (anim, i) in WARP_FRAMES else "drawn"
    return {anim: [retarget(u, src["idle"][0], p, mode(anim, i)) for i, p in enumerate(frames)]
            for anim, frames in src.items()}


def beat_text(u, anim, i):
    """The unit's own override (written for its real facing), else the source set's text, mirrored with its poses."""
    own = u.get("beats", {}).get(anim, {}).get(str(i))
    if own:
        return own
    t = (beats.BEATS_NEAR if u["pose_set"] == "near" else beats.BEATS)[anim][i]["text"]
    return beats.swap_sides(t) if u["mirror"] else t


# ---------------------------------------------------------------- prompt
def prompt(u, anim_label, cells, style):
    guide = gen.GUIDE_TEXT["colour" if style == "colour" else "grey"]
    lines = "\n".join(f"cell {c}: {t}" for c, t in cells)
    text = (
        "The first attached image is a sprite-sheet template from my isometric fantasy strategy game: a 4x2 grid of "
        "equal cells on a flat magenta background (there are no visible grid lines). The top-left cell (cell 1) shows "
        f"the finished character, {u['description']}, drawn in {gen.STYLE}, standing in a 3/4 view facing the "
        f"{u['facing']}. {guide} Every cell must show the SAME character: same size, same face, same outfit and colours, "
        f"same pixel size and outline style, feet on the same ground line as the top-left cell. He faces the {u['facing']} "
        f"in every cell; {u.get('hands', 'the weapon stays in the same hand as in cell 1')}. Leave cell 1 exactly as "
        f"it is. Empty cells stay empty magenta. The cells are consecutive frames of {anim_label}, read left to right, "
        f"top row then bottom row:\n{lines}\nEach cell holds exactly one figure, fully inside its cell. No grid lines, "
        "frames, numbers, text, ground, shadows, motion lines or effects. Background: flat solid pure magenta #FF00FF "
        "filling the whole image, with no gradient. The second attached image is a roster sheet of other units from "
        "the same game: match its outline colour, shading and pixel size exactly.")
    if "{" in text or "}" in text:  # an unformatted placeholder must never reach the model
        raise ValueError(f"unformatted placeholder in the prompt: {text[text.find('{'):text.find('{') + 40]!r}")
    return text


# ---------------------------------------------------------------- the three sheets
SHEETS = {  # sheet -> list of (anim, frame) in cells 2.. ; cell 1 is always the true sprite
    "strike": [("strike", i) for i in range(1, 7)],
    "flinch": [("flinch", i) for i in range(0, 3)],
    "death": [("death", i) for i in range(1, 8)],
}
LABEL = {"strike": "a STRIKE animation", "flinch": "a FLINCH (hit reaction) animation", "death": "a DEATH animation"}


def make_sheet(u, poses, name, wdir, style):
    base = base_canvas(u)
    cells = [base] + [guides.render_guide(poses[a][i], style) for a, i in SHEETS[name]]
    geom = sheet.make_template(cells, out=wdir / f"template_{name}.png")
    texts = [(k + 2, beat_text(u, a, i)) for k, (a, i) in enumerate(SHEETS[name])]
    return geom, prompt(u, LABEL[name], texts, style)


def score_variant(diag, cand, poses=None, facing="lower-left", head_only=False):
    """Lower is better: gate flags (x100), reference-redraw colour error, scale error, size of ground snaps.
    cand = {anim: frames with the right frame indices} for the gate check."""
    g = sum(len(metrics.gates(a, fr, poses=poses, facing=facing, head_only=head_only)) for a, fr in cand.items())
    ref = (diag.get("ref_redraw_dE") or {}).get("dE") or 20
    snap = sum(abs(v) for v in diag.get("ground_snap", {}).values())
    return g * 100 + ref + abs(diag["k"] - 1) * 200 + snap * 0.5


def candidates(name, fr, base):
    if name == "strike":
        return {"strike": [base] + fr[1:7]}
    if name == "flinch":
        return {"flinch": fr[1:4]}
    return {"death": [fr[1]] + fr[1:8]}


def generate(u, out, anims, style, n=2):
    wdir = ROOT / "work" / out
    wdir.mkdir(parents=True, exist_ok=True)
    poses = unit_poses(u)
    need = [s for s in SHEETS if (s in anims) or (s == "flinch" and "death" in anims)]
    calls = []
    for name in need:
        geom, text = make_sheet(u, poses, name, wdir, style)
        calls.append(dict(tag=u.get("tag", "anim/final"), images=[wdir / f"template_{name}.png", gen.ROSTER], prompt=text,
                          out=wdir / f"raw_{name}.png", n=n))
    t0 = time.time()
    res = gen.parallel(calls, workers=len(calls))
    log = {"seconds": round(time.time() - t0, 1), "calls": [{k: r.get(k) for k in ("ok", "files", "usd", "error")} for r in res]}
    (wdir / "gen_log.json").write_text(json.dumps(log, indent=1))
    return log


def x_snap(frame, guide, y0=50, lim=40, min_gain=0.05):
    """The model places each figure a little differently inside its cell, and the slicer only corrects height. Slide a
    standing frame sideways onto its guide: the dx (within +-lim) that best overlaps the two alpha masks below y0 (legs
    and body; arms and weapon vary more). Applied only when the overlap (IoU) gains at least min_gain.
    Returns (frame, dx applied, gain)."""
    import numpy as np
    A = np.asarray(frame.getchannel("A"))[y0:] >= 128
    G = np.asarray(guide.getchannel("A"))[y0:] >= 128
    iou = lambda S: (S & G).sum() / max(1, (S | G).sum())
    i0, best = iou(A), (0, iou(A))
    for dx in range(-lim, lim + 1):
        S = np.zeros_like(A)
        if dx >= 0:
            S[:, dx:] = A[:, :A.shape[1] - dx]
        else:
            S[:, :dx] = A[:, -dx:]
        v = iou(S)
        if v > best[1] + 1e-9:
            best = (dx, v)
    dx, gain = best[0], best[1] - i0
    if dx == 0 or gain < min_gain:
        return frame, 0, round(gain, 3)
    out = Image.new("RGBA", frame.size, (0, 0, 0, 0))
    out.alpha_composite(frame.crop((max(0, -dx), 0, frame.width - max(0, dx), frame.height)), (max(0, dx), 0))
    return out, dx, round(gain, 3)


def prepare(u, name, fr, poses, style, base):
    """Post-process one sliced sheet (fr[0] = reference cell): de-fringe the redrawn frames and slide standing frames
    onto their guides. Legacy unit files get only the sprint's edge de-fringe. Returns (frames, {cell: dx})."""
    legacy = u.get("legacy", False)
    out, moved = [fr[0]], {}
    clean = finish.defringe_applicable(base) and u.get("defringe", True)
    for k, f in enumerate(fr[1:], 1):
        if clean:
            f = finish.defringe(f, interior=not legacy)
        an, i = SHEETS[name][k - 1]
        if not legacy and (an, i) in STANDING:
            f, dx, _ = x_snap(f, guides.render_guide(poses[an][i], style))
            if dx:
                moved[f"{an} {i}"] = dx
        out.append(f)
    return out, moved


def raw_files(wdir, name):
    fs = sorted(wdir.glob(f"raw_{name}_*.png")) or sorted(wdir.glob(f"raw_{name}.png"))
    return [f for f in fs if not f.name.endswith(".prompt.txt")]


def build(u, out, anims, style, picks=None, fx=True, snap=False):
    wdir = ROOT / "work" / out
    poses = unit_poses(u)
    base = base_canvas(u)
    rest = poses["idle"][0]
    picks = picks or {}
    chosen, report = {}, {}
    for name in [s for s in SHEETS if (s in anims) or (s == "flinch" and "death" in anims)]:
        geom = json.loads((wdir / f"template_{name}.geom.json").read_text())
        best = None
        for k, raw in enumerate(raw_files(wdir, name), 1):
            fr, diag = sheet.slice_sheet(raw, geom, n=len(SHEETS[name]) + 1, ref_canvas=base)
            sc = score_variant(diag, candidates(name, fr, base), poses, u["facing"], head_only=not u.get("legacy"))
            report[f"{name}_{k}"] = {"score": round(sc, 2), "k": diag["k"], "ref": diag.get("ref_redraw_dE"),
                                     "snap": diag.get("ground_snap"), "warnings": diag["warnings"]}
            if picks.get(name) == k or (name not in picks and (best is None or sc < best[0])):
                best = (sc if name not in picks else -1, k, fr)
        chosen[name] = best
        review_sheet(wdir, name, base, u, poses, style)
    # round-2 fixes from the judges: de-fringe redrawn frames (magenta keying halo), sharpened warps (crisp idle and
    # settle frames instead of bilinear blur); after the ogre test: interior de-fringe and the sideways snap
    for name in chosen:
        sc, k, fr = chosen[name]
        fr, moved = prepare(u, name, fr, poses, style, base)
        chosen[name] = (sc, k, fr)
        report[f"{name}_{k}"]["x_snap"] = moved
    warp = lambda p: rig.warp_to_pose(base, rest, p, sharpen=u.get("sharpen", 3.0))
    frames = {}
    if "idle" in anims:
        frames["idle"] = [base] + [warp(p) for p in poses["idle"][1:]]
    if "strike" in anims:
        fr = chosen["strike"][2]
        frames["strike"] = [base] + fr[1:7] + [warp(poses["strike"][7])]
    if "flinch" in anims or "death" in anims:
        ff = chosen["flinch"][2]
        flinch = ff[1:4] + [warp(poses["flinch"][3])]
        if "flinch" in anims:
            frames["flinch"] = flinch
    if "death" in anims:
        frames["death"] = [flinch[0]] + chosen["death"][2][1:8]
    if snap:
        frames = {a: [fr[0] if (a in ("idle", "strike") and i == 0) else finish.palette_snap(f) for i, f in enumerate(fr)]
                  for a, fr in frames.items()}
    for suffix, with_fx in (("_nofx", False), ("", True)):
        if with_fx and not fx:
            continue
        d = ROOT / (out + suffix)
        for a, fr in frames.items():
            fr2 = list(fr)
            if with_fx:
                if a == "strike":
                    fr2 = finish.apply_strike_fx(fr2, direction=smear_direction(u),
                                                 min_blade=0 if u.get("legacy") else 20)
                if a in ("flinch", "death"):
                    fr2 = finish.apply_hit_fx(fr2)
            write(d, a, fr2, base, u)
        (d / "unit_poses.json").write_text(json.dumps({"unit": u["name"], "facing": u["facing"], "pose_set": u["pose_set"],
                                                       "mirrored": u["mirror"], "legacy": bool(u.get("legacy")),
                                                       "anims": poses}))
    picked = {k: v[1] for k, v in chosen.items()}
    (wdir / "build_report.json").write_text(json.dumps({"picked": picked, "variants": report}, indent=1))
    return picked, report


def review_sheet(wdir, name, base, u, poses, style):
    """work/<out>/variants_<sheet>.png: every variant of one sheet, one row each, at 2x on a light ground, so a person
    or agent can pick by eye (the automatic score cannot see colour smudges or off-model faces). Frames are shown as
    they will be used (de-fringed, slid onto their guides); a cell label gives that slide and any ground snap."""
    from PIL import ImageDraw
    geom = json.loads((wdir / f"template_{name}.geom.json").read_text())
    rows = []
    for k, raw in enumerate(raw_files(wdir, name), 1):
        fr, diag = sheet.slice_sheet(raw, geom, n=len(SHEETS[name]) + 1, ref_canvas=base)
        fr, moved = prepare(u, name, fr, poses, style, base)
        snaps = diag.get("ground_snap", {})
        cells = [base] + fr[1:]
        row = Image.new("RGBA", (128 * 2 * len(cells), 128 * 2 + 40), (222, 226, 214, 255))
        d = ImageDraw.Draw(row)
        d.text((4, 4), f"VARIANT {k}   re-cut with: --rebuild --pick {name}={k}   (reference scale k={diag['k']})",
               fill=(150, 20, 20, 255))
        for i, f in enumerate(cells):
            row.alpha_composite(f.crop((ak.SAFE_X[0], 0, ak.SAFE_X[1], ak.H)).resize((256, 256), Image.NEAREST), (i * 256, 40))
            lab = "sprite (frame 0)"
            if i:
                an, fi = SHEETS[name][i - 1]
                lab = f"{an} frame {fi}"
                if moved.get(f"{an} {fi}"):
                    lab += f"  slid {moved[f'{an} {fi}']:+d}"
                sn = snaps.get(str(i), snaps.get(i, 0)) if isinstance(snaps, dict) else 0
                if sn and abs(sn) >= 4:
                    lab += f"  ground {sn:+.0f}"
            d.text((i * 256 + 4, 22), lab, fill=(30, 34, 40, 255))
        rows.append(row)
    if rows:
        out = Image.new("RGBA", (rows[0].width, sum(r.height + 6 for r in rows)), (255, 255, 255, 255))
        y = 0
        for row in rows:
            out.alpha_composite(row, (0, y))
            y += row.height + 6
        out.convert("RGB").save(wdir / f"variants_{name}.png")


def check_guides(u, out):
    """work/<out>/guides_check.png, no API calls: row 1 = the frame-0 skeleton over the sprite (bones red, the weapon
    arm and weapon yellow, joints white), then the strike guides 1-6; row 2 = the frame-0 blockout over the sprite,
    then the flinch guides 0-2; row 3 = the death guides 1-7. Every cell is labelled. Use it to check the joint
    annotation and the poses before spending anything."""
    from PIL import ImageDraw
    wdir = ROOT / "work" / out
    wdir.mkdir(parents=True, exist_ok=True)
    poses = unit_poses(u)
    style = u.get("guide_style", "grey")
    base = base_canvas(u)
    rest = poses["idle"][0]
    ground = (126, 170, 60, 255)
    # skeleton over the sprite, drawn at 2x so 1-px lines stay thin
    sk = Image.new("RGBA", (ak.W, ak.H), ground)
    sk.alpha_composite(base)
    sk = sk.resize((ak.W * 2, ak.H * 2), Image.NEAREST)
    d = ImageDraw.Draw(sk)
    for a, b in TREE:
        if a in rest and b in rest:
            col = (255, 220, 0, 255) if (a, b) in WEAPON_CHAIN else (255, 40, 40, 255)
            d.line([(rest[a][0] * 2, rest[a][1] * 2), (rest[b][0] * 2, rest[b][1] * 2)], fill=col, width=2)
    for j, v in rest.items():
        if j in JOINTS:
            d.ellipse([v[0] * 2 - 2, v[1] * 2 - 2, v[0] * 2 + 2, v[1] * 2 + 2], fill=(255, 255, 255, 255))
    # blockout over the sprite, strongly tinted
    g = guides.render_guide(rest, style)
    red = Image.new("RGBA", g.size, (255, 40, 40, 255))
    g2 = Image.blend(g, red, 0.45)
    g2.putalpha(g.getchannel("A").point(lambda a: int(a * 0.75)))
    over = Image.new("RGBA", (ak.W, ak.H), ground)
    over.alpha_composite(base)
    over.alpha_composite(g2)
    cell = lambda im: im if im.size == (ak.W * 2, ak.H * 2) else im.resize((ak.W * 2, ak.H * 2), Image.NEAREST)
    rows = [[("frame 0 skeleton", sk)] + [(f"strike {i}", guides.render_guide(poses["strike"][i], style)) for i in range(1, 7)],
            [("frame 0 blockout", over)] + [(f"flinch {i}", guides.render_guide(poses["flinch"][i], style)) for i in range(3)],
            [(f"death {i}", guides.render_guide(poses["death"][i], style)) for i in range(1, 8)]]
    W = max(len(r) for r in rows) * ak.W * 2
    sheet_im = Image.new("RGBA", (W, ak.H * 2 * len(rows)), (255, 0, 255, 255))
    dd = ImageDraw.Draw(sheet_im)
    for y, row in enumerate(rows):
        for x, (lab, f) in enumerate(row):
            sheet_im.alpha_composite(cell(f), (x * ak.W * 2, y * ak.H * 2))
            dd.text((x * ak.W * 2 + 4, y * ak.H * 2 + 3), lab, fill=(255, 255, 255, 255))
    sheet_im.save(wdir / "guides_check.png")
    short = [f"{a} {i}" for a, fr in poses.items() for i, p in enumerate(fr) if p.get("_weapon_shortened")]
    if short:
        print(f"warning: the guide weapon was shortened to fit the canvas in {', '.join(short)}; the model draws it at full "
              "length, so expect it cut off at the canvas edge there (ANIMATING_UNITS.md, Known limits)", file=sys.stderr)
    return wdir / "guides_check.png"


def write(d, anim, frames, base, u=None):
    """write_anim into an arbitrary folder (final/, final_nofx/, final_<unit>/) with this unit's frame 0; the
    manifest gets the unit's name and facing."""
    import shutil
    arm = d.name
    tmp_root = ROOT / "arms"
    ak.write_anim(arm, anim, frames, force_frame0=False)
    src = tmp_root / arm
    if src.resolve() != d.resolve():
        d.mkdir(parents=True, exist_ok=True)
        if (d / anim).exists():
            shutil.rmtree(d / anim)
        shutil.move(str(src / anim), str(d / anim))
        shutil.move(str(src / f"sheet_{anim}.png"), str(d / f"sheet_{anim}.png"))
        try:
            src.rmdir()
        except OSError:
            pass
    if u:
        mf = d / anim / "manifest.json"
        man = json.loads(mf.read_text())
        man = {"unit": u["name"], "facing": u["facing"], **{k: v for k, v in man.items() if k != "unit"}}
        mf.write_text(json.dumps(man, indent=1))


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--unit"); ap.add_argument("--out", required=True)
    ap.add_argument("--grid", metavar="SPRITE", help="only render work/<out>/grid.png: SPRITE on the animation canvas at "
                    "6x with canvas coordinates, for reading the frame-0 joints (no unit file needed)")
    ap.add_argument("--anims", nargs="+", default=["idle", "strike", "flinch", "death"])
    ap.add_argument("--rebuild", action="store_true"); ap.add_argument("--snap", action="store_true")
    ap.add_argument("--no-fx", action="store_true")
    ap.add_argument("--pick", nargs="*", default=[], help="sheet=variant, e.g. strike=2")
    ap.add_argument("--check-guides", action="store_true", help="only render work/<out>/guides_check.png (no API calls)")
    ap.add_argument("--show-beats", action="store_true", help="only print the facing, pose set and the beat text each "
                    "sheet cell will get, marking texts that name a shield, sword or blade (no API calls)")
    a = ap.parse_args()
    if a.grid:
        sp = Image.open(ROOT / a.grid).convert("RGBA")
        c = Image.new("RGBA", (ak.W, ak.H), (0, 0, 0, 0))
        c.alpha_composite(sp, (int(ak.ANCHOR[0] - sp.width / 2), ak.H - sp.height))
        (ROOT / "work" / a.out).mkdir(parents=True, exist_ok=True)
        print(ak.grid_view(c, ROOT / "work" / a.out / "grid.png"), "sprite offset on canvas:",
              (int(ak.ANCHOR[0] - sp.width / 2), ak.H - sp.height))
        sys.exit(0)
    if not a.unit:
        ap.error("--unit is required (except with --grid)")
    u = load_unit(a.unit)
    style = u.get("guide_style", "grey")
    if a.check_guides:
        print(check_guides(u, a.out))
        sys.exit(0)
    if a.show_beats:
        print(f"{u['name']}: facing {u['facing']}, pose set {u['pose_set']}{' (mirrored)' if u['mirror'] else ''}, "
              f"smear {smear_direction(u)}")
        for name, cells in SHEETS.items():
            for an, i in cells:
                t = beat_text(u, an, i)
                own = "own " if u.get("beats", {}).get(an, {}).get(str(i)) else "    "
                props = ",".join(w for w in ("shield", "sword", "blade") if w in t.lower())
                print(f"{an:6} {i}  {own}{('[' + props + '] ') if props else ''}{t}")
        sys.exit(0)
    t0 = time.time()
    if not a.rebuild:
        print(json.dumps(generate(u, a.out, a.anims, style)))
    picks = {p.split("=")[0]: int(p.split("=")[1]) for p in a.pick}
    print(json.dumps(build(u, a.out, a.anims, style, picks, fx=not a.no_fx, snap=a.snap), indent=1))
    print("seconds", round(time.time() - t0, 1))
