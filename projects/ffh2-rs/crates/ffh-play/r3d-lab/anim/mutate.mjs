// mutate.mjs: the validator's own test (docs/ANIMATION.md "Trusting the judge"). Each mutation
// degrades a good clip the way real animation goes wrong -- no anticipation, linear timing, a
// sliding foot, a floaty fall, a popping frame, a loosed arrow that lingers -- and names the
// metrics that must catch it. selfTest() bakes the clip and each applicable mutant: the good clip
// must pass those metrics, the mutant must fail at least one of them. A metric that no mutant
// trips, or that trips on the good clip, is not measuring what it claims.
//
// Mutations edit the clip (keys, waves, metadata) and bake it like any clip; a few that cannot be
// made in pose space (the IK clamps a stretched limb; a part's mesh cannot stretch) edit the
// baked frames instead (`post`).

import { clipOf, prepareClip, bakeClip } from "../../assets/app/r3d/anim.js";
import { check, groupOf } from "./check.mjs";

const clone = (x) => JSON.parse(JSON.stringify(x));
const raw = (def) => {
  const { tracks, ...r } = def;
  void tracks;
  return clone(r);
};
const scaleVec = (v, k) => (Array.isArray(v) ? v.map((x) => x * k) : v * k);
const POS = ["pelvis", "fl", "fr", "rHand", "lHand", "root"];
const ANG = ["lean", "twist", "roll", "headYaw", "headTilt", "rRot", "lRot", "rootRot"];

/** Adds `pose` to every key (or sets it on a new key at t). */
function addKey(d, t, pose) {
  const k = d.keys.find((x) => Math.abs(x.t - t) < 1e-6);
  if (k) Object.assign(k.pose, pose);
  else d.keys.push({ t, pose: clone(pose) });
  d.keys.sort((a, b) => a.t - b.t);
  return d;
}

/** The key time nearest t among the clip's keys. */
const keyNear = (d, t) => d.keys.reduce((a, k) => (Math.abs(k.t - t) < Math.abs(a.t - t) ? k : a), d.keys[0]);

/**
 * The catalogue: {name, kinds, archer (true: only bows, false: only melee), targets (metric ids;
 * the mutant must fail at least one), clip(d) -> edited raw clip, or post(bake) -> edited bake}.
 */
export const MUTATIONS = [
  // --- body ---
  { name: "sink", kinds: ["idle", "attack", "death"], targets: ["ground.penetration"], clip: (d) => addKey(d, d.dur * 0.5, { root: [0, -0.03, 0] }) },
  {
    name: "footSlide", kinds: ["idle", "attack"], targets: ["feet.slip", "feet.drift"],
    clip: (d) => {
      d.waves = (d.waves ?? []).concat([{ ch: "fl", amp: [0.012, 0, 0.01], n: 1 }]);
      return d;
    },
  },
  { name: "footLift", kinds: ["idle", "attack"], targets: ["ground.hover"], clip: (d) => addKey(d, d.dur * 0.5, { fl: [0, 0.02, 0] }) },
  { name: "wrongContacts", kinds: ["death"], targets: ["ground.hover"], clip: (d) => ({ ...d, contacts: { L: [[0, d.dur]], R: [[0, d.dur]] } }) },
  { name: "overreach", kinds: ["idle", "attack", "death"], targets: ["rigid.reach"], clip: (d) => addKey(d, d.dur * 0.3, { rHand: [0, 0, 0.2] }) },
  {
    name: "stretchProp", kinds: ["attack"], targets: ["rigid.props"],
    post: (b) => {
      const part = b.parts.weapon_r ?? b.parts.weapon_l;
      for (let f = Math.floor(b.frames / 3); f < Math.floor((2 * b.frames) / 3); f++) {
        const h = b.joints[f][part === b.parts.weapon_r ? "handR" : "handL"];
        for (let v = part.range[0]; v < part.range[1]; v++) {
          const q = (f * b.count + v) * 3;
          for (let k = 0; k < 3; k++) b.pos[q + k] = h[k] + (b.pos[q + k] - h[k]) * 1.15;
        }
      }
      return b;
    },
  },
  {
    name: "kneeBack", kinds: ["idle", "attack", "death"], targets: ["joints.kneeDir"],
    post: (b) => {
      // A knee bent backward (the IK cannot make one; a broken solver or a hand-keyed joint can).
      const f = Math.floor(b.frames / 2);
      const J = b.joints[f];
      const mid = J.hipL.map((x, k) => (x + J.ankleL[k]) / 2);
      J.kneeL = J.kneeL.map((x, k) => (k === 2 ? 2 * mid[2] - x : x));
      return b;
    },
  },
  { name: "elbowFold", kinds: ["attack"], targets: ["joints.elbow", "pop.spike", "pop.jitter", "rigid.reach"], clip: (d) => addKey(d, keyNear(d, d.dur * 0.6).t, { rHand: [0.07, 0.12, -0.12], rElbow: [0, 0.9, 0] }) },
  { name: "headTurn", kinds: ["idle", "attack"], targets: ["joints.neck"], clip: (d) => addKey(d, d.dur * 0.5, { headYaw: 2.0 }) },
  { name: "overLean", kinds: ["idle"], targets: ["balance", "joints.spine"], clip: (d) => ({ ...d, waves: (d.waves ?? []).concat([{ ch: "lean", amp: 0.9, n: 1 }]) }) },
  { name: "clipWeapon", kinds: ["attack"], archer: false, targets: ["clear.weapon"], clip: (d) => addKey(d, keyNear(d, (d.events?.impact ?? 0.5) * 0.6).t, { rHand: [0.07, -0.02, 0.0], rRot: [0, 0, 2.4] }) },
  // --- motion ---
  {
    name: "hitch", kinds: ["idle", "attack", "death"], targets: ["pop.trunk", "pop.spike"],
    post: (b) => {
      const f = Math.floor(b.frames * 0.55);
      for (let v = 0; v < b.count; v++) if (b.bone[v] === 1) b.pos[(f * b.count + v) * 3 + 1] += 0.015;
      const J = b.joints[f];
      for (const k of ["head", "headTop"]) J[k] = [J[k][0], J[k][1] + 0.015, J[k][2]];
      return b;
    },
  },
  { name: "jitter", kinds: ["idle", "attack"], targets: ["pop.jitter", "idle.jitter", "pop.trunk", "pop.spike"], clip: (d) => ({ ...d, waves: (d.waves ?? []).concat([{ ch: "rHand", amp: [0.004, 0.004, 0], n: Math.round(d.dur * 23) }, { ch: "lean", amp: 0.03, n: Math.round(d.dur * 17) }]) }) },
  {
    // The breath's wave a quarter cycle short of whole: the loop jumps at its seam.
    name: "loopBreak", kinds: ["idle"], targets: ["idle.seam", "pop.trunk", "pop.spike", "pop.jitter"],
    clip: (d) => {
      d.waves = (d.waves ?? []).map((w, i) => (i === 0 ? { ...w, n: (w.n ?? 1) + 0.25, amp: scaleVec(w.amp, 2) } : w));
      return d;
    },
  },
  { name: "noReturn", kinds: ["attack"], targets: ["boundary.end"], clip: (d) => ({ ...d, returns: false }) },
  {
    name: "linearTiming", kinds: ["attack"], archer: false, targets: ["attack.windup", "attack.contrast", "attack.peak", "attack.strike", "attack.stop", "pop.spike"],
    clip: (d) => {
      const n = d.keys.length;
      d.keys.forEach((k, i) => {
        k.t = (d.dur * (i + 1)) / (n + 1);
        k.i = "linear";
      });
      return d;
    },
  },
  {
    name: "noAnticipation", kinds: ["attack"], archer: false, targets: ["attack.windup", "attack.anticipation", "attack.contrast"],
    clip: (d) => {
      const ti = d.events.impact;
      d.keys = d.keys.filter((k) => k.t >= ti - 1e-6);
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
    name: "easedImpact", kinds: ["attack"], archer: false, targets: ["attack.peak", "attack.stop"],
    clip: (d) => {
      // The blow slows into the target: a key just before the impact that already holds its pose.
      const ti = d.events.impact;
      const imp = d.keys.find((k) => Math.abs(k.t - ti) < 1e-6);
      d.keys.push({ t: ti - 0.05, pose: clone(imp.pose) });
      d.keys.sort((a, b) => a.t - b.t);
      return d;
    },
  },
  {
    name: "mirrorStrike", kinds: ["attack"], archer: false, targets: ["attack.reach", "attack.aim", "attack.anticipation"],
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
  { name: "fastSwing", kinds: ["attack"], archer: false, targets: ["playback.lerp", "pop.spike", "attack.strike"], clip: (d) => ({ ...d, fps: 15 }) },
  {
    name: "freezeTorso", kinds: ["attack"], archer: false, targets: ["craft.commit"],
    clip: (d) => {
      for (const k of d.keys) for (const c of ["lean", "twist", "roll", "pelvis"]) delete k.pose[c];
      return d;
    },
  },
  // --- death ---
  {
    name: "floatyFall", kinds: ["death"], targets: ["death.accelerates", "death.gravity", "death.fallTime", "death.energy"],
    clip: (d) => {
      const t0 = d.keys[1]?.t ?? 0.2;
      for (const k of d.keys) {
        if (k.t > t0) k.t = t0 + (k.t - t0) * 1.6;
        delete k.i;
      }
      d.dur = t0 + (d.dur - t0) * 1.6;
      return d;
    },
  },
  {
    name: "propped", kinds: ["death"], targets: ["death.grounded", "death.low", "read.lying"],
    clip: (d) => {
      for (const k of d.keys) if (k.pose.rootRot) k.pose.rootRot = scaleVec(k.pose.rootRot, 0.6);
      return d;
    },
  },
  {
    name: "kneel", kinds: ["death"], targets: ["death.grounded", "death.low", "read.lying", "death.falls", "death.fallDir"],
    clip: (d) => {
      for (const k of d.keys) delete k.pose.rootRot;
      return d;
    },
  },
  { name: "twitch", kinds: ["death"], targets: ["death.settles"], clip: (d) => addKey(addKey(d, d.dur - 0.25, { lHand: [0.04, 0.04, 0.04] }), d.dur - 0.1, { lHand: [0, -0.02, 0] }) },
  { name: "wrongFall", kinds: ["death"], targets: ["death.fallDir"], clip: (d) => ({ ...d, fall: (d.fall ?? d.away).map((x, i) => (i === 1 ? x : -x)) }) },
  {
    name: "noReaction", kinds: ["death"], targets: ["death.react"],
    clip: (d) => {
      // The blow lands and nothing answers it for a while: the first keys move later.
      for (const k of d.keys) k.t += 0.25;
      d.dur += 0.25;
      for (const e of Object.keys(d.events ?? {})) if (e !== "hit") d.events[e] += 0.25;
      d.contacts = { L: (d.contacts?.L ?? []).map(([a, b]) => [a, b + 0.25]), R: (d.contacts?.R ?? []).map(([a, b]) => [a, b + 0.25]) };
      return d;
    },
  },
  // --- idle ---
  { name: "statue", kinds: ["idle"], targets: ["idle.alive"], clip: (d) => ({ ...d, waves: (d.waves ?? []).map((w) => ({ ...w, amp: scaleVec(w.amp, 0.2) })) }) },
  { name: "swim", kinds: ["idle"], targets: ["idle.calm", "idle.fromRest", "idle.tempo"], clip: (d) => ({ ...d, waves: (d.waves ?? []).map((w) => ({ ...w, amp: scaleVec(w.amp, 4) })) }) },
  { name: "pant", kinds: ["idle"], targets: ["idle.breath", "idle.tempo"], clip: (d) => ({ ...d, waves: (d.waves ?? []).map((w) => (w.ch === "lean" || (w.ch === "pelvis" && w.amp[1]) ? { ...w, n: 3 } : w)) }) },
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
        if (k.t > tr && k.t < tr + 0.25 && k.pose.string !== undefined) delete k.pose.string;
      }
      return addKey(d, tr + 0.25, { string: 0 });
    },
  },
  { name: "noAimHold", kinds: ["attack"], archer: true, targets: ["shot.aimHold"], clip: (d) => addKey(d, d.events.release - 0.04, { lHand: [0, 0.03, 0.02] }) },
  {
    name: "chestAnchor", kinds: ["attack"], archer: true, targets: ["shot.anchor", "shot.stringHand"],
    clip: (d) => {
      // Drawn to the chest, not the face: every key before the release, and from the start.
      d.keys.unshift({ t: 0.05, pose: { rHand: [0, -0.09, 0] } });
      for (const k of d.keys) if (k.t < d.events.release && k.pose.rHand) k.pose.rHand = [k.pose.rHand[0], k.pose.rHand[1] - 0.09, k.pose.rHand[2]];
      return d;
    },
  },
  { name: "cantedBow", kinds: ["attack"], archer: true, targets: ["shot.aimDir", "shot.bowArm", "shot.axis"], clip: (d) => addKey(addKey(d, 0.1, { lRot: [0, 0.5, 0] }), d.events.release - 0.01, { lRot: [0, 0.5, 0] }) },
  {
    name: "noRecoil", kinds: ["attack"], archer: true, targets: ["shot.recoil"],
    clip: (d) => {
      const tr = d.events.release;
      const before = d.keys.filter((k) => k.t <= tr && k.pose.rHand).pop()?.pose.rHand ?? [0, 0, 0];
      for (const k of d.keys) if (k.t > tr && k.t < tr + 0.25 && k.pose.rHand) k.pose.rHand = clone(before);
      return d;
    },
  },
  {
    name: "lingeringArrow", kinds: ["attack"], archer: true, targets: ["shot.release", "rigid.arrow"],
    clip: (d) => {
      for (const k of d.keys) if (k.i && typeof k.i === "object") delete k.i.arrow;
      return d;
    },
  },
];

/** The mutations that apply to a clip. */
export function applicable(def, isBow) {
  return MUTATIONS.filter((m) => m.kinds.includes(def.kind) && (m.archer === undefined || m.archer === isBow));
}

/** Bakes mutation `mu` of (recipe, clip): the mutated bake, or {error} if it cannot be made. */
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
