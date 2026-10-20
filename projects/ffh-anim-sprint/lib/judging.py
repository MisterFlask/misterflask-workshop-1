"""Blind judging packets and score aggregation.

  python lib/judging.py packets --arms A0 R G0 G1 G2 K C --seed 7     # judging/packets/<anim>/..., judging/key.json
  python lib/judging.py aggregate                                     # judging/judge_*.json -> judging/scores.json

Arms are relabelled with letters (a fixed shuffle per seed); judges never see arm names, prompts or notes.
Per animation the packet holds, for every letter:
  compare_2x_j<J>.png   all letters stacked (row order differs per judge J), each row = frames at 2x with ms + HIT
  <L>_strip3x.png       one letter's frames at 3x with ms + HIT
  compare_1x.png        all letters at map scale on grass (rows in letter order)
  <L>_onion.png <L>_sil.png       onion skin with sword-tip trail; silhouette strip
  <L>_shuffled.png      the frames in a shuffled order labelled p, q, r, ... (for the frame-ordering test)
  metrics.txt           the computed metrics per letter (non-rigidity, overlap, colour drift, boil, gate flags)
key.json (NOT shown to judges; move it out of judging/ while they work): letter -> arm, and the true order behind
each shuffled strip.
"""
import argparse, json, random, sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animkit as ak  # noqa: E402
import metrics  # noqa: E402
import preview  # noqa: E402

J = ak.ROOT / "judging"
ANIMS = ["idle", "strike", "flinch", "death"]
LETTERS = "BDFHKMPSTVWXYZ"
SHUF_LABELS = "pqrstuvw"


def labelled_row(frames, man, label, s=3, bg=(222, 226, 214, 255), show_ms=True):
    pad = 24
    lw = 54
    out = Image.new("RGBA", (lw + ak.W * s * len(frames), ak.H * s + pad), bg)
    d = ImageDraw.Draw(out)
    d.text((10, pad + ak.H * s // 2 - 20), label, fill=(20, 24, 40, 255), font=preview.font(40))
    for i, f in enumerate(frames):
        x = lw + i * ak.W * s
        d.line([(x, 0), (x, out.height)], fill=(180, 186, 172, 255))
        d.line([(x, pad + ak.ANCHOR[1] * s), (x + ak.W * s, pad + ak.ANCHOR[1] * s)], fill=(170, 176, 160, 255))
        out.alpha_composite(f.resize((ak.W * s, ak.H * s), Image.NEAREST), (x, pad))
        if show_ms and man:
            meta = man["frames"][i]
            hit = "hit" in meta.get("events", [])
            d.text((x + 6, 3), f"{i}  {meta['ms']} ms" + ("  HIT" if hit else ""), font=preview.font(15),
                   fill=(160, 20, 20, 255) if hit else (30, 34, 40, 255))
    return out


def stack(rows, gap=6, bg=(250, 250, 250, 255)):
    w = max(r.width for r in rows)
    h = sum(r.height for r in rows) + gap * (len(rows) - 1)
    out = Image.new("RGBA", (w, h), bg)
    y = 0
    for r in rows:
        out.alpha_composite(r, (0, y))
        y += r.height + gap
    return out


def arm_root(a):
    """Round-1 arms live in arms/<A>; recipe sets (final, final_nofx, final_<unit>) live at the project root."""
    return ak.ROOT / a if a.startswith("final") else ak.ROOT / "arms" / a


def packets(arms, seed=7, judges=3, anims=None):
    rng = random.Random(seed)
    letters = list(LETTERS[:len(arms)])
    rng.shuffle(letters)
    key = {"letter_to_arm": dict(zip(letters, arms)), "shuffles": {}}
    sprite_row = None
    for anim in (anims or ANIMS):
        d = J / "packets" / anim
        d.mkdir(parents=True, exist_ok=True)
        have = {L: a for L, a in key["letter_to_arm"].items() if (arm_root(a) / anim / "manifest.json").exists()}
        if not have:
            continue
        data = {L: ak.load_anim(a, anim, root=arm_root(a)) for L, a in have.items()}
        order = sorted(have)
        for j in range(1, judges + 1):
            rows = order[:]
            random.Random(seed * 100 + j * 7 + ANIMS.index(anim)).shuffle(rows)
            img = stack([labelled_row([ak.base_canvas()], None, "ref", s=2, show_ms=False)] +
                        [labelled_row(*data[L], L, s=2) for L in rows])
            img.convert("RGB").save(d / f"compare_2x_j{j}.png")
        maps = []
        for L in order:
            m = preview.map1x(data[L][0])
            lab = Image.new("RGBA", (54 + m.width, m.height), (54, 92, 150, 255))
            ImageDraw.Draw(lab).text((12, m.height // 2 - 20), L, fill=(255, 255, 255, 255), font=preview.font(36))
            lab.alpha_composite(m, (54, 0))
            maps.append(lab)
        stack(maps).convert("RGB").save(d / "compare_1x.png")
        mtxt = []
        for L in order:
            frames, man = data[L]
            labelled_row(frames, man, L).convert("RGB").save(d / f"{L}_strip3x.png")
            preview.onion(frames).convert("RGB").save(d / f"{L}_onion.png")
            preview.silhouettes(frames).convert("RGB").save(d / f"{L}_sil.png")
            perm = list(range(len(frames)))
            rs = random.Random(seed * 1000 + ord(L) + ANIMS.index(anim))
            while perm == sorted(perm):
                rs.shuffle(perm)
            labs = SHUF_LABELS[:len(frames)]
            row = labelled_row([frames[k] for k in perm], None, L, show_ms=False)
            dd = ImageDraw.Draw(row)
            for i, lab in enumerate(labs):
                dd.text((54 + i * ak.W * 3 + 8, 2), lab, fill=(20, 24, 120, 255), font=preview.font(20))
            row.convert("RGB").save(d / f"{L}_shuffled.png")
            key["shuffles"][f"{anim}/{L}"] = {lab: perm[i] for i, lab in enumerate(labs)}
            mm = metrics.anim_metrics(frames, anim)
            s = mm["summary"]
            mtxt.append(f"{L}: non-rigidity mean {s['nonrigid_mean']:.3f} (0 = just frame 0 moved), overlap index "
                        f"{s['overlap_index']:.2f} (0 = all parts peak on the same frame), colour drift {s['drift_mean']:.2f} "
                        f"(OKLab x100 to the sprite's colours), frame-to-frame change {s['boil_mean']:.1f}, "
                        f"gate flags: {mm['gates'] or 'none'}")
        (d / "metrics.txt").write_text("\n".join(mtxt) + "\n")
    J.mkdir(exist_ok=True)
    (J / "key.json").write_text(json.dumps(key, indent=1))
    return key


RUBRIC = {"motion": 0.35, "consistency": 0.30, "style": 0.20, "appeal": 0.15}


def aggregate():
    key = json.loads((J / "key.json").read_text())
    l2a = key["letter_to_arm"]
    rows = []
    for f in sorted(J.glob("judge_*.json")):
        jd = json.loads(f.read_text())
        for anim, res in jd.get("animations", {}).items():
            for L, sc in res.get("scores", {}).items():
                arm = l2a.get(L)
                if not arm:
                    continue
                w = sum(sc[k] * v for k, v in RUBRIC.items() if k in sc)
                order_ok = None
                guess = res.get("ordering", {}).get(L)
                truth = key["shuffles"].get(f"{anim}/{L}")
                if guess and truth:
                    true_seq = sorted(truth, key=lambda lab: truth[lab])
                    g = [c for c in str(guess).replace(",", " ").split() if c in truth] or [c for c in str(guess) if c in truth]
                    order_ok = round(sum(1 for a, b in zip(g, true_seq) if a == b) / len(true_seq), 3)
                rows.append({"judge": jd.get("judge"), "anim": anim, "letter": L, "arm": arm, **sc, "weighted": round(w, 3),
                             "rank": res.get("ranking", []).index(L) + 1 if L in res.get("ranking", []) else None,
                             "ordering_accuracy": order_ok, "odd_frame": res.get("odd_frame", {}).get(L),
                             "flaw": res.get("flaws", {}).get(L)})
    out = {"rows": rows, "by_arm_anim": {}, "by_arm": {}}
    for r in rows:
        k = f"{r['arm']}/{r['anim']}"
        out["by_arm_anim"].setdefault(k, []).append(r)
    summ = {}
    for k, rs in out["by_arm_anim"].items():
        arm, anim = k.split("/")
        summ.setdefault(arm, {})[anim] = {
            "weighted": round(float(np.mean([r["weighted"] for r in rs])), 2),
            **{m: round(float(np.mean([r[m] for r in rs if m in r])), 2) for m in RUBRIC},
            "mean_rank": round(float(np.mean([r["rank"] for r in rs if r["rank"]])), 2) if any(r["rank"] for r in rs) else None,
            "ordering_accuracy": round(float(np.mean([r["ordering_accuracy"] for r in rs if r["ordering_accuracy"] is not None])), 2)
            if any(r["ordering_accuracy"] is not None for r in rs) else None,
            "n_judges": len(rs)}
    for arm, d in summ.items():
        d["overall"] = round(float(np.mean([v["weighted"] for v in d.values()])), 2)
    out["by_arm"] = summ
    (J / "scores.json").write_text(json.dumps(out, indent=1))
    return summ


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["packets", "aggregate"])
    ap.add_argument("--arms", nargs="+")
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--anims", nargs="+")
    a = ap.parse_args()
    if a.cmd == "packets":
        print(json.dumps(packets(a.arms, a.seed, anims=a.anims)["letter_to_arm"]))
    else:
        print(json.dumps(aggregate(), indent=1))
