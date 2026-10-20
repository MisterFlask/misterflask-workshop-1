"""OpenAI image wrapper for the sprint, with a hard budget and a spend ledger.

  python tools/imggen.py gen  --tag M6/units --model gpt-image-1.5 --prompt "..." --out a.png
  python tools/imggen.py edit --tag M7/units --model gpt-image-1.5 --image ref.png --image crop.png \
                              --prompt "..." --out b.png [--mask m.png] [--input-fidelity high]
  python tools/imggen.py spend          # totals per tag

Common options: --size 1024x1024|1536x1024|1024x1536|auto  --quality low|medium|high|auto
                --background transparent|opaque|auto  --n N  --prompt-file f.txt
Multiple outputs are written as out_1.png, out_2.png ...
Each call appends to <package>/spend.jsonl (including the full prompt and the ordered --image list). Calls are refused when the global cap or the
tag-prefix cap (<package>/budget.json) would be exceeded. Prints one JSON line.
Unsupported optional params are dropped automatically and the call retried.
"""
import argparse, base64, json, os, sys, time
from pathlib import Path
import requests

ROOT = Path(__file__).resolve().parent.parent  # package root: spend.jsonl + budget.json live here
LEDGER = ROOT / "spend.jsonl"
BUDGET = ROOT / "budget.json"
API = "https://api.openai.com/v1/images"

# $ per 1M tokens (text_in, image_in, image_out). Deliberately conservative (>= published/aggregator prices).
PRICES = {"gpt-image-1-mini": (2.5, 2.5, 10.0)}
DEFAULT_PRICE = (5.0, 10.0, 40.0)
OUT_TOKENS = {  # approx output tokens per image by quality for a 1024x1024 frame
    "low": 272, "medium": 1056, "high": 4160, "auto": 4160}


def price(model):
    return PRICES.get(model, DEFAULT_PRICE)


def area_factor(size):
    if not size or size == "auto":
        return 1.5
    try:
        w, h = (int(v) for v in size.lower().split("x"))
        return (w * h) / (1024 * 1024)
    except Exception:
        return 1.5


def estimate(model, size, quality, n, n_images):
    _, pi, po = price(model)
    out = OUT_TOKENS.get(quality or "auto", 4160) * area_factor(size) * n
    return out * po / 1e6 + n_images * 6000 * pi / 1e6 + 0.002


def ledger():
    rows = []
    if LEDGER.exists():
        for line in LEDGER.read_text(encoding="utf-8").splitlines():
            if line.strip():
                try:
                    rows.append(json.loads(line))
                except Exception:
                    pass
    return rows


def spent(prefix=None):
    return sum(r.get("usd", 0) for r in ledger() if prefix is None or r.get("tag", "").split("/")[0] == prefix)


def caps():
    b = json.loads(BUDGET.read_text()) if BUDGET.exists() else {"total": 25.0, "tags": {}}
    return b["total"], b.get("tags", {})


def check_budget(tag, est):
    total, tags = caps()
    root = tag.split("/")[0]
    if root not in tags:
        raise SystemExit(json.dumps({"ok": False, "error": f"unknown budget tag '{root}'; known: {list(tags)}"}))
    s_all, s_tag = spent(), spent(root)
    if s_all + est > total:
        raise SystemExit(json.dumps({"ok": False, "error": f"GLOBAL BUDGET: spent ${s_all:.2f} + est ${est:.2f} > ${total}"}))
    if s_tag + est > tags[root]:
        raise SystemExit(json.dumps({"ok": False, "error": f"TAG BUDGET {root}: spent ${s_tag:.2f} + est ${est:.2f} > ${tags[root]}"}))
    sub = "/".join(tag.split("/")[:2])
    if sub in tags:
        s_sub = sum(r.get("usd", 0) for r in ledger() if "/".join(r.get("tag", "").split("/")[:2]) == sub)
        if s_sub + est > tags[sub]:
            raise SystemExit(json.dumps({"ok": False, "error": f"GROUP BUDGET {sub}: spent ${s_sub:.2f} + est ${est:.2f} > ${tags[sub]}"}))


def cost_from_usage(model, usage, fallback):
    if not usage:
        return fallback
    pt, pi, po = price(model)
    det = usage.get("input_tokens_details") or {}
    txt = det.get("text_tokens", usage.get("input_tokens", 0) if not det else 0)
    img = det.get("image_tokens", 0)
    out = usage.get("output_tokens", 0)
    return txt * pt / 1e6 + img * pi / 1e6 + out * po / 1e6


def call(kind, model, prompt, size, quality, background, n, images, mask, input_fidelity, extra):
    headers = {"Authorization": "Bearer " + os.environ["OPENAI_API_KEY"]}
    params = {"model": model, "prompt": prompt, "n": n}
    opt = {"size": size, "quality": quality, "background": background, "moderation": "low",
           "input_fidelity": input_fidelity if kind == "edit" else None, "output_format": "png"}
    opt.update(extra or {})
    opt = {k: v for k, v in opt.items() if v not in (None, "")}
    rl_wait, attempts, dropped = 0.0, 0, []
    while True:
        attempts += 1
        body = dict(params, **opt)
        t0 = time.time()
        try:
            if kind == "gen":
                r = requests.post(f"{API}/generations", headers=headers, json=body, timeout=420)
            else:
                files = [("image[]", (Path(p).name, open(p, "rb"), "image/png")) for p in images]
                if mask:
                    files.append(("mask", (Path(mask).name, open(mask, "rb"), "image/png")))
                data = {k: str(v) for k, v in body.items()}
                r = requests.post(f"{API}/edits", headers=headers, data=data, files=files, timeout=420)
        except requests.RequestException as e:
            if attempts < 4:
                time.sleep(5 * attempts); rl_wait += 5 * attempts; continue
            return None, {"error": f"network: {e}"}, time.time() - t0, rl_wait, dropped
        latency = time.time() - t0
        if r.status_code == 200:
            return r.json(), None, latency, rl_wait, dropped
        txt = r.text[:600]
        if r.status_code in (429, 500, 502, 503, 504) and attempts < 6:
            wait = min(60, 4 * 2 ** (attempts - 1))
            time.sleep(wait); rl_wait += wait; continue
        if r.status_code == 400:
            low = txt.lower()
            hit = None
            for k in list(opt):
                if f"'{k}'" in low or f'"{k}"' in low or f" {k} " in low or f"{k}." in low or f"parameter: {k}" in low or f"`{k}`" in low:
                    hit = k; break
            if hit and hit not in ("size",) and attempts < 6:
                dropped.append(hit); opt.pop(hit); continue
            if "transparent" in low and "background" in opt and attempts < 6:
                dropped.append("background"); opt.pop("background"); continue
        return None, {"status": r.status_code, "error": txt}, latency, rl_wait, dropped


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=["gen", "edit", "spend"])
    ap.add_argument("--tag")
    ap.add_argument("--model", default="gpt-image-1.5")
    ap.add_argument("--prompt"); ap.add_argument("--prompt-file")
    ap.add_argument("--size", default="1024x1024")
    ap.add_argument("--quality", default="high")
    ap.add_argument("--background", default=None)
    ap.add_argument("--n", type=int, default=1)
    ap.add_argument("--image", action="append", default=[])
    ap.add_argument("--mask")
    ap.add_argument("--input-fidelity", default=None)
    ap.add_argument("--out")
    a = ap.parse_args()

    if a.cmd == "spend":
        total, tags = caps()
        by = {}
        for r in ledger():
            parts = r.get("tag", "?").split("/")
            for k in {parts[0], "/".join(parts[:2])}:
                by[k] = by.get(k, 0) + r.get("usd", 0)
        print(json.dumps({"total_spent": round(spent(), 3), "cap": total,
                          "by_tag": {k: round(v, 3) for k, v in by.items()}, "tag_caps": tags}, indent=1))
        return
    if not a.tag or not a.out:
        raise SystemExit("--tag and --out are required")
    prompt = a.prompt or Path(a.prompt_file).read_text(encoding="utf-8")
    est = estimate(a.model, a.size, a.quality, a.n, len(a.image))
    check_budget(a.tag, est)
    res, err, latency, rl_wait, dropped = call(a.cmd, a.model, prompt, a.size, a.quality, a.background, a.n,
                                               a.image, a.mask, a.input_fidelity, None)
    usage = (res or {}).get("usage")
    usd = cost_from_usage(a.model, usage, est) if res else 0.0
    files = []
    if res:
        out = Path(a.out); out.parent.mkdir(parents=True, exist_ok=True)
        for i, d in enumerate(res.get("data", [])):
            p = out if a.n == 1 else out.with_name(f"{out.stem}_{i + 1}{out.suffix}")
            if d.get("b64_json"):
                p.write_bytes(base64.b64decode(d["b64_json"]))
            elif d.get("url"):
                p.write_bytes(requests.get(d["url"], timeout=120).content)
            files.append(str(p))
    row = {"t": int(time.time()), "tag": a.tag, "cmd": a.cmd, "model": a.model, "size": a.size,
           "quality": a.quality, "n": a.n, "n_images": len(a.image), "latency": round(latency, 1),
           "rl_wait": round(rl_wait, 1), "usd": round(usd, 4), "usage": usage, "ok": bool(res),
           "dropped_params": dropped, "files": files, "error": err,
           "prompt": prompt, "images": a.image}
    with open(LEDGER, "a", encoding="utf-8") as f:
        f.write(json.dumps(row) + "\n")
    print(json.dumps({"ok": bool(res), "files": files, "latency": row["latency"], "rl_wait": row["rl_wait"],
                      "usd": row["usd"], "spent_total": round(spent(), 3), "dropped_params": dropped,
                      "error": err}))


if __name__ == "__main__":
    main()
