"""Blind packets for the post-rule verification: direction check (every set) and old-vs-new quality pairs.
Run from the project root: python3 judging/post_rule/make_packets.py [seed] ; writes work/verify2/packets/ and work/verify2/key.json
(the key is copied into the round's JSON here once the judges are done)."""
import json, random, shutil, sys
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "work/verify2/packets"
SETS = {  # set id -> (folder, sprite)
    "swordsman_new": ("final_swordsman_nofx", "m7/roster/units/unit_swordsman.png"),
    "swordsman_judged": ("final_nofx", "m7/roster/units/unit_swordsman.png"),
    "hyborem_new": ("final_hyborem_nofx", "m7/roster/units/unit_hyborem.png"),
    "hyborem_v1": ("final_hyborem_v1_nofx", "m7/roster/units/unit_hyborem.png"),
    "ogre_new": ("final_ogre_nofx", "m7/roster/units/unit_ogre_embers.png"),
    "ogre_agent": ("work/compare/ogre_v1_nofx", "m7/roster/units/unit_ogre_embers.png"),
}
ANIMS = ["idle", "strike", "flinch", "death"]


def strip(folder, anim, scale=3):
    d = ROOT / folder / anim
    man = json.loads((d / "manifest.json").read_text())
    frames = [Image.open(d / f["file"]).convert("RGBA") for f in man["frames"]]
    W, H = frames[0].size
    out = Image.new("RGBA", (W * scale * len(frames), H * scale + 24), (222, 226, 214, 255))
    dr = ImageDraw.Draw(out)
    for i, (f, m) in enumerate(zip(frames, man["frames"])):
        out.alpha_composite(f.resize((W * scale, H * scale), Image.NEAREST), (i * W * scale, 24))
        dr.text((i * W * scale + 4, 4), f"frame {i}  {m['ms']} ms" + ("  HIT" if "hit" in m.get("events", []) else ""),
                fill=(150, 20, 20, 255) if "hit" in m.get("events", []) else (30, 34, 40, 255))
        dr.line([(i * W * scale, 0), (i * W * scale, out.height)], fill=(180, 186, 172, 255))
    return out.convert("RGB")


def main():
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    rng = random.Random(int(sys.argv[1]) if len(sys.argv) > 1 else 20261020)  # round A: 20261020, round B: 20261021
    ids = [s for s in SETS if (ROOT / SETS[s][0] / "strike" / "manifest.json").exists()]
    letters = [chr(ord("P") + i) for i in range(len(ids))]
    rng.shuffle(ids)
    key = {"direction": dict(zip(letters, ids))}
    for L, sid in key["direction"].items():
        folder, sprite = SETS[sid]
        sp = Image.open(ROOT / sprite).convert("RGBA")
        bg = Image.new("RGBA", sp.size, (222, 226, 214, 255)); bg.alpha_composite(sp)
        bg.resize((sp.width * 4, sp.height * 4), Image.NEAREST).convert("RGB").save(OUT / f"dir_{L}_sprite.png")
        for anim in ("strike", "flinch", "death"):
            strip(folder, anim).save(OUT / f"dir_{L}_{anim}.png")
    # quality pairs: same unit, old vs new, order randomised per pair
    key["pairs"] = {}
    for unit, (a, b) in {"U1": ("ogre_agent", "ogre_new"), "U2": ("swordsman_judged", "swordsman_new"),
                         "U3": ("hyborem_v1", "hyborem_new")}.items():
        if not all(s in ids for s in (a, b)):
            continue
        pair = [a, b]; rng.shuffle(pair)
        key["pairs"][unit] = {"X": pair[0], "Y": pair[1]}
        for side, sid in zip("XY", pair):
            for anim in ANIMS:
                strip(SETS[sid][0], anim).save(OUT / f"pair_{unit}_{side}_{anim}.png")
    (ROOT / "work/verify2/key.json").write_text(json.dumps(key, indent=1))
    print(json.dumps({k: len(v) for k, v in key.items()}))


if __name__ == "__main__":
    main()
