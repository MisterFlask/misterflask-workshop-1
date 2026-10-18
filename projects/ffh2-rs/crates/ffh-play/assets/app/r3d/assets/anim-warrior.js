// anim-warrior.js: the warrior's animation clips (docs/ANIMATION.md; the format is anim.js's): keyed pose
// deltas from its rest pose (units.js warriorSpec), so one clip serves every race and variant (positions
// in a human's plots; the figure scales them to its race). Times in seconds; the figure faces +z,
// its right hand on -x.

import { clips, turn } from "../anim.js";

// --- unit.warrior: the axe (club, hammer) raised, the round shield forward, braced left foot first

/** The warrior's weapon direction at rest (units.js warriorSpec right.dir). */
const AXE = [-0.55, 0.8, 0.12];

clips("unit.warrior", {
  idle: {
    kind: "idle",
    dur: 3.2,
    loop: true,
    fps: 15,
    contacts: { L: [[0, 3.2]], R: [[0, 3.2]] },
    keys: [],
    waves: [
      // One breath a loop (3.2 s): the chest rises, the pelvis settles.
      { ch: "lean", amp: 0.028, n: 1, phase: 0 },
      { ch: "pelvis", amp: [0, -0.004, 0], n: 1, phase: 0.25 },
      // The weight drifts between the feet, out of step with the breath.
      { ch: "pelvis", amp: [0.0035, 0, 0.0025], n: 1, phase: 0.6 },
      // The weapon arm settles after the breath; a smaller twice-a-loop bob keeps it from moving
      // as one with the chest.
      { ch: "rHand", amp: [0, 0.008, 0.004], n: 1, phase: 0.12 },
      { ch: "rHand", amp: [0.002, 0.003, 0], n: 2, phase: 0.3 },
      { ch: "rRot", amp: [0.06, 0, 0.03], n: 1, phase: 0.18 },
      { ch: "lHand", amp: [0.004, 0.005, 0.002], n: 1, phase: 0.45 },
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
    events: { impact: 0.53 },
    contacts: { L: [[0, 1.2]], R: [[0, 1.2]] },
    keys: [
      // Anticipation: coil back onto the rear foot, the axe cocked behind the right shoulder (out
      // to the side, clear of the head), the shield up in guard.
      { t: 0.34, pose: { pelvis: [0.004, -0.006, -0.016], lean: -0.12, twist: -0.3, headYaw: 0.2, headTilt: 0.04, rHand: [-0.012, 0.03, -0.055], rElbow: [0, 0.1, 0], rRot: turn(AXE, [-0.38, 0.55, -0.74]), lHand: [-0.01, 0.03, 0.03] } },
      // A moving hold at the top: everything still drifting a little.
      { t: 0.43, pose: { pelvis: [0.005, -0.008, -0.019], lean: -0.13, twist: -0.32, headYaw: 0.22, headTilt: 0.05, rHand: [-0.014, 0.034, -0.06], rElbow: [0, 0.1, 0], rRot: turn(AXE, [-0.4, 0.52, -0.75]), lHand: [-0.01, 0.034, 0.032] } },
      // Impact: the axe lands in front at chest height, the body driving down onto the front foot.
      { t: 0.53, pose: { pelvis: [-0.004, -0.022, 0.024], lean: 0.3, twist: 0.2, headYaw: -0.12, headTilt: 0.1, rHand: [0.095, -0.18, 0.14], rElbow: [0, -0.2, 0], rRot: turn(AXE, [0.35, -0.3, 0.88]), lHand: [0.02, -0.01, -0.04] } },
      // Follow-through: it carries down and across, stopping short of the ground.
      { t: 0.62, pose: { pelvis: [-0.005, -0.026, 0.028], lean: 0.36, twist: 0.28, headYaw: -0.16, headTilt: 0.12, rHand: [0.12, -0.215, 0.105], rElbow: [0, -0.2, 0], rRot: turn(AXE, [0.55, -0.52, 0.65]), lHand: [0.024, -0.02, -0.05] } },
      // Recovery toward the guard (the clip returns to rest at the end).
      { t: 0.88, pose: { pelvis: [-0.002, -0.01, 0.01], lean: 0.12, twist: 0.08, headYaw: -0.05, headTilt: 0.04, rHand: [0.04, -0.07, 0.04], rElbow: [0, -0.05, 0], rRot: turn(AXE, [-0.1, 0.6, 0.79]), lHand: [0.008, 0, -0.012] } },
    ],
  },

  death: {
    kind: "death",
    dur: 1.6,
    away: [0, 0, -1],
    fall: [-1, 0, 0],
    pivot: [-0.072, 0, 0],
    events: { hit: 0.0, ground: 0.76, rest: 1.0 },
    contacts: { L: [[0, 0.3]], R: [[0, 0.3]] },
    keys: [
      // The blow: thrown back, the head snapped back, the arms flung up.
      { t: 0.1, pose: { pelvis: [0, 0.003, -0.012], lean: -0.3, twist: -0.08, headTilt: -0.32, rHand: [-0.03, 0.03, -0.04], rRot: [-0.4, 0, 0.2], lHand: [0.04, 0.035, -0.04] } },
      // The knees give: the body sags, starting to turn and tip to its right.
      { t: 0.4, i: { rootRot: "in" }, pose: { pelvis: [-0.006, -0.06, -0.008], lean: 0.18, twist: -0.22, roll: 0.12, headTilt: 0.15, rHand: [-0.02, -0.1, 0.06], rRot: [0.5, 0, 0.6], lHand: [0.03, -0.07, 0.02], rootRot: [0, 0, 0.12] } },
      // The topple: gathering speed until the ground stops it.
      { t: 0.76, i: { rootRot: "out" }, pose: { pelvis: [-0.006, -0.07, -0.008], lean: 0.15, twist: -0.28, roll: 0.2, headTilt: 0.2, rHand: [-0.02, -0.06, 0.12], rRot: [1.0, 0, 0.9], lHand: [0.05, -0.1, 0.05], rootRot: [0, 0, 1.56] } },
      // A small rebound, then it settles and lies still.
      { t: 0.84, i: { rootRot: "out" }, pose: { rootRot: [0, 0, 1.5] } },
      { t: 1.0, pose: { pelvis: [-0.006, -0.06, -0.008], lean: 0.12, twist: -0.28, roll: 0.2, headTilt: 0.12, rHand: [-0.02, -0.05, 0.12], rRot: [1.0, 0, 0.9], lHand: [0.06, -0.11, 0.05], rootRot: [0, 0, 1.57] } },
    ],
  },
});
