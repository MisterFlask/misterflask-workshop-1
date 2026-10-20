"""A0 - the control: procedural motion of the WHOLE static sprite (no new art).

This reproduces the 'stiff / puppet-like' approach (the dropped TS combat scene tweened the whole sprite: lunge +
scale, knock-back, fade) but as 4-8 discrete frames on the shared beats, done as well as a whole-sprite transform
allows: squash/stretch breathing, lean + lunge + shear, recoil, and a tip-over that ends lying along the iso axis.
Round 1 carries no FX (no smear, flash or fade).

  python arms/A0/make_a0.py
"""
import sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "lib"))
import animkit as ak  # noqa: E402

ARM = "A0"
FEET = ak.ANCHOR  # pivot: the feet stay planted unless the frame says otherwise


def T(angle=0.0, sx=1.0, sy=1.0, shx=0.0, dx=0.0, dy=0.0, pivot=FEET):
    f = ak.transform(ak.base_canvas(), angle=angle, sx=sx, sy=sy, shx=shx, pivot=pivot, dx=dx, dy=dy)
    return fit(f)


def fit(f):
    """Keep the transformed sprite on the ground and inside the safe region: lift it if a rotated boot or shield
    dips below the ground row, and pull it back in if it pokes past x 16..143."""
    from PIL import Image
    bb = ak.bbox(f, 40)
    dy = min(0, (ak.ANCHOR[1]) - bb[3])
    dx = max(0, ak.SAFE_X[0] - bb[0]) + min(0, ak.SAFE_X[1] - bb[2])
    if dx or dy:
        g = Image.new("RGBA", f.size, (0, 0, 0, 0))
        g.alpha_composite(f, (int(dx), int(dy)))
        f = g
    return f


def main():
    t0 = time.time()
    # idle: breathing as a 1-2 % vertical stretch about the feet
    idle = [ak.base_canvas(), T(sy=1.012), T(sy=1.022, sx=0.995), T(sy=1.01)]
    ak.write_anim(ARM, "idle", idle)
    # strike: lean back (+ = counter-clockwise; leaning back toward the upper-right is clockwise = negative)
    strike = [ak.base_canvas(),
              T(angle=-4, dx=2, sy=1.01),
              T(angle=-7, dx=3, sy=0.97, sx=1.02),
              T(angle=6, dx=-10, sx=1.05, shx=0.10),
              T(angle=9, dx=-14, sy=0.96, sx=1.04, shx=0.16),
              T(angle=7, dx=-12, sy=0.97, shx=0.12),
              T(angle=3, dx=-6),
              T(angle=1, dx=-2)]
    ak.write_anim(ARM, "strike", strike)
    flinch = [T(angle=-8, dx=6, shx=-0.08), T(angle=-10, dx=8, sy=0.97), T(angle=-5, dx=4), T(angle=-1, dx=1)]
    ak.write_anim(ARM, "flinch", flinch)
    death = [flinch[0],
             T(angle=-10, dx=6, shx=-0.1),
             T(angle=-22, dx=4, sy=0.95),
             T(angle=-35, dx=0, sy=0.92),
             T(angle=-52, dx=-8),
             T(angle=-63.4, dx=-16, sy=0.95),
             T(angle=-60, dx=-16, dy=-2),
             T(angle=-63.4, dx=-16)]
    ak.write_anim(ARM, "death", death)
    ak.log_time(ARM, "all four animations", t0)


if __name__ == "__main__":
    main()
