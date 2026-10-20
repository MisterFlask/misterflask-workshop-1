"""Side probe (arm C): does the OpenAI Responses API with the image_generation tool keep a character consistent
across turns? Turn 1 = the true sprite (5x on magenta, base64 data URL) + a request for strike frame 1; turn 2 =
previous_response_id + a request for frame 2. Outputs in work/C/resp/. One spend.jsonl row per call (tag anim/C,
cmd "responses"), usd computed conservatively from the returned usage at $10/M input and $40/M output tokens for
everything (model tokens and any image-tool tokens the response reports). Not used for the arm's frames.

  python3 arms/C/resp_probe.py
"""
import base64, json, sys, time
from pathlib import Path
import requests

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "m7/lib"))
sys.path.insert(0, str(ROOT / "lib"))
import imggen  # noqa: E402
import beats, gen  # noqa: E402

TAG = "anim/C"
OUT = ROOT / "work/C/resp"
MODEL = "gpt-5.5"
TOOL = {"type": "image_generation", "quality": "medium", "size": "1536x1024"}
URL = "https://api.openai.com/v1/responses"
HEAD = {"Authorization": "Bearer proxy", "Content-Type": "application/json"}
SPRITE = ROOT / "work/C/ref_sprite_5x.png"
IN_PRICE, OUT_PRICE = 10.0, 40.0


def tokens(usage):
    """Sum every input-ish and output-ish token count found anywhere in the usage dicts."""
    tin = tout = 0
    if not usage:
        return 0, 0
    for k, v in usage.items():
        if isinstance(v, dict):
            a, b = tokens(v) if not k.endswith("_details") else (0, 0)
            tin, tout = tin + a, tout + b
        elif isinstance(v, (int, float)):
            if k == "input_tokens":
                tin += v
            elif k == "output_tokens":
                tout += v
    return tin, tout


def log(row):
    with open(imggen.LEDGER, "a", encoding="utf-8") as f:
        f.write(json.dumps(row) + "\n")


def call(body, name, prompt, images):
    imggen.check_budget(TAG, 0.30)
    t0 = time.time()
    err, data = None, None
    try:
        r = requests.post(URL, headers=HEAD, json=body, timeout=400)
        if r.status_code == 200:
            data = r.json()
        else:
            err = {"status": r.status_code, "error": r.text[:800]}
    except requests.RequestException as e:
        err = {"error": f"network: {e}"}
    lat = time.time() - t0
    files, usage, tool_usage = [], None, None
    if data:
        (OUT / f"{name}.json").write_text(json.dumps(
            {k: v for k, v in data.items() if k != "output"} | {"output": [
                {kk: (vv if kk != "result" else f"<{len(vv or '')} b64 chars>") for kk, vv in o.items()}
                for o in data.get("output", [])]}, indent=1))
        for i, o in enumerate(data.get("output", [])):
            if o.get("type") == "image_generation_call" and o.get("result"):
                p = OUT / f"{name}_{i}.png"
                p.write_bytes(base64.b64decode(o["result"]))
                files.append(str(p))
        usage = data.get("usage")
        tool_usage = data.get("tool_usage")
    u = {"usage": usage, "tool_usage": tool_usage}
    tin, tout = tokens(usage or {})
    tin_t, tout_t = tokens(tool_usage or {})
    usd = (tin + tin_t) * IN_PRICE / 1e6 + (tout + tout_t) * OUT_PRICE / 1e6
    est_note = None
    if data and files and tout_t == 0:
        # the image tool's own tokens were not reported: add the imggen estimate for one medium 1536x1024 image
        # so the ledger stays conservative
        usd += imggen.OUT_TOKENS["medium"] * 1.5 * OUT_PRICE / 1e6
        est_note = "image tool tokens not reported; added 1584 output tokens at $40/M"
    row = {"t": int(time.time()), "tag": TAG, "cmd": "responses", "model": MODEL, "size": TOOL["size"],
           "quality": TOOL["quality"], "n": 1, "n_images": len(images), "latency": round(lat, 1), "rl_wait": 0.0,
           "usd": round(usd, 4), "usage": u, "ok": bool(files), "dropped_params": [], "files": files,
           "error": err, "prompt": prompt, "images": [str(i) for i in images], "note": est_note,
           "response_id": (data or {}).get("id")}
    log(row)
    print(json.dumps({k: row[k] for k in ("ok", "files", "latency", "usd", "usage", "error", "note")})[:1500])
    return data, row


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    b64 = base64.b64encode(SPRITE.read_bytes()).decode()
    p1 = (f"This is a finished pixel-art sprite from my isometric fantasy strategy game, shown enlarged 5x on a flat "
          f"magenta background: {gen.CHARACTER}, drawn in {gen.STYLE}, standing in a 3/4 view facing the lower-left. "
          f"I am making an 8-frame STRIKE animation of him; this sprite is frame 0 (the ready pose). Generate frame 1: "
          f"the exact same character (same face, hair, outfit, colours, proportions, sword and shield, same pixel size "
          f"and outline style, same size on screen), still facing the lower-left, in this pose: "
          f"{beats.BEATS['strike'][1]['text']}. The sword stays in his right hand (the far arm, screen-left) and the "
          f"shield on his left arm (screen-right). One figure only, standing on the same ground line, centred. Flat "
          f"solid pure magenta #FF00FF background, no ground, shadow, text, motion lines or effects.")
    body1 = {"model": MODEL, "tools": [TOOL], "tool_choice": {"type": "image_generation"},
             "input": [{"role": "user", "content": [{"type": "input_text", "text": p1},
                                                    {"type": "input_image", "image_url": f"data:image/png;base64,{b64}"}]}]}
    d1, r1 = call(body1, "turn1", p1, [SPRITE])
    if not d1:
        # retry once without tool_choice in case the forced tool choice is what was refused
        body1.pop("tool_choice")
        d1, r1 = call(body1, "turn1b", p1, [SPRITE])
    if not d1:
        return
    p2 = (f"Now generate frame 2 of the same strike animation: the exact same character as in the image you just "
          f"made (same design, colours, proportions, pixel size and size on screen), still facing the lower-left, in "
          f"this pose: {beats.BEATS['strike'][2]['text']}. Same framing and ground line as frame 1, one figure, flat "
          f"solid pure magenta #FF00FF background, no effects or text.")
    body2 = {"model": MODEL, "tools": [TOOL], "previous_response_id": d1["id"], "input": p2}
    if "tool_choice" in body1:
        body2["tool_choice"] = {"type": "image_generation"}
    call(body2, "turn2", p2, [])


if __name__ == "__main__":
    main()
