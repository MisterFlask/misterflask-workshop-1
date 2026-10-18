// anim-spearman.js: the spearman's clips (docs/ANIMATION.md): an idle, a spear thrust and a
// death. Pose deltas from spearmanSpec's rest pose (units.js: braced, left foot forward, the
// spear upright in the right hand at the shoulder, the shield on the left arm in front), in a
// human's plots (figure() scales them to the race).

import { clips, turn } from "../anim.js";

// The spear's rest direction (spearmanSpec `dir`): rRot aims it from there.
const SPEAR = [-0.1, 1, 0.2];
const aim = (d) => turn(SPEAR, d);

clips("unit.spearman", {
  idle: {
    kind: "idle",
    dur: 6,
    fps: 15,
    loop: true,
    contacts: { L: [[0, 6]], R: [[0, 6]] },
    keys: [
      { t: 0, pose: { headYaw: 0 } },
      { t: 1.4, pose: { headYaw: 0.22 } },
      { t: 2.4, pose: { headYaw: 0.2 } },
      { t: 3.4, pose: { headYaw: -0.04 } },
      { t: 4.4, pose: { headYaw: -0.16 } },
      { t: 5.2, pose: { headYaw: -0.12 } },
    ],
    waves: [
      { ch: "lean", amp: 0.012, n: 2 },
      { ch: "rHand", amp: [0, 0.003, 0], n: 2, phase: 0.05 },
      { ch: "lHand", amp: [0, 0.003, 0], n: 2, phase: 0.05 },
      { ch: "pelvis", amp: [0.006, 0, 0], n: 1 },
      { ch: "pelvis", amp: [0, -0.002, 0], n: 2, phase: 0.25 },
      { ch: "roll", amp: -0.012, n: 1 },
      { ch: "rRot", amp: [0.025, 0, 0.012], n: 1, phase: 0.6 },
    ],
  },

  attack: {
    kind: "attack",
    dur: 1.15,
    fps: 60,
    returns: true,
    strike: [0.12, -0.03, 1],
    events: { impact: 0.46 },
    contacts: { L: [[0, 0.28], [0.44, 0.66], [0.82, 1.15]], R: [[0, 1.15]] },
    keys: [
      // Ready: the spear swings down to level as the hand draws back (the point drops toward the
      // foe instead of poking out at it) and the weight starts back onto the rear foot.
      { t: 0.14, pose: { rHand: [-0.005, -0.01, -0.11], rRot: aim([0.06, 0.6, 1]) } },
      { t: 0.26, pose: { rRot: aim([0.1, 0.12, 1]) } },
      { t: 0.28, pose: { fl: [0, 0, 0] } },
      // Wind-up (anticipation): coiled on the rear foot, shoulders turned away, the spear drawn
      // back with its point a little high, the shield up; the front foot lifts to step in.
      { t: 0.34, pose: { pelvis: [0.004, -0.014, -0.034], lean: -0.07 } },
      { t: 0.36, pose: { twist: -0.34, headYaw: 0.12, lHand: [0, 0.02, 0.03], rElbow: [-0.2, 0.1, -0.4] } },
      { t: 0.37, pose: { fl: [0, 0.022, 0.04] } },
      { t: 0.38, i: "in", pose: { rHand: [-0.015, -0.04, -0.19] } },
      // The thrust: the front foot stamps down, hips then shoulders then arm drive through; the
      // hand accelerates into the impact.
      { t: 0.44, i: "hold", pose: { fl: [0, 0, 0.085] } },
      { t: 0.46, pose: { rHand: [0.005, -0.025, 0.1], rRot: aim([0.12, -0.03, 1]) } },
      { t: 0.48, pose: { twist: 0.3 } },
      { t: 0.5, pose: { pelvis: [0, -0.032, 0.07], lean: 0.27, headYaw: -0.12, lHand: [-0.01, 0.01, 0.05], rElbow: [0, -0.2, 0], headTilt: 0 } },
      { t: 0.51, pose: { rHand: [0.01, -0.02, 0.12] } },
      // Follow-through: the arm has stopped, the body keeps going and sinks into the lunge, the
      // head nods a beat late and the point dips.
      { t: 0.57, pose: { pelvis: [0, -0.037, 0.073], lean: 0.3, headTilt: 0.08 } },
      { t: 0.62, pose: { twist: 0.24, rRot: aim([0.12, -0.07, 1]) } },
      { t: 0.65, pose: { rHand: [0.005, -0.02, 0.08] } },
      // Recovery: push back off the front foot, step it home, raise the spear.
      { t: 0.66, pose: { fl: [0, 0, 0.085] } },
      { t: 0.74, pose: { pelvis: [0, -0.024, 0.008], lean: 0.08, fl: [0, 0.02, 0.04] } },
      { t: 0.8, pose: { headTilt: 0 } },
      { t: 0.84, pose: { rHand: [0, -0.004, 0] } },
      { t: 0.85, pose: { twist: 0.02, headYaw: 0, lHand: [0, 0, 0.01] } },
      { t: 0.82, i: "hold", pose: { fl: [0, 0, 0] } },
      { t: 0.92, pose: { rRot: aim([-0.08, 0.8, 0.5]) } },
    ],
  },

  death: {
    kind: "death",
    dur: 1.7,
    fps: 30,
    pivot: [0, 0, -0.04],
    away: [0, 0, -1],
    fall: [0, 0, -1],
    events: { hit: 0.03, ground: 0.9 },
    contacts: { L: [[0, 0.45]], R: [[0, 0.1], [0.3, 0.45]] },
    keys: [],
  },
});
