// anim-archer.js: the archer's animation clips (docs/ANIMATION.md; the format is anim.js's): keyed pose
// deltas from its rest pose (units.js archerSpec), so one clip serves every race and variant (positions
// in a human's plots; the figure scales them to its race). Times in seconds; the figure faces +z,
// its right hand on -x.

import { clips } from "../anim.js";

// The archer's rest hands (a human's plots, figure space): the drawing hand at the anchor under the
// jaw, the bow hand at full stretch. R / L give the hand deltas that put a hand at a point
// (approximately: the shoulders move a little with twist and lean).
const ANCHOR = [0, 0.375, 0.026];
const GRIP = [0.238, 0.37, -0.006];
const R = (x, y, z) => [x - ANCHOR[0], y - ANCHOR[1], z - ANCHOR[2]];
const L = (x, y, z) => [x - GRIP[0], y - GRIP[1], z - GRIP[2]];

/** Keys for a fall, at every frame of `fps` from the start of the turn to the ground (the bake
 *  samples them exactly, so the fall keeps its shape whatever the interpolation between them): the
 *  body turns about the vertical (`turn`: [from frame, to frame, radians], eased) and topples to its
 *  right about the world z axis (`tilt`: [from frame, ground frame, radians]), as rootRot
 *  [0, turn, tilt] (Rz Ry: the turn, then the topple). The topple follows gravity: the height it
 *  takes away grows as the square of the time since it began (after easing in over `ease` frames),
 *  so the body falls fastest as it lands; the `drop` channels ({ch: [from, to]}) move with it. */
function fall({ fps, turn: [t0, t1, tb], tilt: [a0, a1, ta], ease, drop = {} }) {
  const s = (f) => (f <= a0 ? 0 : f - a0 - ease * (1 - Math.exp(-(f - a0) / ease)));
  const c = (1 - Math.cos(ta)) / s(a1) ** 2;
  const keys = [];
  for (let f = Math.min(t0, a0); f <= a1; f++) {
    const w = Math.max(0, Math.min(1, (f - t0) / (t1 - t0))), q = (s(f) / s(a1)) ** 2;
    const pose = { rootRot: [0, tb * w * w * (3 - 2 * w), Math.acos(Math.max(-1, 1 - c * s(f) ** 2))] };
    if (f >= a0) for (const [ch, [from, to]] of Object.entries(drop)) pose[ch] = from.map((x, j) => x + (to[j] - x) * q);
    keys.push({ t: f / fps, i: "linear", pose });
  }
  keys[keys.length - 1].i = "out";
  return keys;
}

// --- unit.archer: side-on, the bow toward +x at full draw, a quiver on the back

clips("unit.archer", {
  idle: {
    kind: "idle",
    dur: 3.6,
    loop: true,
    fps: 10,
    contacts: { L: [[0, 3.6]], R: [[0, 3.6]] },
    keys: [],
    waves: [
      // One slow breath a loop: the chest rises and opens (no whole-body bob while holding a draw).
      { ch: "lean", amp: 0.032, n: 1, phase: 0.3 },
      { ch: "pelvis", amp: [0, -0.002, 0], n: 1, phase: 0.55 },
      // The weight drifts from foot to foot, the shoulders tilting against the hips.
      { ch: "pelvis", amp: [0.0105, 0, 0.0045], n: 1, phase: 0.85 },
      { ch: "roll", amp: 0.034, n: 1, phase: 0.85 },
      { ch: "twist", amp: 0.03, n: 1, phase: 0.15 },
      // Holding the draw: the bow arm tires and sags, then is lifted back onto the mark (a quicker
      // correction than the sag); the bow's tip drifts with it.
      { ch: "lHand", amp: [0, -0.012, 0.004], n: 1, phase: 0.6 },
      { ch: "lHand", amp: [0, 0.004, 0], n: 2, phase: 0.7 },
      { ch: "lRot", amp: [0, 0, 0.025], n: 1, phase: 0.65 },
      // The draw eases a finger's width and is pulled back to the anchor.
      { ch: "rHand", amp: [0.012, -0.002, 0.002], n: 1, phase: 0.92 },
      { ch: "rHand", amp: [0.003, 0, 0], n: 2, phase: 0.8 },
      // The head nods with the breath and checks the mark.
      { ch: "headTilt", amp: 0.04, n: 1, phase: 0.7 },
      { ch: "headYaw", amp: 0.05, n: 2, phase: 0.3 },
    ],
  },

  attack: {
    kind: "attack",
    dur: 1.65,
    fps: 60,
    returns: true,
    strike: [1, 0, 0],
    events: { release: 0.345 },
    contacts: { L: [[0, 1.65]], R: [[0, 1.65]] },
    keys: [
      // Aim: the last hair of draw, the hand settling into the anchor and the bow onto the mark, a held
      // breath; then 0.1 s dead still.
      { t: 0.2, pose: { rHand: R(-0.008, 0.376, 0.022), lHand: [0, 0, 0.007], lean: -0.012, twist: -0.018 } },
      { t: 0.25, pose: { rHand: R(-0.0095, 0.376, 0.0205), lHand: [0, 0, 0.008], lean: -0.013, twist: -0.02 } },
      { t: 20 / 60, i: { string: "hold", arrow: "hold" }, pose: { rHand: R(-0.01, 0.376, 0.02), lHand: [0, 0, 0.008], lean: -0.013, twist: -0.02, string: 1, arrow: 1 } },
      // Release: in one frame the string is home and the arrow gone (held so until the nock).
      { t: 21 / 60, i: { string: "hold", arrow: "hold" }, pose: { string: 0, arrow: 0 } },
      // The drawing hand flies back past the ear; the bow tips forward in the open hand.
      { t: 0.35, i: { rHand: "out", lRot: "out" }, pose: { rHand: R(-0.01, 0.376, 0.02), lRot: [0, 0, 0] } },
      { t: 0.43, pose: { rHand: R(-0.035, 0.405, -0.035), lHand: L(0.234, 0.362, 0.0), lRot: [0, 0, -0.15], twist: -0.05, lean: -0.02 } },
      // Follow-through: watching the shot fly.
      { t: 0.56, pose: { rHand: R(-0.04, 0.41, -0.042), rElbow: [0, 0, 0], lHand: L(0.232, 0.35, 0.004), lRot: [0, 0, -0.2], twist: -0.06, lean: -0.02, headYaw: 0, headTilt: 0 } },
      // Reach back into the quiver and take an arrow.
      { t: 0.72, pose: { rHand: R(-0.047, 0.398, -0.07), rElbow: [0, 0.5, 0], lHand: L(0.19, 0.32, 0.03), lRot: [0, 0, -0.1], twist: -0.12, lean: 0.02, headYaw: -0.25 } },
      { t: 0.76, pose: { rHand: R(-0.048, 0.402, -0.07) } },
      { t: 0.88, pose: { rHand: R(-0.05, 0.5, -0.035), rElbow: [0, 0.8, 0], twist: -0.08, headYaw: -0.35, headTilt: 0.1, pelvis: [0, 0.002, 0] } },
      // Over the shoulder and down to the bow, brought in before the chest; the knees give a little.
      { t: 0.96, pose: { rHand: R(-0.005, 0.455, 0.06), rElbow: [0.2, 0.3, 0.3], lHand: L(0.14, 0.315, 0.065), twist: 0.02, headYaw: -0.4, headTilt: 0.25 } },
      { t: 1.05, pose: { rHand: R(0.05, 0.325, 0.075), rElbow: [0.5, -0.6, 0.6], lHand: L(0.13, 0.31, 0.07), lRot: [0, 0, 0], twist: 0.08, lean: 0.06, headYaw: -0.4, headTilt: 0.3, pelvis: [0, -0.007, 0.002] } },
      // Nock: the new arrow on the string (it appears in the fingers there).
      { t: 64 / 60, pose: { string: 1, arrow: 1 } },
      { t: 1.12, pose: { rHand: R(0.048, 0.326, 0.075) } },
      // Raise the bow and draw: push and pull, rising tall again, the head coming back to the mark.
      { t: 1.3, pose: { rHand: R(0.02, 0.355, 0.05), rElbow: [0.2, -0.2, 0.2], lHand: L(0.2, 0.345, 0.03), twist: 0.03, lean: 0.02, headYaw: -0.1, headTilt: 0.08 } },
      { t: 1.45, pose: { rHand: R(0.003, 0.375, 0.026), rElbow: [0, 0, 0], lHand: L(0.238, 0.37, -0.006), twist: 0, lean: 0, headYaw: 0, headTilt: 0, pelvis: [0, 0, 0] } },
    ],
  },

  death: {
    kind: "death",
    dur: 1.7,
    fps: 60,
    away: [-1, 0, 0],
    fall: [-1, 0, 0],
    pivot: [-0.052, -0.02, -0.05],
    events: { hit: 0.0, ground: 43 / 60, rest: 1.0 },
    contacts: { L: [[0, 0.2]], R: [[0, 0.2]] },
    keys: [
      // Struck: the blow throws the body back and to its right at once, slowing as it reaches the
      // farthest point; the head snaps back, the bow arm jerks up, the fingers still hold the draw.
      { t: 0, i: "out", pose: { lean: 0, roll: 0, headTilt: 0, headYaw: 0, pelvis: [0, 0, 0], twist: 0, lHand: [0, 0, 0], lRot: [0, 0, 0] } },
      { t: 0.13, pose: { lean: -0.16, roll: 0.3, headTilt: -0.4, headYaw: -0.25, pelvis: [-0.02, 0, -0.008], twist: -0.12, rHand: R(-0.01, 0.388, 0.02), lHand: L(0.208, 0.402, 0.0), lRot: [0, 0, 0.06] } },
      // Then they go slack: the string snaps home and the arrow is loosed wild.
      { t: 10 / 60, i: { string: "hold", arrow: "hold" }, pose: { string: 1, arrow: 1 } },
      { t: 11 / 60, i: { string: "hold", arrow: "hold" }, pose: { string: 0, arrow: 0 } },
      // A stagger onto the right foot: the head drops, the right hand goes to the wound, the bow
      // arm sags.
      { t: 0.2, pose: { settle: 0 } },
      { t: 0.3, pose: { settle: 1, lean: -0.08, roll: 0.08, headTilt: 0, headYaw: -0.3, twist: -0.14, rHand: R(-0.02, 0.35, 0.05), rElbow: [0.5, -0.5, 0.5], lHand: L(0.15, 0.32, 0.05), lRot: [0, 0, 0.25], fl: [0, 0, 0], fr: [0, 0, 0] } },
      // The knees go: it turns a little toward the bow and topples to its right over the right foot,
      // faster and faster, landing on its back and right side; the arms are left behind and fly up.
      // From the stagger on, `settle` keeps whatever is lowest on the ground (each race's girth
      // differs, so one pivot cannot) and lays a tail or cape flat.
      ...fall({ fps: 60, turn: [12, 26, Math.PI / 2 - 1], tilt: [15, 44, 1.6], ease: 5, drop: { pelvis: [[-0.045, -0.004, -0.01], [0, -0.045, 0]] } }),
      { t: 44 / 60, pose: { lean: 0.02, roll: 0, headTilt: -0.15, headYaw: -0.5, twist: 0.18, rHand: R(0.0, 0.45, 0.1), rElbow: [1.3, 0.1, 1.1], lHand: L(0.03, 0.37, 0.07), lRot: [0, 0, 0.1], fl: [-0.04, 0.0, -0.02], fr: [-0.01, 0, -0.01] } },
      // A small rebound, the limbs flop down, then still: on its back and side, the right arm
      // stretched out under the head, the bow arm across the hip with the bow along the body (clear of
      // a goblin's big head), the top leg bent over the other.
      { t: 0.82, i: { rootRot: "out" }, pose: { rootRot: [0, Math.PI / 2 - 1, 1.57] } },
      { t: 1.0, pose: { pelvis: [0, -0.015, 0], lean: 0.08, twist: 0.18, headYaw: -0.6, headTilt: 0.1, lHand: L(0.1, 0.22, 0.05), lRot: [0, 0, 0], rHand: R(-0.063, 0.435, 0.053), rElbow: [1.4, 0, 1.2], fl: [-0.059, 0.001, -0.038], fr: [-0.01, 0, -0.006], rootRot: [0, Math.PI / 2 - 1, 1.6], settle: 1 } },
    ],
  },
});
