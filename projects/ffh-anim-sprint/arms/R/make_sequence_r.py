"""Full-sequence GIF/MP4 deliverables for arm R, built with lib/preview.py's own scene, GIF and MP4 helpers.

For every animation R has written, arms/R/sequence_<anim>_{3x,1x}.{gif,mp4}: the unit on a grass tile with banner
and contact shadow, playing ready (frame 0 held) -> the animation -> ready, twice, at the manifest timings.
Once all four animations exist it also writes preview.py's standard sequence_{3x,1x}.{gif,mp4}
(idle x2 -> strike -> idle -> flinch -> idle -> death -> corpse hold).

  python arms/R/make_sequence_r.py
"""
import sys
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402
import preview as pv  # noqa: E402

ARM = "R"
OUT = ROOT / "arms" / ARM
HOLD_MS = 700


def timeline(frames, man, reps=2):
    ms = [f["ms"] for f in man["frames"]]
    segs, t = [], 0
    for _ in range(reps):
        segs.append((frames[0], t, t + HOLD_MS)); t += HOLD_MS
        for f, m in zip(frames, ms):
            segs.append((f, t, t + m)); t += m
        if man.get("hold_last"):
            f, s, e = segs[-1]
            segs[-1] = (f, s, e + 1200); t += 1200
    segs.append((frames[0], t, t + HOLD_MS))
    return segs


def draw3(fs, label):
    g = pv.grass()
    sc = Image.new("RGBA", (232, 200), (54, 92, 150, 255))
    sc.alpha_composite(g, (20, 64))
    pv.unit_on(sc, fs[0], 20 + 96, 64 + 54)
    from PIL import ImageDraw
    ImageDraw.Draw(sc).text((6, 4), label, fill=(240, 244, 255, 255), font=pv.font(12))
    return sc.resize((sc.width * 3, sc.height * 3), Image.NEAREST)


def draw1(fs):
    return pv.tile_cell(fs[0])


def main():
    made = []
    have = [a for a in pv.ANIMS if (OUT / a / "manifest.json").exists()]
    for anim in have:
        frames, man = ak.load_anim(ARM, anim)
        segs = timeline(frames, man)
        seq3 = pv.composite_timeline([segs], lambda fs: draw3(fs, f"{ARM} / {anim}"))
        seq1 = pv.composite_timeline([segs], draw1)
        pv.save_gif(seq3, OUT / f"sequence_{anim}_3x.gif")
        pv.save_mp4(seq3, OUT / f"sequence_{anim}_3x.mp4")
        pv.save_gif(seq1, OUT / f"sequence_{anim}_1x.gif")
        pv.save_mp4(seq1, OUT / f"sequence_{anim}_1x.mp4", scale=2)  # 2x NEAREST so 4:2:0 chroma keeps single pixels
        made += [f"sequence_{anim}_{s}.{e}" for s in ("3x", "1x") for e in ("gif", "mp4")]
    if len(have) == 4:
        pv.sequence(ARM, OUT)
        made += [f"sequence_{s}.{e}" for s in ("3x", "1x") for e in ("gif", "mp4")]
    print("\n".join(str(OUT / m) for m in made))


if __name__ == "__main__":
    main()
