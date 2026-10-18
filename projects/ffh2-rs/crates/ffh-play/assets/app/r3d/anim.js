// anim.js: unit animation for the 3D renderer (docs/ANIMATION.md). A clip is keyframes of POSE
// DELTAS from a figure's rest pose (the channels of units.js figure() `o.pose`), interpolated per
// channel, sampled at FPS and REBUILT by the figure's own recipe each frame (registry.js
// buildPosed): the generator re-solves its leg and arm IK, re-hangs cloth from the moved torso and
// re-bends the bow string, so every frame is the same model as the static one, vertex for vertex.
// The baked frames (positions and normals per vertex) are what the renderer plays back and what
// the validator (anim-check.js) measures.
//
// A clip: {dur (seconds), loop, returns (ends at the rest pose), keys: [{t, pose, i}], waves,
//          events: {name: t}, contacts: {L, R}: [[t0, t1], ...] planted intervals,
//          strike: [x, y, z] direction of an attack in figure space (turned toward the target),
//          away: [x, y, z] the direction a death is thrown (turned away from the attacker),
//          fall: [x, y, z] where a death's head ends up (turned across the screen, deathYaw),
//          fps (the renderer's bake rate; default FPS), kind: "idle" | "attack" | "death"}
//
// Channels (units.js figure() `pose`): vectors pelvis, fl, fr, rHand, lHand ([dx, dy, dz], a
// human's plots), rElbow, lElbow (added to the elbow poles), rRot, lRot (Euler turns of what the
// hand holds), root ([dx, dy, dz]), rootRot (Euler turn of the whole figure about `pivot`);
// scalars lean, twist, roll, headYaw, headTilt (radians), arrow (1 nocked .. 0 loosed), string
// (1 at the drawing hand .. 0 back at the bow), settle (0 .. 1: a fallen body laid on the ground,
// its lowest point resting on it and its tail or cape flat).
//
// Tracks: a channel's track is the keys that set it. Before its first key it holds the rest
// value (or the first key's value when that key is at t = 0); a looping clip ends where it began
// and a `returns` clip ends at rest. Between keys the interpolation is monotone cubic
// (Fritsch-Carlson): it never overshoots a key, eases to rest at a turning point (an extreme pose
// is a held instant) and keeps moving through a passing key. A key's `i` sets the segment after
// it: "hold" (stay until the next key), "linear", "in" (accelerating into the next key: a fall),
// "out" (decelerating from it), default smooth; an object sets it per channel ({arrow: "hold",
// default: "in"}). `bumps` add a smooth local change on top, whatever the keys: {ch, t (centre),
// w (half-width, s), v (a value of the channel's shape)}, a raised cosine. `waves` add sines:
// {ch, amp (a value of the channel's shape), n (whole cycles per clip, so a loop is seamless),
// phase (0..1)}.

import { buildPosed } from "./assets/registry.js";

export const FPS = 30;

const VEC = ["pelvis", "fl", "fr", "rHand", "lHand", "rElbow", "lElbow", "rRot", "lRot", "root", "rootRot"];
const SCALAR = ["lean", "twist", "roll", "headYaw", "headTilt", "arrow", "string", "settle"];
const REST = { arrow: 1, string: 1 };
const restOf = (ch) => (VEC.includes(ch) ? [0, 0, 0] : REST[ch] ?? 0);

const CLIPS = new Map();

/** Registers the clips of recipe `recipe` ({name: clip}); later calls add or replace clips. */
export function clips(recipe, defs) {
  const m = CLIPS.get(recipe) ?? {};
  for (const [name, def] of Object.entries(defs)) m[name] = prepare(def);
  CLIPS.set(recipe, m);
}

export function clipOf(recipe, name) {
  return CLIPS.get(recipe)?.[name] ?? null;
}

export function clipNames(recipe) {
  return Object.keys(CLIPS.get(recipe) ?? {});
}

export function animatedRecipes() {
  return [...CLIPS.keys()].sort();
}

/** A clip ready to sample: `def` with its channels' tracks ({ch: {pts: [{t, v, i}], m}}, the
 *  implicit start and end values added). clips() prepares what it registers; the mutation tests
 *  prepare edited copies. */
export function prepareClip(def) {
  const { tracks: _old, ...raw } = def;
  void _old;
  return prepare(raw);
}

function prepare(def) {
  const dur = def.dur;
  const tracks = {};
  for (const ch of [...VEC, ...SCALAR]) {
    const pts = [];
    for (const k of def.keys ?? []) {
      if (k.pose && k.pose[ch] !== undefined) pts.push({ t: k.t, v: [].concat(k.pose[ch]), i: (k.i && typeof k.i === "object" ? k.i[ch] ?? k.i.default : k.i) ?? "smooth" });
    }
    if (!pts.length) continue;
    pts.sort((a, b) => a.t - b.t);
    for (let k = 1; k < pts.length; k++) if (pts[k].t - pts[k - 1].t < 1e-6) throw new Error(`anim: two keys set ${ch} at t = ${pts[k].t}`);
    if (pts[0].t > 1e-9) pts.unshift({ t: 0, v: [].concat(restOf(ch)), i: "smooth" });
    const last = pts[pts.length - 1];
    if (def.loop && last.t < dur - 1e-9) pts.push({ t: dur, v: pts[0].v.slice(), i: "smooth" });
    else if (def.returns && last.t < dur - 1e-9) pts.push({ t: dur, v: [].concat(restOf(ch)), i: "smooth" });
    // Tangents per component, periodic when a loop's track spans the whole clip.
    const periodic = !!def.loop && pts[0].t < 1e-9 && Math.abs(pts[pts.length - 1].t - dur) < 1e-9;
    const ts = pts.map((p) => p.t);
    const m = pts[0].v.map((_, j) => tangents(ts, pts.map((p) => p.v[j]), periodic));
    tracks[ch] = { pts, m };
  }
  return { ...def, tracks, kind: def.kind ?? "idle" };
}

/** Monotone cubic tangents of one component (PCHIP, Fritsch-Carlson with Moler's weights):
 *  zero at a turning point and at the ends of a clip that does not loop; periodic for a loop. */
function tangents(ts, vs, loop) {
  const n = ts.length;
  const h = [], d = [];
  for (let k = 0; k + 1 < n; k++) {
    h.push(Math.max(1e-9, ts[k + 1] - ts[k]));
    d.push((vs[k + 1] - vs[k]) / h[k]);
  }
  const blend = (h0, d0, h1, d1) => {
    if (d0 * d1 <= 0) return 0;
    const w1 = 2 * h1 + h0, w2 = h1 + 2 * h0;
    return (w1 + w2) / (w1 / d0 + w2 / d1);
  };
  const m = new Array(n).fill(0);
  for (let k = 1; k + 1 < n; k++) m[k] = blend(h[k - 1], d[k - 1], h[k], d[k]);
  if (loop && n > 2) m[0] = m[n - 1] = blend(h[n - 2], d[n - 2], h[0], d[0]);
  return m;
}

function sampleTrack({ pts, m }, t) {
  if (t <= pts[0].t) return pts[0].v.slice();
  const last = pts[pts.length - 1];
  if (t >= last.t) return last.v.slice();
  let k = 0;
  while (k + 1 < pts.length && pts[k + 1].t <= t) k++;
  const a = pts[k], b = pts[k + 1];
  const h = b.t - a.t;
  const u = (t - a.t) / h;
  if (a.i === "hold") return a.v.slice();
  if (a.i === "linear") return a.v.map((x, j) => x + (b.v[j] - x) * u);
  // "in": from rest, speeding up into the next key (a fall: fastest as it lands); "out": leaving
  // the key at speed and slowing into the next.
  if (a.i === "in") return a.v.map((x, j) => x + (b.v[j] - x) * u * u);
  if (a.i === "out") return a.v.map((x, j) => x + (b.v[j] - x) * u * (2 - u));
  const h00 = 2 * u * u * u - 3 * u * u + 1, h10 = u * u * u - 2 * u * u + u, h01 = -2 * u * u * u + 3 * u * u, h11 = u * u * u - u * u;
  return a.v.map((x, j) => h00 * x + h10 * h * m[j][k] + h01 * b.v[j] + h11 * h * m[j][k + 1]);
}

/** The pose of `clip` at time t (seconds): {channel: value} with only the clip's channels. */
export function poseAt(clip, t) {
  const pose = {};
  for (const [ch, pts] of Object.entries(clip.tracks)) {
    const v = sampleTrack(pts, t);
    pose[ch] = VEC.includes(ch) ? v : v[0];
  }
  for (const k of clip.bumps ?? []) {
    const d = Math.abs(t - k.t);
    if (d >= k.w) continue;
    const s = 0.5 + 0.5 * Math.cos((Math.PI * d) / k.w);
    const v = [].concat(k.v);
    const cur = pose[k.ch] !== undefined ? [].concat(pose[k.ch]) : [].concat(restOf(k.ch));
    const out = cur.map((x, j) => x + v[j] * s);
    pose[k.ch] = VEC.includes(k.ch) ? out : out[0];
  }
  for (const w of clip.waves ?? []) {
    const s = Math.sin(2 * Math.PI * ((w.n ?? 1) * (t / clip.dur) + (w.phase ?? 0)));
    const amp = [].concat(w.amp);
    const cur = pose[w.ch] !== undefined ? [].concat(pose[w.ch]) : [].concat(restOf(w.ch));
    const v = cur.map((x, j) => x + amp[j] * s);
    pose[w.ch] = VEC.includes(w.ch) ? v : v[0];
  }
  if (clip.pivot) pose.pivot = clip.pivot;
  return pose;
}

/** The number of frames of a clip at `fps` (a loop's last frame is its first, so not stored). */
export function frameCount(clip, fps = FPS) {
  const n = Math.max(1, Math.round(clip.dur * fps));
  return clip.loop ? n : n + 1;
}

/** Transforms point p by a column-major 4x4 matrix. */
function xpm(m, p) {
  return [m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12], m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13], m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14]];
}

/** Points of interest of what the hands hold, in figure space: per part {at, dir, face} and, for
 *  a bow, the grip, the nock (string centre), the arrow's tip and the limbs' ends. */
function heldPoints(meta) {
  const out = {};
  const root = meta.root;
  for (const [k, part] of Object.entries(meta.parts ?? {})) {
    if (!part.frame) continue;
    const m = part.frame.m;
    const W = (p) => (root ? xpm(root, xpm(m, p)) : xpm(m, p));
    const o = { at: W([0, 0, 0]), up: W([0, 1, 0]) };
    if (part.info?.nock) Object.assign(o, { grip: W(part.info.grip), nock: W(part.info.nock), top: W(part.info.top), bot: W(part.info.bot) });
    if (part.info?.tip) o.tip = W(part.info.tip);
    out[k] = o;
  }
  return out;
}

/**
 * Bakes clip `clipName` of (recipe, variant, param): every frame rebuilt by the recipe.
 * Returns {recipe, variant, param, clipName, clip, fps, frames, count, pos, nrm (Float32Array
 * frames * count * 3), joints[frame], held[frame], poses[frame], times[frame], bone, mat
 * (Uint8Array count), parts (vertex ranges), rest (the zero-pose build), height (rest head top)}.
 * Throws when a frame's topology differs from the rest mesh (vertex count, bone or material).
 * `fps` defaults to the clip's own (`fps`, else FPS): what the renderer plays is what is checked.
 */
export function bakeClip(recipe, variant, param, clipName, opts = {}) {
  const it = bakeSteps(recipe, variant, param, clipName, opts);
  for (;;) {
    const r = it.next();
    if (r.done) return r.value;
  }
}

/** bakeClip as a generator that yields after each frame (the renderer bakes a few frames per
 *  animation frame, so baking never stalls the map). */
export function* bakeSteps(recipe, variant, param, clipName, { fps = null, clip = null, lean = false } = {}) {
  const def = clip ?? clipOf(recipe, clipName);
  if (!def) throw new Error(`anim: no clip "${clipName}" for ${recipe}`);
  fps ??= def.fps ?? FPS;
  const rest = buildPosed(recipe, variant, param, {});
  const count = rest.count;
  const bone = new Uint8Array(count), mat = new Uint8Array(count);
  for (let v = 0; v < count; v++) {
    mat[v] = rest.data[v * 13 + 9];
    bone[v] = rest.data[v * 13 + 11];
  }
  // A nocked arrow keeps the length it has at rest (it slides with the string).
  const bowPart = Object.values(rest.meta?.parts ?? {}).find((p) => p.info?.arrowLen);
  const fixed = bowPart ? { arrowLen: bowPart.info.arrowLen } : {};
  const frames = frameCount(def, fps);
  // A loop's frames divide its duration evenly, so the seam closes exactly: its rate is adjusted
  // to frames / dur (the shader reads the rate from the packed clip).
  if (def.loop) fps = frames / def.dur;
  const pos = new Float32Array(frames * count * 3), nrm = new Float32Array(frames * count * 3);
  const joints = [], held = [], poses = [], times = [], angles = [];
  for (let f = 0; f < frames; f++) {
    const t = def.loop ? (f * def.dur) / frames : Math.min(def.dur, f / fps);
    const pose = { ...fixed, ...poseAt(def, t) };
    const b = buildPosed(recipe, variant, param, pose);
    if (b.count !== count) throw new Error(`anim: ${recipe} ${clipName} frame ${f}: ${b.count} vertices, rest has ${count}`);
    for (let v = 0; v < count; v++) {
      const o = v * 13;
      if (b.data[o + 9] !== mat[v] || b.data[o + 11] !== bone[v]) throw new Error(`anim: ${recipe} ${clipName} frame ${f}: vertex ${v} changed material or bone`);
      const q = (f * count + v) * 3;
      pos[q] = b.data[o];
      pos[q + 1] = b.data[o + 1];
      pos[q + 2] = b.data[o + 2];
      nrm[q] = b.data[o + 3];
      nrm[q + 1] = b.data[o + 4];
      nrm[q + 2] = b.data[o + 5];
    }
    joints.push(b.meta?.joints ?? {});
    held.push(b.meta ? heldPoints(b.meta) : {});
    if (!lean) {
      poses.push(pose);
      angles.push(b.meta?.angles ?? null);
    }
    times.push(t);
    yield f;
  }
  const parts = {};
  for (const [k, p] of Object.entries(rest.meta?.parts ?? {})) {
    parts[k] = { range: p.range, kind: p.kind };
    if (p.info?.arrow) parts.arrow = { range: p.info.arrow };
    if (p.info?.string) parts.string = { range: p.info.string };
  }
  return { recipe, variant, param, clipName, clip: def, fps, frames, count, pos, nrm, joints, held, poses, angles, times, bone, mat, parts, rest, height: rest.meta?.joints?.headTop?.[1] ?? rest.hi[1] };
}

// --- facing ----------------------------------------------------------------------------------------
// A figure faces +z (the camera at yaw 0); an instance's yaw turns it (shaders.js rotY), so a
// figure-space direction at angle a = atan2(x, z) points at world heading yaw + a (heading 0 is
// toward the camera, +pi/2 east). The renderer and the validator use these same rules.

const wrap = (a) => Math.atan2(Math.sin(a), Math.cos(a));
const deg = Math.PI / 180;
/** How far a figure may turn from facing the camera while it fights. Walking keeps within 100
 *  degrees (a figure never walks showing only its back); a blow or a shot goes where its target
 *  is, even if that shows the back for a moment. */
export const FIGHT_TURN = 180 * deg;

/** The yaw that points attack clip `clip`'s `strike` direction at world heading `heading`. */
export function attackYaw(clip, heading) {
  const d = clip.strike ?? [0, 0, 1];
  const yaw = wrap(heading - Math.atan2(d[0], d[2]));
  return Math.max(-FIGHT_TURN, Math.min(FIGHT_TURN, yaw));
}

/** The yaw of a death thrown toward world heading `away`: within 90 degrees of throwing the body
 *  straight away (it never falls toward its killer), the one whose fall (the clip's `fall`, where
 *  the head goes) runs most across the screen -- a body lying toward or away from the camera
 *  keeps a standing outline at the map's pitch. Ties keep the straightest throw. */
export function deathYaw(clip, away) {
  const a = clip.away ?? [0, 0, -1], f = clip.fall ?? a;
  const base = wrap(away - Math.atan2(a[0], a[2]));
  const fallOwn = Math.atan2(f[0], f[2]);
  let best = base, score = -1;
  for (let k = 0; k <= 180; k++) {
    const d = ((k % 2 ? 1 : -1) * Math.ceil(k / 2)) * deg;
    const yaw = wrap(base + d);
    if (Math.abs(yaw) > FIGHT_TURN) continue;
    const sc = Math.abs(Math.sin(yaw + fallOwn));
    if (sc > score + 1e-9) {
      score = sc;
      best = yaw;
    }
  }
  return best;
}

// --- authoring helpers ----------------------------------------------------------------------------

/** The Euler turn ([x, y, z], radians; units.js eulerTurn's order) that takes direction `a` to
 *  direction `b` by the shortest rotation: clips aim what a hand holds at a direction. */
export function turn(a, b) {
  const n = (v) => {
    const l = Math.hypot(...v) || 1;
    return v.map((x) => x / l);
  };
  a = n(a);
  b = n(b);
  const ax = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const s = Math.hypot(...ax), c = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  if (s < 1e-9) return [0, 0, 0];
  const [x, y, z] = ax.map((v) => v / s);
  const th = Math.atan2(s, c), C = Math.cos(th), S = Math.sin(th), t = 1 - C;
  // Rodrigues' rotation matrix, then its Euler angles for R = Rz * Ry * Rx.
  const R = [
    [C + x * x * t, x * y * t - z * S, x * z * t + y * S],
    [y * x * t + z * S, C + y * y * t, y * z * t - x * S],
    [z * x * t - y * S, z * y * t + x * S, C + z * z * t],
  ];
  return [Math.atan2(R[2][1], R[2][2]), -Math.asin(Math.max(-1, Math.min(1, R[2][0]))), Math.atan2(R[1][0], R[0][0])];
}

// --- projectiles ---------------------------------------------------------------------------------

/** Where a loosed arrow leaves the bow: the nocked arrow's nock and tip (figure space) at the
 *  clip's `release` event, or null. */
export function releasePoint(bake) {
  const tr = bake.clip.events?.release;
  if (tr === undefined || !bake.held?.length) return null;
  const f = Math.max(0, Math.min(bake.frames - 1, Math.round(tr * bake.fps) - 1));
  const bow = Object.values(bake.held[f]).find((h) => h.nock && h.tip);
  return bow ? { nock: bow.nock, tip: bow.tip, frame: f } : null;
}

/** A projectile's flight from p0 to p1 (world points) at u in 0..1: a shallow arc peaking at
 *  `arc` times the distance (a shot at the next plot, ~4 m, flies nearly flat). Returns {p, dir}
 *  (dir the unit tangent). Shared by the renderer and the validator (the shot it checks is the
 *  one drawn). */
export function flight(p0, p1, u, arc = 0.05) {
  const d = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
  const len = Math.hypot(d[0], d[2]);
  const h = arc * len;
  const p = [p0[0] + d[0] * u, p0[1] + d[1] * u + 4 * h * u * (1 - u), p0[2] + d[2] * u];
  const dy = d[1] + 4 * h * (1 - 2 * u);
  const l = Math.hypot(d[0], dy, d[2]) || 1;
  return { p, dir: [d[0] / l, dy / l, d[2] / l] };
}

/** A projectile's flight time (seconds) over `dist` plots: fast, but long enough to follow
 *  (at least 5 frames at 30 fps). */
export function flightTime(dist, speed = 4.5) {
  return Math.max(0.17, dist / speed);
}

// --- playback on the GPU --------------------------------------------------------------------------

/** Octahedron encoding of a unit normal into 16 bits (8 + 8). */
function octEncode(x, y, z) {
  const l = Math.abs(x) + Math.abs(y) + Math.abs(z) || 1;
  let u = x / l, v = y / l;
  if (z < 0) [u, v] = [(1 - Math.abs(v)) * (u >= 0 ? 1 : -1), (1 - Math.abs(u)) * (v >= 0 ? 1 : -1)];
  const q = (c) => Math.max(0, Math.min(255, Math.round((c * 0.5 + 0.5) * 255)));
  return q(u) | (q(v) << 8);
}

/**
 * Packs the bakes of one look (one asset, variant and param; all with the same vertex count)
 * into the data of one RGBA16UI texture `w` texels wide: per frame and vertex the position
 * quantised to 16 bits over the frames' box and the normal (octEncode). Returns {data, w, h,
 * lo, span, count, clips: {name: {base, frames, fps, loop, dur, events}}} (shaders.js PROP_VS).
 */
export function packFrames(bakes, w = 2048) {
  const count = bakes[0].count;
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (const b of bakes) {
    if (b.count !== count) throw new Error("anim: a look's clips differ in vertex count");
    for (let i = 0; i < b.pos.length; i += 3) {
      for (let k = 0; k < 3; k++) {
        lo[k] = Math.min(lo[k], b.pos[i + k]);
        hi[k] = Math.max(hi[k], b.pos[i + k]);
      }
    }
  }
  const span = hi.map((h, k) => Math.max(1e-6, h - lo[k]));
  const total = bakes.reduce((a, b) => a + b.frames, 0) * count;
  const h = Math.max(1, Math.ceil(total / w));
  // WebGL2 guarantees textures of 2048 x 2048: a look's clips must fit (about 1000 frames of a
  // 4,000-vertex figure).
  if (h > 2048) throw new Error(`anim: ${bakes[0].recipe} needs a ${w} x ${h} frame texture (more than 2048 rows)`);
  const data = new Uint16Array(w * h * 4);
  const clipsOut = {};
  let base = 0;
  for (const b of bakes) {
    clipsOut[b.clipName] = { base, frames: b.frames, fps: b.fps, loop: !!b.clip.loop, dur: b.clip.dur, events: b.clip.events ?? {}, release: releasePoint(b) };
    for (let i = 0; i < b.frames * count; i++) {
      const o = (base * count + i) * 4, q = i * 3;
      for (let k = 0; k < 3; k++) data[o + k] = Math.round(((b.pos[q + k] - lo[k]) / span[k]) * 65535);
      data[o + 3] = octEncode(b.nrm[q], b.nrm[q + 1], b.nrm[q + 2]);
    }
    base += b.frames;
  }
  return { data, w, h, lo, span, count, clips: clipsOut };
}

/** Seconds a new clip cross-fades from the one it interrupts (shaders.js PROP_VS). */
export const BLEND = 0.15;

/**
 * The renderer's animation state (map3d.js): which looks are baked (a few frames per call of
 * `step`, idle first), what each unit plays, and the dying units' corpses. Pure bookkeeping:
 * map3d uploads the packed frames (Scene.setAnim) and asks `instanceAnim` for each instance.
 *
 * A unit idles (each at its own phase) until `trigger` starts a one-shot clip; that cross-fades
 * from whatever played (BLEND seconds), and when it ends the unit cross-fades back into its idle,
 * restarted at the end so the loop's first frame (the rest pose, where an attack ends) follows.
 */
export class Animator {
  constructor() {
    this.looks = new Map(); // key -> {asset, variant, param, names, bakes, job, packed}
    this.queue = [];
    this.play = new Map(); // unit id -> {clip, start, yaw, hold, prev}
    this.idleFrom = new Map(); // unit id -> {start, prev}: an idle restarted after a one-shot
  }

  /** True when `asset` has clips (only those figures are baked). */
  animates(asset) {
    return CLIPS.has(asset);
  }

  /** The packed frames of look `key` (asset#variant#param) when baked, else null (queued). */
  want(asset, variant, param) {
    const key = `${asset}#${variant}#${param}`;
    let L = this.looks.get(key);
    if (!L) {
      const names = clipNames(asset);
      // Idle first: it plays at once; the others when a fight needs them.
      names.sort((a, b) => (a === "idle" ? -1 : b === "idle" ? 1 : 0));
      L = { key, asset, variant, param, names, bakes: [], job: null, packed: null, failed: false };
      this.looks.set(key, L);
      this.queue.push(L);
    }
    return L.packed;
  }

  /** Bakes for up to `budgetMs`; returns the looks (re)published in this call ([{key, packed}]):
   *  first with the idle alone, then with every clip. */
  step(budgetMs = 6, now = () => performance.now()) {
    const done = [];
    const t0 = now();
    while (this.queue.length && now() - t0 < budgetMs) {
      const L = this.queue[0];
      try {
        if (!L.job) {
          const name = L.names[L.bakes.length];
          L.job = bakeSteps(L.asset, L.variant, L.param, name, { lean: true });
        }
        const r = L.job.next();
        if (r.done) {
          L.bakes.push(r.value);
          L.job = null;
          const all = L.bakes.length === L.names.length;
          // The idle plays as soon as it is baked; the look is published again with every clip.
          if (L.bakes.length === 1 || all) {
            L.packed = packFrames(L.bakes);
            done.push({ key: L.key, packed: L.packed });
          }
          if (all) {
            L.bakes = [];
            this.queue.shift();
          }
        }
      } catch (e) {
        console.warn(`anim: ${L.key}:`, e.message);
        L.failed = true;
        this.queue.shift();
      }
    }
    return done;
  }

  /** Starts clip `clip` on unit `id` at `start` (seconds, the renderer's clock), facing `yaw`,
   *  cross-fading from what instance `it` (idling at `phase`) plays then. */
  trigger(id, clip, start, yaw = null, it = null, phase = 0, { hold = 0 } = {}) {
    const prev = it ? this.instanceAnim(it, id, start, phase).cur : null;
    this.play.set(id, { clip, start, yaw, hold, prev });
  }

  /** The clips of an instance of look (it.asset, it.variant, it.param) for unit `id` at `time`:
   *  {cur, prev} iAnims (prev: the clip cur cross-fades from, or null). cur is the unit's one-shot
   *  while it runs (and holds), else its idle. Null when the look is not baked. */
  instanceAnim(it, id, time, phase = 0) {
    const L = this.looks.get(`${it.asset}#${it.variant ?? 0}#${it.param ?? ""}`);
    const P = L?.packed;
    if (!P) return { cur: null, prev: null };
    const pl = this.play.get(id);
    if (pl && !P.clips[pl.clip]) this.play.delete(id); // not baked when it was asked for: skipped
    else if (pl) {
      const c = P.clips[pl.clip];
      const end = c ? pl.start + c.dur + (pl.hold ?? 0) : -Infinity;
      if (c && (c.loop || time < end)) return { cur: [c.base, c.frames, pl.start, c.loop ? c.fps : -c.fps], prev: pl.prev };
      this.play.delete(id);
      // Back to the idle, restarted where the one-shot ended (its last frame held during the fade).
      if (c) this.idleFrom.set(id, { start: end, prev: [c.base, c.frames, pl.start, -c.fps] });
    }
    const idle = P.clips.idle;
    if (!idle) return { cur: null, prev: null };
    const back = this.idleFrom.get(id);
    if (back && time - back.start < BLEND) return { cur: [idle.base, idle.frames, back.start, idle.fps], prev: back.prev };
    if (back) return { cur: [idle.base, idle.frames, back.start, idle.fps], prev: null };
    return { cur: [idle.base, idle.frames, -phase * idle.dur, idle.fps], prev: null };
  }
}
