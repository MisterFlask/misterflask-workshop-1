#!/usr/bin/env python3
"""Procedurally animated music video for "Stokers of the Underworld".

Soviet constructivist propaganda-poster aesthetic, drawn with pycairo and
driven by the audio analysis in env.json (beats, RMS, low-band thump) and the
line timings in timed_lines.json. Each song section is a scene; scenes
crossfade. A global "freeze" curve moves hell from fire-reds to ice-blues as
the strike takes hold.

Usage: animate.py <frames_dir> [start_frame end_frame]
Then assemble with render_animated.sh.
"""
import json
import math
import os
import sys
from multiprocessing import Pool

import cairo
import numpy as np

W, H = 1920, 1080
TAU = 2 * math.pi

ROOT = os.path.dirname(os.path.abspath(__file__))
ENV = json.load(open(os.path.join(ROOT, "env.json")))
LINES = json.load(open(os.path.join(ROOT, "timed_lines.json")))
FPS = ENV["fps"]
DUR = ENV["duration"]
BEATS = np.array(ENV["beats"])
RMS = np.array(ENV["rms"])
LOW = np.array(ENV["low"])

def clamp(v, lo=0.0, hi=1.0):
    return max(lo, min(hi, v))

def line_u(i, t):
    """Progress through lyric line i: <0 before, 0..1 during, >1 after."""
    l = LINES[i]
    return (t - l["start"]) / max(l["end"] - l["start"], 1e-3)

def during(i, t):
    return 0.0 <= line_u(i, t) <= 1.0

# ---------------------------------------------------------------- palette

def hx(s):
    return tuple(int(s[i:i + 2], 16) / 255 for i in (0, 2, 4))

FIRE = {
    "bg": hx("241109"), "bg2": hx("38180e"), "ink": hx("100a07"),
    "red": hx("c22318"), "red2": hx("e8402a"), "flame": hx("ff7a1a"),
    "ember": hx("ffc832"), "paper": hx("efdfc0"), "dim": hx("6e3020"),
}
ICE = {
    "bg": hx("101a2c"), "bg2": hx("1a2a3e"), "ink": hx("060a10"),
    "red": hx("b02020"), "red2": hx("d83838"), "flame": hx("7ab8d8"),
    "ember": hx("cfe8f4"), "paper": hx("e4ecf2"), "dim": hx("3a5068"),
}

def lerp(a, b, t):
    return a + (b - a) * t

def mix(c1, c2, t):
    return tuple(lerp(a, b, t) for a, b in zip(c1, c2))

def pal(freeze):
    return {k: mix(FIRE[k], ICE[k], freeze) for k in FIRE}

def freeze_at(t):
    """0 = molten hell, 1 = general strike achieved."""
    pts = [(0, 0), (20.6, 0.0), (30, 0.30), (44.7, 0.40), (47, 0.12),
           (74.5, 0.12), (77, 0.55), (87.4, 0.60), (93.5, 0.62),
           (105, 0.85), (116, 1.0), (127.5, 1.0)]
    xs, ys = zip(*pts)
    return float(np.interp(t, xs, ys))

# ---------------------------------------------------------------- audio helpers

def fidx(t):
    return min(len(RMS) - 1, max(0, int(t * FPS)))

def rms(t):
    return float(RMS[fidx(t)])

def low(t):
    return float(LOW[fidx(t)])

def beat_pulse(t, decay=0.18):
    """1.0 right on a beat, exponential decay after."""
    i = np.searchsorted(BEATS, t)
    if i == 0:
        return 0.0
    return math.exp(-(t - BEATS[i - 1]) / decay)

def beat_count(t):
    return int(np.searchsorted(BEATS, t))

def beat_phase(t):
    """0..1 position within the current beat."""
    i = np.searchsorted(BEATS, t)
    if i == 0 or i >= len(BEATS):
        return 0.0
    a, b = BEATS[i - 1], BEATS[i]
    return (t - a) / max(b - a, 1e-6)

# ---------------------------------------------------------------- primitives

def poly(ctx, pts, color, alpha=1.0):
    ctx.set_source_rgba(*color, alpha)
    ctx.move_to(*pts[0])
    for p in pts[1:]:
        ctx.line_to(*p)
    ctx.close_path()
    ctx.fill()

def disc(ctx, x, y, r, color, alpha=1.0):
    ctx.set_source_rgba(*color, alpha)
    ctx.arc(x, y, r, 0, TAU)
    ctx.fill()

def rect(ctx, x, y, w, h, color, alpha=1.0):
    ctx.set_source_rgba(*color, alpha)
    ctx.rectangle(x, y, w, h)
    ctx.fill()

def sunburst(ctx, cx, cy, r0, r1, n, rot, color, alpha):
    ctx.set_source_rgba(*color, alpha)
    for k in range(n):
        a = rot + k * TAU / n
        w = TAU / n * 0.42
        ctx.move_to(cx + r0 * math.cos(a - w / 2), cy + r0 * math.sin(a - w / 2))
        ctx.line_to(cx + r1 * math.cos(a), cy + r1 * math.sin(a))
        ctx.line_to(cx + r0 * math.cos(a + w / 2), cy + r0 * math.sin(a + w / 2))
        ctx.close_path()
    ctx.fill()

def flame(ctx, x, y, w, h, t, seed, P, freeze=0.0, alpha=1.0):
    """Layered licking flame; freezes into a static jagged spike cluster."""
    rng_t = 0.0 if freeze > 0.7 else t  # frozen flames stop moving
    layers = [(1.0, P["red"]), (0.72, P["flame"]), (0.45, P["ember"])]
    for scale, col in layers:
        hw, hh = w * scale / 2, h * scale
        pts = [(x - hw, y)]
        n = 7
        for k in range(n + 1):
            fx = x - hw + 2 * hw * k / n
            wob = (math.sin(rng_t * 7 + seed * 13 + k * 2.1)
                   + 0.6 * math.sin(rng_t * 11 + seed * 7 + k * 3.7))
            peak = hh * (0.55 + 0.45 * abs(math.sin(seed * 31 + k * 1.7))) \
                * (1 + 0.16 * wob)
            tip = 0.5 if k % 2 else 1.0
            pts.append((fx, y - peak * tip))
        pts.append((x + hw, y))
        poly(ctx, pts, col, alpha)

def icicles(ctx, x0, x1, y, count, length, P, seed=0):
    for k in range(count):
        fx = x0 + (x1 - x0) * (k + 0.5) / count
        ln = length * (0.4 + 0.6 * abs(math.sin(seed + k * 2.3)))
        w = (x1 - x0) / count * 0.36
        poly(ctx, [(fx - w, y), (fx + w, y), (fx, y + ln)], P["ember"], 0.9)

def pitchfork(ctx, x, y, h, angle, color, lw=None):
    """Handle bottom at (x, y), pointing up, rotated by angle."""
    lw = lw or max(3.0, h * 0.045)
    ctx.save()
    ctx.translate(x, y)
    ctx.rotate(angle)
    ctx.set_source_rgb(*color)
    ctx.set_line_width(lw)
    ctx.set_line_cap(cairo.LINE_CAP_ROUND)
    ctx.move_to(0, 0)
    ctx.line_to(0, -h * 0.72)
    ctx.stroke()
    hw = h * 0.13
    ctx.move_to(-hw, -h * 0.72)
    ctx.line_to(hw, -h * 0.72)
    ctx.stroke()
    for dx in (-hw, 0, hw):
        ctx.move_to(dx, -h * 0.72)
        ctx.line_to(dx, -h)
        ctx.stroke()
    ctx.restore()

def horned_head(ctx, x, y, r, color):
    disc(ctx, x, y, r, color)
    for s in (-1, 1):
        poly(ctx, [(x + s * r * 0.55, y - r * 0.5),
                   (x + s * r * 1.25, y - r * 1.75),
                   (x + s * r * 0.1, y - r * 0.85)], color)

def worker(ctx, x, y, s, phase, P, tool="shovel", pose="work", aim=None, pu=0.0):
    """Stoker silhouette, feet at (x, y).

    pose: "work"    — shovel cycle, phase 0..1
          "inspect" — pauses, holds his worn pitchfork flat, head bowed to it
          "point"   — one arm flung out along angle `aim`, other on hip
          "fist"    — fist to the sky, pumping with pu (beat pulse)
          "rise"    — pu 0 = cowering wretch, 1 = standing tall
    """
    ink = P["ink"]
    if pose == "work":
        lean = math.radians(-14 + 26 * math.sin(phase * TAU))
    elif pose == "inspect":
        lean = math.radians(4)
    elif pose == "point":
        lean = math.radians(-8)
    elif pose == "fist":
        lean = math.radians(-10 - 4 * pu)
    else:  # rise
        lean = math.radians(lerp(38, -8, pu))
    dip = s * 0.05 * (1 - math.cos(phase * TAU)) / 2 if pose == "work" else 0
    crouch = s * 0.10 * (1 - pu) if pose == "rise" else 0
    ctx.save()
    ctx.translate(x, y - dip + crouch)
    # legs
    ctx.set_source_rgb(*ink)
    ctx.set_line_width(s * 0.14)
    ctx.set_line_cap(cairo.LINE_CAP_ROUND)
    for dx in (-s * 0.16, s * 0.16):
        ctx.move_to(dx, 0)
        ctx.line_to(dx * 0.4, -s * 0.42)
        ctx.stroke()
    # torso
    ctx.translate(0, -s * 0.40)
    ctx.rotate(lean)
    chest = 1.0 + (0.08 * pu if pose in ("rise", "fist") else 0.0)
    poly(ctx, [(-s * 0.2 * chest, 0), (s * 0.2 * chest, 0),
               (s * 0.14 * chest, -s * 0.52), (-s * 0.14 * chest, -s * 0.52)], ink)
    head_dy = {"inspect": -s * 0.60, "rise": -s * lerp(0.56, 0.66, pu)}.get(pose, -s * 0.66)
    head_dx = s * 0.10 if pose == "inspect" else 0
    horned_head(ctx, head_dx, head_dy, s * 0.14, ink)
    ctx.set_line_width(s * 0.11)
    if pose == "work":
        reach = s * (0.55 + 0.15 * math.sin(phase * TAU))
        ang = math.radians(30 + 40 * math.sin(phase * TAU + 0.6))
        hx_, hy_ = reach * math.cos(ang), -s * 0.30 - reach * 0.4 * math.sin(ang)
        ctx.move_to(-s * 0.05, -s * 0.42)
        ctx.line_to(hx_, hy_)
        ctx.stroke()
        if tool == "shovel":
            tx = hx_ + s * 0.34 * math.cos(ang - 0.5)
            ty = hy_ + s * 0.30 * math.sin(ang - 0.5)
            ctx.set_line_width(s * 0.055)
            ctx.move_to(hx_, hy_)
            ctx.line_to(tx, ty)
            ctx.stroke()
            disc(ctx, tx, ty, s * 0.11, ink)
        elif tool == "pitchfork":
            ctx.save()
            ctx.translate(hx_, hy_)
            pitchfork(ctx, 0, s * 0.3, s * 1.05, math.radians(12), ink)
            ctx.restore()
    elif pose == "inspect":
        # both hands on a horizontal, worn-to-the-bone pitchfork
        fy = -s * 0.40
        for hx_ in (-s * 0.26, s * 0.26):
            ctx.move_to(-s * 0.05, -s * 0.42)
            ctx.line_to(hx_, fy)
            ctx.stroke()
        ctx.set_line_width(s * 0.05)
        ctx.move_to(-s * 0.55, fy)
        ctx.line_to(s * 0.55, fy)
        ctx.stroke()
        ctx.set_line_width(s * 0.035)
        for k, dr in enumerate((-0.10, 0.0, 0.10)):
            ctx.move_to(s * 0.55, fy + s * dr * 0.5)
            # tines drooping — six thousand years of wear
            ctx.curve_to(s * 0.65, fy + s * dr * 0.7,
                         s * 0.70, fy + s * dr + s * 0.04,
                         s * 0.74, fy + s * dr + s * 0.09)
            ctx.stroke()
    elif pose == "point":
        a = aim if aim is not None else math.radians(-40)
        ctx.move_to(-s * 0.02, -s * 0.44)
        ctx.line_to(s * 0.62 * math.cos(a), -s * 0.44 + s * 0.62 * math.sin(a))
        ctx.stroke()
        disc(ctx, s * 0.62 * math.cos(a), -s * 0.44 + s * 0.62 * math.sin(a),
             s * 0.055, ink)
        ctx.move_to(s * 0.02, -s * 0.40)  # other arm akimbo
        ctx.line_to(s * 0.24, -s * 0.22)
        ctx.line_to(s * 0.10, -s * 0.06)
        ctx.stroke()
    elif pose == "fist":
        up = s * (0.55 + 0.16 * pu)
        ctx.move_to(-s * 0.02, -s * 0.44)
        ctx.line_to(s * 0.16, -s * 0.44 - up)
        ctx.stroke()
        disc(ctx, s * 0.16, -s * 0.44 - up, s * 0.09, ink)
        ctx.move_to(s * 0.02, -s * 0.42)
        ctx.line_to(s * 0.20, -s * 0.16)
        ctx.stroke()
    else:  # rise: arms wrapped when cowering, opening to fists at sides
        wrap = 1 - pu
        for sgn in (-1, 1):
            ctx.move_to(sgn * s * 0.04, -s * 0.42)
            ex = sgn * s * lerp(0.10, 0.30, pu)
            ey = -s * lerp(0.30, 0.10, pu) - wrap * s * 0.06 * sgn
            ctx.line_to(ex, ey)
            ctx.stroke()
            if pu > 0.5:
                disc(ctx, ex, ey, s * 0.07, ink)
    ctx.restore()

def lucifer(ctx, x, y, s, t, P, act="idle", au=0.0):
    """Fat boss on a stepped obsidian throne, feet-level at (x, y).

    act: "idle" — breathes, raises goblet on the beat
         "sip"  — tips the goblet to his lips over the line (au 0..1)
         "preen"— buffs his crown, sparkles; never singed a single hair
         "recoil"— flinches back from the fist below (au 0..1)
    """
    ink = P["ink"]
    for i in range(3):  # throne steps
        wd = s * (1.9 - i * 0.45)
        rect(ctx, x - wd / 2, y - s * 0.28 * (i + 1), wd, s * 0.28, ink, 0.92)
    ctx.save()
    if act == "recoil":
        k = math.sin(math.pi * clamp(au)) * 0.7 + 0.3 * clamp(au)
        ctx.translate(x, y)
        ctx.rotate(math.radians(-9) * k)
        ctx.translate(-x, -y - s * 0.05 * k)
    breathe = 1 + 0.03 * math.sin(t * 1.3)
    ctx.save()
    ctx.translate(x, y - s * 0.84)
    ctx.scale(1.0, breathe)
    ctx.set_source_rgb(*ink)
    ctx.arc(0, -s * 0.34, s * 0.44, 0, TAU)  # belly
    ctx.fill()
    ctx.restore()
    horned_head(ctx, x, y - s * 1.52, s * 0.15, ink)
    for k in range(3):  # crown
        poly(ctx, [(x - s * 0.12 + k * s * 0.12, y - s * 1.64),
                   (x - s * 0.06 + k * s * 0.12, y - s * 1.80),
                   (x + s * 0.00 + k * s * 0.12, y - s * 1.64)], P["ember"])
    ctx.set_source_rgb(*ink)
    ctx.set_line_width(s * 0.09)
    if act == "sip":
        # goblet travels to the lips and back, held mid-line
        p = math.sin(math.pi * clamp(au)) ** 0.5
        gx = lerp(x + s * 0.62, x + s * 0.16, p)
        gy = lerp(y - s * 1.30, y - s * 1.50, p)
        ctx.move_to(x + s * 0.30, y - s * 1.05)
        ctx.line_to(gx, gy)
        ctx.stroke()
        ctx.save()
        ctx.translate(gx, gy)
        ctx.rotate(-1.1 * p)  # tips it back
        poly(ctx, [(-s * 0.10, 0), (s * 0.10, 0), (0, -s * 0.14)], P["red2"])
        ctx.restore()
    elif act == "preen":
        # hand circling the crown, polishing; little sparkles
        ca = t * 5.0
        hx_ = x - s * 0.02 + s * 0.16 * math.cos(ca)
        hy_ = y - s * 1.70 + s * 0.08 * math.sin(ca)
        ctx.move_to(x + s * 0.28, y - s * 1.06)
        ctx.line_to(x + s * 0.30, y - s * 1.44)
        ctx.line_to(hx_, hy_)
        ctx.stroke()
        disc(ctx, hx_, hy_, s * 0.07, ink)
        for k in range(4):
            sa = t * 3 + k * 1.7
            disc(ctx, x + s * 0.30 * math.cos(sa * 1.3 + k),
                 y - s * 1.75 - s * 0.12 * abs(math.sin(sa)),
                 s * 0.025, P["paper"], 0.5 + 0.4 * math.sin(sa * 2))
    else:
        gob = beat_pulse(t) * s * 0.06
        if act == "recoil":  # goblet flung up defensively
            gob = s * (0.10 + 0.10 * clamp(au))
        ctx.move_to(x + s * 0.30, y - s * 1.05)
        ctx.line_to(x + s * 0.62, y - s * 1.30 - gob)
        ctx.stroke()
        poly(ctx, [(x + s * 0.52, y - s * 1.30 - gob),
                   (x + s * 0.72, y - s * 1.30 - gob),
                   (x + s * 0.62, y - s * 1.44 - gob)], P["red2"])
    ctx.restore()

def chain(ctx, x0, y0, x1, y1, links, sway, color, lw=6):
    ctx.set_source_rgb(*color)
    ctx.set_line_width(lw)
    for k in range(links):
        u = k / max(links - 1, 1)
        cx = lerp(x0, x1, u) + sway * math.sin(u * math.pi) * 18
        cy = lerp(y0, y1, u) + abs(math.sin(u * math.pi)) * 26
        ctx.save()
        ctx.translate(cx, cy)
        ctx.rotate(math.atan2(y1 - y0, x1 - x0) + (0.9 if k % 2 else 0))
        ctx.scale(1.0, 0.62)
        ctx.arc(0, 0, 13, 0, TAU)
        ctx.restore()
        ctx.stroke()

def gear(ctx, x, y, r, teeth, angle, color, alpha=1.0):
    ctx.save()
    ctx.translate(x, y)
    ctx.rotate(angle)
    ctx.set_source_rgba(*color, alpha)
    for k in range(teeth):
        a = k * TAU / teeth
        ctx.save()
        ctx.rotate(a)
        ctx.rectangle(-r * 0.09, -r * 1.18, r * 0.18, r * 0.24)
        ctx.fill()
        ctx.restore()
    ctx.arc(0, 0, r, 0, TAU)
    ctx.fill()
    ctx.set_operator(cairo.OPERATOR_CLEAR)
    ctx.arc(0, 0, r * 0.45, 0, TAU)
    ctx.fill()
    ctx.set_operator(cairo.OPERATOR_OVER)
    ctx.restore()

def snow(ctx, t, density, P, drift=30, alpha=0.85):
    ctx.set_source_rgba(*P["paper"], alpha)
    n = int(density)
    for k in range(n):
        sd = k * 12.9898
        px = (math.sin(sd) * 43758.5453) % 1.0
        py = ((sd * 0.317) % 1.0 + t * (0.03 + 0.05 * ((sd * 7) % 1.0))) % 1.0
        x = px * W + drift * math.sin(t * 0.7 + sd)
        r = 1.5 + 2.5 * ((sd * 3) % 1.0)
        ctx.arc(x, py * H, r, 0, TAU)
        ctx.fill()

def crowd(ctx, t, y_base, rows, P, scale=1.0, freeze=0.0):
    """Marching rows of horned strikers with pitchforks."""
    ink = P["ink"]
    step = beat_count(t)
    for row in range(rows):
        depth = 1 - row / max(rows, 1) * 0.5
        y = y_base - row * 46 * scale
        n = 11 + row * 2
        shade = mix(ink, P["dim"], row / max(rows, 1) * 0.8)
        for k in range(n):
            x = (k + 0.5) / n * W + (18 * scale) * ((row * 17 + k * 29) % 5 - 2)
            bob = math.sin((step + k + row) * 1.7) * 6 * scale \
                + beat_pulse(t) * 8 * scale * (1 if (k + step) % 2 else -1)
            r = 30 * scale * depth
            horned_head(ctx, x, y - bob, r, shade)
            ang = math.radians(8 * math.sin(k * 2.1 + row) + beat_pulse(t) * 6)
            pitchfork(ctx, x + r * 1.2, y - bob + r,
                      170 * scale * depth, ang, shade)

# ---------------------------------------------------------------- scenes

def sc_title(ctx, t, P, fz):
    rect(ctx, 0, 0, W, H, P["bg"])
    sunburst(ctx, W * 0.5, H * 0.38, 120, 1500, 22, t * 0.02, P["dim"], 0.35)
    disc(ctx, W * 0.5, H * 0.38, 300 + beat_pulse(t) * 14, P["red"], 0.30)
    # podium speaker, left
    ink = P["ink"]
    rect(ctx, 180, H - 300, 300, 180, ink)
    ctx.save()
    ctx.translate(330, H - 300)
    horned_head(ctx, 0, -160, 46, ink)
    poly(ctx, [(-60, 0), (60, 0), (40, -130), (-40, -130)], ink)
    # megaphone, mouth raised on shout energy
    a = -0.25 - rms(t) * 0.25
    ctx.rotate(a)
    poly(ctx, [(40, -120), (150, -170), (150, -90)], P["red2"])
    ctx.restore()
    # glowing horizon band so the crowd silhouettes read
    grad = cairo.LinearGradient(0, H - 420, 0, H)
    grad.add_color_stop_rgba(0, *P["red"], 0.0)
    grad.add_color_stop_rgba(1, *P["red2"], 0.55)
    ctx.set_source(grad)
    ctx.rectangle(0, H - 420, W, 420)
    ctx.fill()
    # crowd of pitchforks bottom right, surging with rms
    for k in range(14):
        x = W * 0.45 + k * 95 + 20 * math.sin(k * 3.1)
        hgt = 190 + 130 * ((k * 37) % 7) / 7 + rms(t) * 60
        pitchfork(ctx, x, H + 40, hgt, math.radians(-6 + 12 * math.sin(k)), ink)
    flame(ctx, W * 0.15, H + 30, 500, 260 + rms(t) * 120, t, 1, P, fz, 0.85)

def sc_verse1(ctx, t, P, fz):
    rect(ctx, 0, 0, W, H, P["bg"])
    rect(ctx, 0, 0, W, H * 0.46, P["bg2"])
    ink = P["ink"]
    glow = 0.5 + 0.5 * low(t)
    # broad furnace glow wash so silhouettes read against it
    grad = cairo.RadialGradient(520, H - 320, 120, 520, H - 320, 900)
    grad.add_color_stop_rgba(0, *P["flame"], 0.55 + glow * 0.25)
    grad.add_color_stop_rgba(1, *P["flame"], 0.0)
    ctx.set_source(grad)
    ctx.paint()
    # furnace, left: arch mouth roaring with the low band
    rect(ctx, 130, H - 560, 600, 560, ink)
    ctx.set_source_rgb(*P["flame"])
    ctx.arc(430, H - 260, 190, math.pi, 0)
    ctx.fill()
    rect(ctx, 240, H - 260, 380, 260, P["flame"])
    flame(ctx, 430, H - 10, 380, (330 + 240 * glow), t, 2, P, fz)
    # smoke stack + sparks
    rect(ctx, 330, 0, 200, H - 560, ink)
    for k in range(10):
        sd = k * 7.13
        u = ((t * (0.25 + (sd % 1.0) * 0.3)) + sd) % 1.0
        disc(ctx, 430 + 60 * math.sin(sd * 9 + t), (H - 600) * (1 - u),
             4 + 4 * (sd % 1.0), P["ember"], (1 - u) * 0.8)
    # the stoker, backlit by his own ember halo — acting out each line
    disc(ctx, 950, H - 330, 360, P["red"], 0.5)
    disc(ctx, 950, H - 330, 360 + beat_pulse(t) * 26, P["red2"], 0.25)
    phase = (beat_count(t) % 2 + beat_phase(t)) / 2
    if during(1, t):      # "...my pitchfork's worn to bone" — inspects it
        worker(ctx, 950, H - 60, 420, 0, P, pose="inspect")
    elif during(2, t):    # "While Lucifer sits fat upstairs" — points at him
        aim = math.atan2((H * 0.10 + 250) - (H - 480), (W - 360) - 950)
        worker(ctx, 950, H - 60, 420, 0, P, pose="point", aim=aim)
    elif during(4, t):    # "OURS! IT'S OURS DOWN THERE!" — fist to the sky
        worker(ctx, 950, H - 60, 420, 0, P, pose="fist", pu=beat_pulse(t))
        for k in range(7):  # sweat flung off him — whose sweat indeed
            sd = k * 2.39
            u = ((t * 1.6) + sd) % 1.0
            sgn = -1 if k % 2 else 1
            disc(ctx, 950 + sgn * (60 + 150 * u),
                 H - 500 - 90 * u + 260 * u * u, 6 * (1 - u * 0.5),
                 P["paper"], (1 - u) * 0.9)
    else:                 # line 3 and gaps: back to shoveling while the boss preens
        worker(ctx, 950, H - 60, 420, phase, P, "shovel")
    # Lucifer high right on his obsidian tower, lit by a gilded halo
    rect(ctx, W - 600, H * 0.10, 480, H, ink)
    sunburst(ctx, W - 360, H * 0.10 + 200, 60, 420, 16, -t * 0.05, P["ember"], 0.35)
    disc(ctx, W - 360, H * 0.10 + 230, 260, P["ember"], 0.30)
    if during(2, t):      # takes a lazy sip while being called out
        lucifer(ctx, W - 360, H * 0.10 + 420, 220, t, P, "sip", line_u(2, t))
    elif during(3, t):    # "never singed a single hair" — buffs the crown
        lucifer(ctx, W - 360, H * 0.10 + 420, 220, t, P, "preen")
    elif during(4, t):    # flinches from the fist below
        lucifer(ctx, W - 360, H * 0.10 + 420, 220, t, P, "recoil", line_u(4, t))
    else:
        lucifer(ctx, W - 360, H * 0.10 + 420, 220, t, P)

def sc_chorus1(ctx, t, P, fz):
    rect(ctx, 0, 0, W, H, P["bg"])
    # Lissitzky wedge: red wedge driving into the bosses' black disc
    prog = min(1.0, (t - 20.6) / 24.0)
    cx, cy = W * 0.72, H * 0.36
    disc(ctx, cx, cy, 260, P["ink"])
    disc(ctx, cx, cy, 260, P["dim"], 0.4)
    tip = (lerp(W * 0.2, cx + 40 * beat_pulse(t), 0.3 + 0.7 * prog), cy)
    poly(ctx, [(0, H * 0.72), (0, H * 0.10), tip], P["red2"])
    poly(ctx, [(0, H * 0.62), (0, H * 0.20), (tip[0] * 0.985, cy)], P["red"])
    # marching crowd
    crowd(ctx, t, H - 40, 3, P, 1.0, fz)
    # flames along the bottom fighting the freeze
    for k in range(6):
        flame(ctx, k * 380 + 100, H + 20, 300, 190 * (1 - fz * 0.8) + rms(t) * 90,
              t, k + 3, P, fz, 0.75)
    if fz > 0.05:
        snow(ctx, t, 60 * fz * 4, P, alpha=0.6)

def sc_verse2(ctx, t, P, fz):
    rect(ctx, 0, 0, W, H, P["bg2"])
    ink = P["ink"]
    # lit poster panels so the black machinery reads
    poly(ctx, [(0, 0), (W * 0.52, 0), (W * 0.40, H), (0, H)], P["red"], 0.75)
    poly(ctx, [(W * 0.55, 0), (W, 0), (W, H * 0.62), (W * 0.50, H * 0.62)],
         P["dim"], 0.85)
    # the machinery of pain: gears grinding
    ga = t * 0.9
    gear(ctx, W * 0.18, H * 0.30, 150, 12, ga, ink, 0.95)
    gear(ctx, W * 0.335, H * 0.42, 100, 9, -ga * 1.5 + 0.2, ink, 0.95)
    gear(ctx, W * 0.13, H * 0.55, 80, 8, -ga * 1.9, ink, 0.9)
    # the rack silhouette
    rect(ctx, W * 0.62, H * 0.16, 460, 34, ink)
    rect(ctx, W * 0.62, H * 0.42, 460, 34, ink)
    for u in (0.66, 0.79, 0.93):
        rect(ctx, W * u, H * 0.16, 26, H * 0.30, ink)
    # THE OVERSEER vs THE WORKER — the argument of verse two, line by line
    ox, wx = W * 0.30, W * 0.52
    grabbed = during(13, t) and line_u(13, t) > 0.35   # whip changes hands
    stagger = clamp(line_u(13, t)) if during(13, t) else 0.0
    ctx.save()
    ctx.translate(ox, H - 120)
    if grabbed:  # staggers back, arms up in disbelief
        ctx.rotate(math.radians(-14) * stagger)
        ctx.translate(-60 * stagger, 0)
    horned_head(ctx, 0, -330, 42, ink)
    poly(ctx, [(-70, 0), (70, 0), (44, -300), (-44, -300)], ink)
    ctx.set_source_rgb(*ink)
    ctx.set_line_width(14)
    if during(10, t):
        # "you can't complain!" — condescending finger wag
        wag = math.sin(t * 8.0) * 0.45
        ctx.move_to(30, -260)
        ctx.line_to(120, -350)
        ctx.stroke()
        ctx.save()
        ctx.translate(120, -350)
        ctx.rotate(wag)
        ctx.set_line_width(10)
        ctx.move_to(0, 0)
        ctx.line_to(28, -60)
        ctx.stroke()
        ctx.restore()
    elif during(12, t):
        # "The demons OWN the means of pain!" — sweeping possessive arm
        u = clamp(line_u(12, t))
        sw = lerp(math.radians(210), math.radians(-30), u)  # gears -> rack
        ctx.move_to(20, -270)
        ctx.line_to(20 + 190 * math.cos(sw), -270 - 120 * math.sin(sw))
        ctx.stroke()
        disc(ctx, 20 + 190 * math.cos(sw), -270 - 120 * math.sin(sw), 16, ink)
    elif grabbed:  # both arms thrown up
        for sgn in (-1, 1):
            ctx.move_to(sgn * 20, -270)
            ctx.line_to(sgn * 90, -370 - 20 * sgn)
            ctx.stroke()
    if not grabbed:  # the whip is still his
        crack = beat_pulse(t, 0.12)
        ctx.move_to(30, -260)
        for k in range(1, 9):
            u = k / 8
            ctx.line_to(30 + 320 * u,
                        -260 - 150 * u * math.sin(u * 5 - crack * 9 - t * 2)
                        * (0.3 + crack))
        ctx.stroke()
    ctx.restore()
    # the worker: cowers, rises, then lunges and seizes the whip
    if t < LINES[11]["start"]:
        wpu = 0.0
    elif during(11, t):  # "TORTURE, THAT IS WORK!" — straightens to full height
        wpu = clamp(line_u(11, t))
    else:
        wpu = 1.0
    lunge = clamp(line_u(13, t)) if during(13, t) else 0.0
    wx_now = wx - 120 * lunge
    worker(ctx, wx_now, H - 90, 300, 0, P, pose="rise", pu=wpu)
    if grabbed:  # the whip, seized, cracks skyward from the worker's fist
        ctx.set_source_rgb(*ink)
        ctx.set_line_width(12)
        hx_, hy_ = wx_now - 70, H - 90 - 300 * 0.85
        ctx.move_to(hx_, hy_)
        crack = beat_pulse(t, 0.12)
        for k in range(1, 9):
            u = k / 8
            ctx.line_to(hx_ - 60 * u,
                        hy_ - 300 * u - 90 * u * math.sin(u * 6 - crack * 10)
                        * (0.4 + crack))
        ctx.stroke()
    # the seizing fist, rising through the verse with a torch
    prog = min(1.0, max(0.0, (t - 53.0) / 6.5))
    fy = H + 320 - prog * 640
    rect(ctx, W * 0.70 - 60, fy, 120, 700, P["red2"])
    disc(ctx, W * 0.70, fy, 95, P["red2"])
    rect(ctx, W * 0.70 - 95, fy - 40, 190, 80, P["red2"])
    flame(ctx, W * 0.70, fy - 70, 240, 220 + rms(t) * 130, t, 9, P, 0.0, 0.95)
    crowd(ctx, t, H + 40, 1, P, 0.8, fz)

def sc_verse3(ctx, t, P, fz):
    rect(ctx, 0, 0, W, H, P["bg"])
    ink = P["ink"]
    # hoard-light behind Beelzebub
    grad = cairo.RadialGradient(W * 0.75, H - 240, 80, W * 0.75, H - 240, 760)
    grad.add_color_stop_rgba(0, *P["ember"], 0.40)
    grad.add_color_stop_rgba(1, *P["ember"], 0.0)
    ctx.set_source(grad)
    ctx.paint()
    # Beelzebub hunched over his hoard of souls
    hoard = min(1.0, (t - 58.0) / 12.0)
    for k in range(int(50 * hoard) + 8):
        sd = k * 3.77
        px = W * 0.75 + 340 * (((sd) % 1.0) - 0.5) * (1 - k / 90)
        py = H - 60 - (k // 9) * 44 - 12 * ((sd * 3) % 1.0)
        disc(ctx, px, py, 24, P["ember"], 0.9)
        disc(ctx, px, py, 10, P["paper"], 0.7)
    # tiny workers at the edge, having their anguish skimmed
    for k, tx in enumerate((90, 190, 290)):
        bob = 4 * math.sin(t * 2 + k)
        poly(ctx, [(tx - 28, H - 60), (tx + 28, H - 60),
                   (tx + 18, H - 170 + bob), (tx - 18, H - 170 + bob)], ink)
        horned_head(ctx, tx, H - 190 + bob, 18, ink)
    if during(15, t):
        # "SURPLUS ANGUISH! Skimmed from everyone!" — souls stream to the hoard
        for k in range(10):
            sd = k * 1.618
            u = ((t * 0.45) + sd) % 1.0
            sx = 90 + (sd * 77) % 220
            x = lerp(sx, W * 0.75, u)
            y = H - 200 - 320 * math.sin(math.pi * u) + 40 * math.sin(sd * 9)
            disc(ctx, x, y, 14, P["ember"], 0.9 * min(1, 4 * u * (1 - u) + 0.3))
            disc(ctx, x, y, 6, P["paper"], 0.7)
    # Beelzebub: greedy scooping until the specter scares him out of his hunch
    startled = t >= LINES[16]["start"]
    su = clamp(line_u(16, t)) if startled else 0.0
    jump = math.exp(-3.0 * su) if startled else 0.0
    ctx.save()
    ctx.translate(W * 0.75 + 40 * su * (1 - su), H - 430 - 130 * jump)
    if startled:
        ctx.rotate(math.radians(10) * (jump + 0.3 * su))  # leans away from the ghost
    poly(ctx, [(-160, 220), (160, 220), (90, -120), (-90, -120)], ink)  # hunch
    horned_head(ctx, -60, -160, 44, ink)
    # arms: raking souls onto the pile, one scoop per beat; flung up when startled
    ctx.set_source_rgb(*ink)
    ctx.set_line_width(26)
    if startled:
        for sgn in (-1, 1):
            ctx.move_to(sgn * 70, -80)
            ctx.line_to(sgn * 150, -230 - 30 * sgn * math.sin(t * 9))
            ctx.stroke()
    else:
        big = 1.35 if during(14, t) else 1.0  # hoarding by the TON
        sc = beat_phase(t)
        for sgn, off in ((-1, 0.0), (1, 0.5)):
            u = (sc + off) % 1.0
            ax = sgn * lerp(220, 60, math.sin(math.pi * u)) * big
            ay = lerp(60, 180, u ** 2)
            ctx.move_to(sgn * 60, -60)
            ctx.line_to(ax, ay)
            ctx.stroke()
            disc(ctx, ax, ay, 30, ink)
    # fly wings, buzzing with the music (frantic when startled)
    buzz = 21 + 26 * su
    for s in (-1, 1):
        ctx.save()
        ctx.translate(s * 100, -110)
        ctx.rotate(s * (0.5 + (0.12 + 0.10 * su) * math.sin(t * buzz)))
        ctx.scale(1, 0.45)
        disc(ctx, 0, -90, 95, P["dim"], 0.85)
        ctx.restore()
    ctx.restore()
    # the specter of Pandemonium rises (t ~ 67+)
    sp = min(1.0, max(0.0, (t - 66.5) / 4.0))
    if sp > 0:
        gy = H * 0.72 - sp * H * 0.34
        ctx.save()
        ctx.translate(W * 0.28, gy)
        ctx.set_source_rgba(*P["paper"], 0.85 * sp)
        ctx.arc(0, -120, 120, math.pi, 0)
        wob = [(120, -120)]
        for k in range(9):
            u = k / 8
            wob.append((120 - 240 * u,
                        60 * math.sin(u * 12 + t * 3) * 0.4 + 130))
        for p_ in wob:
            ctx.line_to(*p_)
        ctx.close_path()
        ctx.fill()
        for s in (-1, 1):  # eyes
            disc(ctx, s * 45, -130, 16, ink, 0.9 * sp)
        # it carries the pamphlet — thrust skyward on the beat for
        # "I read one pamphlet in the ashes and now I'm here to set us free!"
        thrust = beat_pulse(t) if during(17, t) else 0.0
        px_, py_ = 90 + 20 * thrust, -60 - 160 * thrust
        if during(17, t):
            sunburst(ctx, px_ + 55, py_ + 40, 60, 220 + 60 * thrust, 12,
                     t * 0.4, P["ember"], 0.35 * sp)
        ctx.set_source_rgba(*P["red2"], sp)
        ctx.save()
        ctx.translate(px_, py_)
        ctx.rotate(0.1 * math.sin(t * 2) - 0.5 * thrust)
        ctx.rectangle(0, 0, 110, 80)
        ctx.fill()
        ctx.restore()
        ctx.restore()
    # pamphlet pages spiralling up
    for k in range(9):
        sd = k * 5.31
        u = ((t * 0.16) + sd) % 1.0
        px = W * 0.28 + 200 * math.sin(u * 9 + sd)
        py = H * 0.9 - u * H * 0.8
        ctx.save()
        ctx.translate(px, py)
        ctx.rotate(u * 7 + sd)
        ctx.set_source_rgba(*P["paper"], (1 - u) * 0.8)
        ctx.rectangle(-30, -20, 60, 40)
        ctx.fill()
        ctx.restore()

def sc_bridge(ctx, t, P, fz):
    rect(ctx, 0, 0, W, H, P["bg"])
    # vast cold sun
    disc(ctx, W * 0.5, H * 0.34, 250, P["paper"], 0.14)
    disc(ctx, W * 0.5, H * 0.34, 190, P["paper"], 0.18)
    # lone stoker contemplating the pitchfork held flat in both hands
    ink = P["ink"]
    ctx.save()
    ctx.translate(W * 0.5, H - 150)
    sway = math.sin(t * 0.8) * 0.02
    ctx.rotate(sway)
    poly(ctx, [(-90, 0), (90, 0), (60, -360), (-60, -360)], ink)
    horned_head(ctx, 0, -430, 52, ink)
    ctx.set_source_rgb(*ink)
    ctx.set_line_width(26)
    ctx.move_to(-150, -240)
    ctx.line_to(150, -240)
    ctx.stroke()
    pitchfork(ctx, -210, -240, 420, math.radians(90), ink, 20)
    ctx.restore()
    icicles(ctx, 0, W, 0, 24, 130 * min(1, fz * 1.6), P, seed=4)
    snow(ctx, t, 90, P, drift=14, alpha=0.5)

def sc_break(ctx, t, P, fz):
    # stark alternating frames on the shouted line
    flash = beat_pulse(t, 0.1) > 0.6
    bg = P["red2"] if flash else P["ink"]
    fg = P["ink"] if flash else P["paper"]
    rect(ctx, 0, 0, W, H, bg)
    zoom = 1.0 + (t - 87.4) * 0.35
    ctx.save()
    ctx.translate(W / 2, H * 0.98)
    ctx.scale(zoom, zoom)
    pitchfork(ctx, 0, 0, 760, 0, fg, 34)
    ctx.restore()
    sunburst(ctx, W / 2, H * 0.30, 90, 1300, 18, t * 0.3, fg, 0.16)

def sc_finale(ctx, t, P, fz):
    rect(ctx, 0, 0, W, H, P["bg"])
    prog = min(1.0, (t - 93.5) / 20.0)
    # wedge fully pierces the shattering disc
    cx, cy = W * 0.70, H * 0.32
    if prog < 0.6:
        disc(ctx, cx, cy, 240 * (1 - prog * 0.6), P["ink"])
    else:  # disc shatters into shards drifting out
        for k in range(10):
            sd = k * 2.7
            a = sd * 2.4
            d = (prog - 0.6) * 900 * (0.4 + (sd % 1.0))
            ctx.save()
            ctx.translate(cx + d * math.cos(a), cy + d * math.sin(a) * 0.6)
            ctx.rotate(a + prog * 3)
            poly(ctx, [(-60, 20), (60, 34), (10, -60)], P["ink"], 1 - (prog - 0.6) * 2)
            ctx.restore()
    poly(ctx, [(0, H * 0.66), (0, H * 0.06), (cx + 60, cy)], P["red2"])
    # frozen flames — the pits gone cold and still
    for k in range(6):
        flame(ctx, k * 380 + 100, H + 20, 300, 150, t, k + 3, P, 1.0, 0.55)
    icicles(ctx, 0, W, 0, 30, 100 + 80 * prog, P, seed=7)
    # the great march, bigger and closer than before
    crowd(ctx, t, H - 20, 4, P, 1.25, fz)
    # chains across the sky, shattering near "we are literally all in chains"
    shatter = t > 108.4
    if not shatter:
        chain(ctx, -40, 120, W * 0.55, 60, 16, math.sin(t * 2), P["dim"], 8)
        chain(ctx, W * 0.45, 40, W + 40, 150, 16, math.cos(t * 2.3), P["dim"], 8)
    else:
        for k in range(22):
            sd = k * 4.9
            u = min(1.0, (t - 108.4) / 2.5)
            x = (sd * 137) % W + 140 * u * math.sin(sd)
            y = 100 + ((sd * 61) % 120) + u * u * 700 * (0.5 + (sd % 1.0))
            ctx.save()
            ctx.translate(x, y)
            ctx.rotate(sd + u * 9)
            ctx.set_source_rgba(*P["dim"], max(0.0, 1 - u * 0.8))
            ctx.set_line_width(8)
            ctx.scale(1.0, 0.62)
            ctx.arc(0, 0, 13, 0, TAU)
            ctx.stroke()
            ctx.restore()
    snow(ctx, t, 140 * fz, P, drift=50, alpha=0.7)

def sc_outro(ctx, t, P, fz):
    rect(ctx, 0, 0, W, H, P["bg"])
    sunburst(ctx, W / 2, H / 2, 200, 1400, 26, t * 0.015, P["dim"], 0.30)
    disc(ctx, W / 2, H / 2, 330, P["red"], 0.85)
    disc(ctx, W / 2, H / 2, 330, P["red2"], 0.35)
    # emblem: crossed pitchfork and shovel inside a gear ring
    ink = P["ink"]
    gear(ctx, W / 2, H / 2, 300, 16, t * 0.05, ink, 0.9)
    disc(ctx, W / 2, H / 2, 265, P["red"], 1.0)
    pitchfork(ctx, W / 2 - 90, H / 2 + 190, 400, math.radians(24), ink, 18)
    ctx.save()  # shovel
    ctx.translate(W / 2 + 90, H / 2 + 190)
    ctx.rotate(math.radians(-24))
    ctx.set_source_rgb(*ink)
    ctx.set_line_width(18)
    ctx.move_to(0, 0)
    ctx.line_to(0, -290)
    ctx.stroke()
    disc(ctx, 0, -320, 52, ink)
    ctx.restore()
    snow(ctx, t, 110, P, drift=10, alpha=0.55)

SCENES = [
    (0.0, 6.4, sc_title),
    (6.4, 20.6, sc_verse1),
    (20.6, 44.7, sc_chorus1),
    (44.7, 60.3, sc_verse2),
    (60.3, 75.0, sc_verse3),
    (75.0, 87.4, sc_bridge),
    (87.4, 93.5, sc_break),
    (93.5, 118.6, sc_finale),
    (118.6, 130.0, sc_outro),
]
XFADE = 0.5

# ---------------------------------------------------------------- frame loop

_grain = None

def grain_surface():
    global _grain
    if _grain is None:
        rng = np.random.default_rng(7)
        g = rng.integers(0, 60, (H, W), dtype=np.uint32)
        argb = (g << 24).astype(np.uint32)  # alpha-only noise
        buf = np.ascontiguousarray(argb)
        _grain = (cairo.ImageSurface.create_for_data(
            buf, cairo.FORMAT_ARGB32, W, H, W * 4), buf)
    return _grain[0]

def draw_scene(ctx, t):
    P = pal(freeze_at(t))
    fz = freeze_at(t)
    for i, (a, b, fn) in enumerate(SCENES):
        if a <= t < b:
            fn(ctx, t, P, fz)
            if b - t < XFADE and i + 1 < len(SCENES):
                nxt = SCENES[i + 1][2]
                ctx.push_group()
                nxt(ctx, t, P, fz)
                ctx.pop_group_to_source()
                ctx.paint_with_alpha(1 - (b - t) / XFADE)
            return

def render_frame(args):
    i, outdir = args
    t = i / FPS
    surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, W, H)
    ctx = cairo.Context(surf)
    # camera shake on heavy hits in break + finale
    shake = 0.0
    if 87.4 <= t < 118.6:
        shake = low(t) * 10
    ctx.save()
    if shake:
        ctx.translate(shake * math.sin(i * 3.1), shake * math.cos(i * 2.3))
    draw_scene(ctx, t)
    ctx.restore()
    # grain + vignette
    ctx.set_source_surface(grain_surface(), (i * 7) % 13 - 6, (i * 5) % 11 - 5)
    ctx.paint_with_alpha(0.5)
    grad = cairo.RadialGradient(W / 2, H / 2, H * 0.45, W / 2, H / 2, H * 0.95)
    grad.add_color_stop_rgba(0, 0, 0, 0, 0)
    grad.add_color_stop_rgba(1, 0, 0, 0, 0.42)
    ctx.set_source(grad)
    ctx.paint()
    surf.write_to_png(os.path.join(outdir, f"f{i:05d}.png"))
    return i

def main():
    outdir = sys.argv[1]
    os.makedirs(outdir, exist_ok=True)
    start = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    end = int(sys.argv[3]) if len(sys.argv) > 3 else ENV["n_frames"]
    todo = [(i, outdir) for i in range(start, end)
            if not os.path.exists(os.path.join(outdir, f"f{i:05d}.png"))]
    with Pool(4) as pool:
        for k, _ in enumerate(pool.imap_unordered(render_frame, todo, chunksize=24)):
            if k % 240 == 0:
                print(f"{k}/{len(todo)}", flush=True)
    print("done")

if __name__ == "__main__":
    main()
