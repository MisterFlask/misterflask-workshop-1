// check.mjs: the animation validator (docs/ANIMATION.md "Judging an animation"). It measures a
// baked clip (anim.js bakeClip: the exact per-frame vertices, joints and held points) against
// limits a viewer would notice, in three families:
//
//   body     -- the physics a viewer's eye expects: nothing sinks into the ground, planted boots
//               do not slide, props and limbs stay rigid (also between frames, as the GPU lerps
//               them), joints stay in range, the weapon does not pass through the head or chest,
//               the centre of mass stays over the feet, a death falls like a weight and lies still;
//   motion   -- timing and continuity: no pops, a seamless idle loop, clips that start and end
//               where the idle can hand over, an attack with a held wind-up, a fast blow that
//               lands at speed in front of the body, a shot with an aim hold and a crisp release;
//   reading  -- what the player sees at the gameplay zoom (96 px a plot, the figure ~31 px):
//               an idle that moves enough to be alive and little enough to be calm, key poses that
//               differ in silhouette, a weapon that shows outside the body, a blow that travels a
//               visible distance at every facing the game uses, a corpse that reads as lying.
//
// Every limit is a number with its reason next to it (LIMITS); each metric has a mutation in
// mutate.mjs that must trip it while the good clip passes (anim.test.mjs checks both). The
// vision judges (tools/r3d-anim.mjs --judge) cover what numbers cannot.

import { camera, renderFrame } from "./raster.mjs";
import { attackYaw, deathYaw, flight, flightTime, releasePoint, bakeClip, clipOf, BLEND } from "../../assets/app/r3d/anim.js";
import { buildAsset } from "../../assets/app/r3d/assets/registry.js";

/** The zooms the checks look through (pixels per plot): the gameplay zoom and the closest. */
export const G = 96;
export const C = 220;
/** Gravity in plots/s^2: a human 0.48 plots tall is ~1.75 m, so a plot is ~3.65 m. */
export const GRAV = 9.81 / 3.65;
const BONE = { BODY: 0, HEAD: 1, ARM_L: 2, ARM_R: 3, LEG_L: 4, LEG_R: 5, CAPE: 6, WEAPON: 7 };
/** World headings of the eight neighbouring plots a fight comes from or goes to (S is toward the
 *  camera, E to the right). */
export const HEADINGS = { E: Math.PI / 2, SE: Math.PI / 4, S: 0, SW: -Math.PI / 4, W: -Math.PI / 2, NW: (-3 * Math.PI) / 4, N: Math.PI, NE: (3 * Math.PI) / 4 };

export const LIMITS = {
  // Ground: a part 0.006 under the ground (1.2 px close, 0.6 px in play) visibly disappears into
  // it; a planted boot 0.004 above its rest sole shows a gap.
  pen: 0.006,
  hover: 0.004,
  // A boot sliding 0.004 over a contact (0.8 px close) is the least the eye catches as skating.
  slipFrame: 0.0015,
  slipRun: 0.004,
  // Rigid props: Kabsch residual / part radius (baked frames, and the half-frame the GPU lerps,
  // which shrinks a rod by cos(dtheta/2): 3% is 28 degrees a frame).
  rigid: 0.01,
  bone: 0.01, // limb segment length change (IK is exact unless broken)
  // A limb at more than 98.5% of its reach is at the IK's clamp: the hand stops following its
  // target, then lurches when the target comes back (an elbow or knee snapping straight). Under
  // 30% it is folded into itself (the IK clamps at 25%) and its joint swings wildly.
  reach: 0.985,
  reachMin: 0.3,
  // The GPU lerps between baked frames: the in-between may be at most 1.5 px off the true path at
  // the gameplay zoom (a fast swing's chord cuts the arc and shrinks the weapon).
  lerpPx: 1.5,
  // Joint ranges (degrees): anatomical maxima, wider than the rest poses.
  knee: 150,
  kneeDir: 0.26, // a bent knee points within 75 degrees of the body's forward
  elbow: 155,
  neckYaw: 80,
  lean: [-35, 85],
  twist: 90, // one twist serves hips and chest: a swing's wind-up needs 80-90 degrees of turn
  // The haft against the forearm: a fist holds it at its rest grip (66-80 degrees in these
  // figures) and the wrist deviates ~30-40 degrees toward the arm (ulnar): the floor is the rest
  // grip less 45, at most 45; along the arm is broken.
  wrist: [45, 140],
  wristDev: 45,
  // Clearance: a weapon or bow at most 0.008 deeper into the head or chest than at rest; up to
  // 0.015 for at most 34 ms in all (a motion-blurred pass in a fast swing), never more.
  clear: 0.008,
  clearOnce: 0.015,
  clearTime: 0.034,
  // Continuity. The trunk (pelvis, chest, head, legs) moves smoothly: its cubic residual (zero for
  // any cubic motion) stays under 0.003 plots (0.6 px close). Anything may move fast, but a
  // single-frame jump of 3 px at the closest zoom, 4x its neighbours' steps, is a pop, and two
  // steps over 2 px that reverse by more than 120 degrees are a jitter.
  popTrunk: 0.003,
  spike: [3 / 220, 4],
  jitter2: [2 / 220, 120],
  // Clip boundaries: an attack or death starts (and an attack ends) on the rest pose. Scored:
  // started from any idle frame, the runtime's cross-fade moves no vertex more than 1.5 px
  // (gameplay zoom) in a 60 Hz frame beyond the same cut from the rest pose. (An idle within
  // idle.calm's 4 px adds at most ~0.7 px: the hard gates already bound the cut.)
  boundary: 0.002,
  transition: 1.5,
  // Balance: centre-of-mass margin inside the feet (idle static; attack the extrapolated XCoM).
  balanceIdle: 0.008,
  balanceAttack: -0.01,
  // Idle in screen pixels at the gameplay zoom: alive (P90 of head, arms, weapon peak-to-peak),
  // calm (body median and everything's maximum), slow (per-frame step), and close enough to the
  // rest pose that an attack can start from any idle frame.
  idleAlive: 1.0, // head and arms (a bobbing prop is not a living body)
  idleTrunk: 0.5, // and the trunk itself
  idleBody: 2.0, // a 1.6 px weight shift read to three blind viewers as too still, not busy
  idleMax: 4.0,
  idleStep: 4.5, // px a second (0.15 px a frame at 30 fps): a calm body and head
  idleFromRest: 2.0,
  idleLoop: [2.5, 8],
  // The seam (the last frame into the first) moves and accelerates like the rest of the loop: a
  // step at most 1.5x and an acceleration at most 2x the loop's largest, unless both are under
  // 0.15 px at the closest zoom (a once-a-loop hitch is easy to see, being periodic).
  seam: [1.5, 2, 0.15 / 220],
  breath: [2.0, 6.5],
  jitter: 0.0005, // amplitude of motion faster than 1.5 Hz (a tremor)
  // Attack phrasing (seconds): a wind-up of >= 200 ms with a hold of >= 67 ms under 15% of the
  // peak speed (about 70-100 ms is the least a pose needs to register), the blow in 67-233 ms
  // (longer reads as a push), the wind-up slower than 40% of the blow, the peak speed at or
  // just before the impact, half the speed gone 67 ms after it, >= 270 ms of recovery.
  windupMin: 0.2,
  holdSpeed: 0.15,
  holdTime: 0.067,
  strike: [0.067, 0.234],
  contrast: 0.4,
  peakAtImpact: 0.067,
  decel: 0.5,
  recover: 0.27,
  duration: [0.6, 1.8],
  power: [1.5, 7], // plots/s at impact (5.5-25 m/s)
  reachFront: 0.1, // the blow lands this far in front of the pelvis
  aim: 45, // degrees between the blow and the target direction
  facing: 60, // the chest within 60 degrees of the target at the impact (not a sideways swing)
  // Reading at the facings fights come from. From behind (the back arc: the figure turned more
  // than 90 degrees from the camera) the body hides a blow landing in front of it, so the blow's
  // travel and pose contrast are asked less, and the weapon must show at the wind-up only.
  travel: [1.0, 3.2], // screen travel of the blow, in rest figure heights (back arc: 0.75)
  travelBack: 0.75,
  iouKeys: 0.6,
  iouKeysBack: 0.7,
  iouRest: 0.7,
  weaponOut: [20, 0.4], // px^2 and share of the weapon outside the body
  // Archer: string at the hand (0.015 = 1.5 px), the tip ahead of the grip, an aim hold of 100 ms
  // turning < 3 degrees, the loosed arrow gone within a frame, the string home within 67 ms, the
  // arrow in flight along the bow within 15 degrees, 130-400 ms of flight (time to follow it).
  stringHand: 0.015,
  tipAhead: 0.005,
  aimHold: 0.1,
  aimTurn: 3,
  aimDir: 6, // the nocked arrow vs the shot direction at the release (degrees)
  bowArm: 25, // the bow arm along the arrow (degrees; natural form is 10-16: the arrow runs from the face)
  anchor: 0.02, // the drawing hand within a head radius + 0.02 of the head centre
  recoil: 0.02, // the drawing hand flies back at least this far after the release (plots)
  snap: 0.067,
  shotAxis: 15,
  flight: [0.13, 0.4],
  arrowPx: 10,
  // Death: a reaction within 133 ms, a fall that accelerates into the ground at 0.4-2.5 g (energy
  // ratio 0.25-1.6 of a free fall), lands 150-700 ms after onset, at most a small rebound, still
  // within 0.5 s, lies where the clip says it falls, and reads as lying at every facing.
  react: [0.134, 0.01],
  gEff: [0.4, 2.5],
  energy: [0.25, 1.6],
  fallTime: [0.15, 0.7],
  fallDir: 35,
  groundEvent: 0.067, // the declared `ground` event within 67 ms of the measured contact
  bounce: 0.008,
  landing: 1.3, // a part's landing speed / free fall from its highest point (a topple's tip: 1.22)
  still: 0.0005,
  settle: 0.5,
  lieCom: 0.45, // lying on the side with gear keeps the centre of mass at ~0.36 of standing
  lieHead: 0.3, // the head's centre at most 0.3 of the standing height
  lieAngle: 40,
  lieHeight: 0.45,
};

// --- vectors ---------------------------------------------------------------------------------------

const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const mul = (a, k) => [a[0] * k, a[1] * k, a[2] * k];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const len = (a) => Math.hypot(a[0], a[1], a[2]);
const nrm = (a) => mul(a, 1 / (len(a) || 1));
const lerp3 = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
const angle = (a, b) => (Math.acos(Math.max(-1, Math.min(1, dot(nrm(a), nrm(b))))) * 180) / Math.PI;
const hxz = (a) => Math.hypot(a[0], a[2]);

function eulerTurn(v, [ax, ay, az]) {
  let [x, y, z] = v;
  let c = Math.cos(ax), s = Math.sin(ax);
  [y, z] = [c * y - s * z, s * y + c * z];
  c = Math.cos(ay);
  s = Math.sin(ay);
  [x, z] = [c * x + s * z, -s * x + c * z];
  c = Math.cos(az);
  s = Math.sin(az);
  [x, y] = [c * x - s * y, s * x + c * y];
  return [x, y, z];
}

// --- the clip's derived data ------------------------------------------------------------------------

/** Precomputes what the metrics share: vertex sets, the sole depth, contacts, the centre of mass,
 *  the striking point, the clip's events as frames. */
export function model(b) {
  const F = b.frames, n = b.count;
  const X = (f, v) => {
    const q = (f * n + v) * 3;
    return [b.pos[q], b.pos[q + 1], b.pos[q + 2]];
  };
  const R0 = (v) => [b.rest.data[v * 13], b.rest.data[v * 13 + 1], b.rest.data[v * 13 + 2]];
  const range = (p) => (p ? Array.from({ length: p.range[1] - p.range[0] }, (_, i) => p.range[0] + i) : []);
  const feet = { L: range(b.parts.foot_l), R: range(b.parts.foot_r) };
  const footSet = new Set([...feet.L, ...feet.R]);
  const d0 = Math.min(...[...footSet].map((v) => R0(v)[1]));
  const weaponKey = b.parts.weapon_r ? "weapon_r" : b.parts.weapon_l ? "weapon_l" : null;
  const weapon = range(b.parts[weaponKey]);
  const isBow = b.parts[weaponKey]?.kind === "bow";
  const arrow = new Set(range(b.parts.arrow));
  const string = new Set(range(b.parts.string));
  const bowLimbs = isBow ? weapon.filter((v) => !arrow.has(v) && !string.has(v)) : [];
  const shield = range(b.parts.shield_l ?? b.parts.shield_r);
  const byBone = new Map();
  for (let v = 0; v < n; v++) {
    if (!byBone.has(b.bone[v])) byBone.set(b.bone[v], []);
    byBone.get(b.bone[v]).push(v);
  }
  // The striking point: the weapon's head, the vertices in its far fifth along the haft (rest).
  let head = [];
  if (weaponKey && !isBow) {
    const h0 = b.held[0][weaponKey];
    const ax = nrm(sub(h0.up, h0.at));
    const proj = weapon.map((v) => dot(sub(X(0, v), h0.at), ax));
    const top = Math.max(...proj);
    head = weapon.filter((_, i) => proj[i] >= top * 0.8);
  }
  const centroid = (f, vs) => {
    const c = [0, 0, 0];
    for (const v of vs) {
      const p = X(f, v);
      c[0] += p[0];
      c[1] += p[1];
      c[2] += p[2];
    }
    return mul(c, 1 / Math.max(1, vs.length));
  };
  const strikeAt = (f) => (head.length ? centroid(f, head) : b.held[f][weaponKey]?.tip ?? b.joints[f].handR);
  // Centre of mass: Winter's segment model on the joints, the feet and the props.
  const com = (f) => {
    const J = b.joints[f];
    const midSh = lerp3(J.shoulderL, J.shoulderR, 0.5);
    const segs = [
      [0.081, J.head],
      [0.497, lerp3(J.pelvis, midSh, 0.5)],
      [0.028, lerp3(J.shoulderL, J.elbowL, 0.436)],
      [0.028, lerp3(J.shoulderR, J.elbowR, 0.436)],
      [0.016, lerp3(J.elbowL, J.handL, 0.43)],
      [0.016, lerp3(J.elbowR, J.handR, 0.43)],
      [0.006, J.handL],
      [0.006, J.handR],
      [0.1, lerp3(J.hipL, J.kneeL, 0.433)],
      [0.1, lerp3(J.hipR, J.kneeR, 0.433)],
      [0.0465, lerp3(J.kneeL, J.ankleL, 0.433)],
      [0.0465, lerp3(J.kneeR, J.ankleR, 0.433)],
      [0.0145, centroid(f, feet.L)],
      [0.0145, centroid(f, feet.R)],
    ];
    if (weapon.length) segs.push([isBow ? 0.015 : 0.03, centroid(f, weapon)]);
    if (shield.length) segs.push([0.04, centroid(f, shield)]);
    let m = 0;
    const c = [0, 0, 0];
    for (const [w, p] of segs) {
      m += w;
      c[0] += w * p[0];
      c[1] += w * p[1];
      c[2] += w * p[2];
    }
    return mul(c, 1 / m);
  };
  const fr = (t) => Math.max(0, Math.min(F - 1, Math.round(t * b.fps)));
  const sec = (t) => Math.round(t * b.fps); // a duration in frames
  const planted = { L: new Array(F).fill(false), R: new Array(F).fill(false) };
  for (const s of ["L", "R"]) for (const [t0, t1] of b.clip.contacts?.[s] ?? []) for (let f = fr(t0); f <= fr(t1); f++) planted[s][f] = true;
  const coms = Array.from({ length: F }, (_, f) => com(f));
  const strikes = Array.from({ length: F }, (_, f) => strikeAt(f));
  // Ground arrivals of the trunk (body, head, legs; every second vertex): a vertex that comes
  // down at least 0.008 within four frames, the last two of them falling, and stops near the
  // ground (within 0.02 of it, still for the next two frames). A landing stops what lands in a frame: a contact, not a pop.
  const arrivals = new Map(); // vertex -> [{f, ground, bone}]
  const near = Math.max(d0, 0) + 0.02;
  for (const bone of [BONE.BODY, BONE.HEAD, BONE.LEG_L, BONE.LEG_R]) {
    for (const v of (byBone.get(bone) ?? []).filter((_, k) => k % 2 === 0)) {
      const y = (f) => b.pos[(f * n + v) * 3 + 1];
      const list = [];
      for (let f = 1; f + 2 < F; f++) {
        if (y(f) > near || Math.abs(y(f + 1) - y(f)) > 0.003 || Math.abs(y(f + 2) - y(f)) > 0.005) continue;
        let hi = -Infinity;
        for (let k = Math.max(0, f - 4); k < f; k++) hi = Math.max(hi, y(k));
        // Coming down for two frames (a fall), not a one-frame pop up and back.
        if (hi < y(f) + 0.008 || f < 2 || y(f - 1) <= y(f) + 0.001 || y(f - 2) <= y(f - 1) + 0.001) continue;
        if (list.length && f - list[list.length - 1].f <= 4) continue;
        list.push({ f, ground: y(f), bone });
      }
      if (list.length) arrivals.set(v, list);
    }
  }
  return { b, F, n, X, R0, feet, footSet, d0, weaponKey, weapon, isBow, arrow, string, bowLimbs, shield, byBone, head, centroid, coms, strikes, planted, arrivals, fr, sec, loop: !!b.clip.loop, dims: b.rest.meta?.dims };
}

/** A metric's result. */
function M(id, family, hard, pass, value, limit, note = "", frame = null) {
  return { id, family, hard, pass: !!pass, value, limit, note, frame };
}
const r4 = (x) => (typeof x === "number" ? Math.round(x * 10000) / 10000 : x);

// --- body -------------------------------------------------------------------------------------------

function ground(m) {
  const { b, F, n, X, R0, footSet, d0, planted, feet } = m;
  let worst = 0, wf = 0;
  for (let f = 0; f < F; f++) {
    for (let v = 0; v < n; v++) {
      const y = X(f, v)[1];
      const pen = footSet.has(v) ? d0 - y : -y - Math.max(0, -R0(v)[1]);
      if (pen > worst) {
        worst = pen;
        wf = f;
      }
    }
  }
  let hover = 0, hf = 0;
  for (const s of ["L", "R"]) {
    for (let f = 0; f < F; f++) {
      if (!planted[s][f]) continue;
      const lo = Math.min(...feet[s].map((v) => X(f, v)[1]));
      if (lo - d0 > hover) {
        hover = lo - d0;
        hf = f;
      }
    }
  }
  void b;
  return [
    M("ground.penetration", "body", true, worst <= LIMITS.pen, r4(worst), LIMITS.pen, "deepest part below the ground (plots, beyond its rest depth)", wf),
    M("ground.hover", "body", true, hover <= LIMITS.hover, r4(hover), LIMITS.hover, "a planted boot above its rest sole (plots)", hf),
  ];
}

function footSkate(m) {
  const { F, X, feet, planted, d0 } = m;
  let worstFrame = 0, worstRun = 0, wf = 0;
  for (const s of ["L", "R"]) {
    let run = 0;
    for (let f = 1; f < F; f++) {
      if (!(planted[s][f] && planted[s][f - 1])) {
        run = 0;
        continue;
      }
      const touch = feet[s].filter((v) => X(f, v)[1] <= d0 + 0.004 && X(f - 1, v)[1] <= d0 + 0.004);
      const vs = touch.length ? touch : feet[s];
      const slip = Math.min(...vs.map((v) => hxz(sub(X(f, v), X(f - 1, v)))));
      run += slip;
      if (slip > worstFrame) {
        worstFrame = slip;
        wf = f;
      }
      worstRun = Math.max(worstRun, run);
    }
  }
  return [
    M("feet.slip", "body", true, worstFrame <= LIMITS.slipFrame, r4(worstFrame), LIMITS.slipFrame, "a planted boot's slide in one frame (plots)", wf),
    M("feet.drift", "body", true, worstRun <= LIMITS.slipRun, r4(worstRun), LIMITS.slipRun, "a planted boot's slide over one contact (plots)"),
  ];
}

/** Best rigid fit (Horn's quaternion method) of points A onto B: the RMS residual. */
export function kabsch(A, B) {
  const k = A.length;
  const ca = A.reduce((c, p) => add(c, p), [0, 0, 0]).map((x) => x / k);
  const cb = B.reduce((c, p) => add(c, p), [0, 0, 0]).map((x) => x / k);
  const S = [[0, 0, 0], [0, 0, 0], [0, 0, 0]];
  for (let i = 0; i < k; i++) {
    const a = sub(A[i], ca), bb = sub(B[i], cb);
    for (let r = 0; r < 3; r++) for (let c = 0; c < 3; c++) S[r][c] += a[r] * bb[c];
  }
  const [[xx, xy, xz], [yx, yy, yz], [zx, zy, zz]] = S;
  const N = [
    [xx + yy + zz, yz - zy, zx - xz, xy - yx],
    [yz - zy, xx - yy - zz, xy + yx, zx + xz],
    [zx - xz, xy + yx, -xx + yy - zz, yz + zy],
    [xy - yx, zx + xz, yz + zy, -xx - yy + zz],
  ];
  const q = topEigen(N);
  const [w, x, y, z] = q;
  const R = [
    [w * w + x * x - y * y - z * z, 2 * (x * y - w * z), 2 * (x * z + w * y)],
    [2 * (x * y + w * z), w * w - x * x + y * y - z * z, 2 * (y * z - w * x)],
    [2 * (x * z - w * y), 2 * (y * z + w * x), w * w - x * x - y * y + z * z],
  ];
  let e = 0, rad = 0;
  for (let i = 0; i < k; i++) {
    const a = sub(A[i], ca);
    const ra = [dot(R[0], a), dot(R[1], a), dot(R[2], a)];
    const d = sub(add(ra, cb), B[i]);
    e += dot(d, d);
    rad += dot(a, a);
  }
  return { rms: Math.sqrt(e / k), radius: Math.sqrt(rad / k) };
}

/** The eigenvector of the largest eigenvalue of a symmetric 4x4 (Jacobi rotations). */
function topEigen(A) {
  const a = A.map((r) => r.slice());
  const V = [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
  for (let sweep = 0; sweep < 50; sweep++) {
    let off = 0;
    for (let p = 0; p < 4; p++) for (let q = p + 1; q < 4; q++) off += a[p][q] * a[p][q];
    if (off < 1e-22) break;
    for (let p = 0; p < 4; p++) {
      for (let q = p + 1; q < 4; q++) {
        if (Math.abs(a[p][q]) < 1e-30) continue;
        const th = (a[q][q] - a[p][p]) / (2 * a[p][q]);
        const t = Math.sign(th || 1) / (Math.abs(th) + Math.sqrt(th * th + 1));
        const c = 1 / Math.sqrt(t * t + 1), s = t * c;
        for (let k = 0; k < 4; k++) {
          const akp = a[k][p], akq = a[k][q];
          a[k][p] = c * akp - s * akq;
          a[k][q] = s * akp + c * akq;
        }
        for (let k = 0; k < 4; k++) {
          const apk = a[p][k], aqk = a[q][k];
          a[p][k] = c * apk - s * aqk;
          a[q][k] = s * apk + c * aqk;
        }
        for (let k = 0; k < 4; k++) {
          const vkp = V[k][p], vkq = V[k][q];
          V[k][p] = c * vkp - s * vkq;
          V[k][q] = s * vkp + c * vkq;
        }
      }
    }
  }
  let best = 0;
  for (let i = 1; i < 4; i++) if (a[i][i] > a[best][best]) best = i;
  return [V[0][best], V[1][best], V[2][best], V[3][best]];
}

function rigid(m) {
  const { b, F, X, weapon, isBow, bowLimbs, shield, arrow } = m;
  const out = [];
  const parts = [];
  if (weapon.length && !isBow) parts.push(["weapon", weapon]);
  if (bowLimbs.length) parts.push(["bow", bowLimbs]);
  if (shield.length) parts.push(["shield", shield]);
  let worst = 0, wf = 0, wpart = "";
  for (const [name, vs0] of parts) {
    const step = Math.max(1, Math.floor(vs0.length / 160));
    const vs = vs0.filter((_, i) => i % step === 0);
    const ref = vs.map((v) => X(0, v));
    for (let f = 1; f < F; f++) {
      const k = kabsch(ref, vs.map((v) => X(f, v)));
      const sg = k.rms / (k.radius || 1);
      if (sg > worst) {
        worst = sg;
        wf = f;
        wpart = name;
      }
    }
  }
  out.push(M("rigid.props", "body", true, worst <= LIMITS.rigid, r4(worst), LIMITS.rigid, `props keep their shape (residual / radius${wpart ? "; worst: " + wpart : ""})`, wf));
  // The nocked arrow is whole or gone, never part-grown, but for one frame at an event (it is
  // measured on its vertices: what is drawn).
  if (arrow.size) {
    const vs = [...arrow];
    const extent = (f) => {
      const c = m.centroid(f, vs);
      let r = 0;
      for (const v of vs) r = Math.max(r, len(sub(X(f, v), c)));
      return r;
    };
    const e0 = extent(0);
    let partial = 0, pf = null;
    for (let f = 0; f < F; f++) {
      const k = extent(f) / e0;
      if (k > 0.03 && Math.abs(k - 1) > 0.02) {
        partial++;
        pf ??= f;
      }
    }
    const events = Object.keys(b.clip.events ?? {}).length;
    out.push(M("rigid.arrow", "body", true, partial <= events, partial, `<= ${events} (one per event)`, "frames with a part-grown or shrunk nocked arrow", pf));
  }
  // Limb segments: lengths as at rest.
  const J0 = b.joints[0];
  const segs = [["shoulderL", "elbowL"], ["elbowL", "handL"], ["shoulderR", "elbowR"], ["elbowR", "handR"], ["hipL", "kneeL"], ["kneeL", "ankleL"], ["hipR", "kneeR"], ["kneeR", "ankleR"]];
  let bw = 0, bf = 0;
  for (let f = 0; f < F; f++) {
    for (const [a, c] of segs) {
      const l0 = len(sub(J0[a], J0[c])), l = len(sub(b.joints[f][a], b.joints[f][c]));
      const e = Math.abs(l / l0 - 1);
      if (e > bw) {
        bw = e;
        bf = f;
      }
    }
  }
  out.push(M("rigid.limbs", "body", true, bw <= LIMITS.bone, r4(bw), LIMITS.bone, "limb segment length change (share)", bf));
  // Reach: a limb at its IK clamp no longer follows its target.
  const limbs = [["shoulderL", "elbowL", "handL"], ["shoulderR", "elbowR", "handR"], ["hipL", "kneeL", "ankleL"], ["hipR", "kneeR", "ankleR"]];
  let rw = 0, rf = 0, rn = "";
  for (const [a, c, e] of limbs) {
    const ext = (J) => len(sub(J[e], J[a])) / (len(sub(J[c], J[a])) + len(sub(J[e], J[c])));
    const hi = Math.max(LIMITS.reach, ext(J0) + 0.004), lo = Math.min(LIMITS.reachMin, ext(J0) - 0.02);
    for (let f = 0; f < F; f++) {
      const x = ext(b.joints[f]);
      const over = Math.max(x - hi, lo - x);
      if (over > rw) {
        rw = over;
        rf = f;
        rn = `${e} at ${x.toFixed(3)}`;
      }
    }
  }
  out.push(M("rigid.reach", "body", true, rw <= 0, r4(rw), 0, `a limb outside ${LIMITS.reachMin}-${LIMITS.reach} of its reach (or its rest extension; ${rn || "none"})`, rf));
  return out;
}

/** What the GPU shows between baked frames (a lerp) against the true pose there (a bake at twice
 *  the rate), in pixels at the gameplay zoom. */
function playback(m) {
  const { b, F, n, X } = m;
  // An idle moves too slowly for the lerp to matter: baked at 1.5 frames a second the in-between
  // is still within 0.3 px (measured), and one moving fast enough to fail fails idle.tempo first.
  if (b.clip.kind === "idle") return [];
  // The true path between two baked frames from a bake at 4x the rate against the GPU's straight
  // in-between, both ways (the Hausdorff distance of path and chord). Only shape counts: a lerp
  // that runs along the path at a different pace within a frame (a hard stop inside one) is
  // retimed, not misshapen; a cut arc or a skipped apex is misshapen.
  const K = 4;
  let fine;
  try {
    fine = bakeClip(b.recipe, b.variant, b.param, b.clipName, { fps: b.fps * K, clip: b.clip });
  } catch {
    return [];
  }
  const cam = camera({ ts: G, yaw: 0.25, w: 64, h: 64 });
  // The arrow and the string snap by design (loosed, nocked: one frame); the rest must lerp true.
  const skip = new Set([...m.arrow, ...m.string]);
  const toSeg = (p, a, c) => {
    const ex = c[0] - a[0], ey = c[1] - a[1], l2 = ex * ex + ey * ey;
    const t = l2 > 0 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * ex + (p[1] - a[1]) * ey) / l2)) : 0;
    return Math.hypot(a[0] + ex * t - p[0], a[1] + ey * t - p[1]);
  };
  let worst = 0, wf = 0;
  for (let f = 0; f + 1 < F; f++) {
    if (K * (f + 1) >= fine.frames) break;
    for (let v = 0; v < n; v += 2) {
      if (skip.has(v)) continue;
      const a = X(f, v), c = X(f + 1, v);
      const path = [];
      for (let g = K * f; g <= K * (f + 1); g++) {
        const q = (g * n + v) * 3;
        path.push(cam.project([fine.pos[q], fine.pos[q + 1], fine.pos[q + 2]]));
      }
      // Both ways: the lerp off the path, and the path off the lerp (an apex or arc it skips).
      const pa = cam.project(a), pc = cam.project(c);
      let e = 0;
      for (const p of path) e = Math.max(e, toSeg(p, pa, pc));
      for (const u of [0.25, 0.5, 0.75]) {
        const pl = cam.project(lerp3(a, c, u));
        let d = Infinity;
        for (let k = 0; k + 1 < path.length; k++) d = Math.min(d, toSeg(pl, path[k], path[k + 1]));
        e = Math.max(e, d);
      }
      if (e > worst) {
        worst = e;
        wf = f;
      }
    }
  }
  return [M("playback.lerp", "motion", true, worst <= LIMITS.lerpPx, r4(worst), LIMITS.lerpPx, `the GPU's in-betweens off the true path (px at the gameplay zoom; baked at ${b.fps} fps)`, wf)];
}

function joints(m) {
  const { b, F } = m;
  let knee = 0, kneeDir = 1, elbow = 0, kf = 0, df = 0, ef = 0;
  for (let f = 0; f < F; f++) {
    const J = b.joints[f];
    const fwd = eulerTurn([0, 0, 1], b.poses[f]?.rootRot ?? [0, 0, 0]);
    for (const s of ["L", "R"]) {
      const hp = J[`hip${s}`], k = J[`knee${s}`], a = J[`ankle${s}`];
      const th = angle(sub(k, hp), sub(a, k));
      if (th > knee) {
        knee = th;
        kf = f;
      }
      if (th > 10) {
        const ha = sub(a, hp);
        const t = Math.max(0, Math.min(1, dot(sub(k, hp), ha) / dot(ha, ha)));
        const q = add(hp, mul(ha, t));
        const dir = dot(nrm(sub(k, q)), fwd);
        if (dir < kneeDir) {
          kneeDir = dir;
          df = f;
        }
      }
      const e = angle(sub(J[`elbow${s}`], J[`shoulder${s}`]), sub(J[`hand${s}`], J[`elbow${s}`]));
      if (e > elbow) {
        elbow = e;
        ef = f;
      }
    }
  }
  // Wrist: the held weapon's haft against the forearm, at most LIMITS.wristDev below the figure's
  // own rest grip (ulnar deviation) and never past LIMITS.wrist.
  let wr = [Infinity, -Infinity], wf = 0, wristLo = LIMITS.wrist[0];
  if (m.weaponKey && !m.isBow) {
    const s = m.weaponKey === "weapon_r" ? "R" : "L";
    const J0 = b.joints[0], h0 = b.held[0][m.weaponKey];
    wristLo = Math.min(LIMITS.wrist[0], angle(sub(J0[`hand${s}`], J0[`elbow${s}`]), sub(h0.up, h0.at)) - LIMITS.wristDev);
    for (let f = 0; f < F; f++) {
      const J = b.joints[f], h = b.held[f][m.weaponKey];
      const a = angle(sub(J[`hand${s}`], J[`elbow${s}`]), sub(h.up, h.at));
      if (a < wr[0] || a > wr[1]) wf = f;
      wr = [Math.min(wr[0], a), Math.max(wr[1], a)];
    }
  }
  const out = [
    M("joints.knee", "body", true, knee <= LIMITS.knee, r4(knee), LIMITS.knee, "knee flexion (degrees)", kf),
    M("joints.kneeDir", "body", true, kneeDir >= LIMITS.kneeDir, r4(kneeDir), LIMITS.kneeDir, "a bent knee points forward (cosine to the body's forward)", df),
    M("joints.elbow", "body", true, elbow <= LIMITS.elbow, r4(elbow), LIMITS.elbow, "elbow flexion (degrees)", ef),
  ];
  if (wr[0] !== Infinity) out.push(M("joints.wrist", "body", true, wr[0] >= wristLo && wr[1] <= LIMITS.wrist[1], `${r4(wr[0])}..${r4(wr[1])}`, `${r4(wristLo)}-${LIMITS.wrist[1]}`, "the weapon's haft against the forearm (degrees; the floor is the rest grip less the wrist's deviation)", wf));
  if (b.angles?.length) {
    const deg = (x) => (x * 180) / Math.PI;
    let neck = 0, nf = 0, lean = [Infinity, -Infinity], twist = 0;
    const a0 = b.angles[0];
    const neck0 = Math.abs(deg(a0.headYaw - a0.twist));
    b.angles.forEach((a, f) => {
      const ny = Math.abs(deg(a.headYaw - a.twist));
      if (ny > neck) {
        neck = ny;
        nf = f;
      }
      lean = [Math.min(lean[0], deg(a.lean)), Math.max(lean[1], deg(a.lean))];
      twist = Math.max(twist, Math.abs(deg(a.twist)));
    });
    const nl = Math.max(LIMITS.neckYaw, neck0 + 10);
    out.push(M("joints.neck", "body", true, neck <= nl, r4(neck), nl, "head turned from the chest (degrees)", nf));
    out.push(M("joints.spine", "body", true, lean[0] >= LIMITS.lean[0] && lean[1] <= LIMITS.lean[1] && twist <= LIMITS.twist, `${r4(lean[0])}..${r4(lean[1])}, twist ${r4(twist)}`, `${LIMITS.lean[0]}..${LIMITS.lean[1]}, twist ${LIMITS.twist}`, "spine lean and twist (degrees)"));
  }
  return out;
}

function clearance(m) {
  const { b, F, X, weapon, isBow, bowLimbs, dims } = m;
  if (!dims || !weapon.length) return [];
  const vs0 = isBow ? bowLimbs : weapon;
  const step = Math.max(1, Math.floor(vs0.length / 200));
  const vs = vs0.filter((_, i) => i % step === 0);
  // Depth inside the head (a sphere of 0.9 head radii) and the chest (a capsule pelvis->neck).
  const depthAt = (f, p) => {
    const J = b.joints[f];
    const dh = 0.9 * dims.headR - len(sub(p, J.head));
    const a = J.pelvis, c = J.neck, ac = sub(c, a);
    const t = Math.max(0, Math.min(1, dot(sub(p, a), ac) / dot(ac, ac)));
    const dc = 0.85 * dims.chestR - len(sub(p, add(a, mul(ac, t))));
    return Math.max(dh, dc);
  };
  const base = vs.map((v) => Math.max(0, depthAt(0, X(0, v))));
  // Rest baseline from the rest build (frame 0 of an idle or attack is rest; a death's too).
  let worst = 0, wf = 0, over = 0;
  for (let f = 0; f < F; f++) {
    let e = 0;
    vs.forEach((v, i) => {
      e = Math.max(e, depthAt(f, X(f, v)) - base[i]);
    });
    if (e > worst) {
      worst = e;
      wf = f;
    }
    if (e > LIMITS.clear) over++;
  }
  const overTime = over / b.fps;
  const pass = worst <= LIMITS.clearOnce && overTime <= LIMITS.clearTime;
  return [M("clear.weapon", "body", true, pass, r4(worst), `${LIMITS.clear} (up to ${LIMITS.clearOnce} for ${LIMITS.clearTime} s)`, `${isBow ? "bow" : "weapon"} deeper into the head or chest than at rest (plots; ${r4(overTime)} s over ${LIMITS.clear})`, wf)];
}

/** Signed distance from point q (xz) to a convex polygon's boundary (positive inside). */
function hullMargin(pts, q) {
  const P = convexHull(pts);
  if (P.length < 3) return -Infinity;
  let inside = true, dmin = Infinity;
  for (let i = 0; i < P.length; i++) {
    const a = P[i], c = P[(i + 1) % P.length];
    const ex = c[0] - a[0], ez = c[1] - a[1];
    const cr = ex * (q[1] - a[1]) - ez * (q[0] - a[0]);
    if (cr < 0) inside = false;
    const t = Math.max(0, Math.min(1, ((q[0] - a[0]) * ex + (q[1] - a[1]) * ez) / (ex * ex + ez * ez)));
    dmin = Math.min(dmin, Math.hypot(a[0] + ex * t - q[0], a[1] + ez * t - q[1]));
  }
  return inside ? dmin : -dmin;
}

function convexHull(pts) {
  const p = pts.slice().sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  if (p.length < 3) return p;
  const cr = (o, a, b) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const lo = [], hi = [];
  for (const q of p) {
    while (lo.length >= 2 && cr(lo[lo.length - 2], lo[lo.length - 1], q) <= 0) lo.pop();
    lo.push(q);
  }
  for (let i = p.length - 1; i >= 0; i--) {
    const q = p[i];
    while (hi.length >= 2 && cr(hi[hi.length - 2], hi[hi.length - 1], q) <= 0) hi.pop();
    hi.push(q);
  }
  return lo.slice(0, -1).concat(hi.slice(0, -1));
}

function balance(m) {
  const { b, F, X, feet, d0, coms } = m;
  const kind = b.clip.kind;
  if (kind === "death") return [];
  // A foot in the air is stepping: the support it steps onto (its sole where it next lands)
  // counts with the stance foot, as the body falls toward it by design.
  const down = (f, s) => feet[s].filter((v) => X(f, v)[1] <= d0 + 0.004);
  let worst = Infinity, wf = 0;
  for (let f = 0; f < F; f++) {
    const pts = [];
    for (const s of ["L", "R"]) {
      let vs = down(f, s), g = f;
      while (!vs.length && ++g < F) vs = down(g, s);
      for (const v of vs) pts.push([X(g, v)[0], X(g, v)[2]]);
    }
    const sole = pts;
    // The static margin. (The extrapolated centre of mass would count the arm's swing, whose
    // momentum the body absorbs, as a fall.)
    const c = coms[f];
    const mg = hullMargin(sole, [c[0], c[2]]);
    if (mg < worst) {
      worst = mg;
      wf = f;
    }
  }
  const lim = kind === "attack" ? LIMITS.balanceAttack : LIMITS.balanceIdle;
  return [M("balance", "body", true, worst >= lim, r4(worst), lim, "centre of mass inside the feet, a stepping foot's next footprint included (plots; a lunge may lean a little past them)", wf)];
}

// --- motion -----------------------------------------------------------------------------------------

/** Points tracked for continuity: the joints, the striking point and up to 48 vertices per bone. */
function tracks(m) {
  const { b, F, X, byBone, arrow, string, strikes } = m;
  const out = [];
  const boneOf = (k) => (/^(hip|knee|ankle)L$/.test(k) ? BONE.LEG_L : /^(hip|knee|ankle)R$/.test(k) ? BONE.LEG_R : /^(head|headTop|neck)$/.test(k) ? BONE.HEAD : BONE.BODY);
  for (const k of Object.keys(b.joints[0])) out.push({ name: k, bone: boneOf(k), trunk: !/hand|elbow|shoulder/.test(k), at: (f) => b.joints[f][k] });
  // A bow's striking point is the nocked arrow's tip, which jumps by design when an arrow is
  // loosed or nocked (its snap is checked by rigid.arrow and shot.release, not here).
  if (!m.isBow) out.push({ name: "strike", trunk: false, at: (f) => strikes[f] });
  for (const [bone, vs0] of byBone) {
    const vs = vs0.filter((v) => !arrow.has(v) && !string.has(v));
    const step = Math.max(1, Math.floor(vs.length / 48));
    for (let i = 0; i < vs.length; i += step) {
      const v = vs[i];
      out.push({ name: `bone${bone}`, bone, trunk: bone === BONE.BODY || bone === BONE.HEAD || bone === BONE.LEG_L || bone === BONE.LEG_R, at: (f) => X(f, v) });
    }
  }
  void F;
  return out;
}

/** Frames where the clip declares a deliberate snap (an impact, a release, the killing blow). */
function snapFrames(m) {
  const { b, fr } = m;
  const s = new Set();
  const ev = b.clip.events ?? {};
  for (const k of ["impact", "release", "hit", "ground"]) {
    if (ev[k] === undefined) continue;
    const f = fr(ev[k]);
    for (let d = -2; d <= 2; d++) s.add(f + d);
  }
  if (b.clip.kind === "death") for (let f = 0; f <= 2; f++) s.add(f);
  return s;
}

function continuity(m) {
  const { F, loop, b } = m;
  const snaps = snapFrames(m);
  const T = tracks(m);
  const idx = (k) => (loop ? ((k % F) + F) % F : k);
  let wt = 0, ft = 0, nt = "";
  let spike = 0, sf = 0, sn = "", jit = 0, jf = 0, jn = "";
  // What lands on the ground stops in a frame (a contact, judged by death.landing in a fall): a
  // part's residual is not a pop within two frames of it landing (a leg lands as one piece).
  const landedB = new Map();
  for (const list of m.arrivals.values()) {
    for (const { f, bone } of list) {
      if (!landedB.has(bone)) landedB.set(bone, new Set());
      for (let d = -2; d <= 2; d++) landedB.get(bone).add(f + d);
    }
  }
  for (const t of T) {
    const P = Array.from({ length: F }, (_, f) => t.at(f));
    // The trunk: cubic residual.
    if (t.trunk) {
      for (let k = loop ? 0 : 2; k < (loop ? F : F - 2); k++) {
        if (snaps.has(k) || landedB.get(t.bone)?.has(k)) continue;
        const p = P[idx(k)], a = P[idx(k - 2)], bb = P[idx(k - 1)], c = P[idx(k + 1)], d = P[idx(k + 2)];
        const pred = [0, 1, 2].map((i) => (-a[i] + 4 * bb[i] + 4 * c[i] - d[i]) / 6);
        const e = len(sub(p, pred));
        if (e > wt) {
          wt = e;
          ft = k;
          nt = t.name;
        }
      }
    }
    // Everything: single-frame spikes and reversing jitters.
    const D = (k) => sub(P[idx(k + 1)], P[idx(k)]);
    for (let k = loop ? 0 : 1; k < (loop ? F : F - 2); k++) {
      if (snaps.has(k) || snaps.has(k + 1)) continue;
      const d0 = len(D(k - 1)), d1 = len(D(k)), d2 = len(D(k + 1));
      const sp = d1 / Math.max(d0, d2, LIMITS.spike[0] / 12);
      if (d1 > LIMITS.spike[0] && sp > spike) {
        spike = sp;
        sf = k;
        sn = t.name;
      }
      if (d0 > LIMITS.jitter2[0] && d1 > LIMITS.jitter2[0]) {
        const a = angle(D(k - 1), D(k));
        if (a > LIMITS.jitter2[1] && a > jit) {
          jit = a;
          jf = k;
          jn = t.name;
        }
      }
    }
  }
  void b;
  return [
    M("pop.trunk", "motion", true, wt <= LIMITS.popTrunk, r4(wt), LIMITS.popTrunk, `cubic residual of pelvis, chest, head and legs (plots${nt ? "; worst: " + nt : ""})`, ft),
    M("pop.spike", "motion", true, spike <= LIMITS.spike[1], r4(spike), LIMITS.spike[1], `a step > 3 px at the closest zoom, against its neighbours' (ratio${sn ? "; worst: " + sn : ""})`, sf),
    M("pop.jitter", "motion", true, jit === 0, r4(jit), `no reversal > ${LIMITS.jitter2[1]} deg`, `two fast steps that reverse (degrees${jn ? "; worst: " + jn : ""})`, jf),
  ];
}

function boundaries(m) {
  const { b, F, n, X, R0 } = m;
  const kind = b.clip.kind;
  const trans = transition(m);
  const dist = (f) => {
    let d = 0;
    for (let v = 0; v < n; v++) d = Math.max(d, len(sub(X(f, v), R0(v))));
    return d;
  };
  const out = [];
  if (kind === "attack" || kind === "death") out.push(M("boundary.start", "motion", true, dist(0) <= LIMITS.boundary, r4(dist(0)), LIMITS.boundary, "the first frame is the rest pose (plots)", 0));
  if (kind === "attack") out.push(M("boundary.end", "motion", true, dist(F - 1) <= LIMITS.boundary, r4(dist(F - 1)), LIMITS.boundary, "the last frame is the rest pose (plots)", F - 1));
  if (trans) out.push(trans);
  return out;
}

/** The cut from the idle into this clip as the runtime plays it: from each idle frame, the idle
 *  keeps playing while the clip starts, cross-faded over BLEND seconds (smoothstep), sampled at
 *  60 Hz. What the idle adds: each vertex's step against the same cut from the rest pose (the
 *  cross-fade's own lag, which retimes any clip's opening along its own path, is not a defect),
 *  in pixels at the gameplay zoom; the worst idle frame. */
function transition(m) {
  const { b, F, n, X, R0 } = m;
  if (b.clip.kind !== "attack" && b.clip.kind !== "death") return null;
  if (!clipOf(b.recipe, "idle")) return null;
  let idle;
  try {
    idle = bakeClip(b.recipe, b.variant, b.param, "idle");
  } catch {
    return null;
  }
  const cam = camera({ ts: G, yaw: 0.25, w: 64, h: 64 });
  const steps = Math.ceil((BLEND + 1 / 60) * 60) + 1;
  const vs = [];
  for (let v = 0; v < n; v += 3) vs.push(v);
  // The clip's own path and the cut from rest, projected once.
  const clipAt = (t, v) => {
    const xc = Math.min(F - 1, t * b.fps), c0 = Math.floor(xc), c1 = Math.min(F - 1, c0 + 1);
    return lerp3(X(c0, v), X(c1, v), xc - c0);
  };
  const wAt = (t) => {
    const w0 = Math.max(0, Math.min(1, t / BLEND));
    return w0 * w0 * (3 - 2 * w0);
  };
  const base = vs.map((v) => {
    const P = Array.from({ length: steps }, (_, k) => cam.project(lerp3(R0(v), clipAt(k / 60, v), wAt(k / 60))));
    return P.slice(1).map((p, k) => Math.hypot(p[0] - P[k][0], p[1] - P[k][1]));
  });
  let worst = 0, wt = 0, wf = 0;
  for (let i = 0; i < idle.frames; i++) {
    vs.forEach((v, vi) => {
      let prev = null;
      for (let k = 0; k < steps; k++) {
        const t = k / 60;
        const xi = (i + t * idle.fps) % idle.frames, i0 = Math.floor(xi), i1 = (i0 + 1) % idle.frames;
        const q = (f) => [idle.pos[(f * n + v) * 3], idle.pos[(f * n + v) * 3 + 1], idle.pos[(f * n + v) * 3 + 2]];
        const p = cam.project(lerp3(lerp3(q(i0), q(i1), xi - i0), clipAt(t, v), wAt(t)));
        if (prev) {
          const e = Math.hypot(p[0] - prev[0], p[1] - prev[1]) - base[vi][k - 1];
          if (e > worst) {
            worst = e;
            wt = t;
            wf = i;
          }
        }
        prev = p;
      }
    });
  }
  return M("boundary.transition", "motion", false, worst <= LIMITS.transition, r4(worst), LIMITS.transition, `cut from any idle frame into the clip, cross-faded over ${BLEND} s: the step the idle adds in a 60 Hz frame (px; worst from idle frame ${wf}, at ${r4(wt)} s)`);
}

// --- screen helpers ---------------------------------------------------------------------------------

/** Screen positions (px) of every vertex in frame f through `cam`. */
/** A gameplay-zoom camera at `yaw` whose canvas holds every frame of the clip (a long spear or a
 *  corpse half a plot from the feet is never cropped, which would read as hidden). */
function fitCamera(m, yaw, pad = 4) {
  const { b, F, n, X } = m;
  const probe = camera({ ts: G, yaw, w: 1, h: 1, ox: 0, oy: 0 });
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (let f = 0; f < F; f++) {
    for (let v = 0; v < n; v += 3) {
      const p = probe.project(X(f, v));
      x0 = Math.min(x0, p[0]);
      x1 = Math.max(x1, p[0]);
      y0 = Math.min(y0, p[1]);
      y1 = Math.max(y1, p[1]);
    }
  }
  void b;
  return camera({ ts: G, yaw, w: Math.ceil(x1 - x0) + 2 * pad, h: Math.ceil(y1 - y0) + 2 * pad, ox: pad - x0, oy: pad - y0 });
}

function screen(m, f, cam) {
  const out = new Float32Array(m.n * 2);
  for (let v = 0; v < m.n; v++) {
    const p = cam.project(m.X(f, v));
    out[v * 2] = p[0];
    out[v * 2 + 1] = p[1];
  }
  return out;
}

/** The rest figure's screen height (head top above the lowest sole) through `cam`. */
function screenHeight(m, cam) {
  const J = m.b.joints[0];
  const top = cam.project(J.headTop)[1];
  const sole = Math.max(cam.project(J.ankleL)[1], cam.project(J.ankleR)[1]);
  return Math.abs(sole - top);
}

function idle(m) {
  const { b, F, n, byBone } = m;
  if (b.clip.kind !== "idle") return [];
  const cam = camera({ ts: G, yaw: 0.25, w: 64, h: 64 });
  const S = Array.from({ length: F }, (_, f) => screen(m, f, cam));
  const rest = new Float32Array(n * 2);
  for (let v = 0; v < n; v++) {
    const p = cam.project(m.R0(v));
    rest[v * 2] = p[0];
    rest[v * 2 + 1] = p[1];
  }
  // Peak-to-peak per vertex; and its visible part, across the surface: the motion along the
  // vertex's normal on screen (a part turning about its own axis of symmetry -- a hat's brim, a
  // round shield -- slides its vertices along its outline and shows no motion).
  const p2p = new Float32Array(n), seen = new Float32Array(n), fromRest = new Float32Array(n), step = new Float32Array(n);
  for (let v = 0; v < n; v++) {
    let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity, st = 0, fr = 0, s0 = Infinity, s1 = -Infinity;
    const nd = cam.screenDir([b.rest.data[v * 13 + 3], b.rest.data[v * 13 + 4], b.rest.data[v * 13 + 5]]);
    const nl = Math.hypot(nd[0], nd[1]) || 1;
    for (let f = 0; f < F; f++) {
      const x = S[f][v * 2], y = S[f][v * 2 + 1];
      x0 = Math.min(x0, x);
      x1 = Math.max(x1, x);
      y0 = Math.min(y0, y);
      y1 = Math.max(y1, y);
      const a = ((x - rest[v * 2]) * nd[0] + (y - rest[v * 2 + 1]) * nd[1]) / nl;
      s0 = Math.min(s0, a);
      s1 = Math.max(s1, a);
      const g = (f + 1) % F;
      st = Math.max(st, Math.hypot(S[g][v * 2] - x, S[g][v * 2 + 1] - y));
      fr = Math.max(fr, Math.hypot(x - rest[v * 2], y - rest[v * 2 + 1]));
    }
    p2p[v] = Math.hypot(x1 - x0, y1 - y0);
    seen[v] = s1 - s0;
    step[v] = st;
    fromRest[v] = fr;
  }
  const pick = (bones) => bones.flatMap((bn) => byBone.get(bn) ?? []);
  const q = (arr, vs, p) => {
    const a = vs.map((v) => arr[v]).sort((x, y) => x - y);
    return a.length ? a[Math.min(a.length - 1, Math.floor(p * a.length))] : 0;
  };
  const alive = q(p2p, pick([BONE.HEAD, BONE.ARM_L, BONE.ARM_R]), 0.9);
  const trunkAlive = q(p2p, pick([BONE.BODY]), 0.9);
  const body = q(p2p, pick([BONE.BODY]), 0.5);
  const all = Math.max(...seen);
  const tempo = Math.max(...pick([BONE.BODY, BONE.HEAD]).map((v) => step[v])) * b.fps;
  const fromR = Math.max(...fromRest);
  // Breathing: the neck's height, its dominant period; jitter: motion faster than 1.5 Hz.
  const dur = b.clip.dur;
  const dft = (xs) => {
    const N = xs.length, mean = xs.reduce((a, x) => a + x, 0) / N;
    const amp = [];
    for (let k = 1; k <= N / 2; k++) {
      let re = 0, im = 0;
      for (let i = 0; i < N; i++) {
        re += (xs[i] - mean) * Math.cos((2 * Math.PI * k * i) / N);
        im -= (xs[i] - mean) * Math.sin((2 * Math.PI * k * i) / N);
      }
      amp.push({ k, f: k / dur, a: (2 * Math.hypot(re, im)) / N });
    }
    return amp;
  };
  const neck = dft(b.joints.map((J) => J.neck[1]));
  const top = neck.reduce((a, x) => (x.a > a.a ? x : a), neck[0]);
  const breath = top ? 1 / top.f : Infinity;
  let jitter = 0;
  for (const k of ["head", "handL", "handR", "pelvis", "neck"]) {
    for (const c of [0, 1, 2]) for (const x of dft(b.joints.map((J) => J[k][c]))) if (x.f > 1.5) jitter = Math.max(jitter, x.a);
  }
  // Rhythm (scored): the loop does not visibly repeat within itself, and more than one thing moves
  // (the joints' motion is not one curve: the largest principal component's share).
  const Y = b.joints.map((J) => Object.keys(J).flatMap((k) => J[k]));
  const mean = Y[0].map((_, i) => Y.reduce((a, y) => a + y[i], 0) / F);
  const Yc = Y.map((y) => y.map((x, i) => x - mean[i]));
  const ac = (L) => {
    let num = 0, den = 0;
    for (let f = 0; f < F; f++) {
      num += Yc[f].reduce((a, x, i) => a + x * Yc[(f + L) % F][i], 0);
      den += Yc[f].reduce((a, x) => a + x * x, 0);
    }
    return num / (den || 1);
  };
  let repeat = F;
  let dipped = false;
  for (let L = 1; L < F; L++) {
    const c = ac(L);
    if (c < 0.5) dipped = true;
    if (dipped && c >= 0.85) {
      repeat = L;
      break;
    }
  }
  const gram = Yc.map((a) => Yc.map((c) => a.reduce((s, x, i) => s + x * c[i], 0)));
  let vec = gram.map((_, i) => 1 + (i % 3));
  let lam = 0;
  for (let it = 0; it < 60; it++) {
    const nv = gram.map((row) => row.reduce((s, x, j) => s + x * vec[j], 0));
    lam = Math.hypot(...nv) / (Math.hypot(...vec) || 1);
    const l = Math.hypot(...nv) || 1;
    vec = nv.map((x) => x / l);
  }
  const trace = gram.reduce((s, row, i) => s + row[i], 0);
  const pc1 = lam / (trace || 1);
  // The seam, on the tracked points: step and acceleration across it against the interior's.
  let seamStep = 0, seamAcc = 0;
  for (const t of tracks(m)) {
    const P = Array.from({ length: F }, (_, f) => t.at(f));
    let st = 0, ac2 = 0;
    for (let k = 1; k < F - 1; k++) {
      st = Math.max(st, len(sub(P[k + 1], P[k])));
      ac2 = Math.max(ac2, len(add(sub(P[k + 1], P[k]), sub(P[k - 1], P[k]))));
    }
    const s0 = len(sub(P[0], P[F - 1]));
    const a0 = Math.max(len(add(sub(P[1], P[0]), sub(P[F - 1], P[0]))), len(add(sub(P[0], P[F - 1]), sub(P[F - 2], P[F - 1]))));
    if (s0 > LIMITS.seam[2]) seamStep = Math.max(seamStep, s0 / Math.max(st, 1e-9));
    if (a0 > LIMITS.seam[2]) seamAcc = Math.max(seamAcc, a0 / Math.max(ac2, 1e-9));
  }
  return [
    M("idle.seam", "motion", true, seamStep <= LIMITS.seam[0] && seamAcc <= LIMITS.seam[1], `step ${r4(seamStep)}x, acceleration ${r4(seamAcc)}x`, `${LIMITS.seam[0]}x, ${LIMITS.seam[1]}x`, "the loop's seam against its largest interior step and acceleration"),
    M("craft.rhythm", "motion", false, repeat >= 0.9 * F && pc1 <= 0.85, `repeats after ${r4(repeat / F)} of the loop, PC1 ${r4(pc1)}`, ">= 0.9, <= 0.85", "no metronome: no repeat inside the loop, several independent motions"),
    M("idle.alive", "reading", true, alive >= LIMITS.idleAlive && trunkAlive >= LIMITS.idleTrunk, `${r4(alive)} head and arms, ${r4(trunkAlive)} trunk`, `${LIMITS.idleAlive}, ${LIMITS.idleTrunk}`, "P90 peak-to-peak at the gameplay zoom (px): the body itself moves, not just what it holds"),
    M("idle.calm", "reading", true, body <= LIMITS.idleBody && all <= LIMITS.idleMax, `${r4(body)} body, ${r4(all)} max`, `${LIMITS.idleBody} body, ${LIMITS.idleMax} max`, "median body peak-to-peak and the largest visible one, across the surface (px)"),
    M("idle.tempo", "reading", true, tempo <= LIMITS.idleStep, r4(tempo), LIMITS.idleStep, "fastest body or head speed (px a second at the gameplay zoom)"),
    M("idle.fromRest", "reading", false, fromR <= LIMITS.idleFromRest, r4(fromR), LIMITS.idleFromRest, "farthest any vertex strays from the rest pose (px; scored: boundary.transition judges the cut itself)"),
    M("idle.loop", "motion", true, dur >= LIMITS.idleLoop[0] && dur <= LIMITS.idleLoop[1], dur, LIMITS.idleLoop.join("-"), "loop length (s)"),
    M("idle.breath", "motion", true, breath >= LIMITS.breath[0] && breath <= LIMITS.breath[1], r4(breath), LIMITS.breath.join("-"), "breathing period from the chest's rise (s)"),
    M("idle.jitter", "motion", true, jitter <= LIMITS.jitter, r4(jitter), LIMITS.jitter, "largest motion faster than 1.5 Hz (plots)"),
  ];
}

// --- silhouettes ------------------------------------------------------------------------------------

/** Aligned IoU of two coverage masks (0..255): the best over integer shifts of up to 2 px. */
export function iouAligned(A, B, w, h, k = 2) {
  let best = 0;
  for (let dy = -k; dy <= k; dy++) {
    for (let dx = -k; dx <= k; dx++) {
      let mn = 0, mx = 0;
      for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
          const a = A[y * w + x];
          const xx = x + dx, yy = y + dy;
          const bb = xx >= 0 && yy >= 0 && xx < w && yy < h ? B[yy * w + xx] : 0;
          mn += Math.min(a, bb);
          mx += Math.max(a, bb);
        }
      }
      if (mx > 0) best = Math.max(best, mn / mx);
    }
  }
  return best;
}

// --- attack -----------------------------------------------------------------------------------------

/** The attack's key frames from the striking point's speed: peak, anticipation (the turnaround
 *  before it) and the declared impact. */
function attackKeys(m) {
  const { b, F, strikes, fr } = m;
  const u = strikes.map((_, k) => (k === 0 || k === F - 1 ? 0 : (len(sub(strikes[k + 1], strikes[k - 1])) * b.fps) / 2));
  const ev = b.clip.events ?? {};
  const kc = fr(ev.impact ?? ev.release ?? 0);
  let tp = 0;
  for (let k = 0; k <= Math.min(F - 1, kc + 1); k++) if (u[k] > u[tp]) tp = k;
  const peak = u[tp] || 1e-9;
  let ka = 0;
  for (let k = tp; k >= 0; k--) {
    if (u[k] <= LIMITS.holdSpeed * peak) {
      ka = k;
      break;
    }
  }
  return { u, kc, tp, ka, peak };
}

function attack(m) {
  const { b, F, strikes, coms } = m;
  if (b.clip.kind !== "attack" || m.isBow) return [];
  const { u, kc, tp, ka, peak } = attackKeys(m);
  const out = [];
  void coms;
  // Phrasing.
  let hold = 0;
  for (let k = ka; k >= 0 && u[k] <= LIMITS.holdSpeed * peak; k--) hold++;
  const windupFast = Math.max(...u.slice(0, Math.max(1, ka)));
  const after = u[Math.min(F - 1, kc + m.sec(0.067))];
  let recover = 0;
  for (let k = kc + m.sec(0.1); k < F; k++) if (u[k] <= 0.5 * peak) recover++;
  const fps = b.fps;
  out.push(M("attack.windup", "motion", true, ka >= m.sec(LIMITS.windupMin) && hold >= m.sec(LIMITS.holdTime), `${r4(ka / fps)} s, hold ${r4(hold / fps)} s`, `>= ${LIMITS.windupMin} s, hold >= ${LIMITS.holdTime} s`, "the wind-up's turnaround and its hold", ka));
  out.push(M("attack.strike", "motion", true, kc - ka >= m.sec(LIMITS.strike[0]) && kc - ka <= m.sec(LIMITS.strike[1]), r4((kc - ka) / fps), LIMITS.strike.join("-"), "seconds from the turnaround to the impact", kc));
  out.push(M("attack.contrast", "motion", true, windupFast <= LIMITS.contrast * peak, r4(windupFast / peak), LIMITS.contrast, "the wind-up's fastest speed / the blow's peak"));
  // The speed arriving at the impact: the step into the first frame at or after it (a speed
  // centred on the impact frame would average in the stop itself).
  const ki = Math.min(F - 1, Math.max(1, Math.ceil(b.clip.events.impact * fps - 1e-6)));
  const ub = strikes.map((s, k) => (k === 0 ? 0 : len(sub(s, strikes[k - 1])) * fps));
  let tb = 1;
  for (let k = 1; k <= ki; k++) if (ub[k] > ub[tb]) tb = k;
  out.push(M("attack.peak", "motion", true, ki - tb <= m.sec(LIMITS.peakAtImpact) && ub[ki] >= 0.6 * ub[tb], `peak ${tb}, impact ${ki}, arriving at ${r4(ub[ki] / (ub[tb] || 1e-9))} of peak`, `impact - peak <= ${LIMITS.peakAtImpact} s, >= 0.6`, "the blow lands at speed (not eased into the target)", tb));
  out.push(M("attack.stop", "motion", true, after <= LIMITS.decel * peak, r4(after / peak), LIMITS.decel, "speed 67 ms after impact / peak (the blow meets the target)", kc + m.sec(0.067)));
  out.push(M("attack.recover", "motion", true, recover >= m.sec(LIMITS.recover), r4(recover / fps), LIMITS.recover, "seconds of unhurried recovery after the blow"));
  out.push(M("attack.duration", "motion", true, b.clip.dur >= LIMITS.duration[0] && b.clip.dur <= LIMITS.duration[1], b.clip.dur, LIMITS.duration.join("-"), "clip length (s)"));
  // Delivery: power, the blow in front of the body, along the strike direction, anticipation back.
  const dir = nrm([b.clip.strike?.[0] ?? 0, 0, b.clip.strike?.[2] ?? 1]);
  const pel = b.joints[kc].pelvis;
  const front = dot(sub(strikes[kc], pel), dir);
  const aimErr = angle([strikes[kc][0] - pel[0], 0, strikes[kc][2] - pel[2]], dir);
  const back = dot(sub(strikes[ka], strikes[0]), dir);
  out.push(M("attack.power", "body", true, u[kc] >= LIMITS.power[0] && u[kc] <= LIMITS.power[1], r4(u[kc]), LIMITS.power.join("-"), "striking point speed at impact (plots/s)", kc));
  out.push(M("attack.reach", "body", true, front >= LIMITS.reachFront && strikes[kc][1] > 0.05, `${r4(front)} ahead, ${r4(strikes[kc][1])} up`, `>= ${LIMITS.reachFront} ahead, above the ground`, "where the blow lands (plots)", kc));
  out.push(M("attack.aim", "body", true, aimErr <= LIMITS.aim, r4(aimErr), LIMITS.aim, "blow vs the strike direction (degrees)", kc));
  const tw = b.angles?.[kc]?.twist ?? 0;
  const chestErr = angle([Math.sin(tw), 0, Math.cos(tw)], dir);
  out.push(M("attack.facing", "body", true, chestErr <= LIMITS.facing, r4(chestErr), LIMITS.facing, "the chest toward the target at the impact (degrees)", kc));
  out.push(M("attack.anticipation", "motion", true, back < 0, r4(back), "< 0", "the wind-up draws the weapon back against the strike (plots)", ka));
  // Craft (scored): the body drives the blow (the chest travels along the strike; it leads or
  // matches the weapon), and the weapon swings on an arc rather than poking.
  const Hh = b.height;
  const neckP = b.joints.map((J) => dot(J.neck, dir));
  const commit = Math.max(...neckP) - Math.min(...neckP);
  const speedOf = (pts) => pts.map((_, k) => (k === 0 || k === F - 1 ? 0 : len(sub(pts[k + 1], pts[k - 1]))));
  const ns = speedOf(b.joints.map((J) => J.neck));
  let nPk = 0;
  for (let k = 0; k <= kc; k++) if (ns[k] > ns[nPk]) nPk = k;
  const lag = nPk - tp;
  const chord = sub(strikes[kc], strikes[ka]);
  let bulge = 0;
  for (let k = ka; k <= kc; k++) {
    const r = sub(strikes[k], strikes[ka]);
    const t = Math.max(0, Math.min(1, dot(r, chord) / (dot(chord, chord) || 1)));
    bulge = Math.max(bulge, len(sub(r, mul(chord, t))));
  }
  const arc = bulge / (len(chord) || 1);
  out.push(M("craft.commit", "motion", false, commit >= 0.13 * Hh && lag <= 0 && lag >= -4, `${r4(commit / Hh)} H, chest peak ${lag} frames from the weapon's`, ">= 0.13 H, chest leads by 0-4 frames", "the body drives the blow"));
  out.push(M("craft.arc", "motion", false, arc >= 0.12, r4(arc), 0.12, "the weapon's path bulges from its chord (an arc, not a poke)"));
  // Reading at the game's facings: travel, silhouette contrast, the weapon outside the body.
  let travelMin = Infinity, travelMax = 0, iouK = 0, iouR = 0, wOut = Infinity, worstDir = "", dirs = [];
  let travelOk = true, keysOk = true;
  for (const [name, heading] of Object.entries(HEADINGS)) {
    const yaw = attackYaw(b.clip, heading);
    const back = Math.abs(yaw) > Math.PI / 2 + 1e-6;
    const cam = fitCamera(m, yaw);
    const H = screenHeight(m, cam);
    const pa = cam.project(strikes[ka]), pc = cam.project(strikes[kc]);
    const tr = Math.hypot(pc[0] - pa[0], pc[1] - pa[1]) / H;
    travelMin = Math.min(travelMin, tr);
    travelMax = Math.max(travelMax, tr);
    if (tr < (back ? LIMITS.travelBack : LIMITS.travel[0]) || tr > LIMITS.travel[1]) travelOk = false;
    const sa = renderFrame(b, ka, cam, { ss: 4, flat: [255, 255, 255], split: BONE.WEAPON });
    const sc = renderFrame(b, kc, cam, { ss: 4, flat: [255, 255, 255], split: BONE.WEAPON });
    const s0 = renderFrame(b, 0, cam, { ss: 4, flat: [255, 255, 255] });
    const ik = iouAligned(sa.mask, sc.mask, cam.w, cam.h);
    const ir = iouAligned(s0.mask, sc.mask, cam.w, cam.h);
    iouK = Math.max(iouK, ik);
    iouR = Math.max(iouR, ir);
    if (ik > (back ? LIMITS.iouKeysBack : LIMITS.iouKeys) || (!back && ir > LIMITS.iouRest)) keysOk = false;
    for (const im of back ? [sa] : [sa, sc]) {
      const wo = weaponOutside(im, cam.w, cam.h);
      const need = Math.max(LIMITS.weaponOut[0], LIMITS.weaponOut[1] * wo.total);
      const ratio = wo.out / need;
      if (ratio < wOut) {
        wOut = ratio;
        worstDir = `${name}: ${wo.out.toFixed(0)} of ${wo.total.toFixed(0)} px^2`;
      }
    }
    dirs.push(`${name}${back ? "(back)" : ""} ${tr.toFixed(2)}H iou ${ik.toFixed(2)}/${ir.toFixed(2)}`);
  }
  out.push(M("read.travel", "reading", true, travelOk, `${r4(travelMin)}..${r4(travelMax)}`, `${LIMITS.travel.join("-")} (back arc from ${LIMITS.travelBack})`, `the blow's screen travel in figure heights at the eight facings (${dirs.join("; ")})`));
  out.push(M("read.keys", "reading", true, keysOk, `${r4(iouK)} wind-up/impact, ${r4(iouR)} rest/impact`, `${LIMITS.iouKeys} (back arc ${LIMITS.iouKeysBack}) / ${LIMITS.iouRest}`, "silhouette overlap of the key poses (worst facing; lower = more distinct)"));
  out.push(M("read.weapon", "reading", true, wOut >= 1, r4(wOut), 1, `weapon outside the body's outline at the wind-up and impact (the wind-up only from behind), worst ${worstDir}`));
  return out;
}

/** Of the weapon's coverage, how much lies outside the rest of the figure dilated by 1 px. */
function weaponOutside(im, w, h) {
  const W = im.split.weapon, O = im.split.other;
  let total = 0, out = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const a = W[y * w + x] / 255;
      total += a;
      let d = 0;
      for (let j = -1; j <= 1; j++) for (let i = -1; i <= 1; i++) {
        const xx = x + i, yy = y + j;
        if (xx >= 0 && yy >= 0 && xx < w && yy < h) d = Math.max(d, O[yy * w + xx] / 255);
      }
      out += Math.max(0, a - d);
    }
  }
  return { total, out };
}

// --- archer -----------------------------------------------------------------------------------------

function shot(m) {
  const { b, F, fr } = m;
  if (b.clip.kind !== "attack" || !m.isBow) return [];
  const ev = b.clip.events ?? {};
  const kr = fr(ev.release ?? 0);
  const H = (f) => b.held[f][m.weaponKey];
  const out = [];
  // The string at the drawing hand until the release; the tip ahead of the grip while nocked.
  let sh = 0, shf = 0, ahead = Infinity, af = 0;
  for (let f = 0; f < F; f++) {
    const p = b.poses[f];
    const h = H(f);
    if (f < kr && (p.string ?? 1) >= 0.999) {
      const d = len(sub(h.nock, b.joints[f].handR));
      if (d > sh) {
        sh = d;
        shf = f;
      }
    }
    if ((p.arrow ?? 1) >= 0.999 && h.tip) {
      const axis = nrm(sub(h.tip, h.nock));
      const a = dot(sub(h.tip, h.grip), axis);
      if (a < ahead) {
        ahead = a;
        af = f;
      }
    }
  }
  out.push(M("shot.stringHand", "body", true, sh <= LIMITS.stringHand, r4(sh), LIMITS.stringHand, "the drawn string's nock from the drawing hand (plots)", shf));
  out.push(M("shot.tipAhead", "body", true, ahead >= LIMITS.tipAhead, r4(ahead), LIMITS.tipAhead, "the nocked arrow's tip ahead of the grip (plots)", af));
  // Aim hold before the release; the full draw.
  let turn = 0, gmove = 0;
  for (let f = Math.max(1, kr - m.sec(LIMITS.aimHold)); f < kr; f++) {
    turn = Math.max(turn, angle(sub(H(f).tip, H(f).nock), sub(H(f - 1).tip, H(f - 1).nock)));
    gmove = Math.max(gmove, len(sub(H(f).grip, H(f - 1).grip)));
  }
  const draw = len(sub(H(Math.max(0, kr - 1)).nock, H(Math.max(0, kr - 1)).grip)), draw0 = len(sub(H(0).nock, H(0).grip));
  out.push(M("shot.aimHold", "motion", true, turn <= LIMITS.aimTurn && gmove <= 0.004, `${r4(turn)} deg, grip ${r4(gmove)}`, `${LIMITS.aimTurn} deg, 0.004`, `the last ${LIMITS.aimHold} s before the release hold the aim`, kr));
  out.push(M("shot.fullDraw", "body", true, draw >= 0.8 * draw0, r4(draw / draw0), 0.8, "draw at the release / the rest pose's full draw", kr - 1));
  // At the release: the drawing hand at the face, the bow arm along the arrow, the arrow along
  // the shot; after it, the drawing hand flies back.
  const kp = Math.max(0, kr - 1), Jp = b.joints[kp], hp = H(kp);
  const anchorD = len(sub(Jp.handR, Jp.head)) - (m.dims?.headR ?? 0.05);
  const arrowAx = sub(hp.tip, hp.nock);
  const sd = b.clip.strike ?? [1, 0, 0];
  const aimE = angle([arrowAx[0], 0, arrowAx[2]], [sd[0], 0, sd[2]]);
  const armE = angle(sub(Jp.handL, Jp.shoulderL), arrowAx);
  let recoil = 0;
  for (let f = kr; f < Math.min(F, kr + m.sec(0.2)); f++) recoil = Math.max(recoil, len(sub(b.joints[f].handR, Jp.handR)));
  out.push(M("shot.anchor", "body", true, anchorD <= LIMITS.anchor, r4(anchorD), LIMITS.anchor, "the drawing hand at the face at the release (beyond a head radius, plots)", kp));
  out.push(M("shot.aimDir", "body", true, aimE <= LIMITS.aimDir, r4(aimE), LIMITS.aimDir, "the nocked arrow vs the shot direction (degrees)", kp));
  out.push(M("shot.bowArm", "body", true, armE <= LIMITS.bowArm, r4(armE), LIMITS.bowArm, "the bow arm along the arrow (degrees)", kp));
  out.push(M("shot.recoil", "motion", true, recoil >= LIMITS.recoil, r4(recoil), LIMITS.recoil, "the drawing hand flies back after the release (plots within 0.2 s)", kr));
  // The release: the hand arrow gone within a frame, the string home within LIMITS.snap frames.
  const a0 = b.poses[Math.max(0, kr - 1)]?.arrow ?? 1, a1 = b.poses[Math.min(F - 1, kr)]?.arrow ?? 1;
  const ks = Math.min(F - 1, kr + m.sec(LIMITS.snap));
  const brace = len(sub(H(ks).nock, H(ks).grip));
  out.push(M("shot.release", "motion", true, a0 >= 0.95 && a1 <= 0.05, `${r4(a0)} -> ${r4(a1)}`, "1 -> 0 in one frame", "the nocked arrow at the frames around the release", kr));
  out.push(M("shot.snap", "motion", true, brace <= 0.35 * draw, r4(brace / draw), 0.35, `string draw ${LIMITS.snap} s after the release / the drawn draw (it snaps home)`, ks));
  // Exactly one arrow: from the release, while the loosed arrow flies, the nocked one is gone
  // (measured on its vertices: what is drawn).
  if (m.arrow.size) {
    const vs = [...m.arrow];
    const extent = (f) => {
      const c = m.centroid(f, vs);
      let r = 0;
      for (const v of vs) r = Math.max(r, len(sub(m.X(f, v), c)));
      return r;
    };
    const e0 = extent(0);
    let left = 0, lf = null;
    for (let f = kr; f < Math.min(F, kr + m.sec(LIMITS.flight[0])); f++) {
      const k = extent(f) / e0;
      if (k > 0.03) {
        left++;
        lf ??= f;
      }
    }
    out.push(M("shot.oneArrow", "reading", true, left === 0, left, 0, "frames after the release that still draw the nocked arrow beside the flying one", lf));
  }
  // The arrow in flight: leaves along the bow toward the target at every facing, flies 4-12
  // frames, and is big enough to see.
  const rel = releasePoint(b);
  let axisErr = 0, times = [Infinity, 0], dirs = [];
  for (const [name, heading] of Object.entries(HEADINGS)) {
    const yaw = attackYaw(b.clip, heading);
    const cy = Math.cos(yaw), sy = Math.sin(yaw);
    const W = (q) => [cy * q[0] + sy * q[2], q[1], -sy * q[0] + cy * q[2]];
    const from = W(rel.tip);
    const to = [Math.sin(heading), 0.22, Math.cos(heading)];
    const dur = flightTime(Math.hypot(to[0] - from[0], to[2] - from[2]));
    times = [Math.min(times[0], dur), Math.max(times[1], dur)];
    const d0 = flight(from, to, 0).dir;
    const axis = W(nrm(sub(rel.tip, rel.nock)));
    const e = angle(d0, axis);
    axisErr = Math.max(axisErr, e);
    dirs.push(`${name} ${e.toFixed(1)}`);
  }
  out.push(M("shot.axis", "reading", true, axisErr <= LIMITS.shotAxis, r4(axisErr), LIMITS.shotAxis, `the loosed arrow flies along the nocked arrow (degrees; ${dirs.join(", ")})`));
  out.push(M("shot.flight", "reading", true, times[0] >= LIMITS.flight[0] && times[1] <= LIMITS.flight[1], `${r4(times[0])}..${r4(times[1])}`, LIMITS.flight.join("-"), "seconds in flight to an adjacent plot"));
  // The arrow in flight is big enough to see at the gameplay zoom (a true-scale shaft is half a
  // pixel wide): its coverage seen side-on and end-on-ish.
  try {
    const a = buildAsset("fx.arrow");
    const ab = { count: a.count, pos: new Float32Array(a.count * 3), nrm: new Float32Array(a.count * 3), rest: a, bone: new Uint8Array(a.count) };
    for (let v = 0; v < a.count; v++) for (let k = 0; k < 3; k++) {
      ab.pos[v * 3 + k] = a.data[v * 13 + k];
      ab.nrm[v * 3 + k] = a.data[v * 13 + 3 + k];
    }
    let cov = Infinity;
    for (const yaw of [Math.PI / 2, Math.PI / 4]) {
      const im = renderFrame(ab, 0, camera({ ts: G, yaw, w: 64, h: 64, oy: 32 }), { ss: 4, flat: [255, 255, 255] });
      cov = Math.min(cov, im.mask.reduce((x, c) => x + c / 255, 0));
    }
    out.push(M("shot.visible", "reading", true, cov >= LIMITS.arrowPx, r4(cov), LIMITS.arrowPx, "the flying arrow's coverage at the gameplay zoom (px^2, across and diagonal)"));
  } catch {
    out.push(M("shot.visible", "reading", true, false, "no fx.arrow", "a recipe", "the flying arrow's model"));
  }
  return out;
}

// --- death ------------------------------------------------------------------------------------------

function death(m) {
  const { b, F, X, n, coms, byBone } = m;
  if (b.clip.kind !== "death") return [];
  const out = [];
  const fps = b.fps;
  // Reaction: within the first frames the head moves away from the blow.
  const away = nrm(b.clip.away ?? [0, 0, -1]);
  let react = 0;
  for (let f = 1; f <= Math.min(F - 1, m.sec(LIMITS.react[0])); f++) react = Math.max(react, dot(sub(b.joints[f].head, b.joints[0].head), away));
  out.push(M("death.react", "motion", true, react >= LIMITS.react[1], r4(react), LIMITS.react[1], `the head thrown away from the blow within ${LIMITS.react[0]} s (plots)`));
  // The fall: from the onset of the final descent (the centre of mass sinking without a pause
  // until the impact; an earlier stagger or buckle that catches itself is not the fall) to the impact (the trunk's core reaching the ground: the body above the hips
  // and the head, not a skirt or a coat tail hanging below them).
  const vy = coms.map((c, k) => (k === 0 ? 0 : (c[1] - coms[k - 1][1]) * fps));
  const hipY = Math.min(b.joints[0].hipL[1], b.joints[0].hipR[1]); // a death starts at rest
  const trunk = (byBone.get(BONE.BODY) ?? []).filter((v) => m.R0(v)[1] >= hipY).concat(byBone.get(BONE.HEAD) ?? []);
  const lowTrunk = (f) => Math.min(...trunk.map((v) => X(f, v)[1]));
  let ts = -1, ti = -1;
  for (let k = 3; k < F; k++) {
    if (lowTrunk(k) <= 0.012) {
      ti = k;
      break;
    }
  }
  if (ti > 0) {
    // The final descent: back from the impact while the centre of mass sinks (faster than 0.03
    // plots/s: a sag slower than that is a hang, still); a descent that never sinks faster than
    // 0.1 plots/s is a body lowered, not a fall.
    ts = ti - 1;
    while (ts > 1 && vy[ts] < -0.03) ts--;
    if (ts >= ti - 1) ts = Math.max(1, ti - 2);
    if (Math.min(...vy.slice(ts, ti + 1)) >= -0.1) ts = -1;
  }
  if (ts < 0 || ti < 0) {
    out.push(M("death.falls", "body", true, false, "no fall", "falls to the ground", "the body never reaches the ground"));
    return out;
  }
  // The speed peaks at the impact (gravity does not ease into the ground).
  let pk = ts;
  for (let k = ts; k <= ti; k++) if (vy[k] < vy[pk]) pk = k;
  out.push(M("death.accelerates", "body", true, ti - pk <= 2, `peak ${pk}, ground ${ti}`, "peak within 2 frames of the ground", "the fall is fastest as it lands", pk));
  // Gravity: a parabola fit of the centre of mass height over the fall.
  const T = [], Y = [];
  for (let k = ts; k <= ti; k++) {
    T.push((k - ts) / fps);
    Y.push(coms[k][1]);
  }
  const g = -2 * quadFit(T, Y)[2];
  out.push(M("death.gravity", "body", true, g >= LIMITS.gEff[0] * GRAV && g <= LIMITS.gEff[1] * GRAV, r4(g / GRAV), LIMITS.gEff.join("-"), "the fall's acceleration / g (a plot is 3.65 m)"));
  const dh = coms[ts][1] - coms[ti][1];
  const vImp = -Math.min(...vy.slice(ts, ti + 1));
  const rho = (vImp * vImp) / (2 * GRAV * Math.max(1e-6, dh));
  out.push(M("death.energy", "body", true, rho >= LIMITS.energy[0] && rho <= LIMITS.energy[1], r4(rho), LIMITS.energy.join("-"), "impact speed^2 / (2 g drop): 1 is free fall, 0.75 a rigid topple"));
  const ge = b.clip.events?.ground;
  if (ge !== undefined) out.push(M("death.groundEvent", "motion", true, Math.abs(ge - ti / fps) <= LIMITS.groundEvent, `declared ${ge} s, measured ${r4(ti / fps)} s`, `within ${LIMITS.groundEvent} s`, "the clip's ground event is when the body reaches the ground (declared metadata must be true: an effect timed by it lands with the body)", ti));
  out.push(M("death.fallTime", "motion", true, (ti - ts) / fps >= LIMITS.fallTime[0] && (ti - ts) / fps <= LIMITS.fallTime[1], r4((ti - ts) / fps), LIMITS.fallTime.join("-"), "seconds from onset to the ground", ts));
  // Landing: a small rebound at most, then still. The rebound is measured from the lowest point
  // so far (a body still settling after its first contact may not rise again either).
  let bounce = 0;
  for (let k = ti, lo = Infinity; k < F; k++) {
    lo = Math.min(lo, coms[k][1]);
    bounce = Math.max(bounce, coms[k][1] - lo);
  }
  // Every part lands no faster than falling from its own highest point since the fall began (a
  // dead limb has no muscle to drive it: a rigid topple's tip arrives at 1.22x free fall, a limb
  // slammed down by its keys at several times it). Slack: 0.15 plots/s.
  let slam = 0, slamF = ts, slamB = "";
  for (const [v, list] of m.arrivals) {
    let from = ts;
    for (const { f, ground, bone } of list) {
      if (f <= ts) continue;
      const y = (k) => b.pos[(k * n + v) * 3 + 1];
      let apex = -Infinity;
      for (let k = from; k <= f; k++) apex = Math.max(apex, y(k));
      const speed = Math.max(y(f - 1) - y(f), f >= 2 ? y(f - 2) - y(f - 1) : 0) * fps; // its last two steps
      const r = speed / (Math.sqrt(2 * GRAV * Math.max(0, apex - ground)) + 0.15);
      if (r > slam) {
        slam = r;
        slamF = f;
        slamB = { [BONE.BODY]: "body", [BONE.HEAD]: "head", [BONE.LEG_L]: "left leg", [BONE.LEG_R]: "right leg" }[bone];
      }
      from = f;
    }
  }
  out.push(M("death.landing", "body", true, slam <= LIMITS.landing, r4(slam), LIMITS.landing, `each part's landing speed against falling from its highest point (ratio${slamB ? "; worst: " + slamB : ""})`, slamF));
  out.push(M("death.bounce", "body", true, bounce <= LIMITS.bounce, r4(bounce), LIMITS.bounce, "the body's rebound after it lands (plots)", ti));
  const stepAt = (f) => {
    let d = 0;
    for (let v = 0; v < n; v += 3) d = Math.max(d, len(sub(X(f, v), X(f - 1, v))));
    return d;
  };
  let settled = F;
  for (let f = F - 1; f > ti; f--) {
    if (stepAt(f) > LIMITS.still) break;
    settled = f;
  }
  const lastStill = Math.max(...Array.from({ length: Math.min(6, F - 1) }, (_, i) => stepAt(F - 1 - i)));
  out.push(M("death.settles", "body", true, lastStill <= LIMITS.still && (settled - ti) / fps <= LIMITS.settle, `still from frame ${settled} (${r4((settled - ti) / fps)} s after landing)`, `still within ${LIMITS.settle} s`, "the body comes to rest and stays", settled));
  // Lying on the ground: the trunk touches it, the centre of mass is low.
  const lt = lowTrunk(F - 1);
  out.push(M("death.grounded", "body", true, Math.abs(lt) <= 0.012, r4(lt), "|y| <= 0.012", "the trunk lies on the ground (its lowest point, plots)", F - 1));
  const ratio = coms[F - 1][1] / coms[0][1];
  const headLow = b.joints[F - 1].head[1] / b.height;
  out.push(M("death.low", "body", true, ratio <= LIMITS.lieCom && headLow <= LIMITS.lieHead, `mass ${r4(ratio)}, head ${r4(headLow)}`, `${LIMITS.lieCom}, ${LIMITS.lieHead}`, "centre of mass height lying / standing; the head's height / standing height", F - 1));
  // The clip says where the body falls (`fall`, deathYaw turns it across the screen): it must.
  const JF = b.joints[F - 1];
  const lay = sub(JF.head, lerp3(JF.ankleL, JF.ankleR, 0.5));
  const decl = b.clip.fall ?? b.clip.away ?? [0, 0, -1];
  const fe = angle([lay[0], 0, lay[2]], [decl[0], 0, decl[2]]);
  out.push(M("death.fallDir", "motion", true, fe <= LIMITS.fallDir, r4(fe), LIMITS.fallDir, "the body lies along the clip's declared `fall` (degrees)", F - 1));
  // Reads as lying at every facing the game gives a death (deathYaw for each attacker side).
  let worstA = 0, worstH = 0, dirs = [];
  for (const [name, heading] of Object.entries(HEADINGS)) {
    const yaw = deathYaw(b.clip, heading);
    const cam = camera({ ts: G, yaw, w: 96, h: 72 });
    const J = b.joints[F - 1];
    const head = cam.project(J.head), ank = cam.project(lerp3(J.ankleL, J.ankleR, 0.5));
    const ang = (Math.atan2(Math.abs(head[1] - ank[1]), Math.abs(head[0] - ank[0])) * 180) / Math.PI;
    const rest = screenHeight(m, cam);
    // Screen-up elevation of the head above the lowest sole, lying / standing.
    let low = -Infinity;
    for (const v of m.footSet) low = Math.max(low, cam.project(X(F - 1, v))[1]);
    const el = (low - cam.project(J.headTop)[1]) / rest;
    worstA = Math.max(worstA, ang);
    worstH = Math.max(worstH, el);
    dirs.push(`${name} ${ang.toFixed(0)} deg ${el.toFixed(2)}H`);
  }
  out.push(M("read.lying", "reading", true, worstA <= LIMITS.lieAngle && worstH <= LIMITS.lieHeight, `${r4(worstA)} deg, ${r4(worstH)} H`, `${LIMITS.lieAngle} deg, ${LIMITS.lieHeight} H`, `the corpse's body axis on screen and its height at E, S, W, N attackers (${dirs.join("; ")})`));
  return out;
}

/** Least squares y = a + b t + c t^2: [a, b, c]. */
function quadFit(T, Y) {
  const S = [[0, 0, 0], [0, 0, 0], [0, 0, 0]], R = [0, 0, 0];
  for (let i = 0; i < T.length; i++) {
    const p = [1, T[i], T[i] * T[i]];
    for (let r = 0; r < 3; r++) {
      R[r] += p[r] * Y[i];
      for (let c = 0; c < 3; c++) S[r][c] += p[r] * p[c];
    }
  }
  // Gaussian elimination.
  for (let i = 0; i < 3; i++) {
    let mx = i;
    for (let r = i + 1; r < 3; r++) if (Math.abs(S[r][i]) > Math.abs(S[mx][i])) mx = r;
    [S[i], S[mx]] = [S[mx], S[i]];
    [R[i], R[mx]] = [R[mx], R[i]];
    for (let r = i + 1; r < 3; r++) {
      const f = S[r][i] / (S[i][i] || 1e-12);
      for (let c = i; c < 3; c++) S[r][c] -= f * S[i][c];
      R[r] -= f * R[i];
    }
  }
  const x = [0, 0, 0];
  for (let i = 2; i >= 0; i--) {
    let s = R[i];
    for (let c = i + 1; c < 3; c++) s -= S[i][c] * x[c];
    x[i] = s / (S[i][i] || 1e-12);
  }
  return x;
}

// --- the whole check --------------------------------------------------------------------------------

/** The metric group (a key of check()'s groups) that computes metric `id`. */
export function groupOf(id) {
  const [head] = id.split(".");
  return { ground: "ground", feet: "footSkate", rigid: "rigid", playback: "playback", joints: "joints", clear: "clearance", balance: "balance", pop: "continuity", boundary: "boundaries", idle: "idle", attack: "attack", shot: "shot", death: "death" }[head] ?? { "read.lying": "death", "craft.rhythm": "idle" }[id] ?? (id.startsWith("read.") || id.startsWith("craft.") ? "attack" : null);
}

/** Every metric that applies to the bake: [{id, family, hard, pass, value, limit, note, frame}]. */
export function check(b, { only = null } = {}) {
  const m = model(b);
  const groups = { ground, footSkate, rigid, playback, joints, clearance, balance, continuity, boundaries, idle, attack, shot, death };
  const out = [];
  for (const [k, f] of Object.entries(groups)) if (!only || only.includes(k)) out.push(...f(m));
  return out;
}

/** True when every hard metric passes. */
export function passes(results) {
  return results.every((r) => !r.hard || r.pass);
}

/** A plain-text table of results. */
export function table(results) {
  const w = Math.max(...results.map((r) => r.id.length));
  return results.map((r) => `${r.pass ? "ok  " : r.hard ? "FAIL" : "warn"} ${r.id.padEnd(w)}  ${String(r.value).padEnd(28)} limit ${r.limit}${r.frame != null ? `  @${r.frame}` : ""}  ${r.note}`).join("\n");
}
