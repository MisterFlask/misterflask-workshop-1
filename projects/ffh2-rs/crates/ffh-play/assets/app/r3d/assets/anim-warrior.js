// anim-warrior.js: the warrior's animation clips (docs/ANIMATION.md; the format is anim.js's): keyed pose
// deltas from its rest pose (units.js warriorSpec), so one clip serves every race and variant (positions
// in a human's plots; the figure scales them to its race). Times in seconds; the figure faces +z,
// its right hand on -x.

import { clips, turn } from "../anim.js";

// --- authoring helpers -------------------------------------------------------------------------
// The axe turns 100-200 degrees between the key poses of a chop. Keyed as Euler angles and
// interpolated per component, such a turn wobbles off its arc (and a key equal to the previous one
// modulo 2 pi spins the blade round), so the weapon arm is keyed by armPath: its poses are sampled
// every 1/30 s along designed paths (the blade's turn along the shortest arc, the hand on a smooth
// curve through its via points) with one timing for hand, elbow and blade.

const nz = (v) => {
  const l = Math.hypot(...v) || 1;
  return v.map((x) => x / l);
};
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const dot = (a, b) => a.reduce((s, x, i) => s + x * b[i], 0);
const DEG = Math.PI / 180;

/** The unit direction at azimuth `az` (degrees from +z toward +x) and elevation `el`. */
const dirOf = (az, el) => [Math.sin(az * DEG) * Math.cos(el * DEG), Math.sin(el * DEG), Math.cos(az * DEG) * Math.cos(el * DEG)];

/** A direction perpendicular to `d`: psi 0 is up (as near +y as it can be), turning about `d`
 *  (for a forward haft: 90 its left (+x), 180 down, 270 its right). */
function edgeOf(d, psi) {
  const e = nz([0, 1, 0].map((c, i) => c - d[i] * d[1]));
  const s = cross(d, e);
  return e.map((c, i) => c * Math.cos(psi * DEG) + s[i] * Math.sin(psi * DEG));
}

/** Rotation matrix (rows) of Euler [x, y, z] as units.js eulerTurn applies it (Rz * Ry * Rx). */
function eulerM([ax, ay, az]) {
  const cx = Math.cos(ax), sx = Math.sin(ax), cy = Math.cos(ay), sy = Math.sin(ay), cz = Math.cos(az), sz = Math.sin(az);
  return [
    [cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx],
    [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx],
    [-sy, cy * sx, cy * cx],
  ];
}
const mEuler = (R) => [Math.atan2(R[2][1], R[2][2]), -Math.asin(Math.max(-1, Math.min(1, R[2][0]))), Math.atan2(R[1][0], R[0][0])];

function mQuat(R) {
  const tr = R[0][0] + R[1][1] + R[2][2];
  if (tr > 0) {
    const s = Math.sqrt(tr + 1) * 2;
    return [s / 4, (R[2][1] - R[1][2]) / s, (R[0][2] - R[2][0]) / s, (R[1][0] - R[0][1]) / s];
  }
  if (R[0][0] > R[1][1] && R[0][0] > R[2][2]) {
    const s = Math.sqrt(1 + R[0][0] - R[1][1] - R[2][2]) * 2;
    return [(R[2][1] - R[1][2]) / s, s / 4, (R[0][1] + R[1][0]) / s, (R[0][2] + R[2][0]) / s];
  }
  if (R[1][1] > R[2][2]) {
    const s = Math.sqrt(1 + R[1][1] - R[0][0] - R[2][2]) * 2;
    return [(R[0][2] - R[2][0]) / s, (R[0][1] + R[1][0]) / s, s / 4, (R[1][2] + R[2][1]) / s];
  }
  const s = Math.sqrt(1 + R[2][2] - R[0][0] - R[1][1]) * 2;
  return [(R[1][0] - R[0][1]) / s, (R[0][2] + R[2][0]) / s, (R[1][2] + R[2][1]) / s, s / 4];
}
const qM = ([w, x, y, z]) => [
  [1 - 2 * (y * y + z * z), 2 * (x * y - w * z), 2 * (x * z + w * y)],
  [2 * (x * y + w * z), 1 - 2 * (x * x + z * z), 2 * (y * z - w * x)],
  [2 * (x * z - w * y), 2 * (y * z + w * x), 1 - 2 * (x * x + y * y)],
];
function slerp(a, b, u) {
  let d = dot(a, b);
  if (d < 0) {
    b = b.map((x) => -x);
    d = -d;
  }
  if (d > 0.9995) return nz(a.map((x, i) => x + (b[i] - x) * u));
  const th = Math.acos(d), s = Math.sin(th);
  return a.map((x, i) => (Math.sin((1 - u) * th) * x + Math.sin(u * th) * b[i]) / s);
}

/** The Euler turn that points a held weapon (rest direction `rest`, rest face +z) along `dir`
 *  with its blade's edge toward `edge` (made perpendicular to the haft): turn() with a roll. */
function aimEdge(rest, dir, edge) {
  const frame = (y, x) => {
    const Y = nz(y), X = nz(x.map((c, i) => c - Y[i] * dot(x, Y)));
    return [X, Y, cross(X, Y)];
  };
  const Y0 = nz(rest), Z0 = nz([0, 0, 1].map((c, i) => c - Y0[i] * Y0[2]));
  const F0 = [cross(Y0, Z0), Y0, Z0], F1 = frame(dir, edge);
  return mEuler([0, 1, 2].map((r) => [0, 1, 2].map((c) => F1[0][r] * F0[0][c] + F1[1][r] * F0[1][c] + F1[2][r] * F0[2][c])));
}

/** The Euler triple equal to turn `e` nearest `prev` (so a linear step between them is short). */
function near(prev, e) {
  const T = 2 * Math.PI;
  let best = null, bd = Infinity;
  for (const c of [e, [e[0] + Math.PI, Math.PI - e[1], e[2] + Math.PI]]) {
    const u = c.map((x, i) => x + T * Math.round((prev[i] - x) / T));
    const d = u.reduce((s, x, i) => s + Math.abs(x - prev[i]), 0);
    if (d < bd) {
      bd = d;
      best = u;
    }
  }
  return best;
}

const EASE = {
  lin: (u) => u,
  in: (u) => u * u, // accelerating into the end (a blow)
  out: (u) => u * (2 - u), // leaving at speed, slowing to a stop
  io: (u) => u * u * (3 - 2 * u), // slow in, slow out
};

/** Catmull-Rom point at s through points P at increasing parameters S (S[0] = 0, last = 1). */
function spline(P, S, s) {
  let k = 0;
  while (k + 2 < S.length && s > S[k + 1]) k++;
  const h = S[k + 1] - S[k], u = (s - S[k]) / h;
  const tan = (j) => {
    const a = P[Math.max(0, j - 1)], b = P[Math.min(P.length - 1, j + 1)];
    const ds = S[Math.min(P.length - 1, j + 1)] - S[Math.max(0, j - 1)];
    return a.map((x, i) => ((b[i] - x) / ds) * h);
  };
  const m0 = tan(k), m1 = tan(k + 1);
  const h00 = 2 * u ** 3 - 3 * u * u + 1, h10 = u ** 3 - 2 * u * u + u, h01 = -2 * u ** 3 + 3 * u * u, h11 = u ** 3 - u * u;
  return P[k].map((x, i) => h00 * x + h10 * m0[i] + h01 * P[k + 1][i] + h11 * m1[i]);
}

/**
 * The weapon arm's keys from poses [{t, hand, elbow, rot, ease, via}] (the first is the start,
 * usually the rest pose at t = 0): `hand`/`elbow` the rHand/rElbow deltas, `rot` the weapon's
 * Euler turn, `ease` the timing of the segment ending at this pose (lin, in, out, io), `via`
 * [{hand, rot, s}] poses the segment passes (at progress s, by default its share of the turn).
 * Returns keys [{t, i, pose: {rHand, rElbow, rRot}}] every `dt` seconds, linear between them (a
 * segment turning less than 25 degrees without vias is one smooth key instead).
 */
function armPath(poses, dt = 1 / 30) {
  const out = [];
  let prevE = [0, 0, 0];
  const emit = (t, hand, elbow, q) => {
    prevE = near(prevE, mEuler(qM(q)));
    out.push({ t: Math.round(t * 1e6) / 1e6, i: { rHand: "linear", rElbow: "linear", rRot: "linear" }, pose: { rHand: hand, rElbow: elbow, rRot: prevE } });
  };
  const Q = (e) => mQuat(eulerM(e));
  emit(poses[0].t, poses[0].hand, poses[0].elbow, Q(poses[0].rot));
  for (let k = 1; k < poses.length; k++) {
    const a = poses[k - 1], b = poses[k];
    const via = b.via ?? [];
    // A via without a turn of its own (it then needs its s) turns on the way from a to b.
    const R = [Q(a.rot), ...via.map((v) => (v.rot ? Q(v.rot) : slerp(Q(a.rot), Q(b.rot), v.s))), Q(b.rot)];
    for (let j = 1; j < R.length; j++) if (dot(R[j - 1], R[j]) < 0) R[j] = R[j].map((x) => -x);
    const ang = R.slice(1).map((q, j) => Math.acos(Math.min(1, Math.abs(dot(q, R[j])))));
    const tot = ang.reduce((x, y) => x + y, 0) || 1;
    let acc = 0;
    const S = [0, ...via.map((v, j) => v.s ?? (acc += ang[j]) / tot), 1];
    const H = [a.hand, ...via.map((v) => v.hand ?? null), b.hand];
    for (let j = 1; j + 1 < H.length; j++) if (!H[j]) H[j] = a.hand.map((x, i) => x + (b.hand[i] - x) * S[j]);
    // A small turn (a moving hold) needs no samples: one key, the engine's smooth curve into it.
    if (!via.length && 2 * ang[0] < 25 * DEG) {
      delete out[out.length - 1].i;
      emit(b.t, b.hand, b.elbow, R[1]);
      continue;
    }
    const n = Math.max(1, Math.round((b.t - a.t) / dt));
    const ease = EASE[b.ease ?? "io"];
    for (let j = 1; j <= n; j++) {
      const s = ease(j / n);
      let r = 0;
      while (r + 2 < S.length && s > S[r + 1]) r++;
      const q = slerp(R[r], R[r + 1], (s - S[r]) / (S[r + 1] - S[r]));
      emit(a.t + ((b.t - a.t) * j) / n, spline(H, S, s), a.elbow.map((x, i) => x + (b.elbow[i] - x) * s), q);
    }
  }
  return out;
}

/** rootRot keys of a fall from standing at t0 to turn `to` at t1: the angle follows
 *  p(u) = 1.5u^2 - 0.5u^3 (starting still, its acceleration easing off smoothly, still at full
 *  speed when it lands), sampled every dt and linear between. */
function fallKeys(t0, t1, to, dt = 1 / 30) {
  const n = Math.round((t1 - t0) / dt), keys = [];
  for (let j = 0; j <= n; j++) {
    const u = j / n, p = 1.5 * u * u - 0.5 * u * u * u;
    keys.push({ t: Math.round((t0 + u * (t1 - t0)) * 1e6) / 1e6, i: { rootRot: "linear" }, pose: { rootRot: to.map((x) => x * p) } });
  }
  return keys;
}

/** Keys at the same time merged into one (poses and per-channel interpolations combined), so a
 *  generated path and the hand-set body keys can share key times. */
function merged(keys) {
  const out = [];
  for (const k of keys.slice().sort((a, b) => a.t - b.t)) {
    const i = typeof k.i === "string" ? Object.fromEntries(Object.keys(k.pose).map((c) => [c, k.i])) : { ...(k.i ?? {}) };
    const last = out[out.length - 1];
    if (last && Math.abs(last.t - k.t) < 1e-6) {
      Object.assign(last.pose, k.pose);
      Object.assign(last.i, i);
    } else out.push({ t: k.t, i, pose: { ...k.pose } });
  }
  for (const k of out) if (!Object.keys(k.i).length) delete k.i;
  return out;
}

// --- unit.warrior: the axe (club, hammer) raised, the round shield forward, braced left foot first

/** The warrior's weapon direction at rest (units.js warriorSpec right.dir). */
const AXE = [-0.55, 0.8, 0.12];
/** The axe aimed at azimuth/elevation (degrees) with its edge at psi about the haft (edgeOf). */
const axe = (az, el, psi) => {
  const d = dirOf(az, el);
  return aimEdge(AXE, d, edgeOf(d, psi));
};
/** The axe aimed at azimuth/elevation with its edge toward a given direction. */
const axeEdge = (az, el, edge) => aimEdge(AXE, dirOf(az, el), edge);

// The chop. Wind-up: the axe drawn back over the right shoulder, its head dropped behind the back,
// edge up; the body coils back and rises onto the rear foot. The blow swings up over the top (via
// the axe upright) and down across the body, accelerating all the way, onto a target in front at
// chest height, the blade's edge leading down and to the left; the hips and chest unwind first and
// the shield arm swings back against the blow. Follow-through: carried on down to just above the
// ground and stopped there. Recovery: the axe brought back up across the body to the guard.
const chopArm = armPath([
  { t: 0, hand: [0, 0, 0], elbow: [0, 0, 0], rot: [0, 0, 0] },
  { t: 0.26, ease: "io", hand: [-0.022, 0.046, -0.055], elbow: [0, 0.2, 0], rot: axe(-150, -5, 65) },
  { t: 0.36, ease: "io", hand: [-0.025, 0.05, -0.06], elbow: [0, 0.22, 0], rot: axe(-155, -10, 65) },
  { t: 0.6, ease: "in", hand: [0.05, -0.135, 0.14], elbow: [0, -0.2, 0], rot: axe(-20, 15, 250), via: [{ hand: [0.015, 0.055, 0.085], rot: axeEdge(-170, 40, [0.42, 0.72, 0.55]) }] },
  { t: 0.665, ease: "out", hand: [0.03, -0.17, 0.12], elbow: [0, -0.2, 0], rot: axe(-25, -15, 245) },
  { t: 0.74, ease: "io", hand: [0.028, -0.173, 0.118], elbow: [0, -0.2, 0], rot: axe(-25, -17, 245) },
  { t: 1.02, ease: "in", hand: [0.0, -0.075, 0.09], elbow: [0, -0.05, 0], rot: axeEdge(-40, 35, [0.6, 0.6, 0.3]) },
  { t: 1.2, ease: "out", hand: [0, 0, 0], elbow: [0, 0, 0], rot: [0, 0, 0], via: [{ s: 0.4, hand: [-0.035, -0.025, 0.055] }] },
]);

// The death: thrown back by the blow (the chest and head first, the arms flung up a beat later),
// the knees give, and the body topples backward over the rear heel, turning as it goes, to land
// on its back with its head to its right (so the corpse lies across its facing, `fall`); a small
// rebound as it lands, the arms flop to the ground a beat after the body and the left leg drops
// flat; then it lies still.
const LIE = [-1.57, 1.57, 0];
const topple = fallKeys(0.3, 0.733, LIE);
topple[topple.length - 1].i = { rootRot: "out" };

clips("unit.warrior", {
  idle: {
    kind: "idle",
    dur: 3.2,
    loop: true,
    fps: 15,
    contacts: { L: [[0, 3.2]], R: [[0, 3.2]] },
    // The weight held a little toward the front foot's side and back: the narrow-footed races
    // (undead) keep their balance margin through the breath.
    keys: [{ t: 0, pose: { pelvis: [0.0025, 0, -0.002] } }],
    waves: [
      // One breath a loop (3.2 s): the chest rises, the pelvis settles.
      { ch: "lean", amp: 0.028, n: 1, phase: 0 },
      { ch: "pelvis", amp: [0, -0.004, 0], n: 1, phase: 0.25 },
      // The weight drifts between the feet, out of step with the breath.
      { ch: "pelvis", amp: [0.002, 0, 0.002], n: 1, phase: 0.6 },
      // The weapon arm settles after the breath; a smaller twice-a-loop bob keeps it from moving
      // as one with the chest.
      { ch: "rHand", amp: [0, 0.007, 0.0035], n: 1, phase: 0.12 },
      { ch: "rHand", amp: [0.002, 0.003, 0], n: 2, phase: 0.3 },
      { ch: "rRot", amp: [0.035, 0, 0.018], n: 1, phase: 0.18 },
      // The shield hand sways across its arm, not along it (a goblin's shield arm is at full reach).
      { ch: "lHand", amp: [0, 0.0045, 0.004], n: 1, phase: 0.45 },
      // The head looks about, slowly, and nods a little with the breath.
      { ch: "headYaw", amp: 0.13, n: 1, phase: 0.7 },
      { ch: "headTilt", amp: 0.035, n: 1, phase: 0.05 },
      { ch: "twist", amp: 0.025, n: 1, phase: 0.75 },
    ],
  },

  attack: {
    kind: "attack",
    dur: 1.2,
    fps: 60,
    returns: true,
    strike: [0, 0, 1],
    events: { impact: 0.6 },
    contacts: { L: [[0, 1.2]], R: [[0, 1.2]] },
    keys: merged([
      ...chopArm,
      // Anticipation: coil back and up, leaning away from the blow, the shield raised in guard.
      { t: 0.26, pose: { pelvis: [0.003, 0.002, -0.016], lean: -0.18, twist: -0.3, headYaw: 0.18, headTilt: 0.04, lHand: [-0.012, 0.045, -0.004] } },
      // The moving hold at the top of the wind-up.
      { t: 0.36, pose: { pelvis: [0.004, 0.003, -0.019], lean: -0.21, twist: -0.33, headYaw: 0.2, headTilt: 0.05, lHand: [-0.014, 0.05, -0.005] } },
      // The hips and chest drive ahead of the arm, dropping onto the front foot.
      { t: 0.565, pose: { pelvis: [0.009, -0.022, 0.012], lean: 0.2, twist: 0.16, headYaw: -0.05, headTilt: 0.09, lHand: [0.045, -0.012, -0.045] } },
      // Impact: low over the front foot, the shield swung back and out against the blow.
      { t: 0.6, pose: { pelvis: [0.012, -0.03, 0.016], lean: 0.28, twist: 0.3, headYaw: -0.12, headTilt: 0.1, lHand: [0.065, -0.01, -0.06] } },
      { t: 0.72, pose: { pelvis: [0.012, -0.033, 0.016], lean: 0.29, twist: 0.33, headYaw: -0.14, headTilt: 0.11, lHand: [0.066, -0.014, -0.062] } },
      // Recovery toward the guard (the clip returns to rest at the end).
      { t: 0.95, pose: { pelvis: [0.005, -0.013, 0.006], lean: 0.1, twist: 0.1, headYaw: -0.05, headTilt: 0.04, lHand: [0.008, 0, -0.012] } },
    ]),
  },

  death: {
    kind: "death",
    dur: 1.6,
    away: [0, 0, -1],
    fall: [-1, 0, 0],
    // The back of the rear (right) heel: the fall rotates over it; lying, the back rests on the ground.
    pivot: [-0.048, 0, -0.08],
    events: { hit: 0.0, ground: 0.733, rest: 1.0 },
    contacts: { L: [[0, 0.3]], R: [[0, 0.3]] },
    keys: merged([
      // The blow: the chest thrown back and the head snapped back first...
      { t: 0.14, pose: { pelvis: [0, 0.002, -0.01], lean: -0.17, twist: -0.09, headTilt: -0.27 } },
      // ...the arms flung up and out a beat later.
      { t: 0.2, pose: { rHand: [-0.035, 0.03, -0.03], rRot: turn(AXE, [-0.62, 0.74, -0.2]), lHand: [0.035, 0.035, -0.025] } },
      // The knees give, sagging back onto the rear heel; the feet still planted.
      { t: 0.3, i: { pelvis: "in" }, pose: { pelvis: [0, -0.012, -0.018], lean: -0.17, twist: -0.14, headTilt: -0.18, fl: [0, 0, 0] } },
      { t: 0.36, pose: { rHand: [-0.045, 0.012, -0.022], lHand: [0.042, 0.012, -0.032] } },
      // The topple, gathering speed until the ground stops it.
      ...topple,
      { t: 0.733, pose: { pelvis: [0, -0.05, 0], lean: -0.2, twist: -0.12, headTilt: -0.24, rHand: [-0.06, 0.03, 0], lHand: [0.06, 0.03, -0.06] } },
      // A small rebound; the arms land a beat later and the left leg drops flat.
      { t: 0.81, i: { rootRot: "in" }, pose: { rootRot: [-1.545, 1.57, 0], twist: -0.1, headTilt: -0.3, rHand: [-0.085, 0.015, -0.02], rElbow: [0, 0, 1], rRot: turn(AXE, [-0.6, 0.8, 0.03]), lHand: [0.08, 0.04, -0.12], lElbow: [0, 0, 1], fl: [0, 0, -0.1] } },
      { t: 0.93, pose: { rootRot: LIE } },
    ]),
  },
});
