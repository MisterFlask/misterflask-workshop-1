"""M7 art mode: reference-anchored gpt-image-2.5-sunburst sprites for the FfH port.

Every generating command fires N concurrent single-image edit calls (default 3, medium quality),
cuts each variant out on the spec canvas, writes work/<name>/review_<round>.png (variants enlarged +
each variant placed on a small test map) and logs prompt + ordered images + cost to
work/<name>/calls.jsonl. You then LOOK at the review sheet and accept one variant.

  python m7.py unit     --name unit_lich --height 90 --desc "a LICH from Fall from Heaven: ..."
  python m7.py building --name bld_mage_guild --footprint 1 --desc "the Mage Guild, ..." [--ref ...]
  python m7.py feature  --name feat_jungle --desc "Paint ONE NEW sprite in exactly this art style: ..."
  python m7.py tile     --name tile_desert --mode swatch --desc "Paint ONE NEW image: a seamless ..." --skirt 214,176,108
  python m7.py tile     --name tile_pasture --mode inset --base roster/terrain/tile_grass.png --desc "..."
  python m7.py unit     --name unit_lich --round r2 --revise r1:2 --desc "..., redrawn as a cleaner version of the last image ..."
  python m7.py recut    --name unit_lich --round r1 --height 94          (re-process raws, no API cost)
  python m7.py retile   --name tile_desert --round r1 --k 2.2 --p 2      (re-run seamless-tile maths, no API cost)
  python m7.py accept   --name unit_lich --variant 2 [--round r1] [--mirror]
  python m7.py roster-sheet --out anchors/anchor_roster_elves.png roster/units/unit_a.png roster/units/unit_b.png ...
  python m7.py scene    [--layout layouts/layout_ref.json] [--out work/scene.png]
  python m7.py spend
  python m7.py pair --name tile_desert   (make tile_desert_b tile with tile_desert; no API cost)
  python m7.py export   (the roster -> the game's isometric renderer, crates/ffh-play/assets/app/iso)
Add --dry-run to any generating command to print the prompt and image list without calling the API.
Needs OPENAI_API_KEY; budget caps live in budget.json (tag root must be listed there).
"""
import argparse, json, shutil, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent
LIB = ROOT / "lib"
sys.path.insert(0, str(LIB))
from PIL import Image, ImageDraw, ImageOps  # noqa: E402
import cutout as co  # noqa: E402
import tiles  # noqa: E402
import cleanup  # noqa: E402
import compose  # noqa: E402

MODEL = "gpt-image-2.5-sunburst"
GRASS = (126, 170, 60)
ROSTER_EXTRA = ("Match the outline colour, shading and pixel size of the attached roster sheet exactly "
                "(it shows the other units of this same game).")
# Without this the model draws walled and fenced 1-tile footprints at about 1.4:1, not 2:1 (base cover 0.60-0.71);
# with it, 0.85-0.96 on the first try (the ffh2-rs buildings batch, Oct 2026).
CAMERA_CHECK = "CAMERA CHECK: the footprint is a wide, FLAT 2:1 isometric diamond exactly like the ground tiles of the attached map, twice as wide as it is deep, so its left and right corners sit only one quarter of the image width above its bottom corner and the front edges rise gently (about 27 degrees); not a steeper top-down square."
GEOMETRY = {
    1: ("The base of the building is an isometric diamond whose bottom corner touches the bottom centre of the image "
        "and whose left and right corners touch the left and right image edges; no part (canopy, flags, smoke) is wider "
        "than the base diamond. Tall composition, nothing cropped. " + CAMERA_CHECK),
    2: ("The whole complex sits on a 2x2 isometric diamond footprint: its bottom corner touches the bottom centre of the "
        "image and its left and right corners touch the left and right image edges."),
}
MAGENTA_BG = "Background: flat solid pure magenta #FF00FF filling the whole image."
KIND_DIR = {"unit": "units", "building": "buildings", "city": "buildings", "feature": "terrain", "tile": "terrain"}
KEY_NAMES = {"#ff00ff": "magenta", "#00ffff": "cyan", "#ffff00": "yellow", "#0000ff": "blue", "#00ff00": "green"}
POST_KEYS = ("height", "width", "pixel", "footprint", "mode", "k", "p", "m", "skirt", "seed", "cx", "cy", "base",
             "obj_width", "bottom", "lift", "shadow", "edge_colour", "key", "holes", "size", "quality", "model")


def apply_key(prompt, key):
    """Swap the background colour named in a prompt (all M7 prompts ask for flat pure magenta #FF00FF)."""
    key = key.lower()
    if key == "#ff00ff":
        return prompt
    name = KEY_NAMES.get(key, "solid")
    return prompt.replace("pure magenta #FF00FF", f"pure {name} {key.upper()}").replace("#FF00FF", key.upper())


def is_magenta(job):
    return job.get("key", "#ff00ff").lower() == "#ff00ff"


# ---------------------------------------------------------------- helpers
def res(p):
    p = Path(p)
    if p.is_absolute():
        return p
    return p if p.exists() else ROOT / p


def style(name):
    return (ROOT / "prompts" / f"style_{name}.txt").read_text(encoding="utf-8").strip()


def wdir(name):
    d = ROOT / "work" / name
    for s in ("raw", "cut"):
        (d / s).mkdir(parents=True, exist_ok=True)
    return d


def call_api(job, k, quality, prompt_file, images, raw):
    rel = lambda q: (str(Path(q).relative_to(ROOT)) if str(q).startswith(str(ROOT)) else str(q)).replace("\\", "/")
    cmd = [sys.executable, str(LIB / "imggen.py"), "edit", "--tag", job["tag"], "--model", job["model"],
           "--quality", quality, "--size", job["size"], "--prompt-file", rel(prompt_file), "--out", rel(raw)]
    for im in images:
        cmd += ["--image", rel(im)]
    p = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
    try:
        out = json.loads(p.stdout.strip().splitlines()[-1])
    except Exception:
        out = {"ok": False, "error": (p.stdout + p.stderr)[-600:]}
    return out


def log_call(d, row):
    with open(d / "calls.jsonl", "a", encoding="utf-8") as f:
        f.write(json.dumps(row) + "\n")


# ---------------------------------------------------------------- post-processing per kind
def keyed(raw, job):
    ks = {}
    im = co.remove_bg(Image.open(raw).convert("RGBA"), job.get("key", "#ff00ff"), job.get("tol", 48), job.get("holes", 30), ks)
    return co.trim(co.clean_specks(im)), ks


def clean(im, kind, job, **kw):
    if not is_magenta(job):
        return im, {"removed": 0, "recoloured": 0}
    return cleanup.clean(im, kind, **kw)


def cut_unit(raw, out, job):
    im, ks = keyed(raw, job)
    im = co.scale_to(im, h=job["height"])
    im = co.pixelate(im, job.get("pixel", 1))
    im = co.place(im, 96, 120, bottom_pad=2)
    im, st = clean(im, "unit", job)
    im = cleanup.refit(im, "unit")
    im.save(out)
    st.update(ks)
    return st


def cut_building(raw, out, job):
    kind = "city" if job["footprint"] == 2 else "building"
    im, ks = keyed(raw, job)
    im = co.scale_to(im, w=job["width"])
    im = co.pixelate(im, job.get("pixel", 2))
    im = co.place(im, 384 if kind == "city" else 192, im.height)
    im, st = clean(im, "building", job)
    st.update(ks)
    im = cleanup.refit(im, kind)
    im.save(out)
    st.update(geometry_building(im, job["footprint"]))
    return st


def cut_feature(raw, out, job):
    im, ks = keyed(raw, job)
    im = co.scale_to(im, w=job["width"])
    im = co.pixelate(im, job.get("pixel", 2))
    im = co.place(im, 192, im.height)
    tmp = Path(str(out) + ".tmp.png")
    im.save(tmp)
    tiles.lift_shadow(tmp, tmp, job.get("lift", 6), 4, 3, job.get("shadow", 0.35))
    im, st = clean(Image.open(tmp), "feature", job, edge_colour=tuple(job.get("edge_colour", (60, 96, 36))))
    im.save(out)
    tmp.unlink()
    st["leftover_magenta"] = cleanup.magenta_report(im)
    st.update(ks)
    return st


def cut_tile(raw, out, job):
    if job["mode"] == "swatch":
        tiles.swatch_to_tile(raw, out, k=job["k"], p=job["p"], m=job.get("m", 0.25), skirt=job["skirt"],
                             seed=job.get("seed", 1), cx=job.get("cx", 0.5), cy=job.get("cy", 0.5))
        return {"k": job["k"], "p": job["p"], "seed": job.get("seed", 1), "cx": job.get("cx", 0.5), "cy": job.get("cy", 0.5)}
    w, h, x, y = tiles.inset_tile(raw, res(job["base"]), out, job["obj_width"], job["bottom"], job["p"], job["skirt"],
                                  key=job.get("key", "#ff00ff"))
    return {"placed": [w, h, x, y], "top_cropped": y < 0}


CUT = {"unit": cut_unit, "building": cut_building, "feature": cut_feature, "tile": cut_tile}


def geometry_building(im, fp):
    """Checks from the sprint: bottom row centred on the anchor, front edges on the footprint diamond, base coverage."""
    import numpy as np
    a = np.asarray(im)[..., 3] > 8
    H, W = a.shape
    rows = np.nonzero(a.any(1))[0]
    bottom = int(rows.max()) if len(rows) else -1
    xs = np.nonzero(a[bottom])[0] if bottom >= 0 else []
    half = 96 * fp
    lr_row = H - half // 2  # row of the footprint's left/right vertices
    span = np.nonzero(a[max(0, lr_row)])[0] if 0 <= lr_row < H else []
    cover = (span.max() - span.min() + 1) / (2 * half) if len(span) else 0
    return {"bottom_row": bottom, "height": H, "bottom_centre_x": float((xs.min() + xs.max()) / 2) if len(xs) else None,
            "anchor_x": W / 2, "base_cover": round(float(cover), 2),
            "warn": [w for w in [
                "sprite floats above anchor" if bottom != H - 1 else "",
                (f"bottom corner {abs((xs.min() + xs.max()) / 2 - W / 2):.1f} px off the anchor: pick another variant or recut"
                 if len(xs) and abs((xs.min() + xs.max()) / 2 - W / 2) > 3 else ""),
                "base covers < 85% of footprint width: raise --width or re-prompt" if cover < 0.85 else "",
                f"height {H} over limit" if H > (256 if fp == 1 else 320) else ""] if w]}


# ---------------------------------------------------------------- review sheet
def scene_for(job, cand):
    """A small test map with the candidate placed next to accepted roster assets."""
    name, kind = job["name"], job["kind"]
    terrain = ["GGFG", "GGGG", "GHGW", "GGWW"]
    feats = ["....", "....", "....", "...."]
    L = {"terrain": terrain, "features": feats, "buildings": [], "units": [], "water_drop": 8, "clouds": False,
         "terrain_codes": {}, "feature_codes": {}}
    if kind == "unit":
        L["units"] = [{"rc": [1, 1], "type": "swordsman"}, {"rc": [1, 2], "type": name[5:], "highlight": True, "banner": "purple"},
                      {"rc": [2, 2], "type": "hyborem", "banner": "red"}]
    elif kind == "building":
        fp = job["footprint"]
        L["buildings"] = [{"rc": [1, 1], "type": name[4:], "footprint": fp}]
        L["units"] = [{"rc": [3, 0], "type": "swordsman"}]
        L["features"] = ["f...", "....", "....", "...."]
    elif kind == "feature":
        L["feature_codes"] = {"x": name}
        L["features"] = [".x..", "xx..", "..f.", "...."]
    else:
        L["terrain_codes"] = {"X": name}
        L["terrain"] = ["GXXG", "XXXG", "GXGW", "GGWW"] if job["mode"] == "swatch" else ["GGXG", "GXGG", "XGGW", "GGWW"]
    assets = compose.Assets([ROOT / "roster"], {name: str(cand)})
    return compose.render(L, assets, margin_top=200, decorate=False)


def review(job, d, rnd, cuts):
    tiles_ = []
    sc = 3 if job["kind"] == "unit" else 2
    refs = {"unit": ["units/unit_swordsman", "units/unit_hyborem"], "tile": ["terrain/tile_grass", "terrain/tile_farm"],
            "feature": ["terrain/feat_forest"], "building": []}[job["kind"]]
    for ref in refs:
        p = ROOT / "roster" / f"{ref}.png"
        if p.exists():
            tiles_.append(("roster " + ref.split("/")[-1], Image.open(p).convert("RGBA")))
    for k, c in cuts:
        tiles_.append((f"v{k}", Image.open(c).convert("RGBA")))

    def scale_of(lab):
        return sc
    big = [(lab, im.resize((im.width * scale_of(lab), im.height * scale_of(lab)), Image.NEAREST)) for lab, im in tiles_]
    scenes = []
    for k, c in cuts:
        s = scene_for(job, c)
        import numpy as np
        arr = np.asarray(s.convert("RGB")).astype(int)
        ys, xs = np.nonzero(np.abs(arr - np.array([40, 40, 48])).sum(-1) > 12)
        s = s.crop((max(0, xs.min() - 8), max(0, ys.min() - 8), xs.max() + 8, ys.max() + 8))
        scenes.append((f"v{k} on the test map", s))
    W = max(sum(i.width + 16 for _, i in big) + 16, sum(i.width + 16 for _, i in scenes) + 16)
    H = max(i.height for _, i in big) + max(i.height for _, i in scenes) + 90
    sheet = Image.new("RGBA", (W, H), (40, 44, 52, 255))
    dr = ImageDraw.Draw(sheet)
    x, top = 16, 30
    row_h = max(i.height for _, i in big)
    for lab, im in big:
        bg = Image.new("RGBA", im.size, GRASS + (255,)); bg.alpha_composite(im)
        sheet.alpha_composite(bg, (x, top + row_h - im.height))
        dr.text((x, top - 18), lab, fill=(255, 255, 255, 255), font=compose.font(14))
        x += im.width + 16
    x, top2 = 16, top + row_h + 40
    for lab, im in scenes:
        sheet.alpha_composite(im, (x, top2))
        dr.text((x, top2 - 18), lab, fill=(255, 255, 255, 255), font=compose.font(14))
        x += im.width + 16
    out = d / f"review_{rnd}.png"
    sheet.convert("RGB").save(out)
    return out


# ---------------------------------------------------------------- generate
def build_job(a, kind):
    job = {"kind": kind, "name": a.name, "model": a.model, "quality": a.quality, "n": a.n,
           "tag": a.tag or f"m7/{KIND_DIR[kind]}", "round": a.round, "desc": a.desc, "extra": a.extra or "",
           "refs": [str(r) for r in a.ref], "key": a.key.lower(), "holes": a.holes}
    images = []
    if kind == "unit":
        job.update(height=a.height, pixel=a.pixel, size=a.size or "1024x1024")
        images = [] if a.no_anchor else [ROOT / "anchors" / "anchor_units.png"]
        roster = None if a.no_roster else res(a.roster or "anchors/anchor_roster_r1.png")
        if roster:
            images.append(roster)
        images += [res(r) for r in a.ref]
        extra = a.extra if a.extra is not None else (ROSTER_EXTRA if roster else "")
        prompt = style("unit") + " " + a.desc + (" " + extra if extra else "")
    elif kind == "building":
        fp = a.footprint
        job.update(footprint=fp, width=a.width or (372 if fp == 2 else 184), pixel=a.pixel,
                   size=a.size or ("1536x1024" if fp == 2 else "1024x1024"))
        refs = a.ref or (["ref/crop_city.png", "anchors/buildings/crop_houses.png"] if fp == 2
                         else ["anchors/buildings/crop_houses.png", "anchors/buildings/crop_keep.png"])
        images = [res(r) for r in refs]
        if not a.no_house_style:
            images.append(ROOT / "anchors" / "buildings" / "house_style_city_raw.png")
        prompt = style("building") + " " + a.desc + ("" if a.no_geometry else " " + GEOMETRY[fp]) + (" " + a.extra if a.extra else "")
    elif kind == "feature":
        job.update(width=a.width or 186, pixel=a.pixel, lift=a.lift, shadow=a.shadow, size=a.size or "1024x1024",
                   edge_colour=[int(v) for v in a.edge_colour.split(",")])
        refs = a.ref or ["ref/crop_forest.png", "ref/crop_mountains.png", "anchors/terrain/anchor_mapmid.png"]
        images = [res(r) for r in refs]
        prompt = style("terrain") + " " + a.desc + ("" if "#FF00FF" in a.desc.upper() else " " + MAGENTA_BG)
    else:  # tile
        job.update(mode=a.mode, k=a.k, p=a.p if a.p is not None else (3 if a.mode == "swatch" else 2), m=0.25, skirt=a.skirt,
                   base=a.base, obj_width=a.obj_width, bottom=a.bottom, seed=a.seed, cx=a.cx, cy=a.cy,
                   size=a.size or ("1024x1024" if a.mode == "swatch" else "1536x1024"))
        refs = a.ref or (["ref/crop_forest.png", "ref/crop_farm_fields.png", "anchors/terrain/anchor_mapmid.png"] if a.mode == "swatch"
                         else ["ref/crop_farm_fields.png", "ref/crop_forest.png", "anchors/terrain/anchor_mapmid.png"])
        images = [res(r) for r in refs]
        prompt = style("terrain") + " " + a.desc
        if a.mode == "inset" and "#FF00FF" not in a.desc.upper():
            prompt += " " + MAGENTA_BG
    prompt = apply_key(prompt, a.key)
    if a.revise:
        rr, kk = a.revise.split(":")
        images.append(ROOT / "work" / a.name / "raw" / f"{a.name}_{rr}_{kk}.png")
    for im in images:
        if not Path(im).exists():
            raise SystemExit(f"missing image: {im}")
    job["images"] = [(str(Path(i).relative_to(ROOT)) if str(i).startswith(str(ROOT)) else str(i)).replace("\\", "/") for i in images]
    return job, prompt, images


def generate(a, kind):
    job, prompt, images = build_job(a, kind)
    if a.dry_run:
        print(json.dumps({"prompt": prompt, "images": job["images"], "job": job}, indent=1)); return
    d = wdir(a.name)
    pfile = d / f"prompt_{a.round}.txt"
    pfile.write_text(prompt, encoding="utf-8")
    (d / f"job_{a.round}.json").write_text(json.dumps(job, indent=1), encoding="utf-8")
    t0 = time.time()
    plan = [(k, a.quality) for k in range(1, a.n + 1)] + ([(a.n + 1, "high")] if a.also_high else [])

    def one(kq):
        k, q = kq
        raw = d / "raw" / f"{a.name}_{a.round}_{k}.png"
        r = call_api(job, k, q, pfile, images, raw)
        log_call(d, {"t": int(time.time()), "round": a.round, "k": k, "quality": q, "size": job["size"],
                     "images": job["images"], "prompt_file": pfile.name, "usd": r.get("usd"), "latency": r.get("latency"),
                     "ok": r.get("ok"), "error": r.get("error")})
        if not r.get("ok"):
            return k, None, r
        cut = d / "cut" / f"{a.name}_{a.round}_{k}.png"
        r["post"] = CUT[kind](raw, cut, job)
        return k, cut, r

    with ThreadPoolExecutor(max_workers=len(plan)) as ex:
        results = list(ex.map(one, plan))
    cuts = [(k, c) for k, c, _ in results if c]
    for k, c, r in results:
        print(json.dumps({"variant": k, "cut": str(c) if c else None, "usd": r.get("usd"), "latency": r.get("latency"),
                          "post": r.get("post"), "error": r.get("error")}))
    if cuts:
        print("review", review(job, d, a.round, cuts))
    print(json.dumps({"seconds": round(time.time() - t0, 1)}))


def reprocess(a):
    d = ROOT / "work" / a.name
    job = json.loads((d / f"job_{a.round}.json").read_text())
    print("note: overwrites this round's cut/*.png, job and review sheet")
    for key in ("height", "width", "pixel", "k", "p", "skirt", "obj_width", "bottom", "lift", "shadow", "seed", "cx", "cy", "holes"):
        v = getattr(a, key, None)
        if v is not None:
            job[key] = v
    cuts = []
    for raw in sorted((d / "raw").glob(f"{a.name}_{a.round}_*.png")):
        k = raw.stem.rsplit("_", 1)[-1]
        cut = d / "cut" / f"{a.name}_{a.round}_{k}.png"
        print(json.dumps({"variant": k, "post": CUT[job["kind"]](raw, cut, job)}))
        cuts.append((k, cut))
    (d / f"job_{a.round}.json").write_text(json.dumps(job, indent=1))
    print("review", review(job, d, a.round, cuts))


def accept(a):
    d = ROOT / "work" / a.name
    rounds = sorted(p.stem[4:] for p in d.glob("job_*.json"))
    if not rounds:
        raise SystemExit(f"nothing generated for {a.name} yet (no {d}/job_*.json)")
    rnd = a.round or rounds[-1]
    job = json.loads((d / f"job_{rnd}.json").read_text())
    key = a.as_ or a.name
    src = d / "cut" / f"{a.name}_{rnd}_{a.variant}.png"
    if not src.exists():
        raise SystemExit(f"no such variant: {src}")
    dst = ROOT / "roster" / KIND_DIR[job["kind"]] / f"{key}.png"
    dst.parent.mkdir(parents=True, exist_ok=True)
    im = Image.open(src).convert("RGBA")
    if a.mirror:
        im = ImageOps.mirror(im)
    im.save(dst)
    pa = ROOT / "prompts" / "assets"
    pa.mkdir(parents=True, exist_ok=True)
    if (d / f"prompt_{rnd}.txt").exists():
        shutil.copy(d / f"prompt_{rnd}.txt", pa / f"{key}.txt")
    else:
        print(f"warning: no prompt_{rnd}.txt in {d} - prompt not saved")
    images = []
    for i in job["images"]:  # keep revision anchors (work/<name>/raw/...) with the prompt so work/ can be deleted
        if i.startswith("work/"):
            keep = pa / f"{key}__{Path(i).name}"
            shutil.copy(ROOT / i, keep)
            i = str(keep.relative_to(ROOT)).replace("\\", "/")
        images.append(i)
    man_p = ROOT / "roster" / "manifest.json"
    man = json.loads(man_p.read_text()) if man_p.exists() else {}
    entry = {"kind": job["kind"], "file": str(dst.relative_to(ROOT)).replace("\\", "/"), "size": list(im.size),
             "anchor": {"unit": "canvas bottom-centre on tile centre", "tile": "canvas top-left = tile top-left (192x128, diamond rows 0-95)",
                        "building": "canvas bottom-centre on bottom vertex of front-most footprint tile",
                        "feature": "canvas bottom-centre on tile bottom vertex"}[job["kind"]],
             "round": rnd, "variant": int(a.variant), "mirrored": bool(a.mirror), "images": images,
             "prompt": f"prompts/assets/{key}.txt", "accepted": time.strftime("%Y-%m-%d %H:%M")}
    for k in POST_KEYS:
        if job.get(k) is not None:
            entry["gen_size" if k == "size" else k] = job[k]  # "size" stays the sprite's own
    man[key] = entry
    if job["kind"] == "tile" and job.get("mode") == "inset":
        # The object layer too (lib/tiles.py inset_tile): tile_farm -> ovl_farm, drawn by the game over any base.
        ov_src = tiles.overlay_path(src)
        if ov_src.exists():
            okey = "ovl_" + (key[5:] if key.startswith("tile_") else key)
            ov_dst = dst.with_name(f"{okey}.png")
            ov = Image.open(ov_src).convert("RGBA")
            (ImageOps.mirror(ov) if a.mirror else ov).save(ov_dst)
            man[okey] = dict(entry, kind="overlay", file=str(ov_dst.relative_to(ROOT)).replace("\\", "/"),
                             anchor="canvas top-left = tile top-left (192x128); the object only, clipped to the diamond",
                             tile=key)
            print(json.dumps({"overlay": str(ov_dst)}))
    tmp = man_p.with_suffix(".tmp")
    tmp.write_text(json.dumps(man, indent=1))
    tmp.replace(man_p)  # atomic; still: use one package folder per concurrent session
    print(json.dumps({"accepted": str(dst), **entry}, indent=1))


def roster_sheet(a):
    ims = []
    for p in a.files:
        im = Image.open(res(p)).convert("RGBA")
        im = im.crop(im.getchannel("A").getbbox())
        ims.append(im.resize((im.width * 4, im.height * 4), Image.NEAREST))
    pad = 24
    W = sum(i.width for i in ims) + pad * (len(ims) + 1)
    H = max(i.height for i in ims) + 2 * pad
    sheet = Image.new("RGBA", (W, H), (255, 0, 255, 255))
    x = pad
    for i in ims:
        sheet.alpha_composite(i, (x, H - pad - i.height)); x += i.width + pad
    out = res(a.out) if Path(a.out).is_absolute() else ROOT / a.out
    sheet.convert("RGB").save(out)
    print(out, sheet.size)


def scene(a):
    layout = json.load(open(res(a.layout)))
    assets = compose.Assets([ROOT / "roster"] + [res(x) for x in a.extra_assets], {})
    s = compose.render(layout, assets)
    missing = [k for k, src in assets.source.items() if "placeholders" in src.replace("\\", "/")]
    if missing:
        print("WARNING: not found in roster/ or --extra-assets, drawn as placeholders:", ", ".join(sorted(missing)))
    out = ROOT / a.out
    out.parent.mkdir(parents=True, exist_ok=True)
    s.convert("RGB").save(out)
    print(out, s.size)


def pair(a):
    """Makes roster tile <name>_b (or --variant) tile with <name>: its border band becomes <name>'s
    (lib/tiles.py pair_variant). Variants cut from other windows of a raw are each seamless only with
    themselves; paired, the engine can mix them freely. Run once per variant, after accepting it."""
    man_p = ROOT / "roster" / "manifest.json"
    man = json.loads(man_p.read_text())
    base = ROOT / "roster" / "terrain" / f"{a.name}.png"
    var = a.variant or f"{a.name}_b"
    vp = ROOT / "roster" / "terrain" / f"{var}.png"
    if man.get(var, {}).get("paired_with"):
        raise SystemExit(f"{var} is already paired with {man[var]['paired_with']}; re-accept it first")
    px = man.get(var, {}).get("p") or man.get(a.name, {}).get("p") or 3
    n = tiles.pair_variant(base, vp, vp, p=int(px))
    man.setdefault(var, {})["paired_with"] = a.name
    man_p.write_text(json.dumps(man, indent=1))
    print(json.dumps({"paired": var, "with": a.name, "own_px": n, "p": px}))


GAME_ISO = ROOT.parent.parent / "crates" / "ffh-play" / "assets" / "app" / "iso"


def export(a):
    """Copies the roster into the game client (the isometric renderer's sprites, crates/ffh-play/assets/app/iso)
    and writes its index, iso/sprites.js. Inset tiles go as their overlays (ovl_*), not baked on their base."""
    import numpy as np
    out = Path(a.out) if a.out else GAME_ISO
    out.mkdir(parents=True, exist_ok=True)
    man = json.loads((ROOT / "roster" / "manifest.json").read_text())
    baked = {k for k, e in man.items() if e.get("kind") == "tile" and e.get("mode") == "inset"}
    index = {}
    for png in sorted((ROOT / "roster").glob("*/*.png")):
        key = png.stem
        if key in baked:
            continue
        im = Image.open(png).convert("RGBA")
        im.save(out / f"{key}.png", optimize=True)
        ys, xs = np.nonzero(np.asarray(im)[..., 3] > 8)
        box = [int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1] if len(xs) else [0, 0, im.width, im.height]
        index[key] = {"w": im.width, "h": im.height, "box": box}
    stale = [p for p in out.glob("*.png") if p.stem not in index]
    for p in stale:
        p.unlink()
    lines = ["// The isometric renderer's sprites (iso.js): key -> { w, h, box } in art px, box = the opaque bounds",
             "// [x0, y0, x1, y1]. Written by `python3 tools/m7/m7.py export` from tools/m7/roster; do not edit.",
             "export const SPRITES = {"]
    lines += [f'  {k}: {{ w: {v["w"]}, h: {v["h"]}, box: [{", ".join(map(str, v["box"]))}] }},' for k, v in index.items()]
    lines.append("};")
    (out / "sprites.js").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(json.dumps({"out": str(out), "sprites": len(index), "removed": [p.name for p in stale]}))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    def gen_args(p):
        p.add_argument("--name", required=True, help="asset key: unit_*, bld_*, feat_*, tile_*")
        p.add_argument("--desc", help="subject paragraph (see README templates)"); p.add_argument("--desc-file")
        p.add_argument("--extra", default=None); p.add_argument("--ref", action="append", default=[])
        p.add_argument("--n", type=int, default=3); p.add_argument("--quality", default="medium")
        p.add_argument("--also-high", action="store_true", help="add one high-quality call")
        p.add_argument("--size"); p.add_argument("--round", default="r1"); p.add_argument("--revise", help="ROUND:K raw to append as LAST image")
        p.add_argument("--tag"); p.add_argument("--model", default=MODEL); p.add_argument("--dry-run", action="store_true")
        p.add_argument("--key", default="#ff00ff", help="background key colour; use #00ffff for magenta/pink-heavy art")
        p.add_argument("--holes", type=int, default=30, help="clear enclosed background pockets >= N raw px (0 = off)")

    p = sub.add_parser("unit"); gen_args(p)
    p.add_argument("--height", type=int, default=86); p.add_argument("--pixel", type=int, default=1)
    p.add_argument("--roster"); p.add_argument("--no-roster", action="store_true"); p.add_argument("--no-anchor", action="store_true")
    p = sub.add_parser("building"); gen_args(p)
    p.add_argument("--footprint", type=int, choices=[1, 2], default=1); p.add_argument("--width", type=int)
    p.add_argument("--pixel", type=int, default=2); p.add_argument("--no-house-style", action="store_true"); p.add_argument("--no-geometry", action="store_true")
    p = sub.add_parser("feature"); gen_args(p)
    p.add_argument("--width", type=int); p.add_argument("--pixel", type=int, default=2)
    p.add_argument("--lift", type=int, default=6); p.add_argument("--shadow", type=float, default=0.35)
    p.add_argument("--edge-colour", default="60,96,36")
    p = sub.add_parser("tile"); gen_args(p)
    p.add_argument("--mode", choices=["swatch", "inset"], required=True)
    p.add_argument("--k", type=float, default=1.8); p.add_argument("--p", type=int, default=None, help="pixel block: swatch 3, inset 2")
    p.add_argument("--seed", type=int, default=1); p.add_argument("--cx", type=float, default=0.5); p.add_argument("--cy", type=float, default=0.5)
    p.add_argument("--skirt", default="earth", help="earth | water | none | 'r,g,b' body colour")
    p.add_argument("--base", default="roster/terrain/tile_grass.png"); p.add_argument("--obj-width", type=int, default=168)
    p.add_argument("--bottom", type=int, default=92)
    for nm in ("recut", "retile"):
        p = sub.add_parser(nm); p.add_argument("--name", required=True); p.add_argument("--round", default="r1")
        for key, typ in (("--height", int), ("--width", int), ("--pixel", int), ("--k", float), ("--p", int), ("--skirt", str),
                         ("--obj-width", int), ("--bottom", int), ("--lift", int), ("--shadow", float), ("--seed", int),
                         ("--cx", float), ("--cy", float), ("--holes", int)):
            p.add_argument(key, type=typ)
    p = sub.add_parser("accept"); p.add_argument("--name", required=True); p.add_argument("--variant", required=True)
    p.add_argument("--as", dest="as_", help="save under another key, e.g. a terrain variant tile_desert_b")
    p.add_argument("--round"); p.add_argument("--mirror", action="store_true")
    p = sub.add_parser("roster-sheet"); p.add_argument("--out", required=True); p.add_argument("files", nargs="+")
    p = sub.add_parser("scene"); p.add_argument("--layout", default="layouts/layout_ref.json")
    p.add_argument("--out", default="work/scene.png"); p.add_argument("--extra-assets", nargs="*", default=[])
    sub.add_parser("spend")
    p = sub.add_parser("pair"); p.add_argument("--name", required=True, help="the base tile, e.g. tile_desert")
    p.add_argument("--variant", help="default: <name>_b")
    p = sub.add_parser("export"); p.add_argument("--out", help="default: the game's crates/ffh-play/assets/app/iso")
    a = ap.parse_args()

    if a.cmd in ("unit", "building", "feature", "tile"):
        if a.desc_file:
            a.desc = res(a.desc_file).read_text(encoding="utf-8").strip()
        if not a.desc:
            raise SystemExit("--desc or --desc-file is required")
        generate(a, a.cmd)
    elif a.cmd in ("recut", "retile"):
        reprocess(a)
    elif a.cmd == "accept":
        accept(a)
    elif a.cmd == "roster-sheet":
        roster_sheet(a)
    elif a.cmd == "scene":
        scene(a)
    elif a.cmd == "pair":
        pair(a)
    elif a.cmd == "export":
        export(a)
    elif a.cmd == "spend":
        subprocess.run([sys.executable, str(LIB / "imggen.py"), "spend"])


if __name__ == "__main__":
    main()
