// anim-soldier.js: the clips of unit.soldier (docs/ANIMATION.md): a sword (axe, mace) in the right
// hand raised at the rest pose, a shield on the left arm, the left foot forward (soldierSpec).
//
// idle    a guard at ease: breathing, the weight drifting between the feet, one glance aside.
// attack  a descending diagonal cut: the blade cocked back over the right shoulder, a step in on
//         the front foot, the blade whipped over and down across the body, the shield arm pulled
//         back against it, a held follow-through low before the front knee, a step back to the
//         guard.
// death   struck from the front: thrown back, a stagger step, the knees give, a fall onto the
//         back with the arms trailing, a small rebound, still.
//
// Positions are deltas in a human's plots (the figure scales them to its race); the weapon's turn
// (rRot) is authored as blade directions (`blade` below) because a blow turns the blade through
// half a circle, which per-component Euler keys would not follow.

import { clips } from "../anim.js";

// --- the blade: directions in, rRot keys out ----------------------------------------------------

/** soldierSpec's right.dir: the weapon's axis at rest (sword, axe and mace alike). */
const REST_DIR = [-0.5, 0.85, 0.18];

const nrm = (v) => {
  const l = Math.hypot(...v) || 1;
  return v.map((x) => x / l);
};
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const qAxis = (ax, th) => {
  const s = Math.sin(th / 2), n = nrm(ax);
  return [Math.cos(th / 2), n[0] * s, n[1] * s, n[2] * s];
};
const qMul = (a, b) => [
  a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
  a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
  a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
  a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
];
const qDot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
const qAngle = (a, b) => 2 * Math.acos(Math.min(1, Math.abs(qDot(a, b))));
/** The shortest turn taking direction a to direction b. */
function qTurn(a, b) {
  a = nrm(a);
  b = nrm(b);
  const ax = cross(a, b), s = Math.hypot(...ax), c = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  return s < 1e-9 ? [1, 0, 0, 0] : qAxis(ax, Math.atan2(s, c));
}
function qSlerp(a, b, u) {
  let d = qDot(a, b);
  if (d < 0) {
    b = b.map((x) => -x);
    d = -d;
  }
  if (d > 0.9995) return nrm(a.map((x, i) => x + (b[i] - x) * u));
  const th = Math.acos(d), s = Math.sin(th);
  return a.map((x, i) => (Math.sin((1 - u) * th) * x + Math.sin(u * th) * b[i]) / s);
}
/** Euler [x, y, z] of q in units.js eulerTurn's order (R = Rz Ry Rx): of its two forms (and their
 *  2 pi wraps) the one nearest `prev`, so a track of them turns continuously. */
function qEuler(q, prev) {
  const [w, x, y, z] = q;
  const r20 = 2 * (x * z - w * y), r21 = 2 * (y * z + w * x), r22 = 1 - 2 * (x * x + y * y);
  const r10 = 2 * (x * y + w * z), r00 = 1 - 2 * (y * y + z * z);
  const ex = Math.atan2(r21, r22), ey = -Math.asin(Math.max(-1, Math.min(1, r20))), ez = Math.atan2(r10, r00);
  let best = null, bd = Infinity;
  for (const c of [[ex, ey, ez], [ex + Math.PI, Math.PI - ey, ez + Math.PI]]) {
    const v = c.map((a, i) => a + 2 * Math.PI * Math.round((prev[i] - a) / (2 * Math.PI)));
    const d = v.reduce((s, a, i) => s + (a - prev[i]) ** 2, 0);
    if (d < bd) {
      bd = d;
      best = v;
    }
  }
  return best;
}
/** Monotone cubic tangents (as anim.js): zero at the ends and at turning points. */
function tangents(ts, vs) {
  const n = ts.length, h = [], d = [];
  for (let k = 0; k + 1 < n; k++) {
    h.push(Math.max(1e-9, ts[k + 1] - ts[k]));
    d.push((vs[k + 1] - vs[k]) / h[k]);
  }
  const m = new Array(n).fill(0);
  for (let k = 1; k + 1 < n; k++) {
    if (d[k - 1] * d[k] <= 0) continue;
    const w1 = 2 * h[k] + h[k - 1], w2 = h[k] + 2 * h[k - 1];
    m[k] = (w1 + w2) / (w1 / d[k - 1] + w2 / d[k]);
  }
  return m;
}

/**
 * rRot keys from blade keys [{t, dir, roll, stop, i}]: `dir` the weapon's axis (figure space),
 * `roll` (radians) a turn about it, `stop` a turning point (the blade comes to rest there; the
 * first and last keys are), `i` "in" | "out" the segment after it accelerating or decelerating
 * (a fall, a recoil). Between keys the blade turns by the shortest way at the pace of a monotone
 * cubic through the angle it has turned, sampled every `step` seconds, so the Euler keys follow it.
 */
function blade(list, step) {
  const Q = list.map((k) => qMul(qAxis(nrm(k.dir), k.roll ?? 0), qTurn(REST_DIR, k.dir)));
  // The progress through each stroke (from one stop to the next): turned angle against time.
  const s = [0];
  for (let k = 1; k < list.length; k++) s.push(s[k - 1] + qAngle(Q[k - 1], Q[k]));
  const m = new Array(list.length).fill(0);
  let a = 0;
  for (let k = 1; k < list.length; k++) {
    if (!list[k].stop && k < list.length - 1) continue;
    const ts = list.slice(a, k + 1).map((x) => x.t), mt = tangents(ts, s.slice(a, k + 1));
    for (let j = 0; j < mt.length; j++) m[a + j] = mt[j];
    a = k;
  }
  const at = (k, t) => {
    const h = list[k + 1].t - list[k].t, u = (t - list[k].t) / h;
    const span = s[k + 1] - s[k];
    if (span < 1e-9) return 0;
    if (list[k].i === "in") return u * u;
    if (list[k].i === "out") return u * (2 - u);
    const h00 = 2 * u ** 3 - 3 * u * u + 1, h10 = u ** 3 - 2 * u * u + u, h01 = -2 * u ** 3 + 3 * u * u, h11 = u ** 3 - u * u;
    return (h00 * s[k] + h10 * h * m[k] + h01 * s[k + 1] + h11 * h * m[k + 1] - s[k]) / span;
  };
  const out = [];
  let prev = [0, 0, 0];
  const push = (t, q) => {
    prev = qEuler(q, prev);
    out.push({ t, pose: { rRot: prev.slice() } });
  };
  for (let k = 0; k < list.length; k++) {
    push(list[k].t, Q[k]);
    if (k + 1 === list.length) break;
    for (let n = Math.floor(list[k].t / step) + 1; n * step < list[k + 1].t - step * 0.3; n++) {
      if (n * step < list[k].t + step * 0.3) continue;
      push(n * step, qSlerp(Q[k], Q[k + 1], Math.max(0, Math.min(1, at(k, n * step)))));
    }
  }
  return out;
}

// --- attack: a descending diagonal cut ----------------------------------------------------------
//
// 0.00-0.36  anticipation: the weight settles back onto the rear foot, the torso coils to the
//            right, the sword is drawn up above the head and back over the right shoulder (a
//            moving hold 0.30-0.36); the shield comes forward to cover; the head stays on target.
// 0.36-0.44  the blow: the front foot steps in (planted 0.43), the torso uncoils to the left and
//            leans in, the blade trails the hand over the top, then whips down across the body;
//            impact at 0.44, the blade still at speed.
// 0.44-0.60  follow-through: the blade carries on down before the front knee and holds there; the
//            shield arm is pulled back against the cut. The arm stays extended (a hunched race's
//            head would meet the hand); the hand stops at belly height and the point short of the
//            ground (weapons keep their length on small races: a goblin's sword or axe would
//            otherwise end in the ground).
// 0.60-1.10  recovery: the blade comes back up on the sword arm's side into the guard; the front
//            foot steps back.

const ATTACK_BLADE = [
  { t: 0, dir: REST_DIR, stop: true },
  // Drawn back, the wrist turning the edge toward the coming cut.
  { t: 0.12, dir: [-0.5, 0.82, -0.15], roll: -0.5 },
  { t: 0.3, dir: [-0.42, 0.6, -0.68], roll: -1.2 },
  { t: 0.36, dir: [-0.4, 0.56, -0.72], roll: -1.26, stop: true },
  // The cut, edge first: trailing the hand over the top, then whipped down across the body.
  { t: 0.4, dir: [-0.3, 0.95, 0.1], roll: -1.4 },
  { t: 0.44, dir: [0.25, -0.2, 0.95], roll: -1.62 },
  { t: 0.5, dir: [0.3, -0.6, 0.74], roll: -1.6, stop: true },
  { t: 0.6, dir: [0.29, -0.56, 0.78], roll: -1.55, stop: true },
  // Back up on the sword arm's side (outside the body's outline) into the guard.
  { t: 0.8, dir: [-0.45, 0.88, 0.02], roll: -0.4 },
  { t: 1.1, dir: REST_DIR, stop: true },
];

const attack = {
  kind: "attack",
  dur: 1.1,
  fps: 60,
  returns: true,
  strike: [0.07, 0, 0.998],
  events: { impact: 0.44 },
  contacts: { L: [[0, 0.32], [0.43, 0.72], [0.88, 1.1]], R: [[0, 1.1]] },
  keys: [
    { t: 0.12, pose: { pelvis: [0.003, -0.004, -0.008], lean: -0.015, twist: -0.16, roll: -0.02, headYaw: 0.05, rHand: [-0.024, 0.063, -0.021] } },
    { t: 0.3, pose: { pelvis: [0.006, -0.012, -0.024], lean: -0.07, twist: -0.42, roll: -0.06, headYaw: 0.13, rHand: [-0.047, 0.158, -0.062], lHand: [-0.001, 0.036, 0.022] } },
    { t: 0.32, pose: { fl: [0, 0, 0] } },
    { t: 0.36, pose: { pelvis: [0.007, -0.014, -0.028], lean: -0.08, twist: -0.46, roll: -0.065, headYaw: 0.14, rHand: [-0.052, 0.17, -0.064], lHand: [0, 0.044, 0.024], fl: [0, 0.016, 0.012] } },
    { t: 0.4, pose: { pelvis: [0.002, -0.02, 0.012], lean: 0.05, twist: -0.1, roll: -0.02, headYaw: 0.03, rHand: [0.005, 0.169, -0.014], lHand: [0.015, 0.032, -0.036], fl: [0, 0.01, 0.042] } },
    { t: 0.43, pose: { fl: [0, 0, 0.05] } },
    { t: 0.44, pose: { pelvis: [-0.004, -0.026, 0.036], lean: 0.18, twist: 0.3, roll: 0.05, headYaw: -0.1, rHand: [0.071, 0.023, 0.008], lHand: [0.043, 0.027, -0.101] } },
    { t: 0.5, pose: { pelvis: [-0.006, -0.032, 0.042], lean: 0.26, twist: 0.46, roll: 0.09, headYaw: -0.16, rHand: [0.117, 0.055, 0.014], lHand: [0.063, 0.054, -0.149] } },
    { t: 0.6, pose: { pelvis: [-0.005, -0.029, 0.04], lean: 0.23, twist: 0.42, roll: 0.08, headYaw: -0.15, rHand: [0.115, 0.06, 0.021], lHand: [0.058, 0.055, -0.135] } },
    { t: 0.72, pose: { fl: [0, 0, 0.05] } },
    { t: 0.8, pose: { pelvis: [-0.002, -0.012, 0.02], lean: 0.08, twist: 0.15, roll: 0.02, headYaw: -0.05, rHand: [0.012, 0.005, 0.007], lHand: [0.022, 0.012, -0.051], fl: [0, 0.016, 0.028] } },
    { t: 0.88, pose: { fl: [0, 0, 0] } },
    ...blade(ATTACK_BLADE, 1 / 60),
  ],
};

// --- idle: a guard at ease ------------------------------------------------------------------------
//
// Three slow breaths a loop (in over 0.8 s: the chest straightens and the body rises; out over
// 0.8 s; a 0.4 s pause), the hands following each breath 0.15 s late; the weight drifting onto the
// front foot and back once (hips over the foot, shoulders tilted against them), the sword's point
// wandering a little; at 2.4 s a glance off to the shield side, held, then back past centre and
// settled. The feet never move. Every track is at rest at t = 0 (the loop starts where an attack
// ends).

/** The breathing keys: `n` breaths of `period` seconds from t = 0. */
function breaths(n, period) {
  const keys = [];
  for (let c = 0; c < n; c++) {
    const t = c * period;
    keys.push(
      { t: t + 0.8, pose: { pelvis: [0, 0.007, 0], lean: -0.025 } },
      { t: t + 1.6, pose: { pelvis: [0, 0, 0], lean: 0 } },
      { t: t + 0.95, pose: { rHand: [0, 0.01, 0], lHand: [0, 0.008, 0] } },
      { t: t + 1.75, pose: { rHand: [0, 0, 0], lHand: [0, 0, 0] } },
    );
    if (c + 1 < n) keys.push({ t: t + period, pose: { pelvis: [0, 0, 0], lean: 0 } }, { t: t + period + 0.15, pose: { rHand: [0, 0, 0], lHand: [0, 0, 0] } });
  }
  return keys;
}

const idle = {
  kind: "idle",
  dur: 6,
  fps: 12,
  loop: true,
  contacts: { L: [[0, 6]], R: [[0, 6]] },
  keys: [
    ...breaths(3, 2),
    { t: 0, pose: { headYaw: 0, headTilt: 0 } },
    { t: 2.4, pose: { headYaw: 0, headTilt: 0 } },
    { t: 2.9, pose: { headYaw: 0.34, headTilt: 0.05 } },
    { t: 3.6, pose: { headYaw: 0.3, headTilt: 0.04 } },
    { t: 4.2, pose: { headYaw: -0.04, headTilt: 0 } },
    { t: 4.7, pose: { headYaw: 0, headTilt: 0 } },
  ],
  waves: [
    { ch: "pelvis", amp: [0.009, 0, 0.004], n: 1 },
    { ch: "roll", amp: -0.025, n: 1 },
    { ch: "twist", amp: 0.04, n: 1 },
    { ch: "rRot", amp: [-0.04, 0, -0.03], n: 1 },
  ],
};

// --- death: struck from the front, thrown onto the back ------------------------------------------
//
// 0.00       hit: the upper body and arms snap back at full speed (decelerating to 0.14), the hips
//            following late; the head whips back, the arms fly out.
// 0.04-0.34  a stagger: the front foot leaves the ground and steps back behind the body (planted
//            0.34), the body still drifting back.
// 0.34-0.50  the knees give: the hips drop, the torso slumps forward, the head drops, the arms hang.
// 0.50-0.88  the fall: the whole body topples back about the heels, accelerating, the legs
//            straightening and the arms trailing (raised as it lands); ground at 0.88.
// 0.88-1.10  impact: a small rebound of the upper body, the arms and sword slap down, the head
//            knocks back; the legs flop straight.
// 1.10-1.35  it settles: the head, lolled back, rolls aside; still from 1.35 to the end.
// `settle` keeps the lowest point on the ground from the knees giving onwards (a no-op while a foot
// is planted), so every race's body lies on the ground whatever its girth.

const DEATH_BLADE = [
  { t: 0, dir: REST_DIR, i: "out" },
  { t: 0.14, dir: [-0.62, 0.62, -0.48], roll: -0.2, stop: true },
  { t: 0.34, dir: [-0.8, 0.2, -0.3], roll: -0.2 },
  { t: 0.5, dir: [-0.85, -0.22, 0.45], roll: 0, stop: true },
  { t: 0.88, dir: [-0.55, 0.7, 0.45], roll: 0, stop: true },
  { t: 1.0, dir: [-0.4, 0.91, -0.08], roll: 0, stop: true },
];

const death = {
  kind: "death",
  dur: 1.8,
  fps: 30,
  away: [0, 0, -1],
  fall: [-0.15, 0, -0.99],
  pivot: [0, 0, -0.065],
  events: { hit: 0, ground: 0.88, rest: 1.35 },
  contacts: { L: [[0, 0.04], [0.34, 0.52]], R: [[0, 0.52]] },
  keys: [
    { t: 0, pose: { lean: 0, twist: 0, roll: 0, headTilt: 0, headYaw: 0, rHand: [0, 0, 0], lHand: [0, 0, 0] }, i: "out" },
    { t: 0.04, pose: { fl: [0, 0, 0] } },
    { t: 0.14, pose: { pelvis: [0.004, -0.008, -0.04], lean: -0.34, twist: -0.28, roll: 0.06, headTilt: -0.3, headYaw: 0.15, rHand: [-0.063, 0.098, 0.004], lHand: [0.069, 0.029, -0.001] } },
    { t: 0.16, pose: { fl: [0, 0.022, -0.03] } },
    { t: 0.25, pose: { fl: [0.006, 0.012, -0.072] } },
    { t: 0.34, pose: { pelvis: [0.004, -0.03, -0.065], lean: -0.24, twist: -0.2, roll: 0.04, headTilt: -0.12, headYaw: 0.1, rHand: [-0.061, 0.015, -0.01], lHand: [0.07, 0.019, -0.015], lRot: [-0.35, 0, 0.3], fl: [0.008, 0, -0.085] } },
    { t: 0.44, pose: { settle: 0 } },
    { t: 0.5, pose: { pelvis: [0.004, -0.07, -0.06], lean: -0.06, twist: -0.15, roll: 0.02, headTilt: 0.25, headYaw: 0.05, rHand: [-0.051, -0.074, -0.013], lHand: [0.059, 0.034, -0.012], lRot: [-0.75, 0, 0.4], fl: [0.008, 0, -0.085], fr: [0, 0, 0], rootRot: [0, 0, 0], settle: 1 }, i: { rootRot: "in" } },
    { t: 0.88, pose: { pelvis: [0.004, -0.03, -0.06], lean: -0.12, twist: -0.08, roll: 0, headTilt: 0.15, headYaw: 0.05, rHand: [-0.074, 0.11, 0.03], lHand: [0.086, 0.03, 0.011], lRot: [-0.4, 0, 0.3], fl: [0.01, 0, -0.14], fr: [-0.01, 0, -0.04], rootRot: [-1.571, 0.18, 0] }, i: { rootRot: "out" } },
    { t: 0.97, pose: { rootRot: [-1.5, 0.18, 0] }, i: { rootRot: "in" } },
    { t: 1.0, pose: { pelvis: [0.004, 0.005, -0.06], lean: -0.14, twist: -0.06, headTilt: -0.5, rHand: [-0.103, 0.095, -0.108], lHand: [0.107, -0.015, -0.11], lRot: [0, 0, 0], fl: [0.012, 0, -0.155], fr: [-0.012, 0, -0.05] } },
    { t: 1.08, pose: { rootRot: [-1.571, 0.18, 0] } },
    { t: 1.12, pose: { headTilt: -0.42 } },
    { t: 1.35, pose: { headYaw: 0.45, headTilt: -0.4 } },
    ...blade(DEATH_BLADE, 1 / 30),
  ],
};

clips("unit.soldier", { idle, attack, death });
