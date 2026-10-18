// mutate.mjs: the validator's own test (docs/ANIMATION.md "Trusting the judge"). Each mutation
// degrades a good clip the way real animation goes wrong -- no anticipation, linear timing, a
// sliding foot, a floaty fall, a popping frame, a loosed arrow that lingers -- and names the
// metrics that must catch it. selfTest() bakes the clip and each applicable mutant: the good clip
// must pass those metrics, the mutant must fail at least one of them. A metric that no mutant
// trips, or that trips on the good clip, is not measuring what it claims.
//
// A mutation must express its defect whatever way the clip was authored (a few sparse keys or a
// key every frame): it edits the clip with smooth additive `bumps`, whole-clip changes (waves,
// metadata, scaling or shifting every key), or edits the baked frames (`post`: retiming the
// motion, freezing or exaggerating it) -- never one key, which a densely keyed clip dilutes.

import { clipOf, prepareClip, bakeClip } from "../../assets/app/r3d/anim.js";
import { BONE } from "../../assets/app/r3d/mesh.js";
import { check, groupOf, model } from "./check.mjs";

const clone = (x) => JSON.parse(JSON.stringify(x));
const raw = (def) => {
  const { tracks, ...r } = def;
  void tracks;
  return clone(r);
};
const scaleVec = (v, k) => (Array.isArray(v) ? v.map((x) => x * k) : v * k);
const POS = ["pelvis", "fl", "fr", "rHand", "lHand", "root"];
const ANG = ["lean", "twist", "roll", "headYaw", "headTilt", "rRot", "lRot", "rootRot"];

/** Adds a smooth local change (anim.js bumps) to clip d. */
function bump(d, ch, t, w, v) {
  d.bumps = (d.bumps ?? []).concat([{ ch, t, w, v }]);
  return d;
}

// --- editing baked frames --------------------------------------------------------------------------

const lerp = (a, b, u) => a + (b - a) * u;
/** Interpolates two values of the same shape (numbers, arrays, objects of them). */
function mix(a, b, u) {
  if (typeof a === "number") return lerp(a, b, u);
  if (Array.isArray(a)) return a.map((x, i) => mix(x, b[i], u));
  if (a && typeof a === "object") {
    const o = {};
    for (const k of Object.keys(a)) o[k] = b && k in b ? mix(a[k], b[k], u) : a[k];
    return o;
  }
  return a;
}

/** The bake resampled on a new timeline: frame g shows the original at time tau(g) (seconds),
 *  every per-frame field interpolated between the two original frames around it. */
function retime(b, frames, tau) {
  const n = b.count, F = b.frames;
  const pos = new Float32Array(frames * n * 3), nrm = new Float32Array(frames * n * 3);
  const out = { ...b, frames, pos, nrm, joints: [], held: [], poses: [], angles: [], times: [] };
  for (let g = 0; g < frames; g++) {
    const x = Math.max(0, Math.min(F - 1, tau(g) * b.fps));
    const f0 = Math.floor(x), f1 = Math.min(F - 1, f0 + 1), u = x - f0;
    for (let i = 0; i < n * 3; i++) {
      pos[g * n * 3 + i] = lerp(b.pos[f0 * n * 3 + i], b.pos[f1 * n * 3 + i], u);
      nrm[g * n * 3 + i] = lerp(b.nrm[f0 * n * 3 + i], b.nrm[f1 * n * 3 + i], u);
    }
    out.joints.push(mix(b.joints[f0], b.joints[f1], u));
    out.held.push(mix(b.held[f0], b.held[f1], u));
    out.poses.push(mix(b.poses[f0], b.poses[f1], u));
    out.angles.push(b.angles?.[f0] ? mix(b.angles[f0], b.angles[f1], u) : null);
    out.times.push(g / b.fps);
  }
  return out;
}

/** Moves frame f's vertices of set `vs` (and its joints `js`) by fn(p) -> p. */
function edit(b, f, vs, fn, js = []) {
  for (const v of vs) {
    const q = (f * b.count + v) * 3;
    const p = fn([b.pos[q], b.pos[q + 1], b.pos[q + 2]]);
    b.pos[q] = p[0];
    b.pos[q + 1] = p[1];
    b.pos[q + 2] = p[2];
  }
  for (const k of js) b.joints[f][k] = fn(b.joints[f][k]);
}

/** Rodrigues: rotates p about point c from direction a toward direction bdir, by weight w. */
function rotateToward(p, c, a, bdir, w) {
  const na = Math.hypot(...a), nb = Math.hypot(...bdir);
  const A = a.map((x) => x / na), B = bdir.map((x) => x / nb);
  let ax = [A[1] * B[2] - A[2] * B[1], A[2] * B[0] - A[0] * B[2], A[0] * B[1] - A[1] * B[0]];
  const s = Math.hypot(...ax);
  if (s < 1e-9) return p;
  ax = ax.map((x) => x / s);
  const th = Math.atan2(s, A[0] * B[0] + A[1] * B[1] + A[2] * B[2]) * w;
  const v = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
  const cs = Math.cos(th), sn = Math.sin(th), d = ax[0] * v[0] + ax[1] * v[1] + ax[2] * v[2];
  const cr = [ax[1] * v[2] - ax[2] * v[1], ax[2] * v[0] - ax[0] * v[2], ax[0] * v[1] - ax[1] * v[0]];
  return [0, 1, 2].map((k) => c[k] + v[k] * cs + cr[k] * sn + ax[k] * d * (1 - cs));
}

const sub3 = (a, c) => [a[0] - c[0], a[1] - c[1], a[2] - c[2]];

/** The first frame a fallen body's trunk reaches the ground (as check.mjs: the body above the
 *  hips and the head, not a skirt hanging below them). */
function groundFrame(b) {
  const trunk = [];
  const hipY = Math.min(b.joints[0].hipL[1], b.joints[0].hipR[1]);
  for (let v = 0; v < b.count; v++) if ((b.bone[v] === BONE.BODY && b.pos[v * 3 + 1] >= hipY) || b.bone[v] === BONE.HEAD) trunk.push(v);
  for (let f = 3; f < b.frames; f++) {
    let lo = Infinity;
    for (const v of trunk) lo = Math.min(lo, b.pos[(f * b.count + v) * 3 + 1]);
    if (lo <= 0.012) return f;
  }
  return b.frames - 1;
}

const range = (p) => (p ? Array.from({ length: p.range[1] - p.range[0] }, (_, i) => p.range[0] + i) : []);

/** Deviations from the clip's mean pose scaled by k (0.2: a statue; 4: a swimming figure). */
function scaleMotion(b, k) {
  const n = b.count, F = b.frames;
  const mean = new Float64Array(n * 3);
  for (let f = 0; f < F; f++) for (let i = 0; i < n * 3; i++) mean[i] += b.pos[f * n * 3 + i] / F;
  for (let f = 0; f < F; f++) for (let i = 0; i < n * 3; i++) b.pos[f * n * 3 + i] = mean[i] + (b.pos[f * n * 3 + i] - mean[i]) * k;
  const jm = {};
  for (const key of Object.keys(b.joints[0])) jm[key] = [0, 1, 2].map((c) => b.joints.reduce((a, J) => a + J[key][c], 0) / F);
  for (const J of b.joints) for (const key of Object.keys(J)) J[key] = J[key].map((x, c) => jm[key][c] + (x - jm[key][c]) * k);
  return b;
}

/** Sets channel ch to v at time t (in the key at t, or a new one), with segment mode i after it. */
function setKey(d, t, ch, v, i) {
  let k = d.keys.find((x) => Math.abs(x.t - t) < 1e-6);
  if (!k) {
    k = { t, pose: {} };
    d.keys.push(k);
    d.keys.sort((a, c) => a.t - c.t);
  }
  k.pose[ch] = v;
  if (i) k.i = { ...(typeof k.i === "object" ? k.i : k.i ? { default: k.i } : {}), [ch]: i };
  return d;
}

/** The clip played k times as fast: key, bump and event times, the duration and contacts. */
function timeScale(d, k) {
  for (const x of d.keys) x.t *= k;
  for (const x of d.bumps ?? []) {
    x.t *= k;
    x.w *= k;
  }
  d.dur *= k;
  for (const e of Object.keys(d.events ?? {})) d.events[e] *= k;
  if (d.contacts) d.contacts = { L: (d.contacts.L ?? []).map(([a, c]) => [a * k, c * k]), R: (d.contacts.R ?? []).map(([a, c]) => [a * k, c * k]) };
  return d;
}

/** What the hands hold (the right hand's weapon, else the left's: a bow) and that hand's joints. */
const weaponOf = (b) => (b.parts.weapon_r ? ["weapon_r", "handR", "elbowR"] : ["weapon_l", "handL", "elbowL"]);

// --- the catalogue -----------------------------------------------------------------------------------

/**
 * {name, kinds, archer (true: only bows, false: only melee), targets (metric ids; the mutant must
 * fail at least one), clip(d) -> edited raw clip, or post(bake) -> edited bake}.
 */
export const MUTATIONS = [
  // --- body ---
  {
    // A boot skids on its planted ground: a quick 2 cm slide and back.
    name: "footSkid", kinds: ["idle", "attack", "death"], targets: ["feet.slip"],
    clip: (d) => {
      const c = (d.contacts?.L ?? [[0, d.dur]])[0];
      return bump(d, "fl", (c[0] + Math.min(c[1], d.dur)) / 2, 0.06, [0.015, 0, 0.015]);
    },
  },
  {
    // The body thrown out over its feet: the pelvis forward and the torso leaning after it.
    name: "topple", kinds: ["idle", "attack"], targets: ["balance"],
    clip: (d) => bump(bump(d, "pelvis", d.dur * 0.5, Math.min(0.4, d.dur * 0.3), [0, 0, 0.09]), "lean", d.dur * 0.5, Math.min(0.4, d.dur * 0.3), 0.5),
  },
  {
    // The body sunk into the ground over the middle of the clip (on the baked frames: a clip that
    // settles its corpse would put a sunk root straight back on the ground).
    name: "sink", kinds: ["idle", "attack", "death"], targets: ["ground.penetration"],
    post: (b) => {
      const all = Array.from({ length: b.count }, (_, v) => v);
      const f0 = Math.floor(b.frames * 0.1), f1 = Math.floor(b.frames * 0.9);
      for (let f = f0; f <= f1; f++) {
        const dy = -0.03 * Math.sin((Math.PI * (f - f0)) / Math.max(1, f1 - f0));
        edit(b, f, all, (p) => [p[0], p[1] + dy, p[2]], Object.keys(b.joints[f]));
      }
      return b;
    },
  },
  { name: "footSlide", kinds: ["idle", "attack", "death"], targets: ["feet.slip", "feet.drift"], clip: (d) => ({ ...d, waves: (d.waves ?? []).concat([{ ch: "fl", amp: [0.012, 0, 0.01], n: 1 }]) }) },
  { name: "footLift", kinds: ["idle", "attack"], targets: ["ground.hover"], clip: (d) => bump(d, "fl", d.dur * 0.5, 0.2, [0, 0.02, 0]) },
  { name: "wrongContacts", kinds: ["death"], targets: ["ground.hover"], clip: (d) => ({ ...d, contacts: { L: [[0, d.dur]], R: [[0, d.dur]] } }) },
  { name: "overreach", kinds: ["idle", "attack", "death"], targets: ["rigid.reach"], clip: (d) => bump(d, "rHand", d.dur * 0.3, 0.15, [0, 0, 0.25]) },
  {
    // The held weapon (or bow) stretched 15% about the hand over the middle third.
    name: "stretchProp", kinds: ["idle", "attack", "death"], targets: ["rigid.props"],
    post: (b) => {
      const [key, hand] = weaponOf(b);
      for (let f = Math.floor(b.frames / 3); f < Math.floor((2 * b.frames) / 3); f++) {
        const h = b.joints[f][hand];
        edit(b, f, range(b.parts[key]), (p) => p.map((x, k) => h[k] + (x - h[k]) * 1.15));
      }
      return b;
    },
  },
  {
    name: "kneeBack", kinds: ["idle", "attack", "death"], targets: ["joints.kneeDir"],
    post: (b) => {
      // A knee bent backward (the IK cannot make one; a broken solver or a hand-keyed joint can),
      // early in the clip, while the body stands.
      for (let f = 1; f <= 3; f++) {
        const J = b.joints[f];
        const mid = J.hipL.map((x, k) => (x + J.ankleL[k]) / 2);
        J.kneeL = J.kneeL.map((x, k) => (k === 2 ? 2 * mid[2] - x - 0.02 : x));
      }
      return b;
    },
  },
  {
    // An elbow folded flat: the forearm (and what the hand holds) turned about the elbow until it
    // nearly lies along the upper arm, over the middle of the clip.
    name: "elbowFold", kinds: ["idle", "attack", "death"], targets: ["joints.elbow", "rigid.reach"],
    post: (b) => {
      const arm = [];
      for (let v = 0; v < b.count; v++) if (b.bone[v] === BONE.ARM_R) arm.push(v);
      const held = b.parts.weapon_r ? range(b.parts.weapon_r) : [];
      const f0 = Math.floor(b.frames * 0.35), f1 = Math.floor(b.frames * 0.65);
      for (let f = f0; f <= f1; f++) {
        const w = 0.9 * Math.sin((Math.PI * (f - f0)) / Math.max(1, f1 - f0));
        const J = b.joints[f], e = J.elbowR, fore = sub3(J.handR, e), up = sub3(J.shoulderR, e);
        const L2 = fore[0] ** 2 + fore[1] ** 2 + fore[2] ** 2;
        const rot = (p) => rotateToward(p, e, fore, up, w);
        const distal = arm.filter((v) => {
          const q = (f * b.count + v) * 3;
          return ((b.pos[q] - e[0]) * fore[0] + (b.pos[q + 1] - e[1]) * fore[1] + (b.pos[q + 2] - e[2]) * fore[2]) / L2 > 0;
        });
        edit(b, f, distal.concat(held), rot, ["handR"]);
      }
      return b;
    },
  },
  { name: "headTurn", kinds: ["idle", "attack", "death"], targets: ["joints.neck"], clip: (d) => bump(d, "headYaw", d.dur * 0.5, 0.3, 2.0) },
  { name: "overLean", kinds: ["idle", "attack", "death"], targets: ["balance", "joints.spine"], clip: (d) => ({ ...d, waves: (d.waves ?? []).concat([{ ch: "lean", amp: 0.9, n: 1 }]) }) },
  {
    // The weapon swung through the head (turned about the hand toward it), or the bow pulled into
    // the chest, over a stretch of the clip's first half.
    name: "clipWeapon", kinds: ["idle", "attack", "death"], targets: ["clear.weapon"],
    post: (b) => {
      const [key, hand] = weaponOf(b);
      const vs = range(b.parts[key]);
      const loose = new Set([...range(b.parts.string), ...range(b.parts.arrow)]);
      const solid = vs.filter((v) => !loose.has(v));
      const f0 = Math.floor(b.frames * 0.15), f1 = Math.floor(b.frames * 0.35);
      for (let f = f0; f <= f1; f++) {
        const w = Math.sin((Math.PI * (f - f0)) / Math.max(1, f1 - f0));
        // Its far end (the axe's head, a bow's limb tip: not the drawn string's nock, already at
        // the face) turned toward the head.
        const h = b.joints[f][hand], at = (v) => [0, 1, 2].map((k) => b.pos[(f * b.count + v) * 3 + k]);
        const tip = solid.reduce((a, v) => (Math.hypot(...sub3(at(v), h)) > Math.hypot(...sub3(at(a), h)) ? v : a), solid[0]);
        if (b.parts[key].kind !== "bow") edit(b, f, vs, (p) => rotateToward(p, h, sub3(at(tip), h), sub3(b.joints[f].head, h), w));
        else {
          // A bow at arm's length cannot reach the head by turning: it is pulled into the chest.
          const J = b.joints[f], chest = [0, 1, 2].map((k) => (J.neck[k] + J.pelvis[k]) / 2);
          const c = [0, 1, 2].map((k) => solid.reduce((a, v) => a + at(v)[k], 0) / solid.length);
          const d = sub3(chest, c).map((x) => x * 0.8 * w);
          edit(b, f, vs, (p) => p.map((x, k) => x + d[k]));
        }
      }
      return b;
    },
  },
  {
    // A stretched forearm (a hand keyed past the arm, a broken chain): the hand and what it
    // holds 12% farther from the elbow over the middle of the clip.
    name: "stretchLimb", kinds: ["idle", "attack", "death"], targets: ["rigid.limbs"],
    post: (b) => {
      const arm = [];
      for (let v = 0; v < b.count; v++) if (b.bone[v] === BONE.ARM_R) arm.push(v);
      const f0 = Math.floor(b.frames * 0.3), f1 = Math.floor(b.frames * 0.7);
      for (let f = f0; f <= f1; f++) {
        const w = Math.sin((Math.PI * (f - f0)) / Math.max(1, f1 - f0));
        const J = b.joints[f], e = J.elbowR, ax = sub3(J.handR, e), L2 = ax[0] ** 2 + ax[1] ** 2 + ax[2] ** 2;
        const grow = (p) => {
          const t = Math.max(0, Math.min(1, ((p[0] - e[0]) * ax[0] + (p[1] - e[1]) * ax[1] + (p[2] - e[2]) * ax[2]) / L2));
          return p.map((x, k) => x + ax[k] * 0.12 * w * t);
        };
        const shift = ax.map((x) => x * 0.12 * w);
        edit(b, f, arm, grow);
        if (b.parts.weapon_r) edit(b, f, range(b.parts.weapon_r), (p) => p.map((x, k) => x + shift[k]));
        J.handR = J.handR.map((x, k) => x + shift[k]);
      }
      return b;
    },
  },
  // (a death's while it still stands)
  { name: "squat", kinds: ["idle", "attack", "death"], targets: ["joints.knee", "rigid.reach"], clip: (d) => (d.kind === "death" ? bump(d, "pelvis", 0.12, 0.1, [0, -0.17, 0]) : bump(d, "pelvis", d.dur * 0.5, Math.min(0.3, d.dur * 0.25), [0, -0.17, 0])) },
  {
    // A broken wrist: the weapon turned about the hand toward the forearm's line.
    name: "brokenWrist", kinds: ["idle", "attack", "death"], archer: false, targets: ["joints.wrist"],
    post: (b) => {
      const [key, hand, elbow] = weaponOf(b);
      const vs = range(b.parts[key]);
      const f0 = Math.floor(b.frames * 0.4), f1 = Math.floor(b.frames * 0.6);
      for (let f = f0; f <= f1; f++) {
        const w = 0.9 * Math.sin((Math.PI * (f - f0)) / Math.max(1, f1 - f0));
        const J = b.joints[f], h = J[hand], hd = b.held[f][key];
        const axis = sub3(hd.up, hd.at), fore = sub3(h, J[elbow]);
        const rot = (p) => rotateToward(p, h, axis, fore, w);
        edit(b, f, vs, rot);
        b.held[f][key] = { ...hd, at: rot(hd.at), up: rot(hd.up) };
      }
      return b;
    },
  },
  {
    // A weapon too small to read at the gameplay zoom (or tucked into the body).
    name: "smallWeapon", kinds: ["attack"], archer: false, targets: ["read.weapon"],
    post: (b) => {
      const [key, hand] = weaponOf(b);
      for (let f = 0; f < b.frames; f++) {
        const h = b.joints[f][hand];
        edit(b, f, range(b.parts[key]), (p) => p.map((x, k) => h[k] + (x - h[k]) * 0.3));
      }
      return b;
    },
  },
  // --- start and length ---
  {
    // The one-shot starts away from the rest pose: the idle it cuts from jumps into it.
    name: "offStart", kinds: ["attack", "death"], targets: ["boundary.start"],
    clip: (d) => bump(bump(d, "rHand", 0, 0.25, [0, 0.06, 0.06]), "lean", 0, 0.25, 0.25),
  },
  { name: "shortLoop", kinds: ["idle"], targets: ["idle.loop"], clip: (d) => timeScale(d, 0.45) },
  { name: "rushed", kinds: ["attack"], archer: false, targets: ["attack.duration"], clip: (d) => timeScale(d, 0.45) },
  {
    // After the blow the figure snaps back to rest in 0.15 s: no recovery.
    name: "snapBack", kinds: ["attack"], archer: false, targets: ["attack.recover"],
    post: (b) => {
      const t0 = b.clip.events.impact + 0.1, T = b.clip.dur, shortT = 0.15;
      const frames = Math.round((t0 + shortT) * b.fps) + 1;
      const out = retime(b, frames, (g) => {
        const t = g / b.fps;
        return t <= t0 ? t : t0 + ((t - t0) / shortT) * (T - t0);
      });
      out.clip = { ...b.clip, dur: (frames - 1) / b.fps };
      return out;
    },
  },
  // --- motion ---
  {
    name: "hitch", kinds: ["idle", "attack", "death"], targets: ["pop.trunk", "pop.spike"],
    post: (b) => {
      const f = Math.floor(b.frames * 0.55);
      const head = [];
      for (let v = 0; v < b.count; v++) if (b.bone[v] === BONE.HEAD) head.push(v);
      edit(b, f, head, (p) => [p[0], p[1] + 0.015, p[2]], ["head", "headTop"]);
      return b;
    },
  },
  { name: "jitter", kinds: ["idle", "attack", "death"], targets: ["pop.jitter", "idle.jitter", "pop.trunk", "pop.spike"], clip: (d) => ({ ...d, waves: (d.waves ?? []).concat([{ ch: "rHand", amp: [0.004, 0.004, 0], n: Math.round(d.dur * 23) }, { ch: "lean", amp: 0.03, n: Math.round(d.dur * 17) }]) }) },
  {
    // The loop's last fifth of a second dropped: it jumps at its seam.
    name: "loopBreak", kinds: ["idle"], targets: ["idle.seam", "pop.trunk", "pop.spike", "pop.jitter"],
    post: (b) => retime(b, b.frames - Math.max(2, Math.round(b.fps * 0.2)), (g) => g / b.fps),
  },
  {
    // A pose that jumps in one frame (two keys too close, a channel keyed in the wrong units):
    // the weapon arm and what it holds step 2 cm and ease back over 0.3 s, where the arm moves
    // least (a jump inside fast motion is masked by it, and the metric rightly lets it pass).
    name: "snap", kinds: ["idle", "attack", "death"], targets: ["pop.spike"],
    post: (b) => {
      const arm = [];
      for (let v = 0; v < b.count; v++) if (b.bone[v] === BONE.ARM_R || b.bone[v] === BONE.WEAPON) arm.push(v);
      const n = Math.max(3, Math.round(0.3 * b.fps));
      const step = (f) => Math.max(...arm.filter((_, i) => i % 5 === 0).map((v) => Math.hypot(...[0, 1, 2].map((k) => b.pos[(f * b.count + v) * 3 + k] - b.pos[((f - 1) * b.count + v) * 3 + k]))));
      // Not at a declared event, where the validator allows a deliberate snap.
      const ev = Object.values(b.clip.events ?? {}).map((t) => Math.round(t * b.fps));
      const free = (f) => ev.every((e) => Math.abs(f - e) > 3);
      let f0 = -1;
      for (let f = Math.floor(b.frames * 0.2); f < Math.min(b.frames - n, Math.floor(b.frames * 0.8)); f++) if (free(f) && (f0 < 0 || step(f) < step(f0))) f0 = f;
      for (let f = f0; f < Math.min(b.frames, f0 + n); f++) {
        const u = (f - f0) / n, k = 0.02 * (1 - u * u * (3 - 2 * u));
        edit(b, f, arm, (p) => [p[0] + k, p[1] + k, p[2]], ["handR", "elbowR"]);
      }
      return b;
    },
  },
  {
    // The wind-up rushed: everything before the blow's last 0.25 s plays in 40% of its time,
    // then the figure waits at the top (the blow lands on time).
    name: "rushedWindup", kinds: ["attack"], archer: false, targets: ["attack.contrast", "attack.windup"],
    post: (b) => {
      const t0 = b.clip.events.impact - 0.25;
      return retime(b, b.frames, (g) => {
        const t = g / b.fps;
        return t >= t0 ? t : Math.min(t0, t / 0.4);
      });
    },
  },
  {
    // A sideways swing: the chest turned 75 degrees off the target at the impact.
    name: "sideways", kinds: ["attack"], archer: false, targets: ["attack.facing"], clip: (d) => bump(d, "twist", d.events.impact, 0.3, 1.3),
  },
  {
    // The clip stops mid-recovery: it does not hand back to the idle's rest pose.
    name: "noReturn", kinds: ["attack"], targets: ["boundary.end"],
    post: (b) => retime(b, Math.round(b.frames * 0.75), (g) => g / b.fps),
  },
  {
    // Constant speed along the blow's path from start to end: no anticipation hold, no snap.
    name: "linearTiming", kinds: ["attack"], archer: false, targets: ["attack.windup", "attack.contrast", "attack.peak", "attack.strike", "attack.stop", "pop.spike"],
    post: (b) => {
      const m = model(b);
      const s = [0];
      for (let f = 1; f < b.frames; f++) s.push(s[f - 1] + Math.hypot(...[0, 1, 2].map((k) => m.strikes[f][k] - m.strikes[f - 1][k])));
      const total = s[b.frames - 1];
      return retime(b, b.frames, (g) => {
        const want = (total * g) / (b.frames - 1);
        let f = 0;
        while (f + 1 < b.frames - 1 && s[f + 1] < want) f++;
        const u = (want - s[f]) / Math.max(1e-9, s[f + 1] - s[f]);
        return (f + Math.max(0, Math.min(1, u))) / b.fps;
      });
    },
  },
  {
    name: "noAnticipation", kinds: ["attack"], archer: false, targets: ["attack.windup", "attack.anticipation", "attack.contrast"],
    clip: (d) => {
      const ti = d.events.impact;
      d.keys = d.keys.filter((k) => k.t >= ti - 1e-6);
      d.bumps = (d.bumps ?? []).filter((k) => k.t >= ti);
      return d;
    },
  },
  {
    name: "slowStrike", kinds: ["attack"], archer: false, targets: ["attack.strike", "attack.peak", "attack.power"],
    clip: (d) => {
      const ti = d.events.impact;
      for (const k of d.keys) if (k.t >= ti - 1e-6) k.t += 0.2;
      d.events = { ...d.events, impact: ti + 0.2 };
      d.dur += 0.2;
      return d;
    },
  },
  {
    // The blow slows to a stop as it reaches the target (eased into it): the 0.12 s before the
    // impact retimed to decelerate.
    name: "easedImpact", kinds: ["attack"], archer: false, targets: ["attack.peak", "attack.stop"],
    post: (b) => {
      const ti = b.clip.events.impact, w = 0.12;
      return retime(b, b.frames, (g) => {
        const t = g / b.fps;
        if (t < ti - w || t > ti) return t;
        const u = (t - (ti - w)) / w;
        return ti - w + w * (1 - (1 - u) * (1 - u) * (1 - u));
      });
    },
  },
  {
    name: "mirrorStrike", kinds: ["attack"], archer: false, targets: ["attack.reach", "attack.aim", "attack.anticipation", "attack.facing"],
    clip: (d) => {
      for (const k of d.keys) {
        for (const c of ["rHand", "lHand", "pelvis"]) if (k.pose[c]) k.pose[c][2] = -k.pose[c][2];
        if (k.pose.lean !== undefined) k.pose.lean = -k.pose.lean;
        if (k.pose.rRot) k.pose.rRot = [-k.pose.rRot[0] - 2.6, k.pose.rRot[1], k.pose.rRot[2]];
      }
      return d;
    },
  },
  {
    name: "timid", kinds: ["attack"], archer: false, targets: ["read.travel", "read.keys", "attack.power", "attack.reach"],
    clip: (d) => {
      for (const k of d.keys) for (const c of [...POS, ...ANG]) if (k.pose[c] !== undefined) k.pose[c] = scaleVec(k.pose[c], 0.3);
      return d;
    },
  },
  { name: "fastSwing", kinds: ["attack", "death"], targets: ["playback.lerp", "pop.spike", "attack.strike"], clip: (d) => ({ ...d, fps: 10 }) },
  {
    name: "freezeTorso", kinds: ["attack"], archer: false, targets: ["craft.commit"],
    clip: (d) => {
      for (const k of d.keys) for (const c of ["lean", "twist", "roll", "pelvis"]) delete k.pose[c];
      return d;
    },
  },
  // --- death ---
  {
    // The fall lowered gently: from its onset to the ground it takes 2.5 times as long and eases
    // to rest on the ground (a body put down, not falling).
    name: "floatyFall", kinds: ["death"], targets: ["death.accelerates", "death.gravity", "death.fallTime", "death.energy", "death.groundEvent"],
    post: (b) => {
      const m = model(b);
      const vy = m.coms.map((c, k) => (k === 0 ? 0 : (c[1] - m.coms[k - 1][1]) * b.fps));
      let ts = 3;
      while (ts < b.frames - 1 && vy[ts] > -0.1) ts++;
      const ground = Math.round((b.clip.events?.ground ?? b.clip.dur * 0.5) * b.fps);
      const ti = Math.max(ts + 2, ground);
      const L = ti - ts, add = Math.round(1.5 * L);
      const out = retime(b, b.frames + add, (g) => {
        if (g <= ts) return g / b.fps;
        if (g >= ti + add) return (g - add) / b.fps;
        const u = (g - ts) / (L + add);
        return (ts + L * (u * u * (3 - 2 * u))) / b.fps;
      });
      out.clip = { ...b.clip, dur: b.clip.dur + add / b.fps };
      return out;
    },
  },
  {
    // A slow-motion fall: from its onset to the ground played at a third of the speed.
    name: "slowFall", kinds: ["death"], targets: ["death.fallTime", "death.gravity"],
    post: (b) => {
      const m = model(b);
      const vy = m.coms.map((c, k) => (k === 0 ? 0 : (c[1] - m.coms[k - 1][1]) * b.fps));
      let ts = 3;
      while (ts < b.frames - 1 && vy[ts] > -0.1) ts++;
      const ti = Math.max(ts + 2, groundFrame(b));
      const add = 2 * (ti - ts);
      const out = retime(b, b.frames + add, (g) => (g <= ts ? g : g >= ti + add ? g - add : ts + (g - ts) / 3) / b.fps);
      out.clip = { ...b.clip, dur: b.clip.dur + add / b.fps };
      return out;
    },
  },
  {
    // Propped up: from the landing the upper body turns back toward upright about the pelvis
    // (a body sitting against something, not lying).
    name: "sitUp", kinds: ["death"], targets: ["death.low", "read.lying"],
    post: (b) => {
      const kg = groundFrame(b), ramp = Math.round(0.25 * b.fps);
      const upper = [];
      for (let v = 0; v < b.count; v++) if (![BONE.LEG_L, BONE.LEG_R, BONE.TAIL].includes(b.bone[v])) upper.push(v);
      const js = ["head", "headTop", "neck", "shoulderL", "shoulderR", "elbowL", "elbowR", "handL", "handR"];
      for (let f = kg; f < b.frames; f++) {
        const u = Math.min(1, (f - kg) / ramp), w = 0.6 * u * u * (3 - 2 * u);
        const J = b.joints[f], c = J.pelvis.slice(), axis = sub3(J.head, c);
        edit(b, f, upper, (p) => rotateToward(p, c, axis, [0, 1, 0], w), js.filter((k) => J[k]));
      }
      return b;
    },
  },
  {
    // The corpse lands, then floats up off the ground.
    name: "rise", kinds: ["death"], targets: ["death.grounded", "death.bounce"],
    post: (b) => {
      const kg = groundFrame(b), ramp = Math.round(0.3 * b.fps);
      const all = Array.from({ length: b.count }, (_, v) => v);
      for (let f = kg + 1; f < b.frames; f++) {
        const u = Math.min(1, (f - kg) / ramp), dy = 0.03 * u * u * (3 - 2 * u);
        edit(b, f, all, (p) => [p[0], p[1] + dy, p[2]], Object.keys(b.joints[f]));
      }
      return b;
    },
  },
  {
    name: "kneel", kinds: ["death"], targets: ["death.grounded", "death.low", "read.lying", "death.falls", "death.fallDir"],
    clip: (d) => {
      for (const k of d.keys) {
        delete k.pose.rootRot;
        delete k.pose.settle;
      }
      return d;
    },
  },
  { name: "twitch", kinds: ["death"], targets: ["death.settles"], clip: (d) => bump(d, "lHand", d.dur - 0.2, 0.1, [0.04, 0.05, 0.04]) },
  { name: "wrongFall", kinds: ["death"], targets: ["death.fallDir"], clip: (d) => ({ ...d, fall: (d.fall ?? d.away).map((x, i) => (i === 1 ? x : -x)) }) },
  {
    name: "noReaction", kinds: ["death"], targets: ["death.react"],
    clip: (d) => {
      // The blow lands and nothing answers it for a while: the whole clip moves later.
      const sh = 0.25;
      for (const k of d.keys) k.t += sh;
      for (const k of d.bumps ?? []) k.t += sh;
      d.dur += sh;
      for (const e of Object.keys(d.events ?? {})) if (e !== "hit") d.events[e] += sh;
      d.contacts = { L: (d.contacts?.L ?? []).map(([a, c]) => [a, c + sh]), R: (d.contacts?.R ?? []).map(([a, c]) => [a, c + sh]) };
      return d;
    },
  },
  // --- idle ---
  {
    // An agitated idle: twice the motion, played three times over in the loop's length.
    name: "hurried", kinds: ["idle"], targets: ["idle.tempo", "idle.breath"],
    post: (b) => scaleMotion(retime(b, b.frames, (g) => ((3 * g) / b.fps) % b.clip.dur), 2),
  },
  { name: "statue", kinds: ["idle"], targets: ["idle.alive"], post: (b) => scaleMotion(b, 0.2) },
  { name: "swim", kinds: ["idle"], targets: ["idle.calm", "idle.fromRest", "idle.tempo"], post: (b) => scaleMotion(b, 4) },
  { name: "pant", kinds: ["idle"], targets: ["idle.breath", "idle.tempo"], clip: (d) => ({ ...d, waves: (d.waves ?? []).map((w) => (w.ch === "lean" || (w.ch === "pelvis" && Array.isArray(w.amp) && w.amp[1]) ? { ...w, n: 3 } : w)) }) },
  { name: "lockstep", kinds: ["idle"], targets: ["craft.rhythm"], clip: (d) => ({ ...d, waves: (d.waves ?? []).map((w) => ({ ...w, n: 2, phase: 0 })) }) },
  // --- archer ---
  {
    name: "softRelease", kinds: ["attack"], archer: true, targets: ["shot.snap", "shot.release"],
    clip: (d) => {
      const tr = d.events.release;
      for (const k of d.keys) {
        if (k.i && typeof k.i === "object") {
          delete k.i.string;
          delete k.i.arrow;
        }
        if (k.t > tr && k.t < tr + 0.3 && k.pose.string !== undefined) delete k.pose.string;
      }
      const at = d.keys.find((k) => Math.abs(k.t - (tr + 0.3)) < 1e-6);
      if (at) at.pose.string = 0;
      else d.keys.push({ t: tr + 0.3, pose: { string: 0 } });
      d.keys.sort((a, c) => a.t - c.t);
      return d;
    },
  },
  { name: "noAimHold", kinds: ["attack"], archer: true, targets: ["shot.aimHold"], clip: (d) => bump(d, "lHand", d.events.release - 0.03, 0.06, [0, 0.03, 0.02]) },
  { name: "chestAnchor", kinds: ["attack"], archer: true, targets: ["shot.anchor", "shot.stringHand"], clip: (d) => bump(d, "rHand", d.events.release - 0.04, d.events.release, [0, -0.09, 0]) },
  { name: "cantedBow", kinds: ["attack"], archer: true, targets: ["shot.aimDir", "shot.bowArm", "shot.axis"], clip: (d) => bump(d, "lRot", d.events.release, 0.3, [0, 0.6, 0]) },
  {
    // The drawing hand stays where it let go: no recoil (its arm frozen after the release).
    name: "noRecoil", kinds: ["attack"], archer: true, targets: ["shot.recoil"],
    post: (b) => {
      const kr = Math.round(b.clip.events.release * b.fps);
      const arm = [];
      for (let v = 0; v < b.count; v++) if (b.bone[v] === BONE.ARM_R) arm.push(v);
      for (let f = kr; f < Math.min(b.frames, kr + Math.round(0.3 * b.fps)); f++) {
        for (const v of arm) for (let k = 0; k < 3; k++) b.pos[(f * b.count + v) * 3 + k] = b.pos[((kr - 1) * b.count + v) * 3 + k];
        for (const j of ["handR", "elbowR"]) b.joints[f][j] = b.joints[kr - 1][j].slice();
      }
      return b;
    },
  },
  {
    // The nocked arrow is still drawn after the release, beside the flying one.
    name: "lingeringArrow", kinds: ["attack"], archer: true, targets: ["shot.oneArrow", "rigid.arrow"],
    post: (b) => {
      const kr = Math.round(b.clip.events.release * b.fps);
      const vs = range(b.parts.arrow);
      for (let f = kr; f < Math.min(b.frames, kr + Math.round(0.1 * b.fps)); f++) {
        for (const v of vs) for (let k = 0; k < 3; k++) b.pos[(f * b.count + v) * 3 + k] = b.pos[((kr - 1) * b.count + v) * 3 + k];
      }
      return b;
    },
  },
  {
    // Overdrawn: the drawing hand pulls the nock back until the arrow's tip is behind the bow.
    name: "overdraw", kinds: ["attack"], archer: true, targets: ["shot.tipAhead"],
    clip: (d) => {
      const s0 = d.strike ?? [0, 0, 1], n = Math.hypot(...s0);
      return bump(d, "rHand", d.events.release - 0.12, 0.12, s0.map((x) => (-0.06 * x) / n));
    },
  },
  {
    // Loosed half drawn: the drawing hand comes forward toward the bow before the release.
    name: "underdraw", kinds: ["attack"], archer: true, targets: ["shot.fullDraw", "shot.anchor"],
    clip: (d) => {
      const s0 = d.strike ?? [0, 0, 1], n = Math.hypot(...s0);
      return bump(d, "rHand", d.events.release - 0.02, 0.3, s0.map((x) => (0.1 * x) / n));
    },
  },
  {
    // The nocked arrow fades out over 0.1 s instead of leaving in one frame.
    name: "fadeArrow", kinds: ["attack"], archer: true, targets: ["shot.release", "rigid.arrow"],
    clip: (d) => {
      const tr = d.events.release;
      for (const k of d.keys) if (k.t > tr - 0.1 && k.t < tr + 0.1 && k.pose.arrow !== undefined) delete k.pose.arrow;
      setKey(d, tr - 0.05, "arrow", 1, "linear");
      return setKey(d, tr + 0.05, "arrow", 0, "hold");
    },
  },
  {
    // The nocked arrow part-shrunk (a mis-keyed `arrow` channel) for several frames while it is
    // nocked.
    name: "shrinkArrow", kinds: ["idle", "attack", "death"], archer: true, targets: ["rigid.arrow"],
    post: (b) => {
      const vs = range(b.parts.arrow);
      const ext = (f) => {
        const c = [0, 1, 2].map((k) => vs.reduce((a, v) => a + b.pos[(f * b.count + v) * 3 + k], 0) / vs.length);
        return Math.max(...vs.map((v) => Math.hypot(...[0, 1, 2].map((k) => b.pos[(f * b.count + v) * 3 + k] - c[k]))));
      };
      const e0 = ext(0);
      let n = 0;
      while (n < b.frames && ext(n) > 0.5 * e0) n++; // the first run of frames with the arrow whole
      const f0 = Math.floor(n * 0.2), f1 = Math.max(f0 + 5, Math.floor(n * 0.8));
      for (let f = f0; f < Math.min(f1, n); f++) {
        const c = [0, 1, 2].map((k) => vs.reduce((a, v) => a + b.pos[(f * b.count + v) * 3 + k], 0) / vs.length);
        edit(b, f, vs, (p) => p.map((x, k) => c[k] + (x - c[k]) * 0.6));
      }
      return b;
    },
  },
];

/** Metrics of the runtime rather than the clip (the projectile's speed and model): a clip
 *  mutation cannot break them, so no mutant targets them. */
export const RUNTIME = ["shot.flight", "shot.visible"];

/** The mutations that apply to a clip. */
export function applicable(def, isBow) {
  return MUTATIONS.filter((m) => m.kinds.includes(def.kind) && (m.archer === undefined || m.archer === isBow));
}

/** Bakes mutation `mu` of (recipe, clip): the mutated bake. */
export function bakeMutant(recipe, clipName, mu, { param = "human", variant = 0 } = {}) {
  const def = clipOf(recipe, clipName);
  if (mu.post) return mu.post(bakeClip(recipe, variant, param, clipName));
  const edited = prepareClip(mu.clip(raw(def)));
  return bakeClip(recipe, variant, param, clipName, { clip: edited });
}

/**
 * The self-test of (recipe, clip): [{name, ok, note, bake}]. The first row is the good clip (ok
 * when every hard metric passes); then each applicable mutation (ok when one of its targets fails
 * while it passed on the good clip). `collateral` counts other metrics the mutant broke.
 */
export function selfTest(recipe, clipName, { param = "human", variant = 0 } = {}) {
  const def = clipOf(recipe, clipName);
  const good = bakeClip(recipe, variant, param, clipName);
  const isBow = (good.parts.weapon_l ?? good.parts.weapon_r)?.kind === "bow";
  const base = check(good);
  const byId = new Map(base.map((r) => [r.id, r]));
  const hardFails = base.filter((r) => r.hard && !r.pass).map((r) => r.id);
  const out = [{ name: "(good clip)", ok: hardFails.length === 0, note: hardFails.length ? `fails ${hardFails.join(", ")}` : "passes every hard metric", bake: null }];
  for (const mu of applicable(def, isBow)) {
    let b;
    try {
      b = bakeMutant(recipe, clipName, mu, { param, variant });
    } catch (e) {
      out.push({ name: mu.name, ok: false, note: `could not bake: ${e.message}`, bake: null });
      continue;
    }
    // Only the groups that compute the targets (the good clip was checked in full).
    const res = check(b, { only: [...new Set(mu.targets.map(groupOf).filter(Boolean))] });
    const got = new Map(res.map((r) => [r.id, r]));
    const caught = mu.targets.filter((id) => got.get(id) && !got.get(id).pass);
    const cleanBefore = mu.targets.filter((id) => byId.get(id)?.pass !== false);
    const collateral = res.filter((r) => !r.pass && byId.get(r.id)?.pass && !mu.targets.includes(r.id)).map((r) => r.id);
    const ok = caught.length > 0 && cleanBefore.length > 0;
    out.push({
      name: mu.name,
      ok,
      note: ok ? `caught by ${caught.join(", ")}${collateral.length ? `; also ${collateral.join(", ")}` : ""}` : caught.length ? `targets already fail on the good clip (${mu.targets.join(", ")})` : `NOT caught (targets ${mu.targets.join(", ")}: ${mu.targets.map((id) => `${id} ${got.get(id)?.value ?? "absent"}`).join("; ")})`,
      caught,
      collateral,
      bake: b,
    });
  }
  return out;
}
