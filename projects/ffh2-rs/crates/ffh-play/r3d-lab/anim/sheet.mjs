// sheet.mjs: review sheets of a baked clip (docs/ANIMATION.md "Review sheets"), drawn by the
// software renderer. One PNG shows what an animator and a vision judge need to see:
//
//   A  every frame at the true gameplay size (96 px a plot, the figure ~31 px) and enlarged 3x
//      nearest-neighbour, on a ground tile with a faint plot grid (so a sliding foot shows);
//   B  a timing chart at the closest zoom: an onion skin of the clip and per-frame dots on the
//      striking point, the head and the pelvis -- the dots' spacing is the speed, the way an
//      animator reads spacing (a still frame cannot show timing);
//   C  curves over time: the striking point's speed, the head's and the centre of mass's height,
//      with the clip's events marked;
//   D  (not blind) the validator's failed and scored metrics.
//
// `blind` drops every name, number and verdict except frame indices (vision judges must not be
// told what they are looking at, nor what the validator thinks).

import { Image, camera, renderFrame, text } from "./raster.mjs";
import { model, check } from "./check.mjs";

const BG = [228, 222, 204, 255];
const GROUND = [176, 168, 132];
const INK = [40, 32, 24];

/** A ground tile with a faint plot grid under a figure at `cam` (plots around the feet). */
function groundTile(cam) {
  const im = new Image(cam.w, cam.h, [...GROUND, 255]);
  for (let gx = -2; gx <= 2; gx++) {
    for (let k = 0; k <= 40; k++) {
      const z = -2 + (4 * k) / 40;
      const a = cam.project([gx + 0.5, 0, z]), c = cam.project([gx + 0.5, 0, z + 0.1]);
      im.line(a, c, [150, 142, 110]);
    }
  }
  for (let gz = -2; gz <= 2; gz++) {
    const a = cam.project([-2, 0, gz + 0.5]), c = cam.project([2, 0, gz + 0.5]);
    im.line(a, c, [150, 142, 110]);
  }
  return im;
}

/** Frames to show: all of a short clip, else an even sample of `max`, always with the events'. */
function sample(b, max) {
  const out = new Set();
  const step = Math.max(1, Math.ceil(b.frames / max));
  for (let f = 0; f < b.frames; f += step) out.add(f);
  out.add(b.frames - 1);
  for (const t of Object.values(b.clip.events ?? {})) out.add(Math.max(0, Math.min(b.frames - 1, Math.round(t * b.fps))));
  return [...out].sort((x, y) => x - y);
}

/**
 * The review sheet of bake `b` (anim.js bakeClip) as an Image.
 * opts: {yaw (default 0.25), blind, results (check() output, computed when absent and not blind),
 *        cells (frames in A, default 24), team [r, g, b] linear}
 */
export function sheet(b, opts = {}) {
  const yaw = opts.yaw ?? 0.25;
  const blind = !!opts.blind;
  const results = blind ? null : opts.results ?? check(b);
  const frames = sample(b, opts.cells ?? 24);
  // Cells frame everything the clip covers at the gameplay zoom (a corpse lies half a plot from
  // its feet), at least 56 px.
  const probe = camera({ ts: 96, yaw, w: 1, h: 1, ox: 0, oy: 0 });
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (let f = 0; f < b.frames; f += Math.max(1, Math.floor(b.frames / 12))) {
    for (let v = 0; v < b.count; v += 5) {
      const q = (f * b.count + v) * 3;
      const p = probe.project([b.pos[q], b.pos[q + 1], b.pos[q + 2]]);
      x0 = Math.min(x0, p[0]);
      x1 = Math.max(x1, p[0]);
      y0 = Math.min(y0, p[1]);
      y1 = Math.max(y1, p[1]);
    }
  }
  const cell = Math.max(56, Math.ceil(Math.max(x1 - x0, y1 - y0) + 8));
  const fx = cell / 2 - (x0 + x1) / 2, fy = cell / 2 - (y0 + y1) / 2;
  const K = cell > 80 ? 2 : 3, per = cell > 80 ? 6 : 8;
  const W = Math.max(per * (cell * K + 4) + 20, frames.length * (cell + 2) + 20, 1240);
  const rowsA = Math.ceil(frames.length / per);
  const H = 30 + cell + 26 + rowsA * (cell * K + 18) + 440 + (blind ? 0 : 20 + Math.min(40, results.filter((r) => !r.pass).length + 6) * 12);
  const out = new Image(W, H, BG);
  let y = 8;
  if (!blind) {
    const fails = results.filter((r) => r.hard && !r.pass).length;
    text(out, 10, y, `${b.recipe} ${b.clipName} ${b.param} v${b.variant}  ${b.frames} frames at ${b.fps} fps  ${b.clip.dur}s  ${fails ? fails + " hard fails" : "pass"}`, fails ? [170, 20, 20] : INK, 2);
  } else text(out, 10, y, `frames left to right, ${b.fps} per second`, INK, 2);
  y += 22;
  // A: the true-size strip, then the enlarged cells.
  const game = (f, w = cell, h = cell) => {
    const cam = camera({ ts: 96, yaw, w, h, ox: fx, oy: fy });
    const im = groundTile(cam);
    renderFrame(b, f, cam, { into: im, team: opts.team });
    return im;
  };
  frames.forEach((f, i) => out.paste(game(f), 10 + i * (cell + 2), y));
  y += cell + 26;
  frames.forEach((f, i) => {
    const cx = 10 + (i % per) * (cell * K + 4), cy = y + Math.floor(i / per) * (cell * K + 18);
    out.paste(game(f), cx, cy, K);
    const ev = Object.entries(b.clip.events ?? {}).filter(([, t]) => Math.round(t * b.fps) === f).map(([n]) => n);
    text(out, cx + 2, cy + cell * K + 3, blind ? `${i + 1}` : `F${f} ${b.times[f].toFixed(2)}S ${ev.join(" ")}`, INK, 2);
  });
  y += rowsA * (cell * K + 18) + 6;
  // B: the timing chart at the closest zoom.
  const m = model(b);
  const TS = 300, BW = 520, BH = 420;
  const cam = camera({ ts: TS, yaw, w: BW, h: BH, oy: BH * 0.72 });
  const chart = groundTile(cam);
  const step = Math.max(1, Math.round(b.frames / 14));
  for (let f = 0; f < b.frames; f += step) {
    const t = f / Math.max(1, b.frames - 1);
    renderFrame(b, f, cam, { into: chart, ghost: 0.28, ss: 2, tint: [Math.round(60 + 160 * t), 90, Math.round(220 - 160 * t)] });
  }
  renderFrame(b, b.frames - 1, cam, { into: chart, ghost: 0.85, ss: 2, team: opts.team });
  const trail = (pts, c) => {
    for (let f = 0; f + 1 < pts.length; f++) chart.line(cam.project(pts[f]), cam.project(pts[f + 1]), c, 0.35);
    for (const p of pts) chart.dot(cam.project(p), 2, c);
  };
  trail(m.strikes, [200, 30, 30]);
  trail(b.joints.map((J) => J.head), [40, 70, 200]);
  trail(b.joints.map((J) => J.pelvis), [30, 140, 60]);
  out.paste(chart, 10, y);
  if (!blind) text(out, 14, y + 4, "TIMING: RED STRIKE POINT, BLUE HEAD, GREEN PELVIS, ONE DOT A FRAME", INK, 2);
  // C: curves.
  const CX = 10 + BW + 20, CW = W - CX - 10, CH = BH;
  out.fill(CX, y, CW, CH, [240, 236, 222]);
  const plot = (vals, c, lo, hi) => {
    for (let f = 0; f + 1 < vals.length; f++) {
      const p = (k) => [CX + 6 + ((CW - 12) * k) / Math.max(1, vals.length - 1), y + CH - 8 - ((CH - 30) * (vals[k] - lo)) / Math.max(1e-9, hi - lo)];
      out.line(p(f), p(f + 1), c);
    }
  };
  const speed = m.strikes.map((_, k) => (k === 0 || k === b.frames - 1 ? 0 : (Math.hypot(...[0, 1, 2].map((i) => m.strikes[k + 1][i] - m.strikes[k - 1][i])) * b.fps) / 2));
  plot(speed, [200, 30, 30], 0, Math.max(...speed, 1e-6));
  const hh = b.joints.map((J) => J.head[1]), ch = m.coms.map((c) => c[1]);
  const top = Math.max(...hh, ...ch);
  plot(hh, [40, 70, 200], 0, top);
  plot(ch, [30, 140, 60], 0, top);
  for (const [name, t] of Object.entries(b.clip.events ?? {})) {
    const x = CX + 6 + ((CW - 12) * Math.round(t * b.fps)) / Math.max(1, b.frames - 1);
    out.line([x, y + 18], [x, y + CH - 4], [150, 120, 40]);
    if (!blind) text(out, x + 2, y + 20, name, [150, 120, 40], 2);
  }
  if (!blind) text(out, CX + 6, y + 4, "SPEED OF STRIKE POINT (RED), HEAD AND MASS HEIGHT", INK, 2);
  y += BH + 14;
  // D: verdicts.
  if (!blind) {
    const show = results.filter((r) => !r.pass).concat(results.filter((r) => r.pass && !r.hard)).slice(0, 40);
    for (const r of show) {
      text(out, 10, y, `${r.pass ? "OK  " : r.hard ? "FAIL" : "WARN"} ${r.id} ${String(r.value).slice(0, 60)} LIMIT ${String(r.limit).slice(0, 40)}${r.frame != null ? " AT F" + r.frame : ""}`, r.pass ? INK : r.hard ? [170, 20, 20] : [150, 100, 20], 2);
      y += 12;
    }
  }
  out.frames = frames; // the frame shown in each cell (cell k, from 1, is frames[k - 1])
  return out;
}
