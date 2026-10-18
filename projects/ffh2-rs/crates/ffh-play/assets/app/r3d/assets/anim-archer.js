// anim-archer.js: the archer's animation clips (docs/ANIMATION.md; the format is anim.js's): keyed pose
// deltas from its rest pose (units.js archerSpec), so one clip serves every race and variant (positions
// in a human's plots; the figure scales them to its race). Times in seconds; the figure faces +z,
// its right hand on -x.

import { clips, turn } from "../anim.js";

// The archer's rest hands (a human's plots, figure space): the drawing hand at the anchor under the
// jaw, the bow hand at full stretch. R / L give the hand deltas that put a hand at a point
// (approximately: the shoulders move a little with twist and lean).
const ANCHOR = [0, 0.375, 0.026];
const GRIP = [0.238, 0.37, -0.006];
const R = (x, y, z) => [x - ANCHOR[0], y - ANCHOR[1], z - ANCHOR[2]];
const L = (x, y, z) => [x - GRIP[0], y - GRIP[1], z - GRIP[2]];

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
      { ch: "lean", amp: 0.022, n: 1, phase: 0 },
      { ch: "pelvis", amp: [0, -0.0035, 0], n: 1, phase: 0.25 },
      { ch: "pelvis", amp: [0.003, 0, 0.002], n: 1, phase: 0.55 },
      // Holding the draw: the aim drifts and is corrected; the drawing hand eases and recovers.
      { ch: "lHand", amp: [0, 0.007, 0.004], n: 1, phase: 0.3 },
      { ch: "lHand", amp: [0, 0.0025, 0], n: 3, phase: 0.1 },
      { ch: "rHand", amp: [0.007, 0.002, 0], n: 1, phase: 0.62 },
      { ch: "headTilt", amp: 0.03, n: 1, phase: 0.4 },
      { ch: "twist", amp: 0.02, n: 1, phase: 0.85 },
    ],
  },

  attack: {
    kind: "attack",
    dur: 1.65,
    fps: 60,
    returns: true,
    strike: [1, 0, 0],
    events: { release: 0.35 },
    contacts: { L: [[0, 1.65]], R: [[0, 1.65]] },
    keys: [
      // Aim: the last hair of draw and a held breath, then 0.1 s dead still.
      { t: 0.2, pose: { rHand: R(-0.008, 0.376, 0.025), lean: -0.012 } },
      { t: 0.25, pose: { rHand: R(-0.0095, 0.376, 0.025), lean: -0.013 } },
      { t: 20 / 60, i: { string: "linear", arrow: "linear" }, pose: { rHand: R(-0.01, 0.376, 0.025), lean: -0.013, string: 1, arrow: 1 } },
      // Release (one frame: the string home, the arrow gone; the GPU's in-between is the true one).
      { t: 21 / 60, i: { string: "hold", arrow: "hold" }, pose: { string: 0, arrow: 0 } },
      // The drawing hand flies back past the ear; the bow tips forward in the open hand.
      { t: 0.35, i: { rHand: "out", lRot: "out" }, pose: { rHand: R(-0.01, 0.376, 0.025), lRot: [0, 0, 0] } },
      { t: 0.43, pose: { rHand: R(-0.035, 0.405, -0.035), lHand: L(0.234, 0.362, 0.0), lRot: [0, 0, -0.15], twist: -0.05, lean: -0.02 } },
      // Follow-through: watching the shot fly.
      { t: 0.56, pose: { rHand: R(-0.04, 0.41, -0.042), lHand: L(0.232, 0.35, 0.004), lRot: [0, 0, -0.2], twist: -0.06, lean: -0.02 } },
      // Reach back into the quiver and take an arrow.
      { t: 0.72, pose: { rHand: R(-0.047, 0.398, -0.07), rElbow: [0, 0.5, 0], lHand: L(0.19, 0.32, 0.03), lRot: [0, 0, -0.1], twist: -0.12, lean: 0.02, headYaw: -0.25 } },
      { t: 0.76, pose: { rHand: R(-0.048, 0.402, -0.07) } },
      { t: 0.88, pose: { rHand: R(-0.05, 0.5, -0.035), rElbow: [0, 0.8, 0], twist: -0.08, headYaw: -0.35, headTilt: 0.1 } },
      // Over the shoulder and down to the bow, brought in before the chest.
      { t: 0.96, pose: { rHand: R(-0.005, 0.455, 0.06), rElbow: [0.2, 0.3, 0.3], lHand: L(0.14, 0.315, 0.065), twist: 0.02, headYaw: -0.4, headTilt: 0.25 } },
      { t: 1.05, pose: { rHand: R(0.05, 0.325, 0.075), rElbow: [0.5, -0.6, 0.6], lHand: L(0.13, 0.31, 0.07), lRot: [0, 0, 0], twist: 0.08, lean: 0.06, headYaw: -0.4, headTilt: 0.3 } },
      // Nock: the new arrow on the string (it appears in the fingers there).
      { t: 63 / 60, i: { string: "linear", arrow: "linear" }, pose: { string: 0, arrow: 0 } },
      { t: 64 / 60, pose: { string: 1, arrow: 1 } },
      { t: 1.12, pose: { rHand: R(0.048, 0.326, 0.075) } },
      // Raise the bow and draw: push and pull, the head coming back to the mark.
      { t: 1.3, pose: { rHand: R(0.02, 0.355, 0.05), rElbow: [0.2, -0.2, 0.2], lHand: L(0.2, 0.345, 0.03), twist: 0.03, lean: 0.02, headYaw: -0.1, headTilt: 0.08 } },
      { t: 1.45, pose: { rHand: R(0.003, 0.375, 0.026), rElbow: [0, 0, 0], lHand: L(0.238, 0.37, -0.006), twist: 0, lean: 0, headYaw: 0, headTilt: 0 } },
    ],
  },

  death: {
    kind: "death",
    dur: 1.7,
    away: [0, 0, -1],
    fall: [0, 0, -1],
    pivot: [0, 0, -0.07],
    events: { hit: 0.0, ground: 0.78, rest: 1.05 },
    contacts: { L: [[0, 0.25]], R: [[0, 0.25]] },
    keys: [
      // Struck: thrown back, the string slips, the drawing hand flies off the string.
      { t: 0.1, i: { arrow: "hold" }, pose: { string: 0.3, arrow: 1, lean: -0.25, headTilt: -0.3, rHand: [-0.03, 0.03, -0.04], lHand: [0, 0.04, 0], pelvis: [0, 0.003, -0.01] } },
      { t: 0.14, i: { arrow: "hold" }, pose: { arrow: 0 } },
      // The knees go.
      { t: 0.42, i: { rootRot: "in" }, pose: { string: 0, lean: 0.18, pelvis: [0, -0.065, 0], headTilt: 0.22, rHand: [0.02, -0.15, 0.04], lHand: [-0.07, -0.11, 0.04], rootRot: [-0.1, 0, 0] } },
      // Toppling backward, faster and faster until the ground.
      { t: 0.78, i: { rootRot: "out" }, pose: { string: 0, lean: 0.08, pelvis: [0, -0.075, 0], headTilt: 0.18, rHand: [0.03, -0.11, 0.06], lHand: [-0.1, -0.09, 0.05], rootRot: [-1.56, 0, 0.12] } },
      { t: 0.86, i: { rootRot: "out" }, pose: { rootRot: [-1.49, 0, 0.12] } },
      { t: 1.05, pose: { string: 0, lean: 0.05, pelvis: [0, -0.06, 0], headTilt: 0.1, rHand: [0.04, -0.1, 0.08], lHand: [-0.12, -0.08, 0.06], rootRot: [-1.57, 0, 0.12] } },
    ],
  },
});
