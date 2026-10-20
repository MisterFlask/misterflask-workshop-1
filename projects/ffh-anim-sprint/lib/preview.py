"""Review images, animated previews and the full-sequence GIF/MP4 deliverables.

  python lib/preview.py --arm G1                 # everything for one arm (whatever animations exist)
  python lib/preview.py --arm G1 --only sequence # just sequence_{3x,1x}.{gif,mp4}
  python lib/preview.py --vignette G1            # arms/G1/vignette.{gif,mp4}: A strikes, B flinches, A strikes, B dies

Per animation (arms/<arm>/preview/): <anim>_strip3x.png (ms labels, HIT marker), <anim>_map1x.png (frames on a grass
tile with shadow + banner), <anim>_sil.png (silhouettes), <anim>_onion.png (onion skin + sword-tip trail),
<anim>_diff.png (frame-to-frame change heat), <anim>_1x.gif / <anim>_3x.gif.
Per arm: sequence_3x.{gif,mp4} (one tile, 3x NEAREST) and sequence_1x.{gif,mp4} (3x3 grass map at 1x; the MP4 is
stored at 2x NEAREST so H.264's 4:2:0 chroma doesn't smear single pixels). Sequence = idle x2 -> strike -> idle ->
flinch -> idle -> death -> 1.5 s corpse hold, using the manifest timings.
"""
import argparse, hashlib, json, subprocess, sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402
import metrics  # noqa: E402
import compose  # noqa: E402  (m7/lib)

GRASS = ak.ROOT / "m7/roster/terrain/tile_grass.png"
STRIP_BG = (222, 226, 214, 255)
ANIMS = ["idle", "strike", "flinch", "death"]


def font(n):
    try:
        return ImageFont.load_default(size=n)
    except TypeError:
        return ImageFont.load_default()


def grass():
    return Image.open(GRASS).convert("RGBA")


_UNIT_BASE = {}  # the current unit's idle frame 0 (set_unit), for banner placement


def set_unit(d):
    """Use this output folder's idle frame 0 (the unit's static sprite) for banner placement; swordsman if none."""
    f = Path(d) / "idle" / "frame_00.png"
    _UNIT_BASE["img"] = Image.open(f).convert("RGBA") if f.exists() else ak.base_canvas()


def facing(d):
    """The facing recorded in an output folder's manifests by recipe.py (older sets: lower-left)."""
    for anim in ("strike", "idle"):
        mf = Path(d) / anim / "manifest.json"
        if mf.exists():
            return json.loads(mf.read_text()).get("facing", "lower-left")
    return "lower-left"


def banner_x(ax, mirrored=False):
    base = _UNIT_BASE.get("img") or ak.base_canvas()
    bb = ak.bbox(ak.mirror(base) if mirrored else base, 40)  # spec: banner from the idle frame-0 bbox, fixed per unit
    left = bb[0] - ak.W / 2
    return ax + min(-34, left - 10)


def unit_on(scene, frame, ax, ay, banner="blue", shadow=True, mirrored=False):
    if banner:
        compose.draw_banner(scene, banner_x(ax, mirrored), ay - 6, banner)
    if shadow:
        compose.draw_shadow(scene, ax, ay - 2, w=48)
    scene.alpha_composite(frame, (int(ax - ak.ANCHOR[0]), int(ay - ak.ANCHOR[1])))


# ---------------------------------------------------------------- stills
def strip3x(frames, man, title=""):
    n = len(frames)
    s = 3
    pad = 26
    out = Image.new("RGBA", (ak.W * s * n, ak.H * s + pad * 2), STRIP_BG)
    d = ImageDraw.Draw(out)
    f = font(16)
    for i, fr in enumerate(frames):
        x = i * ak.W * s
        d.line([(x, pad), (x, pad + ak.H * s)], fill=(180, 186, 172, 255))
        d.line([(x, pad + ak.ANCHOR[1] * s), (x + ak.W * s, pad + ak.ANCHOR[1] * s)], fill=(170, 176, 160, 255))
        out.alpha_composite(fr.resize((ak.W * s, ak.H * s), Image.NEAREST), (x, pad))
        meta = man["frames"][i]
        lab = f"{i}  {meta['ms']} ms" + ("   HIT" if "hit" in meta.get("events", []) else "")
        d.text((x + 6, 4), lab, fill=(150, 20, 20, 255) if "hit" in meta.get("events", []) else (30, 34, 40, 255), font=f)
    if title:
        d.text((6, ak.H * s + pad + 4), title, fill=(30, 34, 40, 255), font=f)
    return out


def tile_cell(frame, banner="blue"):
    g = grass()
    cell = Image.new("RGBA", (224, 232), (54, 92, 150, 255))
    tx, ty = 16, 96
    cell.alpha_composite(g, (tx, ty))
    unit_on(cell, frame, tx + 96, ty + 54, banner)
    return cell


def map1x(frames):
    cells = [tile_cell(f) for f in frames]
    out = Image.new("RGBA", (224 * len(cells), 232), (54, 92, 150, 255))
    for i, c in enumerate(cells):
        out.alpha_composite(c, (i * 224, 0))
    return out


def silhouettes(frames):
    out = Image.new("RGBA", (ak.W * len(frames), ak.H), (255, 255, 255, 255))
    for i, f in enumerate(frames):
        m = metrics.mask(f, 100)
        a = np.zeros((ak.H, ak.W, 4), np.uint8)
        a[m] = (20, 20, 28, 255)
        out.alpha_composite(Image.fromarray(a, "RGBA"), (i * ak.W, 0))
    return out.resize((out.width * 2, out.height * 2), Image.NEAREST)


def onion(frames):
    s = 3
    base = Image.new("RGBA", (ak.W, ak.H), STRIP_BG)
    n = len(frames)
    for i, f in enumerate(frames):
        a = np.asarray(f).copy()
        k = 0.25 + 0.75 * (i / max(1, n - 1))
        a[..., 3] = (a[..., 3] * (0.35 if i < n - 1 else 1.0) * k).astype(np.uint8)
        base.alpha_composite(Image.fromarray(a, "RGBA"))
    up = base.resize((ak.W * s, ak.H * s), Image.NEAREST)
    d = ImageDraw.Draw(up)
    pts = [metrics.tip_point(f) for f in frames]
    prev = None
    for i, p in enumerate(pts):
        if p is None:
            prev = None
            continue
        q = (p[0] * s + 1, p[1] * s + 1)
        if prev:
            d.line([prev, q], fill=(220, 40, 40, 255), width=2)
        d.ellipse([q[0] - 4, q[1] - 4, q[0] + 4, q[1] + 4], outline=(220, 40, 40, 255), width=2)
        d.text((q[0] + 5, q[1] - 6), str(i), fill=(160, 20, 20, 255), font=font(14))
        prev = q
    return up


def diffheat(frames):
    out = Image.new("RGBA", (ak.W * len(frames), ak.H), (12, 12, 16, 255))
    for i in range(1, len(frames)):
        a = np.asarray(frames[i - 1]).astype(np.float32)
        b = np.asarray(frames[i]).astype(np.float32)
        d = np.abs(a[..., :3] * a[..., 3:4] / 255 - b[..., :3] * b[..., 3:4] / 255).mean(-1)
        d = np.clip(d / 80.0, 0, 1)
        heat = np.dstack([d * 255, d ** 2 * 200, (1 - d) * 40 * (d > 0.02), np.full_like(d, 255)]).astype(np.uint8)
        out.alpha_composite(Image.fromarray(heat, "RGBA"), (i * ak.W, 0))
    return out.resize((out.width * 2, out.height * 2), Image.NEAREST)


# ---------------------------------------------------------------- timelines
def actor_timeline(arm, script, root=None, mirror=False):
    """script: list of (anim, duration_ms or None). None = play once (death holds its last frame for hold_ms).
    Returns [(frame, start_ms, end_ms)], and a dict of event times (e.g. 'hit')."""
    cache = {}
    t, segs, ev = 0, [], []
    d = Path(root) if root else ak.ROOT / "arms" / arm
    for item in script:
        anim, dur = item[0], item[1]
        if anim not in cache:
            if not (d / anim / "manifest.json").exists():
                if anim != "idle":
                    continue  # this arm made no such animation: skip it in the sequence
                # no idle made: hold the static sprite instead (sequence label says so)
                cache[anim] = ([ak.base_canvas()] * 4, {"frames": [{"ms": 250, "events": []}] * 4})
                if mirror:
                    cache[anim] = ([ak.mirror(ak.base_canvas())] * 4, cache[anim][1])
            else:
                fr, man = ak.load_anim(arm, anim, root=root)
                if mirror:
                    fr = [ak.mirror(f) for f in fr]
                cache[anim] = (fr, man)
        fr, man = cache[anim]
        ms = [x["ms"] for x in man["frames"]]
        if dur is None:
            for i, f in enumerate(fr):
                if "hit" in man["frames"][i].get("events", []):
                    ev.append(("hit", t))
                segs.append((f, t, t + ms[i]))
                t += ms[i]
            if len(item) > 2:  # hold last frame
                f, s, e = segs[-1]
                segs[-1] = (f, s, e + item[2])
                t += item[2]
        else:
            end = t + dur
            i = 0
            while t < end:
                step = min(ms[i % len(ms)], end - t)
                segs.append((fr[i % len(fr)], t, t + step))
                t += step
                i += 1
    return segs, ev


def composite_timeline(actors, draw):
    """actors: list of segment lists. draw(list_of_frames) -> RGBA image. Returns [(image, ms)]."""
    cuts = sorted({s for segs in actors for _, s, _ in segs} | {e for segs in actors for _, _, e in segs})
    out = []
    for a, b in zip(cuts, cuts[1:]):
        if b <= a:
            continue
        cur = []
        for segs in actors:
            f = None
            for fr, s, e in segs:
                if s <= a < e:
                    f = fr
                    break
            if f is None:
                f = segs[-1][0] if a >= segs[-1][2] else segs[0][0]
            cur.append(f)
        out.append((draw(cur), b - a))
    return out


def save_gif(frames_ms, path):
    imgs = [f.convert("RGB") for f, _ in frames_ms]
    uniq, seen = [], set()
    for im in imgs:
        h = hashlib.md5(im.tobytes()).hexdigest()
        if h not in seen:
            seen.add(h)
            uniq.append(im)
    w, h = uniq[0].size
    mosaic = Image.new("RGB", (w, h * min(len(uniq), 24)))
    for i, im in enumerate(uniq[:24]):
        mosaic.paste(im, (0, i * h))
    pal = mosaic.quantize(colors=255, method=Image.Quantize.MEDIANCUT)
    q = [im.quantize(palette=pal, dither=Image.Dither.NONE) for im in imgs]
    q[0].save(path, save_all=True, append_images=q[1:], duration=[max(20, int(ms)) for _, ms in frames_ms], loop=0,
              optimize=False, disposal=1)


def save_mp4(frames_ms, path, scale=1, fps=100):
    """Constant-frame-rate H.264: each still is repeated round(ms * fps / 1000) times and piped as raw RGB, so the
    timing is exact (the concat demuxer's per-image durations proved unreliable for PNG inputs)."""
    first = frames_ms[0][0]
    w, h = first.width * scale, first.height * scale
    W2, H2 = w + w % 2, h + h % 2
    cmd = ["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W2}x{H2}",
           "-r", str(fps), "-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "14", "-preset", "slow",
           "-tune", "animation", "-movflags", "+faststart", str(path)]
    p = subprocess.Popen(cmd, stdin=subprocess.PIPE)
    acc = 0.0
    for f, ms in frames_ms:
        im = f.convert("RGB")
        if scale != 1:
            im = im.resize((w, h), Image.NEAREST)
        if (W2, H2) != (w, h):
            c = Image.new("RGB", (W2, H2), im.getpixel((0, 0)))
            c.paste(im, (0, 0))
            im = c
        acc += ms * fps / 1000.0
        reps = int(round(acc))
        acc -= reps
        buf = im.tobytes()
        for _ in range(reps):
            p.stdin.write(buf)
    p.stdin.close()
    if p.wait() != 0:
        raise RuntimeError(f"ffmpeg failed for {path}")


SEQ_SCRIPT = [("idle", 2000), ("strike", None), ("idle", 1000), ("flinch", None), ("idle", 1000), ("death", None, 1500)]


def sequence(arm, outdir, root=None, label=None):
    segs, _ = actor_timeline(arm, SEQ_SCRIPT, root=root)
    g = grass()

    def draw3(fs):
        sc = Image.new("RGBA", (232, 200), (54, 92, 150, 255))
        sc.alpha_composite(g, (20, 64))
        unit_on(sc, fs[0], 20 + 96, 64 + 54)
        if label:
            ImageDraw.Draw(sc).text((6, 4), label, fill=(240, 244, 255, 255), font=font(12))
        return sc.resize((sc.width * 3, sc.height * 3), Image.NEAREST)

    def draw1(fs):
        rows = cols = 3
        X0, Y0 = 20 + (rows - 1) * 96, 70
        sc = Image.new("RGBA", (rows * 96 + cols * 96 + 40, (rows + cols) * 48 + 110), (54, 92, 150, 255))
        for r in range(rows):
            for c in range(cols):
                pass
        order = sorted(((r, c) for r in range(rows) for c in range(cols)), key=lambda t: (t[0] + t[1], t[0]))
        for r, c in order:
            sc.alpha_composite(g, (X0 + (c - r) * 96, Y0 + (c + r) * 48))
        tx, ty = X0, Y0 + 2 * 48  # tile (1,1)
        unit_on(sc, fs[0], tx + 96, ty + 54)
        return sc

    out = Path(outdir)
    f3 = composite_timeline([segs], draw3)
    save_gif(f3, out / "sequence_3x.gif")
    save_mp4(f3, out / "sequence_3x.mp4")
    f1 = composite_timeline([segs], draw1)
    save_gif(f1, out / "sequence_1x.gif")
    save_mp4(f1, out / "sequence_1x.mp4", scale=2)
    return sum(ms for _, ms in f3)


def vignette(arm, outdir, root=None, scale=2):
    """A (blue) strikes B (mirrored, red) on the neighbouring tile on the side A looks: B flinches; A strikes again:
    B dies and stays down. A lower-left facer stands on the right, a lower-right facer on the left."""
    d = Path(root) if root else ak.ROOT / "arms" / arm
    set_unit(d)
    a_left = facing(d) == "lower-right"
    a_script = [("idle", 1000), ("strike", None), ("idle", 1600), ("strike", None), ("idle", 2400)]
    a_segs, a_ev = actor_timeline(arm, a_script, root=root)
    hits = [t for k, t in a_ev if k == "hit"]
    b_script = [("idle", hits[0]), ("flinch", None)]
    b_segs, _ = actor_timeline(arm, b_script, root=root, mirror=True)
    t_after = b_segs[-1][2]
    more, _ = actor_timeline(arm, [("idle", hits[1] - t_after)], root=root, mirror=True)
    death, _ = actor_timeline(arm, [("death", None, 10000)], root=root, mirror=True)
    b_segs = b_segs + [(f, s + t_after, e + t_after) for f, s, e in more]
    t2 = b_segs[-1][2]
    end = a_segs[-1][2]
    b_segs += [(f, s + t2, min(end, e + t2)) for f, s, e in death if s + t2 < end]
    g = grass()

    def draw(fs):
        rows = cols = 3
        X0, Y0 = 40 + (rows - 1) * 96, 60
        sc = Image.new("RGBA", (rows * 96 + cols * 96 + 80, (rows + cols) * 48 + 90), (54, 92, 150, 255))
        order = sorted(((r, c) for r in range(rows) for c in range(cols)), key=lambda t: (t[0] + t[1], t[0]))
        for r, c in order:
            sc.alpha_composite(g, (X0 + (c - r) * 96, Y0 + (c + r) * 48))
        ax_, ay_ = X0 + (2 - 1) * 96 + 96, Y0 + (2 + 1) * 48 + 54   # A on tile (1,2)
        bx_, by_ = X0 + (1 - 2) * 96 + 96, Y0 + (1 + 2) * 48 + 54   # B on tile (2,1)
        if a_left:
            ax_, bx_ = bx_, ax_
        unit_on(sc, fs[1], bx_, by_, "red", mirrored=True)
        unit_on(sc, fs[0], ax_, ay_, "blue")
        lx, rx = min(ax_, bx_), max(ax_, bx_)
        sc = sc.crop((lx - 150, ay_ - 170, rx + 120, ay_ + 50))
        return sc.resize((sc.width * scale, sc.height * scale), Image.NEAREST)

    fr = composite_timeline([a_segs, b_segs], draw)
    out = Path(outdir)
    save_gif(fr, out / "vignette.gif")
    save_mp4(fr, out / "vignette.mp4")
    return sum(ms for _, ms in fr)


def anim_gifs(frames, man, outdir, anim):
    ms = [x["ms"] for x in man["frames"]]
    if man.get("hold_last"):
        ms = ms[:-1] + [ms[-1] + 1200]
    one = [(tile_cell(f), m) for f, m in zip(frames, ms)]
    save_gif(one, outdir / f"{anim}_1x.gif")
    three = [(c.resize((c.width * 3, c.height * 3), Image.NEAREST), m) for c, m in one]
    save_gif(three, outdir / f"{anim}_3x.gif")


def arm_previews(arm, root=None, only=None):
    d = Path(root) if root else ak.ROOT / "arms" / arm
    set_unit(d)
    pv = d / "preview"
    pv.mkdir(parents=True, exist_ok=True)
    have = [a for a in ANIMS if (d / a / "manifest.json").exists()]
    if only in (None, "stills"):
        for anim in have:
            frames, man = ak.load_anim(arm, anim, root=root)
            strip3x(frames, man, f"{arm} / {anim}").save(pv / f"{anim}_strip3x.png")
            map1x(frames).save(pv / f"{anim}_map1x.png")
            silhouettes(frames).save(pv / f"{anim}_sil.png")
            onion(frames).save(pv / f"{anim}_onion.png")
            diffheat(frames).save(pv / f"{anim}_diff.png")
            anim_gifs(frames, man, pv, anim)
    res = {}
    if only in (None, "sequence") and "strike" in have:
        missing = [a for a in ANIMS if a not in have]
        lab = None if not missing else f"{arm}: no {', '.join(missing)} (static sprite / skipped)"
        res["sequence_ms"] = sequence(arm, d, root=root, label=lab)
    if only in (None, "vignette") and {"idle", "strike", "flinch", "death"} <= set(have):
        res["vignette_ms"] = vignette(arm, d, root=root)
    return have, res


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm"); ap.add_argument("--root"); ap.add_argument("--only", choices=["stills", "sequence", "vignette"])
    ap.add_argument("--vignette")
    a = ap.parse_args()
    if a.vignette:
        print(vignette(a.vignette, Path(a.root) if a.root else ak.ROOT / "arms" / a.vignette, root=a.root))
    else:
        print(arm_previews(a.arm, a.root, a.only))
