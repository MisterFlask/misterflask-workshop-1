"""Report prose and layout. build.py imports TITLE, ARM_ORDER, ARMS, EXTRA_ASSETS and render(ctx).

Numbers (scores, spend, minutes, metrics) come from the data build.py collects; prose lives in PROSE / DOSSIERS
below and is written after judging.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TITLE = "FfH Animation Sprint"
ANIMS = ["idle", "strike", "flinch", "death"]
ARM_ORDER = ["FINAL", "A0", "R", "G0", "G1", "G2", "K", "C"]
TRIAL_ARMS = ["A0", "R", "G0", "G1", "G2", "K", "C"]
ARMS = {
    "FINAL": {"name": "Recommended recipe", "family": "recipe"},
    "A0": {"name": "Whole-sprite tween (control)", "family": "code"},
    "R": {"name": "Cut-out rig + mesh warp", "family": "code"},
    "G0": {"name": "Sheet, no pose guides", "family": "model"},
    "G1": {"name": "Sheet with pose blockouts", "family": "model"},
    "G2": {"name": "Rig frames repainted", "family": "hybrid"},
    "K": {"name": "Keys, then in-betweens", "family": "model"},
    "C": {"name": "Chained per-frame", "family": "model"},
}

# map-scale strips shown in each dossier (copied by build.py)
EXTRA_ASSETS = {}
for _arm in TRIAL_ARMS + ["FINAL"]:
    _d = ROOT / ("final" if _arm == "FINAL" else f"arms/{_arm}") / "preview"
    for _anim in ANIMS:
        if (_d / f"{_anim}_map1x.png").exists():
            EXTRA_ASSETS[f"{_arm}_{_anim}_map1x.png"] = str((_d / f"{_anim}_map1x.png").relative_to(ROOT))
for _name in ("template_strike_colour.png", "view_sunburst_1.png"):
    if (ROOT / "arms/calib/review" / _name).exists():
        EXTRA_ASSETS[f"calib_{_name}"] = f"arms/calib/review/{_name}"

PROSE = {}       # filled after judging: answer, findings, final, quiz pairs, recipe (see bottom of file for the rest)

PROSE["how"] = """
<div class="cols">
  <div>
    <h3>One shared contract</h3>
    <ul class="plain">
      <li>Every method animated the same accepted M7 swordsman (96×120, figure 86 px, facing lower-left) on a
        160×128 canvas anchored bottom-centre like an M7 unit, so idle frame 0 <em>is</em> the static sprite.</li>
      <li>A written beat sheet fixed the frames and timings: idle 4 frames looping at 250 ms; strike 8 frames,
        about 870 ms, hit on frame 4; flinch 4 frames, about 440 ms; death 8 frames, ending on a held corpse.</li>
      <li>One set of 2D joint positions per frame (<code>poses.json</code>) drove both the pose blockouts and the
        rig, so methods differ only in how the pixels are made.</li>
      <li>Shared code did everything around the pixels: keying the magenta, one scale per sheet calibrated from
        the reference cell, snapping each frame onto the ground row, metrics, previews, and the GIF and MP4
        sequences.</li>
    </ul>
  </div>
  <div>
    <h3>Calibration before the trials</h3>
    <ul class="plain">
      <li>One guided strike sheet went to three image models for $0.36 in 26 s. gpt-image-2.5-sunburst kept the
        reference cell almost untouched (scale 1.004, silhouette overlap 0.97); gpt-image-2.5-flare redrew it and
        drew its bottom row 14 px off the ground; gpt-image-2 scaled everything up 19%.</li>
      <li>Sunburst does not accept <code>input_fidelity</code>. High quality looked the same as medium at twice
        the price. Grey mannequins worked as well as coloured blockouts.</li>
      <li>Frozen for every image-model method: sunburst, medium, 1536×1024, two variants per call, the roster
        sheet as the second image.</li>
    </ul>
  </div>
</div>
"""

PROSE["next"] = """
<ul class="gap-list read">
  <li><strong>Video models.</strong> Sora 2 and Sora 2 Pro were shut down on 24 September 2026 and the
    <code>/v1/videos</code> endpoint returns 404 for this key, so image-to-video-to-frames could not be tried.</li>
  <li><strong>Local diffusion with pose control</strong> (SDXL or Flux with an OpenPose ControlNet and an
    IP-Adapter for identity) on your RTX 4080. This container has no GPU.</li>
  <li><strong>Sprite-animation services</strong> such as PixelLab's skeleton animation or Retro Diffusion. They need
    an account and an API key.</li>
  <li><strong>Other facings.</strong> Every animation here faces lower-left; the engine can mirror for lower-right,
    but the two upper facings need back views.</li>
  <li><strong>A second unit.</strong> The recipe was only proven on the swordsman; a robed caster, an archer and a
    big monster are the obvious next checks.</li>
  <li><strong>Engine integration.</strong> The client does not load M7 sprites yet; the spec here (canvas, anchor,
    manifest with per-frame ms and a hit event) is written so a loader can use it directly.</li>
</ul>
"""
DOSSIERS = {}    # arm -> {verdict, how, worked, failed}


def fmt_money(x):
    return f"${x:.2f}"


def arm(data, aid):
    return next((a for a in data["arms"] if a["id"] == aid), None)


def results_table(ctx):
    data, esc = ctx["data"], ctx["esc"]
    sc = data.get("scores", {})
    head = ("<tr><th>Method</th>" + "".join(f'<th class="num">{a}</th>' for a in ANIMS) +
            '<th class="num">Overall</th><th class="num">Image API</th><th class="num">Calls</th><th class="num">Agent min</th></tr>')
    best = {an: max((sc.get(a, {}).get(an, {}).get("weighted", -1) for a in TRIAL_ARMS), default=-1) for an in ANIMS}
    rows = []
    for aid in TRIAL_ARMS:
        a = arm(data, aid)
        if not a:
            continue
        s = sc.get(aid, {})
        cells = []
        for an in ANIMS:
            v = s.get(an, {}).get("weighted")
            cls = ' class="num best"' if v is not None and v == best[an] else ' class="num"'
            cells.append(f"<td{cls}>{'' if v is None else f'{v:.2f}'}</td>")
        mins = sum(a["minutes"].values()) if a["minutes"] else 0
        rows.append(f'<tr><td><span class="arm-id">{aid}</span>{esc(a["name"])}</td>{"".join(cells)}'
                    f'<td class="num">{s.get("overall", "") if s else ""}</td><td class="num">{fmt_money(a["spend"]["usd"])}</td>'
                    f'<td class="num">{a["spend"]["calls"]}</td><td class="num">{mins:.0f}</td></tr>')
    return f'<div class="tablewrap"><table class="results"><thead>{head}</thead><tbody>{"".join(rows)}</tbody></table></div>'


def metrics_table(ctx):
    data, esc = ctx["data"], ctx["esc"]
    head = "<tr><th>Method</th>" + "".join(f'<th class="num">{a}: non-rigid / overlap / drift</th>' for a in ANIMS) + "</tr>"
    rows = []
    for aid in TRIAL_ARMS + ["FINAL"]:
        a = arm(data, aid)
        if not a:
            continue
        cells = []
        for an in ANIMS:
            m = (a.get("metrics") or {}).get(an)
            cells.append('<td class="num"></td>' if not m else
                         f'<td class="num">{m["nonrigid_mean"]:.2f} / {m["overlap_index"]:.1f} / {m["drift_mean"]:.1f}</td>')
        rows.append(f'<tr><td><span class="arm-id">{aid}</span>{esc(a["name"])}</td>{"".join(cells)}</tr>')
    return f'<div class="tablewrap"><table class="results"><thead>{head}</thead><tbody>{"".join(rows)}</tbody></table></div>'


def dossier(ctx, aid):
    data, esc = ctx["data"], ctx["esc"]
    a = arm(data, aid)
    if not a:
        return ""
    d = DOSSIERS.get(aid, {})
    s = data.get("scores", {}).get(aid, {})
    mins = sum(a["minutes"].values()) if a["minutes"] else 0
    vids = ""
    if "sequence_3x.mp4" in a["files"]:
        vids += (f'<figure><video src="{a["files"]["sequence_3x.mp4"]}" controls loop muted playsinline preload="metadata"></video>'
                 '<figcaption>Full sequence at 3×: idle, strike, idle, flinch, idle, death, corpse hold</figcaption></figure>')
    if "vignette.mp4" in a["files"]:
        vids += (f'<figure><video src="{a["files"]["vignette.mp4"]}" controls loop muted playsinline preload="metadata"></video>'
                 '<figcaption>Combat vignette at 2×: blue strikes, red flinches; blue strikes again, red dies</figcaption></figure>')
    strips = "".join(f'<figure><div class="strip"><img src="assets/{aid}_{an}_map1x.png" alt="{esc(a["name"])}, {an}, every frame at game scale" loading="lazy"></div><figcaption>{an} at 1×</figcaption></figure>'
                     for an in ANIMS if f"{aid}_{an}_map1x.png" in EXTRA_ASSETS)
    para = lambda k, lab: f"<p><strong>{lab}</strong> {d[k]}</p>" if d.get(k) else ""
    return f"""
<section class="method" id="m-{aid.lower()}">
  <header class="mhead"><span class="badge">{aid}</span><div><h3>{esc(a["name"])}</h3><p class="verdict">{d.get("verdict", "")}</p></div></header>
  <dl class="facts"><div><dt>Judge score</dt><dd>{s.get("overall", "–") if s else "–"}</dd></div><div><dt>Animations</dt><dd>{len(a["anims"])} of 4</dd></div>
  <div><dt>Image API</dt><dd>{fmt_money(a["spend"]["usd"])}</dd></div><div><dt>Calls</dt><dd>{a["spend"]["calls"]}</dd></div><div><dt>Agent time</dt><dd>{mins:.0f} min</dd></div></dl>
  <div class="prose">{para("how", "How it worked.")}{para("worked", "What came out well.")}{para("failed", "What went wrong.")}</div>
  {f'<div class="videos">{vids}</div>' if vids else ""}
  {f'<details><summary>Every frame at game scale</summary><div>{strips}</div></details>' if strips else ""}
</section>"""


def render(ctx):
    g, data = ctx["grid"], ctx["data"]
    P = PROSE
    trial_present = [a for a in TRIAL_ARMS if arm(data, a)]
    return f"""
<header class="top wrap">
  <p class="eyebrow">Research sprint · Fall from Heaven port · 20 October 2026</p>
  <h1>{TITLE}</h1>
  <p class="lede">{P.get("lede", "Draft: results are still coming in.")}</p>
  <nav class="toc" aria-label="Sections"><a href="#answer">Short answer</a><a href="#final">Recommended set</a><a href="#compare">Side by side</a><a href="#quiz">Less stiff?</a><a href="#results">Results</a><a href="#methods">Method by method</a><a href="#recipe">Pipeline</a><a href="#how">How it ran</a><a href="#next">Not tested</a></nav>
</header>
<main>
<section class="block wrap" id="answer"><h2>The short answer</h2>{P.get("answer", "")}</section>
<section class="block wrap" id="final"><h2>The recommended set</h2>{P.get("final", "")}</section>
<section class="block wrap" id="compare"><h2>Every method, side by side</h2>
  <p class="read">{P.get("compare_intro", "All players share one clock. Pause to step through frames; zoom changes every cell.")}</p>
  {g(trial_present, zoom=1)}</section>
<section class="block wrap" id="quiz"><h2>Which looks less stiff?</h2>{P.get("quiz", "")}</section>
<section class="block wrap" id="results"><h2>Results</h2>{P.get("results_intro", "")}{results_table(ctx)}{P.get("results_note", "")}
  <h3>Measured motion</h3>{P.get("metrics_intro", "")}{metrics_table(ctx)}</section>
<section class="block wrap" id="methods"><h2>Method by method</h2>{P.get("methods_intro", "")}{"".join(dossier(ctx, a) for a in P.get("method_order", trial_present))}</section>
<section class="block wrap" id="recipe"><h2>Recommended pipeline</h2>{P.get("recipe", "")}</section>
<section class="block wrap" id="how"><h2>How the sprint ran</h2>{P.get("how", "")}</section>
<section class="block wrap" id="next"><h2>Not tested yet</h2>{P.get("next", "")}</section>
</main>
<footer class="wrap"><p>{P.get("footer", "")}</p></footer>
"""
