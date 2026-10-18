// anims.js: the unit animations (docs/ANIMATION.md): the archetypes' clips (one module each, so
// they can be authored in parallel) and the effects their clips spawn.

import { recipe } from "./registry.js";
import { MAT } from "../mesh.js";
import { pal } from "./palette.js";
import "./anim-warrior.js";
import "./anim-archer.js";
import "./anim-spearman.js";
import "./anim-soldier.js";

// --- fx.arrow: a loosed arrow in flight (map3d.js spawns it at an archer's release) -------------
// Drawn larger than the nocked arrow (a true-scale shaft is half a pixel wide at the gameplay zoom)
// with a pale fletching and a faint streak, pointing along +z.
recipe("fx.arrow", {
  build(mb) {
    mb.mat = MAT.MATTE;
    mb.color = pal("#8a6a48");
    mb.tube([[0, 0, -0.1], [0, 0, 0.1]], 0.0045, 4, { caps: true });
    mb.color = pal("#f0e8d8");
    for (const a of [0, Math.PI / 2]) {
      mb.part(() => {
        mb.rotateZ(a);
        mb.card([0, 0, -0.1], [0, 0.014, -0.085], [0, 0.014, -0.05], [0, 0, -0.06]);
        mb.card([0, 0, -0.1], [0, -0.014, -0.085], [0, -0.014, -0.05], [0, 0, -0.06]);
      });
    }
    mb.mat = MAT.METAL;
    mb.color = pal("#c8ccd0");
    mb.part(() => {
      mb.translate(0, 0, 0.1);
      mb.rotateX(Math.PI / 2);
      mb.cone(0.009, 0.03, 4);
    });
    // The streak: a thin emissive card trailing the shaft, so the flight reads as a line.
    mb.mat = MAT.EMISSIVE;
    mb.color = pal("#fff4d0");
    mb.card([-0.003, 0, -0.32], [0.003, 0, -0.32], [0.003, 0, -0.1], [-0.003, 0, -0.1]);
    mb.card([0, -0.003, -0.32], [0, 0.003, -0.32], [0, 0.003, -0.1], [0, -0.003, -0.1]);
  },
});
