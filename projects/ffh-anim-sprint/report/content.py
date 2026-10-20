"""Report prose and layout. build.py imports TITLE, ARM_ORDER, ARMS, ARM_DIRS, EXTRA_ASSETS and render(ctx).

Numbers (scores, spend, minutes, metrics) come from the data build.py collects plus judging/verify_scores.json;
the prose below was written after both judging rounds.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TITLE = "FfH Animation Sprint"
ANIMS = ["idle", "strike", "flinch", "death"]
ARM_ORDER = ["FINAL", "FN", "HYB", "A0", "R", "G0", "G1", "G2", "K", "C"]
TRIAL_ARMS = ["A0", "R", "G0", "G1", "G2", "K", "C"]
ARM_DIRS = {"FINAL": "final", "FN": "final_nofx", "HYB": "final_hyborem_v1"}
ARMS = {
    "FINAL": {"name": "Recommended recipe", "family": "recipe"},
    "FN": {"name": "Recommended recipe (no FX)", "family": "recipe"},
    "HYB": {"name": "Recipe on Hyborem", "family": "recipe"},
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
for _arm in TRIAL_ARMS + ["FINAL", "HYB"]:
    _d = ROOT / ARM_DIRS.get(_arm, f"arms/{_arm}") / "preview"
    for _anim in ANIMS:
        if (_d / f"{_anim}_map1x.png").exists():
            EXTRA_ASSETS[f"{_arm}_{_anim}_map1x.png"] = str((_d / f"{_anim}_map1x.png").relative_to(ROOT))
for _name in ("template_strike_colour.png", "view_sunburst_1.png"):
    if (ROOT / "arms/calib/review" / _name).exists():
        EXTRA_ASSETS[f"calib_{_name}"] = f"arms/calib/review/{_name}"
if (ROOT / "work/final/template_strike.png").exists():
    EXTRA_ASSETS["recipe_template_strike.png"] = "work/final/template_strike.png"
if (ROOT / "work/final/raw_strike_2.png").exists():
    EXTRA_ASSETS["recipe_raw_strike.png"] = "work/final/raw_strike_2.png"

VERIFY = json.loads((ROOT / "judging/verify_scores.json").read_text()) if (ROOT / "judging/verify_scores.json").exists() else {}


def fmt_money(x):
    return f"${x:.2f}"


def arm(data, aid):
    return next((a for a in data["arms"] if a["id"] == aid), None)


def score(data, aid, anim=None):
    s = data.get("scores", {}).get(aid, {})
    if anim is None:
        return s.get("overall")
    return s.get(anim, {}).get("weighted")


def vscore(aid, anim):
    return VERIFY.get("by_arm", {}).get(aid, {}).get(anim, {}).get("weighted")


def f2(x):
    return "–" if x is None else f"{x:.2f}"


# ---------------------------------------------------------------- tables
def results_table(ctx):
    data, esc = ctx["data"], ctx["esc"]
    sc = data.get("scores", {})
    head = ("<tr><th>Method</th>" + "".join(f'<th class="num">{a}</th>' for a in ANIMS) +
            '<th class="num">Mean</th><th class="num">Image API</th><th class="num">Calls</th><th class="num">Agent min</th></tr>')
    rows_ids = TRIAL_ARMS + ["final_nofx"]
    best = {an: max((sc.get(a, {}).get(an, {}).get("weighted", -1) for a in rows_ids), default=-1) for an in ANIMS}
    rows = []
    for aid in rows_ids:
        a = arm(data, "FINAL" if aid == "final_nofx" else aid)
        s = sc.get(aid, {})
        if not a or not s:
            continue
        name = "Recommended recipe, first build (no FX)" if aid == "final_nofx" else a["name"]
        cells = []
        for an in ANIMS:
            v = s.get(an, {}).get("weighted")
            cls = ' class="num best"' if v is not None and v == best[an] else ' class="num"'
            cells.append(f"<td{cls}>{'' if v is None else f'{v:.2f}'}</td>")
        spend = a["spend"] if aid != "final_nofx" else {"usd": 0.232, "calls": 3}
        mins = a["minutes"].get("_wall", 0) if aid != "final_nofx" else None
        tag = "FN" if aid == "final_nofx" else aid
        rows.append(f'<tr><td><span class="arm-id">{tag}</span>{esc(name)}</td>{"".join(cells)}'
                    f'<td class="num">{s.get("overall", ""):.2f}</td><td class="num">{fmt_money(spend["usd"])}</td>'
                    f'<td class="num">{spend["calls"]}</td><td class="num">{"–" if mins is None else f"{mins:.0f}"}</td></tr>')
    return f'<div class="tablewrap"><table class="results"><thead>{head}</thead><tbody>{"".join(rows)}</tbody></table></div>'


def verify_table(ctx):
    esc = ctx["esc"]
    if not VERIFY:
        return '<p class="note">Verification round still running.</p>'
    names = {"final_nofx": "Recipe, fixed (no FX)", "final_v1_nofx": "Recipe, first build (no FX)", "R": "Cut-out rig + mesh warp",
             "A0": "Whole-sprite tween", "G0": "Sheet, no pose guides", "G2": "Rig frames repainted", "K": "Keys, then in-betweens"}
    rows = []
    for anim in ANIMS:
        ent = sorted(VERIFY["by_anim"][anim].items(), key=lambda kv: -kv[1]["weighted"])
        for i, (aid, v) in enumerate(ent):
            rows.append(f'<tr><td>{anim if i == 0 else ""}</td><td>{esc(names.get(aid, aid))}</td>'
                        f'<td class="num{" best" if i == 0 else ""}">{v["weighted"]:.2f}</td><td class="num">{v["mean_rank"]:.2f}</td>'
                        f'<td class="num">{v["motion"]:.1f}</td><td class="num">{v["consistency"]:.1f}</td>'
                        f'<td class="num">{v["style"]:.1f}</td><td class="num">{v["appeal"]:.1f}</td></tr>')
    head = ('<tr><th>Animation</th><th>Method</th><th class="num">Score</th><th class="num">Mean rank</th><th class="num">Motion</th>'
            '<th class="num">Consistency</th><th class="num">Style</th><th class="num">Appeal</th></tr>')
    return f'<div class="tablewrap"><table class="results"><thead>{head}</thead><tbody>{"".join(rows)}</tbody></table></div>'


def metrics_table(ctx):
    data, esc = ctx["data"], ctx["esc"]
    head = "<tr><th>Method</th>" + "".join(f'<th class="num">{a}</th>' for a in ANIMS) + "</tr>"
    rows = []
    for aid in TRIAL_ARMS + ["FN"]:
        a = arm(data, aid)
        if not a:
            continue
        cells = []
        for an in ANIMS:
            m = (a.get("metrics") or {}).get(an)
            if not m:
                cells.append('<td class="num"></td>')
                continue
            extra = f' · boil {m["boil_mean"]:.0f}' if an == "idle" else ""
            cells.append(f'<td class="num">{m["nonrigid_mean"]:.2f} · {m["overlap_index"]:.1f} · {m["drift_mean"]:.1f}{extra}</td>')
        rows.append(f'<tr><td><span class="arm-id">{aid}</span>{esc(a["name"])}</td>{"".join(cells)}</tr>')
    return f'<div class="tablewrap"><table class="results"><thead>{head}</thead><tbody>{"".join(rows)}</tbody></table></div>'


# ---------------------------------------------------------------- players
def feature(ctx, aid, zoom=3):
    data, esc = ctx["data"], ctx["esc"]
    a = arm(data, aid)
    if not a:
        return ""
    cells = "".join(f'<figure class="cell"><canvas data-arm="{aid}" data-anim="{an}" width="{128 * zoom}" height="{128 * zoom}" '
                    f'aria-label="{esc(a["name"])}, {an}"></canvas><figcaption><strong>{an}</strong> · <span data-frame></span></figcaption></figure>'
                    for an in ANIMS if an in a["anims"])
    return (f'<div class="player" data-clock data-zoom="{zoom}">{controls_html()}<div class="feature">{cells}</div></div>')


def controls_html():
    return ('<div class="controls" role="toolbar" aria-label="Playback">'
            '<button type="button" data-ctl="play">Pause</button>'
            '<button type="button" data-ctl="prev" aria-label="Previous frame">◀ frame</button>'
            '<button type="button" data-ctl="next" aria-label="Next frame">frame ▶</button>'
            '<button type="button" data-ctl="restart">Restart</button><span class="sep"></span>'
            '<button type="button" data-ctl="zoom" data-v="1">1×</button><button type="button" data-ctl="zoom" data-v="2">2×</button>'
            '<button type="button" data-ctl="zoom" data-v="3">3×</button><button type="button" data-ctl="tile">Grass tile</button></div>')


def videos(ctx, aid, which=("sequence_3x.mp4", "vignette.mp4"), autoplay=False):
    a = arm(ctx["data"], aid)
    if not a:
        return ""
    cap = {"sequence_3x.mp4": "Full sequence at 3×: idle, strike, idle, flinch, idle, death, corpse hold",
           "vignette.mp4": "Combat vignette at 2×: blue strikes, red flinches; blue strikes again, red dies"}
    attrs = "autoplay muted loop playsinline controls" if autoplay else 'muted loop playsinline controls preload="none"'
    out = "".join(f'<figure><video src="{a["files"][w]}" poster="{a["files"].get(w + ":poster", "")}" {attrs}></video>'
                  f'<figcaption>{cap[w]}</figcaption></figure>' for w in which if w in a["files"])
    return f'<div class="videos">{out}</div>' if out else ""


QUIZ = [  # (anim, left arm, right arm): which looks less stiff? Recipe without FX so effects do not decide it;
    # left/right mixed so the answer is not always one side
    ("strike", "FN", "A0"),
    ("death", "R", "FN"),
    ("idle", "G1", "FN"),
    ("strike", "R", "G2"),
]


def quiz(ctx):
    data, esc = ctx["data"], ctx["esc"]
    out = []
    for i, (anim, L, Rr) in enumerate(QUIZ, 1):
        if not (arm(data, L) and arm(data, Rr)):
            continue
        out.append(f'''<div class="quiz player" data-clock data-zoom="2" data-quiz data-left="{L}" data-right="{Rr}">
  <h3>Pair {i}: {anim}</h3>
  <div class="pair"><figure class="cell"><canvas data-arm="{L}" data-anim="{anim}" width="256" height="256" aria-label="Left {anim}"></canvas><figcaption data-frame></figcaption></figure>
  <figure class="cell"><canvas data-arm="{Rr}" data-anim="{anim}" width="256" height="256" aria-label="Right {anim}"></canvas><figcaption data-frame></figcaption></figure></div>
  <div class="picks"><button type="button" data-pick="L">Left is less stiff</button><button type="button" data-pick="R">Right is less stiff</button></div>
  <p data-reveal hidden></p></div>''')
    return ('<p class="read">Agents judged stills; motion is yours to judge. Each pair plays the same animation by two '
            'methods, unlabelled. Pick the one that looks less like a moved cut-out, then see which was which.</p>'
            f'<div class="quizzes">{"".join(out)}</div>')


# ---------------------------------------------------------------- dossiers
DOSSIERS = {
    "A0": {
        "verdict": "The control, and the failure you described: moving the whole sprite cannot draw a new pose. Last for strike, flinch and death in every judge's ranking.",
        "how": "Built in code in a few minutes, no image-model calls. Every frame is the static sprite rotated, sheared, squashed or slid about the feet, following the same beats as every other method: a 1–2% vertical stretch for breathing, a lean back and lunge with shear for the strike, a recoil for the flinch, and a tip-over that ends lying along the isometric axis for the death. Like every round-1 method it carries no smear, flash or fade.",
        "worked": "Idle: a gentle stretch was the least bad use of the method (6.28 in round 1), because breathing is the one action that really is a small whole-body change. No new art is introduced, though rotating and shearing resamples the sprite, so outlines shimmer a little from frame to frame.",
        "failed": "The sword never leaves the ready grip, so the strike has no wind-up, swing or contact (judges: “the whole sprite just slides left and right as a cut-out”). Judges could put its shuffled strike frames back in order only 50–75% of the time, against 100% for most other methods. The death is a rigid figure tilting over with the sword still raised; rotated outlines go jagged and the lying frames sink into the tile.",
    },
    "R": {
        "verdict": "A well-made puppet: the best idle of the trials, and the right tool for small motion, but it still reads as a cut-out in the big actions.",
        "how": "The sprite was cut into eleven parts (head, torso, upper and lower arms with the sword, the shield arm, thighs, shins with boots) with polygon masks. Pixels hidden behind other parts were filled in code. Each frame rotates the parts about their joints to match <code>poses.json</code>, draws the blade in code at any angle, and uses a rigid mesh warp (moving least squares) for leans and squash. Idle is a sharpened mesh warp of the whole sprite, with the sword and shield moving one frame late as rigid layers. No image-model calls.",
        "worked": "Idle won round 1 (8.07): the breath follows the beat exactly, loops with no pop and never changes a pixel it does not have to. The character stays identical in every frame (colour drift 0.41 on the strike, the lowest of any method). Limbs and sword get real overlapping action (overlap index 1.30).",
        "failed": "It cannot draw what the sprite does not contain. The face and torso never change, a sleeve rotated 180° reads as a lump, stretched forearms go thin, and the kneel shows upside-down shins. Judges saw resampled, softer frames popping against the crisp frame 0 and a corpse that is “the standing sprite rotated and squashed”. Strike 4.95, flinch 5.20, death 3.97.",
    },
    "G0": {
        "verdict": "The model's own posing is excellent: the best strike of the trials. It is also the least predictable, which is why the recipe adds guides.",
        "how": "Each image call draws a 4×2 sprite sheet: the strike on one, idle and flinch together on a second, the death on a third. Cell 1 holds the real sprite at 3×, and the other cells are empty magenta. The model poses the character from the beat-sheet text alone. Shared code keys the magenta, takes one scale from the model's redraw of cell 1 and snaps every frame onto the ground row. Two variants per call, up to three rounds of rewording, best variant picked by eye.",
        "worked": "Strike 7.60, the top round-1 strike score: clear anticipation, the blade behind the head on the wind-up, a level swing, a low lunge on contact and a follow-through near the ground. Facing and handedness were right in all 42 cells it generated for the strike.",
        "failed": "Nothing tells the model where the ground or the canvas edge is: every bottom row floated 27–42 px high, one variant in two came back 6–7% bigger than cell 1, and the raised blade was cropped. Idle boiled badly (3.80): each “breathing” frame is a fresh drawing with a different face, fist and stance.",
    },
    "G1": {
        "verdict": "The same sheet with flat-colour pose blockouts: predictable placement and size on the first call, slightly tamer posing. The recipe is built on it.",
        "how": "As G0, but each cell holds a flat-colour blockout of that frame's pose, drawn from <code>poses.json</code> in the character's own colours. The prompt asks the model to redraw the character from cell 1 in the pose of each blockout and to replace the blockout entirely. Calibration ran this exact setup once before the trials and it worked on the first call.",
        "worked": "Frames landed on the ground line at the sprite's size: standing frames 85–87 px against the sprite's 86, sword tips within 1–7 px of the poses. Strike 7.17 and death 7.00, with only one call needed for the death sheet.",
        "failed": "Like every redraw, the idle boils (4.13). The poses follow the blockouts closely, which makes them a little tamer than G0's (“timid legs in the anticipation”). Four of its six strike variants pushed the blade tip outside the safe region (one also clipped it at the top of the canvas) until reworded, a thin magenta fringe survives keying, and the chosen flinch has a brown strap artifact on the shorts.",
    },
    "G2": {
        "verdict": "The most consistent model method: the rig's poses, redrawn as clean pixel art. Strong everywhere except idle, and it needs a rig per unit.",
        "how": "The same one-call sheet, but each cell holds arm R's finished rig frame for that beat. The prompt asks the model to keep the draft's pose and position and redraw it as hand-made pixel art: repair joints, let hair, tunic and limbs bend.",
        "worked": "Second-best strike (7.35) and flinch (7.20) in round 1, and the steadiest across animations (6.88 mean). It fixed the rig's weakest frames: a real kneel, a forward slump and a side-view corpse face. Asking it to copy the sprite's own pixels halved the idle shimmer compared with G1's blockouts.",
        "failed": "It inherits the rig's poses, including the tame ones, and loses some of the rig's overlapping action. The idle still shimmers (5.78). It costs a rig build for every unit, which the other model methods do not need.",
    },
    "K": {
        "verdict": "Keys first, then in-betweens drawn against the finished keys: the best death of round 1, from two passes per animation (7 calls for strike and death, about what the single-sheet methods used).",
        "how": "Pass 1 draws only the key poses (strike: wind-up, contact, follow-through; death: impact, kneel, hitting the ground) on the guided sheet. Pass 2 pastes the accepted keys back into their cells as finished art, puts blockouts in the remaining cells, and asks for in-betweens that connect them.",
        "worked": "Death 7.62, ranked first by all three round-1 judges, with the sword visibly spinning out of the hand on frame 4; it won on consistency and style, while G0's fall scored higher on motion. The model kept the pasted keys almost unchanged in pass 2, so the in-betweens really are drawn against fixed neighbours.",
        "failed": "Two passes per animation and a by-eye pick in each. One of the two heaviest magenta halos (the kind of fringe the recipe's de-fringe step now removes), the shield flipping to its back side in two strike frames, and a corpse lying flatter than the isometric axis. Made only strike and death.",
    },
    "C": {
        "verdict": "Chaining frames one call at a time carries drift forward. Its strike took three times the calls and about three and a half times the cost of a sheet method's three-round strike, and was no more consistent.",
        "how": "Each frame came from its own call: a two-cell template with the previous accepted frame in cell 1 and the next pose's blockout in cell 2, plus the real sprite as a separate reference image. Strike only. A side probe tried the Responses API's multi-turn image tool.",
        "worked": "Each step on its own is a clean continuation, and the strike reads (6.63).",
        "failed": "Small changes accumulate: the shield grows frame by frame (520 to 691 px), the hair jumps 18% on the first redraw, outlines turn purple and the last frame pops back to idle. Nine calls for one animation, where a sheet takes one call per attempt. The Responses-API probe worked mechanically but drew the character about ten times larger, mirrored and older, with no shared registration, so its frames could not be used.",
    },
    "FINAL": {
        "verdict": "The recommendation: draw the big actions with the model on a guided sheet, and warp the real sprite for idle and for the frames next to idle.",
        "how": "Three guided-sheet calls in parallel (strike, flinch, death), cell 1 the real sprite and the other cells blockouts from the poses. Idle and the last frame of the strike and of the flinch are the real sprite bent by a sharpened mesh warp toward the pose, so nothing boils and nothing pops when the unit returns to idle. Death frame 0 is the flinch's impact frame. Code then removes the magenta keying fringe and, for the shipped look, adds a smear arc and impact spark to the strike and a hit flash to flinch and death. Two commands: <code>python3 lib/recipe.py --unit units/swordsman.json --out final</code>, then a free re-cut with <code>--rebuild --pick strike=2</code> after looking at both variants (the automatic score had picked the smudged one).",
        "worked": "In round 1 its first build was the best complete set (7.28 mean over four animations), first for flinch and second for idle. After the fixes it had the top mean score from three fresh blind judges for idle (7.90), strike (7.68) and flinch (7.62), each time ranked first by two of the three, and for death (7.85, first for every judge). A full set costs 3 calls, about $0.23 and 21 s of model calls plus 6 s of assembly.",
        "failed": "The model still decides details. The fixed strike still has small tan smudges on the shorts (smaller than the first build's), the flinch recovers in one jump and its hit frames have swollen hair, the death's bounce reads as a sit-up, and the corpse lies flatter than the isometric axis. Picking between two variants by eye remains a step, and big units need a bigger canvas.",
    },
}


def dossier(ctx, aid):
    data, esc = ctx["data"], ctx["esc"]
    a = arm(data, aid)
    if not a:
        return ""
    d = DOSSIERS.get(aid, {})
    s = data.get("scores", {}).get("final_nofx" if aid == "FINAL" else aid, {})
    mins = a["minutes"].get("_wall", 0) if a["minutes"] else 0
    spend = a["spend"]["usd"] if aid != "FINAL" else 0.232
    calls = a["spend"]["calls"] if aid != "FINAL" else 3
    strips = "".join(f'<figure><div class="strip"><img src="assets/{aid}_{an}_map1x.png" alt="{esc(a["name"])}, {an}, every frame at game scale" loading="lazy"></div><figcaption>{an} at 1×</figcaption></figure>'
                     for an in ANIMS if f"{aid}_{an}_map1x.png" in EXTRA_ASSETS)
    para = lambda k, lab: f"<p><strong>{lab}</strong> {d[k]}</p>" if d.get(k) else ""
    timefact = (f'<div><dt>Agent time</dt><dd>{mins:.0f} min</dd></div>' if aid not in ("A0", "FINAL") else
                f'<div><dt>Run time</dt><dd>{"21 s + 6 s" if aid == "FINAL" else "under 1 s"}</dd></div>')
    return f"""
<section class="method" id="m-{aid.lower()}">
  <header class="mhead"><span class="badge">{aid if aid != "FINAL" else "R+"}</span><div><h3>{esc(a["name"])}</h3><p class="verdict">{d.get("verdict", "")}</p></div></header>
  <dl class="facts"><div><dt>Round-1 mean</dt><dd>{f2(s.get("overall")) if s else "–"}</dd></div><div><dt>Animations</dt><dd>{len(a["anims"])} of 4</dd></div>
  <div><dt>Image API</dt><dd>{fmt_money(spend)}</dd></div><div><dt>Calls</dt><dd>{calls}</dd></div>{timefact}</dl>
  <div class="prose">{para("how", "How it worked.")}{para("worked", "What came out well.")}{para("failed", "What went wrong.")}</div>
  {videos(ctx, aid)}
  {f'<details><summary>Every frame at game scale</summary><div>{strips}</div></details>' if strips else ""}
</section>"""


# ---------------------------------------------------------------- page
def answer(ctx):
    data = ctx["data"]
    v_fix = {an: vscore("final_nofx", an) for an in ANIMS}
    v_mean = (sum(v for v in v_fix.values() if v) / max(1, len([v for v in v_fix.values() if v]))) if VERIFY else None
    kpi_v = (f'<div><dt>Verification round, fixed recipe vs best rival</dt><dd>{v_mean:.2f} vs 7.38<small>mean of four animations, same judges; first build 7.03</small></dd></div>'
             if v_mean else "")
    return f"""
<div class="answer">
  <p><strong>Draw the big actions with the image model, and move the sprite's own pixels for the small ones.</strong>
  gpt-image-2.5-sunburst draws a convincing strike, flinch and death when it gets one sprite sheet per animation with the
  real sprite in the first cell and a flat-colour pose blockout in every other cell. It cannot draw an idle: every
  redraw shimmers. So the recipe warps the real sprite for idle and for the frames next to idle. That hybrid was the
  best complete set in blind judging, and after three cheap fixes it had the top mean score in all four animations
  (first for every judge in the death). With a unit file and one fix to its pose retargeting, the same tool also gave a
  second unit, Hyborem, a usable-looking set on its first calls.</p>
  <dl class="kpis">
    <div><dt>Round-1 judge score, recipe vs control</dt><dd>{f2(score(data, "final_nofx"))} vs {f2(score(data, "A0"))}<small>weighted, out of 10, mean of four animations</small></dd></div>
    {kpi_v}
    <div><dt>One full set (four animations)</dt><dd>3 calls · $0.23<small>21 s of model calls + 6 s of assembly</small></dd></div>
    <div><dt>Image API for the whole sprint</dt><dd>{fmt_money(data["spend_total"])}<small>of the $20 cap, pessimistic prices</small></dd></div>
  </dl>
</div>
<ol class="findings">
  <li><strong>Stiffness was a posing problem, not a rendering problem.</strong> Moving the whole sprite (A0) or its cut-out
    parts (R) cannot produce a pose the sprite does not already contain. Both came last for strike, flinch and death in every
    judge's ranking, and judges could restore the order of A0's shuffled strike frames only half to three-quarters of the time.</li>
  <li><strong>One sheet per animation, with the real sprite in cell 1, keeps the character consistent.</strong> The four
    sheet methods (G0, G1, G2, K) scored 7.0–7.6 on strike and death in round 1, and judges restored the order of almost
    every shuffled strike they drew. Chaining frames one call at a time (C) scored lower (6.63) at three and a half times the
    cost, and the Responses API's multi-turn image tool lost scale and facing entirely.</li>
  <li><strong>Pose guides buy predictability more than quality.</strong> With no guides (G0) the model posed the best strike
    of the trials, but ignored the ground line (rows 27–42 px high), grew 6–7% in half the variants and cropped the blade.
    With blockouts (G1) the frames came back on the ground line at the sprite's own size (standing frames 85–87 px against
    86), so the shared code could place them without large corrections. That predictability is what lets the recipe run
    with little supervision.</li>
  <li><strong>The image model cannot do idle.</strong> A 1-pixel breath is smaller than the difference between any two of
    its drawings, so every sheet idle boiled (judge scores 3.8–5.8). Warping the real sprite scored 7.8–8.1 in round 1;
    in verification the sharpened warp scored 7.9 and the first build's softer, unsharpened warp 6.6.</li>
  <li><strong>Small, targeted fixes were measurable.</strong> The round-1 comments led to three fixes that cost no new image
    calls: switching to the strike variant with smaller smudges, sharpening the warped frames and removing the magenta
    fringe. In a fresh blind round the fixed recipe had the top mean in all four animations, 0.5–1.3 points above its own
    first build. Against the best single method per animation the margins were 0.2 (idle, the rig), 0.3 (strike, G0), 0.6
    (flinch, G0) and 0.5 (death, K): only the death was ranked first by every judge, and the idle and strike gaps are too
    small to call.</li>
  <li><strong>What is left is shared by every model method</strong>: a thin magenta keying fringe (the recipe now removes
    it), small colour smudges, corpses lying flatter than the isometric axis, off-model faces on some hit frames, and
    variants that often differ in their defects, so a person still picks between two variants.</li>
</ol>"""


def render(ctx):
    g, data, esc = ctx["grid"], ctx["data"], ctx["esc"]
    trial_present = [a for a in TRIAL_ARMS if arm(data, a)]
    hyb = arm(data, "HYB")
    return f"""
<header class="top wrap">
  <p class="eyebrow">Research sprint · Fall from Heaven port · 20 October 2026</p>
  <h1>{TITLE}</h1>
  <p class="lede">Seven ways to give the M7 swordsman an idle, a strike, a flinch and a death, all on the same beat sheet,
  judged blind by three agents, and a hybrid recipe built mid-trial from the strongest results, judged with them, fixed,
  judged again and tried on a second unit. Every animation on this page plays.</p>
  <nav class="toc" aria-label="Sections"><a href="#answer">Short answer</a><a href="#final">Recommended set</a><a href="#compare">Side by side</a><a href="#quiz">Less stiff?</a><a href="#results">Results</a><a href="#methods">Method by method</a><a href="#recipe">Pipeline</a><a href="#how">How it ran</a><a href="#next">Not tested</a></nav>
</header>
<main>
<section class="block wrap" id="answer"><h2>The short answer</h2>{answer(ctx)}</section>

<section class="block wrap" id="final"><h2>The recommended set</h2>
  <p class="read">The recipe's output for the swordsman, with the finishing touches a game would add: a smear arc on the
  swing, an impact spark on the hit frame and a white hit flash on the flinch and death. Pause and step to see single
  frames; the captions give each frame's duration from the beat sheet.</p>
  {feature(ctx, "FINAL", zoom=2)}
  {videos(ctx, "FINAL", autoplay=True)}
  <h3>The same tool on Hyborem</h3>
  <p class="read">To check the recipe is not swordsman-specific, it ran on the M7 Hyborem sprite: a 106 px demon lord with
  horns, cape and a flaming greatsword. The new work was a unit file (his frame-0 joints, a description, size settings and
  nine reworded beats that swap the shield for his empty fist and cape) and one fix to the recipe's pose retargeting, so that
  bones rotate relative to his own rest pose. The swordsman's poses were then retargeted onto his skeleton and drawn as grey
  blockouts. Three calls in 22 s and $0.23 gave the set below on the first try. It looks usable by eye but was not judged,
  and the automatic gates do not work on his palette. His raised greatsword is cut off at the top of the canvas in the
  anticipation frame and his swing reaches the canvas's left edge: big units need a bigger animation canvas.</p>
  {feature(ctx, "HYB", zoom=2) if hyb else ""}
  {videos(ctx, "HYB", autoplay=True)}
</section>

<section class="block wrap" id="compare"><h2>Every method, side by side</h2>
  <p class="read">All players share one clock. Pause to step through frames; zoom and the grass tile change every cell.
  Round-1 frames carry no effects, so motion has to come from the poses; the recipe is shown without effects too.</p>
  {g(trial_present + ["FN"], zoom=1)}</section>

<section class="block wrap" id="quiz"><h2>Which looks less stiff?</h2>{quiz(ctx)}</section>

<section class="block wrap" id="results"><h2>Results</h2>
  <p class="read">Round 1: three judges scored every method's four animations from stills (strips at 2× and 3×, the 1× map
  view, onion skins and silhouettes), blind to which method made what. Score is the weighted mean: motion 35%,
  consistency 30%, style 20%, appeal 15%. The recipe's first build took part as one more unnamed letter.</p>
  {results_table(ctx)}
  <p class="note">Best per animation in green. K made only strike and death and C only the strike, so their means cover
  fewer animations. Image API is the ledger's pessimistic estimate. Agent minutes are as each agent logged them (some
  include reading the brief, some do not), with windows nested inside another window counted once; agents ran in parallel.
  A0 was scripted by the lead in a few minutes; the recipe's run is about 21 s of model calls plus 6 s of assembly, and its
  design time was not logged. The same frames scored up to 1.2 points apart between the two judge panels (the first
  build's idle: 7.80 here, 6.63 in verification), so compare methods within one round only, and treat gaps under about
  0.5 as unresolved.</p>
  <h3>Verification: the fixed recipe against the winners</h3>
  <p class="read">The round-1 comments named the recipe's flaws: a smudge on the shorts in the chosen strike variant, a
  soft settle frame, and the magenta fringe. The recipe then switched to the other strike variant, whose smudges are smaller, sharpened its warps and
  removed the fringe, without new image calls. Three fresh judges, with new letters, then compared it per animation with
  its own first build and the two best round-1 methods for that animation.</p>
  {verify_table(ctx)}
  <h3>Measured motion</h3>
  <p class="read">Each cell gives non-rigidity · overlap · colour drift. Non-rigidity is 1 minus the best silhouette overlap
  after rotating, scaling and shearing frame 0 onto a frame, so 0 means the frame is just frame 0 moved. Overlap is the
  spread, in frames, of when the sword, shield, hair and tunic each move most, so 0 means everything moves in lockstep.
  Drift is the colour distance to the sprite's own palette. For idle, boil is the mean change between consecutive frames
  in the loop.</p>
  {metrics_table(ctx)}
</section>

<section class="block wrap" id="methods"><h2>Method by method</h2>
  <p class="read">Ordered as the recipe uses them: the control and the rig first, then the image-model methods, then the
  recipe. Each has its full-sequence video (and the combat vignette where it made all four animations).</p>
  {"".join(dossier(ctx, a) for a in ["A0", "R", "G0", "G1", "G2", "K", "C", "FINAL"])}
</section>

<section class="block wrap" id="recipe"><h2>Recommended pipeline</h2>{RECIPE}</section>
<section class="block wrap" id="how"><h2>How the sprint ran</h2>{HOW}</section>
<section class="block wrap" id="next"><h2>Not tested yet</h2>{NEXT}</section>
</main>
<footer class="wrap"><p>Everything is in <code>projects/ffh-anim-sprint/</code> on branch <code>ccr-4acd4727-7uk22m</code>:
<code>spec.md</code>, <code>beats.md</code>, <code>poses.json</code>; <code>lib/</code> (harness and <code>recipe.py</code>);
<code>arms/&lt;method&gt;/</code> with frames, manifests, sheets, notes, timings and <code>sequence_*.{{gif,mp4}}</code>;
<code>final/</code> and <code>final_hyborem_v1/</code> for the recipe; <code>judging/</code> with every judge's JSON, the keys and
<code>scores.json</code>; <code>spend.jsonl</code>.</p></footer>
"""


RECIPE = """
<ol class="steps read">
  <li><strong>Annotate the unit once.</strong> Write <code>units/&lt;unit&gt;.json</code>: the sprite path, one line saying what the
    model must keep (outfit, weapon, which hand), and the frame-0 joints in canvas pixels. <code>ak.grid_view</code> draws the sprite
    enlarged with coordinates for reading the joints. Beats that mention props the unit lacks (the shield) can be reworded
    per unit.</li>
  <li><strong>Run the recipe.</strong> Three sheet calls go out in parallel; idle and the settle frames are warped locally.
    About 21 s of model calls, 6 s of assembly and $0.23 per unit.</li>
  <li><strong>Pick variants by eye.</strong> Each sheet comes back in two variants that often differ in their defects
    (smudges, size, clipped blades), and the automatic score does not see smudges. Look at
    <code>work/&lt;out&gt;/build_report.json</code> and the strips, then re-cut with another variant for free.</li>
  <li><strong>Preview and hand over.</strong> The preview step writes strips, the full-sequence GIF and MP4 and the combat vignette.
    The engine reads <code>sheet_&lt;anim&gt;.png</code> plus <code>manifest.json</code> (per-frame ms, the hit event), anchors
    canvas point (80, 126) on the tile centre and places the banner from the idle frame-0 box.</li>
</ol>
<pre><code>cd projects/ffh-anim-sprint
python3 lib/recipe.py --unit units/swordsman_judged.json --out final --rebuild --pick strike=2   # re-cut, no API calls
python3 lib/preview.py --arm final --root final                               # strips, sequence_*.gif/mp4, vignette
python3 lib/recipe.py --unit units/hyborem_v1.json --out final_hyborem_v1 --rebuild   # another unit, re-cut</code></pre>
<p class="read">Since the sprint, the recipe follows a further rule (the strike goes where the sprite looks); the sets
above predate it. The how-to for new units is <code>ANIMATING_UNITS.md</code>.</p>
<p class="read">A unit with purple or magenta art (Sheaim, liches) would need a cyan key, which M7's static pipeline offers but
this recipe does not support yet (its prompt and slicer assume magenta). The de-fringe step already skips itself when the
sprite contains purple.</p>
"""

HOW = """
<div class="cols">
  <div>
    <h3>One shared contract</h3>
    <ul class="plain">
      <li>Every method animated the same accepted M7 swordsman (96×120, figure 86 px, facing lower-left) on a
        160×128 canvas anchored bottom-centre like an M7 unit, so idle frame 0 is the static sprite.</li>
      <li>A written beat sheet fixed the frames and timings: idle 4 frames looping at 250 ms; strike 8 frames,
        about 870 ms, hit on frame 4; flinch 4 frames, about 440 ms; death 8 frames, ending on a held corpse.</li>
      <li>One set of 2D joint positions per frame (<code>poses.json</code>) drove the blockouts and the rig used by most
        methods; G0 posed from the beat text alone and A0 from hand-set transforms.</li>
      <li>Shared code did everything around the pixels: keying the magenta, one scale per sheet calibrated from
        the reference cell, snapping each frame onto the ground row, metrics, previews and the GIF and MP4 sequences.</li>
    </ul>
    <h3>Calibration before the trials</h3>
    <ul class="plain">
      <li>Six calibration calls of one guided strike sheet to three image models cost $0.36; the five parallel ones finished
        in about 26 s. gpt-image-2.5-sunburst kept the
        reference cell almost untouched (scale 1.004, silhouette overlap 0.97); gpt-image-2.5-flare redrew it and drew its
        bottom row 14 px off the ground; gpt-image-2 scaled everything up 19%.</li>
      <li>Sunburst does not accept <code>input_fidelity</code>. High quality was only marginally crisper than medium at twice
        the price. Grey mannequins worked as well as coloured blockouts. Frozen for every image-model method: sunburst,
        medium, 1536×1024, two variants per call, the roster sheet as a reference image (C's chain added the sprite as an
        extra image; its Responses-API probe was outside the contract).</li>
    </ul>
  </div>
  <div>
    <h3>Trials and judging</h3>
    <ul class="plain">
      <li>Each method except the A0 control was run by agents (usually one for the strike and one for the rest) with a
        60-minute timebox per batch, up to three rounds per sheet, notes and timings logged. For G0, R and G2 the strike came
        first and was checked before the other animations; K's death and G1's other animations ran alongside their strikes.</li>
      <li>Round 1: three judge agents per animation, letters instead of names, row order shuffled per judge. Each did a
        frame-ordering test on shuffled frames before seeing the ordered strips, then flagged defects and scored the rubric.
        The whole-sprite control came last for strike, flinch and death in every judge's ranking, which is the sanity check
        that the judging works.</li>
      <li>Verification: three new judges and new letters compared the fixed recipe per animation with its first build and
        the two best round-1 methods.</li>
    </ul>
    <h3>Caveats</h3>
    <ul class="plain">
      <li>The judges are Claude agents, the same model family that ran the methods, and they judge stills, not motion. No
        human scored anything; the players and the pairs above are for your eye.</li>
      <li>Each method ran once, with two variants per call. The same frames scored up to 1.2 points apart between the two
        judge panels, so gaps under about 0.5 are unresolved and scores are only comparable within a round.</li>
      <li>The recipe was designed mid-trial from the outputs and notes available by then (G0 and G1 complete, most strikes,
        K's death), before any judging; its first build then entered round 1 blind. Its fixes came from the round-1 judges'
        comments, which is why the verification round used fresh judges.</li>
      <li>Dollar figures are image-API spend only, at the M7 ledger's deliberately pessimistic prices; the Claude tokens behind
        every agent are not costed.</li>
      <li>The session's container restarted once mid-sprint, and the round-1 idle, flinch and death judging, which had not
        written any results yet, was run again from the same packets.</li>
    </ul>
  </div>
</div>
"""

NEXT = """
<ul class="gap-list read">
  <li><strong>Video models.</strong> The models endpoint lists Sora 2 and Sora 2 Pro with a shutdown date of 24 September 2026,
    and <code>/v1/videos</code> returns 404 for this key (probe logged in <code>arms/calib/video_probe.txt</code>), so
    image-to-video-to-frames could not be tried.</li>
  <li><strong>Local diffusion with pose control</strong> (SDXL or Flux with an OpenPose ControlNet and an IP-Adapter for
    identity) on your RTX 4080. This container has no GPU.</li>
  <li><strong>Sprite-animation services</strong> such as PixelLab's skeleton animation or Retro Diffusion. They need an
    account and an API key.</li>
  <li><strong>Other facings.</strong> Every animation faces lower-left. The engine can mirror for lower-right, but the two
    upper facings need back views, which means a second set of sheets per unit.</li>
  <li><strong>More units, and big ones.</strong> Hyborem's first calls gave a set that looks usable but was not judged; a judged
    second unit, then casters with staves, archers, mounted units and the 134 px Hill Giant are the next checks. Big units need
    a bigger canvas than 160×128.</li>
  <li><strong>A walk cycle.</strong> Moving between tiles is the animation players see most, and it was out of scope here.</li>
  <li><strong>Engine integration.</strong> The client does not load M7 sprites yet. The spec here (canvas, anchor, manifest
    with per-frame ms and a hit event, banner from the idle box) is written so a loader can use it directly.</li>
</ul>
"""
