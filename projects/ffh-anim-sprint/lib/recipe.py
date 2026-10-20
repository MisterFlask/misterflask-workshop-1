"""The recommended recipe as one tool: a full idle/strike/flinch/death set for an M7 unit.

  python lib/recipe.py --unit units/swordsman.json --out final            # generate + build (3 model calls, n=2)
  python lib/recipe.py --unit units/swordsman.json --out final --rebuild  # re-cut from saved raws, no API calls
  python lib/recipe.py --unit units/hyborem.json --out final_hyborem --anims strike death

How it works (each choice comes from the sprint's results; see the report):
  idle    MLS mesh warp of the TRUE sprite toward each idle pose (sub-pixel breathing, no redraw, so no boil).
  strike  one guided sprite sheet (cell 1 = true sprite, cells 2-8 = blockout guides from the poses), sliced with one
          global scale; the last frame (settle) is the true sprite warped toward the settle pose, so the return to
          idle does not pop.
  flinch  guided sheet for the big frames 0-2; frame 3 (settle) warped from the true sprite.
  death   guided sheet for frames 1-7; frame 0 = flinch frame 0 (same pose by the beat sheet).
  finish  smear + spark on the strike, hit flash on flinch/death frame 0 (FX) - written to <out>/ ; the same set
          without FX goes to <out>_nofx/ for fair comparison with round-1 arms.

A unit file (JSON) gives: name, sprite path, description (one line, what the model must keep), guide style
("colour" for the swordsman's own palette, "grey" for any other unit), the unit's frame-0 joints in 1x canvas
coordinates (same keys as poses.json; omit "shield" if it has none), optional beat-text overrides, and optional
poses_from (default poses.json). Poses for other units are retargeted bone by bone: each bone keeps the unit's own
frame-0 angle plus the swordsman bone's rotation away from its rest angle, at the unit's own bone lengths.
"""
import argparse, json, math, sys, time
from pathlib import Path
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402
import beats, finish, gen, guides, metrics, rig, sheet  # noqa: E402

ROOT = ak.ROOT
TREE = [("pelvis", "chest"), ("chest", "neck"), ("neck", "head"), ("chest", "r_sh"), ("r_sh", "r_el"), ("r_el", "r_ha"),
        ("r_ha", "tip"), ("chest", "l_sh"), ("l_sh", "l_el"), ("l_el", "l_ha"), ("pelvis", "r_hip"), ("r_hip", "r_kn"),
        ("r_kn", "r_ft"), ("pelvis", "l_hip"), ("l_hip", "l_kn"), ("l_kn", "l_ft")]


# ---------------------------------------------------------------- unit + poses
def load_unit(path):
    u = json.loads(Path(path).read_text())
    u["sprite_img"] = Image.open(ROOT / u["sprite"]).convert("RGBA")
    sp = u["sprite_img"]
    u["offset"] = (int(ak.ANCHOR[0] - sp.width / 2), ak.H - sp.height)  # M7: feet 2 px above the bottom
    return u


def base_canvas(u):
    c = Image.new("RGBA", (ak.W, ak.H), (0, 0, 0, 0))
    c.alpha_composite(u["sprite_img"], u["offset"])
    return c


def retarget(u, src_rest, src_pose):
    """Unit pose = unit rest skeleton driven by the source pose's bone angles (lengths stay the unit's)."""
    rest = u["joints"]
    out = {k: v for k, v in src_pose.items() if k not in rest and k not in ("shield", "sword", "tip")}
    s = u.get("scale", 1.0)
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
        out[b] = nb
    if "shield" in rest and "shield" in src_pose:
        dx, dy = src_pose["shield"][0] - src_pose["l_ha"][0], src_pose["shield"][1] - src_pose["l_ha"][1]
        rr = rest["shield"]
        out["shield"] = (out["l_ha"][0] + dx * s, out["l_ha"][1] + dy * s, rr[2] * src_pose["shield"][2] / src_rest["shield"][2],
                         rr[3] * src_pose["shield"][3] / src_rest["shield"][3])
    out["head_tilt"] = src_pose.get("head_tilt", 0)
    out["eyes"] = src_pose.get("eyes", "open")
    out["order"] = [p for p in src_pose.get("order", ak.POSES["default_order"]) if p != "shield" or "shield" in rest]
    out["head_r"] = u.get("head_r", ak.POSES["head_r"])
    out["limb"] = u.get("limb", 1.0)
    if u.get("palette"):
        out["palette"] = u["palette"]
    return out


def unit_poses(u):
    src = json.loads((ROOT / u.get("poses_from", "poses.json")).read_text())["anims"]
    if u.get("same_as_source"):
        return src
    return {anim: [retarget(u, src["idle"][0], p) for p in frames] for anim, frames in src.items()}


def beat_text(u, anim, i):
    return u.get("beats", {}).get(anim, {}).get(str(i)) or beats.BEATS[anim][i]["text"]


# ---------------------------------------------------------------- prompt
def prompt(u, anim_label, cells, style):
    guide = gen.GUIDE_TEXT["colour" if style == "colour" else "grey"]
    lines = "\n".join(f"cell {c}: {t}" for c, t in cells)
    return (
        "The first attached image is a sprite-sheet template from my isometric fantasy strategy game: a 4x2 grid of "
        "equal cells on a flat magenta background (there are no visible grid lines). The top-left cell (cell 1) shows "
        f"the finished character, {u['description']}, drawn in {gen.STYLE}, standing in a 3/4 view facing the "
        f"lower-left. {guide} Every cell must show the SAME character: same size, same face, same outfit and colours, "
        "same pixel size and outline style, feet on the same ground line as the top-left cell. He faces the lower-left "
        f"in every cell; {u.get('hands', 'the weapon stays in the same hand as in cell 1')}. Leave cell 1 exactly as "
        f"it is. Empty cells stay empty magenta. The cells are consecutive frames of {anim_label}, read left to right, "
        f"top row then bottom row:\n{lines}\nEach cell holds exactly one figure, fully inside its cell. No grid lines, "
        "frames, numbers, text, ground, shadows, motion lines or effects. Background: flat solid pure magenta #FF00FF "
        "filling the whole image, with no gradient. The second attached image is a roster sheet of other units from "
        "the same game: match its outline colour, shading and pixel size exactly.")


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


def score_variant(diag, cand):
    """Lower is better: gate flags (x100), reference-redraw colour error, scale error, size of ground snaps.
    cand = {anim: frames with the right frame indices} for the gate check."""
    g = sum(len(metrics.gates(a, fr)) for a, fr in cand.items())
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
            sc = score_variant(diag, candidates(name, fr, base))
            report[f"{name}_{k}"] = {"score": round(sc, 2), "k": diag["k"], "ref": diag.get("ref_redraw_dE"),
                                     "snap": diag.get("ground_snap"), "warnings": diag["warnings"]}
            if picks.get(name) == k or (name not in picks and (best is None or sc < best[0])):
                best = (sc if name not in picks else -1, k, fr)
        chosen[name] = best
    frames = {}
    if "idle" in anims:
        frames["idle"] = [base] + [rig.warp_to_pose(base, rest, p) for p in poses["idle"][1:]]
    if "strike" in anims:
        fr = chosen["strike"][2]
        frames["strike"] = [base] + fr[1:7] + [rig.warp_to_pose(base, rest, poses["strike"][7])]
    if "flinch" in anims or "death" in anims:
        ff = chosen["flinch"][2]
        flinch = ff[1:4] + [rig.warp_to_pose(base, rest, poses["flinch"][3])]
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
                    fr2 = finish.apply_strike_fx(fr2)
                if a in ("flinch", "death"):
                    fr2 = finish.apply_hit_fx(fr2)
            write(d, a, fr2, base)
    picked = {k: v[1] for k, v in chosen.items()}
    (wdir / "build_report.json").write_text(json.dumps({"picked": picked, "variants": report}, indent=1))
    return picked, report


def write(d, anim, frames, base):
    """write_anim into an arbitrary folder (final/, final_nofx/, final_<unit>/) with this unit's frame 0."""
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


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--unit", required=True); ap.add_argument("--out", required=True)
    ap.add_argument("--anims", nargs="+", default=["idle", "strike", "flinch", "death"])
    ap.add_argument("--rebuild", action="store_true"); ap.add_argument("--snap", action="store_true")
    ap.add_argument("--no-fx", action="store_true")
    ap.add_argument("--pick", nargs="*", default=[], help="sheet=variant, e.g. strike=2")
    a = ap.parse_args()
    u = load_unit(a.unit)
    style = u.get("guide_style", "grey")
    t0 = time.time()
    if not a.rebuild:
        print(json.dumps(generate(u, a.out, a.anims, style)))
    picks = {p.split("=")[0]: int(p.split("=")[1]) for p in a.pick}
    print(json.dumps(build(u, a.out, a.anims, style, picks, fx=not a.no_fx, snap=a.snap), indent=1))
    print("seconds", round(time.time() - t0, 1))
