"""Image-model calls for the sprint (thin wrapper over m7/lib/imggen.py: budget caps + spend ledger) and the shared
prompt skeleton for sheet-based arms.

  import gen
  res = gen.edit(tag="anim/G1", images=[template, gen.ROSTER], prompt=text, out="work/G1/strike_r1.png", n=2)
  res = gen.parallel([dict(...), dict(...)])          # several calls at once

Every call is logged by imggen to spend.jsonl (full prompt + ordered images). Defaults: gpt-image-2.5-sunburst,
medium quality, 1536x1024 (overridable; P1 calibration fixes the contract in CONTRACT below).
"""
import json, subprocess, sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IMGGEN = ROOT / "m7/lib/imggen.py"
ROSTER = ROOT / "m7/anchors/anchor_roster_r1.png"
STYLE_ANCHOR = ROOT / "m7/anchors/anchor_units.png"

# Frozen after P1 calibration (see notes in arms/calib/notes.md). Arms must use these unless their notes say why.
CONTRACT = {"model": "gpt-image-2.5-sunburst", "quality": "medium", "size": "1536x1024", "n": 2,
            "input_fidelity": None, "guide_style": "colour"}


def edit(tag, images, prompt, out, model=None, quality=None, size=None, n=None, input_fidelity=None, mask=None,
         prompt_file=None):
    model = model or CONTRACT["model"]
    quality = quality or CONTRACT["quality"]
    size = size or CONTRACT["size"]
    n = n or CONTRACT["n"]
    out = Path(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    pf = Path(prompt_file) if prompt_file else out.with_suffix(".prompt.txt")
    pf.write_text(prompt, encoding="utf-8")
    cmd = [sys.executable, str(IMGGEN), "edit", "--tag", tag, "--model", model, "--quality", quality, "--size", size,
           "--n", str(n), "--prompt-file", str(pf), "--out", str(out)]
    if input_fidelity:
        cmd += ["--input-fidelity", input_fidelity]
    if mask:
        cmd += ["--mask", str(mask)]
    for im in images:
        cmd += ["--image", str(im)]
    p = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
    try:
        res = json.loads(p.stdout.strip().splitlines()[-1])
    except Exception:
        res = {"ok": False, "error": (p.stdout + p.stderr)[-800:]}
    res["out"] = str(out)
    return res


def parallel(calls, workers=4):
    with ThreadPoolExecutor(workers) as ex:
        return list(ex.map(lambda c: edit(**c), calls))


# ---------------------------------------------------------------- prompt skeleton (G0 / G1 / G2 / K share it)
CHARACTER = ("a swordsman foot soldier: blue sleeveless tunic over a cream shirt, brown leather belt, blue shorts, "
             "tan leggings, brown leather boots, brown tousled hair, a short steel sword in his RIGHT hand and a round "
             "wooden shield with a steel boss on his LEFT arm")
STYLE = ("bright saturated HD pixel art, soft chunky pixels, dark coloured outlines (deep brown/navy, not thin black "
         "lines), simple flat shading with strong light from the upper-left, cheerful 16-bit SNES-era look, chibi "
         "proportions about 3 heads tall")

GUIDE_TEXT = {
    "none": ("The other cells are empty. Draw the character from the top-left cell once in each of the other cells, "
             "in the pose described for that cell below."),
    "colour": ("In every other cell there is a flat-colour blockout mannequin showing a pose. Redraw the character "
               "from the top-left cell in each of those cells, in exactly the pose of that cell's blockout, at the "
               "same position in the cell. The blockouts are only pose guides: replace them entirely with fully "
               "detailed pixel art; do not keep their flat colours or simplified shapes."),
    "grey": ("In every other cell there is a grey mannequin showing a pose. Redraw the character from the top-left "
             "cell in each of those cells, in exactly the pose of that cell's mannequin, at the same position in the "
             "cell, in the character's own full colours. The mannequins are only pose guides: replace them entirely."),
    "rig": ("In every other cell there is a rough draft of a pose, made by cutting the reference sprite into parts and "
            "rotating them. Redraw each draft as clean, finished pixel art of the same character in exactly that "
            "pose and position: repair broken joints, redraw rotated parts so their pixels and shading look "
            "hand-drawn, and let the hair, tunic and limbs bend naturally."),
}


def sheet_prompt(anim, cell_texts, guide="colour", layout="4x2", extra=""):
    """cell_texts: list of (cell_number, text) for cells 2..N (1-based, row-major). Cell 1 is the reference."""
    lines = [f"cell {c}: {t}" for c, t in cell_texts]
    return (
        f"The first attached image is a sprite-sheet template from my isometric fantasy strategy game: a {layout} grid "
        "of equal cells on a flat magenta background (there are no visible grid lines). The top-left cell (cell 1) "
        f"shows the finished character, {CHARACTER}, drawn in {STYLE}, standing in a 3/4 view facing the lower-left. "
        f"{GUIDE_TEXT[guide]} "
        "Every cell must show the SAME character: same size, same face and hair, same outfit and colours, same pixel "
        "size and outline style, feet on the same ground line as the top-left cell. He faces the lower-left in every "
        "cell; the sword stays in his right hand on the screen-left side and the shield on his left arm on the "
        "screen-right side. Leave cell 1 exactly as it is. "
        f"The cells are consecutive frames of a {anim.upper()} animation, read left to right, top row then bottom row:\n"
        + "\n".join(lines) + "\n"
        "Each cell holds exactly one figure, fully inside its cell. No grid lines, frames, numbers, text, ground, "
        "shadows, motion lines or effects. Background: flat solid pure magenta #FF00FF filling the whole image, "
        "with no gradient. The second attached image is a roster sheet of other units from the same game: match "
        "its outline colour, shading and pixel size exactly." + (" " + extra if extra else "")
    )
