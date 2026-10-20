"""Build the sprint report: report/site/index.html + report/site/assets/* (published as an HTML artifact).

  python report/build.py          # collects sheets, manifests, metrics, scores, spend, timings; writes report/site/

Prose lives in report/content.py (written after judging); this file only assembles data and layout.
"""
import html, json, shutil, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "lib"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import content as C  # noqa: E402

SITE = ROOT / "report" / "site"
ASSETS = SITE / "assets"
ANIMS = ["idle", "strike", "flinch", "death"]


def esc(s):
    return html.escape(str(s), quote=True)


def load_json(p, default=None):
    p = Path(p)
    return json.loads(p.read_text()) if p.exists() else default


def spend_by_arm():
    out = {}
    p = ROOT / "spend.jsonl"
    if p.exists():
        for line in p.read_text().splitlines():
            if not line.strip():
                continue
            r = json.loads(line)
            tag = r.get("tag", "")
            parts = tag.split("/")
            if len(parts) > 1:
                out.setdefault(parts[1], {"usd": 0.0, "calls": 0, "images": 0})
                out[parts[1]]["usd"] += r.get("usd", 0)
                out[parts[1]]["calls"] += 1
                out[parts[1]]["images"] += r.get("n", 1) if r.get("ok") else 0
    return out


def minutes_by_arm(arm_dir):
    """Minutes per logged item, plus "_wall": agent minutes with double counting removed. A window that lies fully
    inside another logged window (one agent logging two sheets it ran in parallel, or a probe inside a larger task)
    is dropped; separate agents working at the same time still add up, because that is real agent time."""
    p = arm_dir / "timings.jsonl"
    if not p.exists():
        return {}
    out, spans = {}, []
    for line in p.read_text().splitlines():
        if line.strip():
            r = json.loads(line)
            out[r["what"]] = out.get(r["what"], 0) + r.get("seconds", 0) / 60
            if r.get("start") and r.get("end"):
                spans.append((r["start"], r["end"]))
    spans = sorted(set(spans), key=lambda w: (w[0], -w[1]))
    keep = [w for i, w in enumerate(spans)
            if not any(j != i and o[0] <= w[0] and w[1] <= o[1] and o != w for j, o in enumerate(spans))]
    out["_wall"] = sum(b - a for a, b in keep) / 60
    return out


def collect():
    if SITE.exists():
        shutil.rmtree(SITE)
    ASSETS.mkdir(parents=True)
    shutil.copy(ROOT / "m7/roster/terrain/tile_grass.png", ASSETS / "tile_grass.png")
    spend = spend_by_arm()
    data = {"tile": "assets/tile_grass.png", "arms": []}
    for arm in C.ARM_ORDER:
        d = ROOT / getattr(C, "ARM_DIRS", {}).get(arm, f"arms/{arm}")
        if not d.exists():
            continue
        entry = {"id": arm, "name": C.ARMS[arm]["name"], "family": C.ARMS[arm]["family"], "anims": {}, "files": {}}
        for anim in ANIMS:
            mf = d / anim / "manifest.json"
            if not mf.exists():
                continue
            man = json.loads(mf.read_text())
            src = d / f"sheet_{anim}.png"
            dst = ASSETS / f"{arm}_{anim}.png"
            shutil.copy(src, dst)
            hit = next((i for i, f in enumerate(man["frames"]) if "hit" in f.get("events", [])), None)
            entry["anims"][anim] = {"sheet": f"assets/{dst.name}", "ms": [f["ms"] for f in man["frames"]], "hit": hit,
                                    "loop": man["loop"], "hold": man["hold_last"]}
        for name, t in (("sequence_3x.mp4", "2.55"), ("vignette.mp4", "1.55")):
            if (d / name).exists():
                dst = ASSETS / f"{arm}_{name}"
                shutil.copy(d / name, dst)
                entry["files"][name] = f"assets/{dst.name}"
                poster = ASSETS / f"{arm}_{name[:-4]}_poster.jpg"
                subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", t, "-i", str(d / name), "-frames:v", "1", "-q:v", "3",
                                str(poster)], check=True)
                entry["files"][name + ":poster"] = f"assets/{poster.name}"
        entry["metrics"] = {a: v.get("summary") for a, v in (load_json(d / "metrics.json", {}) or {}).items()}
        entry["spend"] = spend.get(arm, {"usd": 0.0, "calls": 0, "images": 0})
        entry["minutes"] = minutes_by_arm(d)
        data["arms"].append(entry)
    for name, src in getattr(C, "EXTRA_ASSETS", {}).items():
        shutil.copy(ROOT / src, ASSETS / name)
    data["scores"] = (load_json(ROOT / "judging/scores.json", {}) or {}).get("by_arm", {})
    data["spend_total"] = round(sum(v["usd"] for v in spend.values()), 2)
    return data


CSS = (Path(__file__).resolve().parent / "style.css").read_text()
JS = (Path(__file__).resolve().parent / "player.js").read_text()


def grid(data, arms, anims=ANIMS, zoom=2, gid="grid"):
    present = [a for a in data["arms"] if a["id"] in arms]
    head = "".join(f'<th scope="col">{esc(x)}</th>' for x in anims)
    rows = []
    for a in present:
        cells = []
        for an in anims:
            if an in a["anims"]:
                cells.append(f'<td><figure class="cell"><canvas data-arm="{a["id"]}" data-anim="{an}" width="{128 * zoom}" '
                             f'height="{128 * zoom}" aria-label="{esc(a["name"])}, {an}"></canvas>'
                             f'<figcaption data-frame></figcaption></figure></td>')
            else:
                cells.append('<td><p class="none">not made</p></td>')
        rows.append(f'<tr><th scope="row"><span class="arm-id">{esc(a["id"])}</span>{esc(a["name"])}</th>{"".join(cells)}</tr>')
    return (f'<div class="player" data-clock data-zoom="{zoom}">{controls()}'
            f'<div class="gridwrap"><table class="anim-grid"><thead><tr><th></th>{head}</tr></thead>'
            f'<tbody>{"".join(rows)}</tbody></table></div></div>')


def controls():
    return ('<div class="controls" role="toolbar" aria-label="Playback">'
            '<button type="button" data-ctl="play">Pause</button>'
            '<button type="button" data-ctl="prev" aria-label="Previous frame">◀ frame</button>'
            '<button type="button" data-ctl="next" aria-label="Next frame">frame ▶</button>'
            '<button type="button" data-ctl="restart">Restart</button>'
            '<span class="sep"></span>'
            '<button type="button" data-ctl="zoom" data-v="1">1×</button>'
            '<button type="button" data-ctl="zoom" data-v="2">2×</button>'
            '<button type="button" data-ctl="zoom" data-v="3">3×</button>'
            '<button type="button" data-ctl="tile">Grass tile</button></div>')


def build():
    data = collect()
    ctx = {"data": data, "grid": lambda arms, **kw: grid(data, arms, **kw), "esc": esc}
    body = C.render(ctx)
    page = (f'<title>{esc(C.TITLE)}</title>\n<meta name="claude:viewport-fit-sides" content="cover">\n'
            '<link rel="preconnect" href="https://fonts.googleapis.com">\n'
            '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>\n'
            '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500&'
            'family=IBM+Plex+Sans:ital,wght@0,400;0,500;0,600;1,400&family=Pixelify+Sans:wght@500;700&display=swap">\n'
            f'<style>{CSS}</style>\n{body}\n'
            f'<script>window.SPRINT = {json.dumps(data, separators=(",", ":"))};</script>\n<script>{JS}</script>\n')
    (SITE / "index.html").write_text(page, encoding="utf-8")
    return SITE / "index.html"


if __name__ == "__main__":
    print(build())
