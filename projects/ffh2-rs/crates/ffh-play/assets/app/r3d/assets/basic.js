// Fallback recipes (docs/R3D.md "Asset recipes"): one simple, serviceable model per category, used
// when no specialised recipe claims a thing -- and the smallest worked examples of the recipe API.
// The art-directed recipes live beside this file (vegetation.js, architecture.js, units.js, ...)
// and replace these by registering more specific names.

import { recipe } from "./registry.js";
import { MAT, BONE } from "../mesh.js";
import { pal, jitter, mixc } from "./palette.js";
import { noise3 } from "../noise.js";

/** A broadleaf tree: a leaning trunk, a few limbs and a canopy of lumpy leaf masses whose
 *  normals bend out from the crown's centre (soft, painted shading). */
recipe("basic.tree", {
  variants: 4,
  build(mb, r) {
    const h = r.range(0.17, 0.22);
    const lean = [r.range(-0.02, 0.02), 0, r.range(-0.02, 0.02)];
    mb.color = jitter(pal("barkOak"), r);
    mb.tube([[0, 0, 0], [lean[0] * 0.5, h * 0.5, lean[2] * 0.5], [lean[0], h, lean[2]]], [0.024, 0.016, 0.01], 6);
    const top = [lean[0], h + 0.06, lean[2]];
    const m0 = mb.mark();
    mb.mat = MAT.FOLIAGE;
    const n = r.int(4, 6);
    for (let i = 0; i < n; i++) {
      const a = (i / n) * Math.PI * 2 + r.range(-0.4, 0.4);
      const d = i === 0 ? 0 : r.range(0.04, 0.07);
      const c = [top[0] + Math.cos(a) * d, top[1] + r.range(-0.02, 0.05) - d * 0.3, top[2] + Math.sin(a) * d];
      const rad = r.range(0.06, 0.085);
      mb.color = jitter(mixc("oak", "oakLight", r.range(0, 0.5)), r, 0.08);
      const seed = r.range(0, 100);
      mb.part(() => {
        mb.translate(c[0], c[1], c[2]);
        mb.blob(rad, { detail: 1, amp: 0.22, squash: 0.85, noise: (v) => noise3(v[0] * 2.2 + seed, v[1] * 2.2, v[2] * 2.2) });
      });
    }
    mb.sphereNormals(m0, top, 0.6);
    // Darker under the crown, sunlit on top; leaves sway more toward the top.
    mb.tint(m0, (x, y) => 0.62 + 0.55 * Math.max(0, Math.min(1, (y - h + 0.02) / 0.14)));
    mb.heightSway(m0, h - 0.02, h + 0.16, 1);
  },
});

/** A conifer: a short trunk and stacked, ragged cones. */
recipe("basic.pine", {
  variants: 4,
  build(mb, r) {
    const h = r.range(0.3, 0.38);
    mb.color = jitter(pal("barkPine"), r);
    mb.cylinder(0.014, 0.008, h * 0.4, 5);
    const m0 = mb.mark();
    mb.mat = MAT.FOLIAGE;
    const tiers = 4;
    for (let i = 0; i < tiers; i++) {
      const t = i / tiers;
      const y0 = h * (0.18 + t * 0.62);
      const rad = 0.085 * (1 - t * 0.68) * r.range(0.9, 1.1);
      const tall = h * 0.34 * (1 - t * 0.3);
      mb.color = jitter(mixc("pineDark", "pineLight", t * 0.7), r, 0.07);
      const ph = r.range(0, 6);
      mb.part(() => {
        mb.translate(0, y0, 0);
        mb.lathe([[rad * 0.25, -0.012], [rad, 0], [rad * 0.55, tall * 0.4], [0, tall]], 9, { wobble: (a) => 0.18 * Math.sin(a * 5 + ph) + 0.08 * Math.sin(a * 11 + ph) });
      });
    }
    mb.sphereNormals(m0, [0, h * 0.45, 0], 0.45);
    mb.tint(m0, (x, y) => 0.7 + 0.45 * Math.min(1, y / h));
    mb.heightSway(m0, h * 0.2, h, 0.8);
  },
});

/** A figure on foot: legs, a body in the owner's colours, arms, a head and a blade. */
recipe("basic.unit", {
  variants: 1,
  build(mb, r, { param }) {
    const skin = pal(param === "orc" ? "skinOrc" : param === "elf" ? "skinElf" : param === "undead" ? "skinUndead" : "skinHuman");
    const H = 0.44;
    mb.color = pal("leatherDark");
    for (const s of [-1, 1]) {
      mb.bone = s < 0 ? BONE.LEG_L : BONE.LEG_R;
      mb.tube([[s * 0.025, 0, 0], [s * 0.026, H * 0.24, 0], [s * 0.022, H * 0.46, 0]], [0.016, 0.018, 0.021], 6, { caps: true });
    }
    mb.bone = BONE.BODY;
    mb.mat = MAT.TEAM;
    mb.color = "#c8c8c8";
    mb.part(() => {
      mb.translate(0, H * 0.4, 0);
      mb.lathe([[0.05, 0], [0.058, H * 0.08], [0.048, H * 0.26], [0.06, H * 0.36], [0.03, H * 0.42]], 8);
    });
    mb.mat = MAT.METAL;
    mb.color = pal("steel");
    mb.part(() => {
      mb.translate(0, H * 0.74, 0);
      mb.lathe([[0.03, 0], [0.068, 0.012], [0.055, 0.04], [0, 0.05]], 8);
    });
    mb.mat = MAT.SKIN;
    mb.color = skin;
    mb.bone = BONE.HEAD;
    mb.part(() => {
      mb.translate(0, H * 0.87, 0);
      mb.sphere(0.04, 1);
    });
    mb.mat = MAT.METAL;
    mb.color = pal("iron");
    mb.part(() => {
      mb.translate(0, H * 0.94, 0);
      mb.lathe([[0.046, -0.01], [0.044, 0.01], [0.025, 0.035], [0, 0.045]], 8);
    });
    for (const s of [-1, 1]) {
      mb.bone = s < 0 ? BONE.ARM_L : BONE.ARM_R;
      mb.mat = MAT.TEAM;
      mb.color = "#b0b0b0";
      mb.tube([[s * 0.06, H * 0.78, 0], [s * 0.08, H * 0.62, 0.01], [s * 0.075, H * 0.5, 0.04]], [0.016, 0.015, 0.013], 6, { caps: true });
    }
    mb.bone = BONE.WEAPON;
    mb.mat = MAT.METAL;
    mb.color = pal("steel");
    mb.part(() => {
      mb.translate(0.075, H * 0.5, 0.05);
      mb.rotateX(-0.5);
      mb.box(0.012, H * 0.5, 0.004, { top: 0.5 });
    });
    mb.bone = BONE.ARM_L;
    mb.mat = MAT.TEAM;
    mb.color = "#d0d0d0";
    mb.part(() => {
      mb.translate(-0.085, H * 0.58, 0.02);
      mb.rotateZ(Math.PI / 2);
      mb.cylinder(0.06, 0.06, 0.012, 10);
    });
  },
});

/** A small house: plastered walls under a gable roof. */
recipe("basic.house", {
  variants: 3,
  build(mb, r) {
    const w = r.range(0.1, 0.14), d = r.range(0.08, 0.1), h = r.range(0.06, 0.08);
    mb.color = jitter(pal("plaster"), r);
    mb.box(w, h, d);
    mb.color = jitter(pal("roofTile"), r, 0.1);
    mb.part(() => {
      mb.translate(0, h, 0);
      mb.gable(w, d, d * 0.7, { over: 0.012, sag: 0.004 });
    });
    mb.color = pal("window");
    mb.mat = MAT.EMISSIVE;
    mb.part(() => {
      mb.translate(w * 0.2, h * 0.35, d / 2 + 0.001);
      mb.box(0.014, 0.018, 0.002);
    });
  },
});

/** A boulder. */
recipe("basic.rock", {
  variants: 4,
  build(mb, r) {
    mb.color = jitter(pal("stone"), r, 0.1);
    const s = r.range(0, 100);
    mb.part(() => {
      mb.translate(0, 0.02, 0);
      mb.blob(r.range(0.04, 0.07), { detail: 1, amp: 0.35, squash: 0.7, noise: (v) => noise3(v[0] * 1.8 + s, v[1] * 1.8, v[2] * 1.8), flat: true });
    });
  },
});

/** A humpbacked stone bridge along z (deck at y = 0, the banks' height; the arch and the piers
 *  reach down to the water), for a road crossing a river edge. Length ~0.42 plot. */
recipe("basic.bridge", {
  variants: 2,
  build(mb, r) {
    const L = 0.42, W = 0.085, N = 10;
    const stone = jitter(pal("stone"), r, 0.05);
    const top = (z) => 0.012 + 0.018 * Math.cos((Math.PI * z) / L);
    const arch = (z) => Math.min(top(z) - 0.02, -0.075 + 0.07 * Math.cos((Math.PI * z) / (L * 0.62)));
    mb.color = stone;
    for (let i = 0; i < N; i++) {
      const z0 = -L / 2 + (L * i) / N, z1 = -L / 2 + (L * (i + 1)) / N;
      const t0 = top(z0), t1 = top(z1), a0 = arch(z0), a1 = arch(z1);
      // Deck, the arch's underside, and the two side walls with the arch cut out.
      mb.color = jitter(pal("stoneLight"), r, 0.05);
      mb.quad([-W / 2, t0, z0], [-W / 2, t1, z1], [W / 2, t1, z1], [W / 2, t0, z0]);
      mb.color = jitter(stone, r, 0.08);
      mb.quad([-W / 2, a0, z0], [W / 2, a0, z0], [W / 2, a1, z1], [-W / 2, a1, z1]);
      for (const s of [-1, 1]) {
        const x = (s * W) / 2;
        mb.quad([x, a0, z0], [x, a1, z1], [x, t1, z1], [x, t0, z0]);
        // A low parapet.
        mb.quad([x, t0, z0], [x, t1, z1], [x, t1 + 0.014, z1], [x, t0 + 0.014, z0]);
        mb.quad([x * 0.8, t0 + 0.014, z0], [x * 0.8, t1 + 0.014, z1], [x, t1 + 0.014, z1], [x, t0 + 0.014, z0]);
      }
    }
    // Abutments into the banks.
    for (const s of [-1, 1]) {
      mb.part(() => {
        mb.translate(0, -0.09, (s * L) / 2);
        mb.box(W * 1.25, 0.1, 0.06);
      });
    }
    mb.heightAO(0, -0.09, 0.01, 0.55);
  },
});
