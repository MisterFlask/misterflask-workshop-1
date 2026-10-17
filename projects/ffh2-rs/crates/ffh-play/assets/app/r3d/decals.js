// World-anchored paint layers of the 3D map (docs/R3D.md "Terrain"), rasterised on the CPU with a
// 2D canvas whenever the map changes and sampled by the terrain shader:
//
//   decal texture (DRES px per plot): r roads, g fields, b wet ground (river banks), a trodden
//                                     settlement ground
//   light texture (LRES px per plot): rgb pools of local light (towns, lava, mana) that
//                                     the terrain and the props are lit by
//
// Roads run from each routed plot's centre to its routed neighbours, bowed a little so a network
// reads as tracks rather than a grid. Map wrap: anything near the east/west seam is drawn twice.

import { tname } from "../state.js";
import { FEAT, KIND, isHell } from "./world.js";
import { hashf } from "./noise.js";
import { MANA } from "./assets/palette.js";

export const DRES = 16;
export const LRES = 4;

const DIRS8 = [[1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1]];

function canvas(w, h) {
  const c = typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(w, h) : Object.assign(document.createElement("canvas"), { width: w, height: h });
  return [c, c.getContext("2d", { willReadFrequently: true })];
}

const FIELD = /FARM|PLANTATION|PASTURE|WINERY|VINEYARD|MUSHROOM|LAND_WORKED/;
const VILLAGE = /COTTAGE|HAMLET|VILLAGE|TOWN|CAMP/;

/** The decal texture's pixels (RGBA, DRES px per plot; row 0 is map row 0, the south). */
export function rasterDecals(world, map, cities) {
  const W = world.w * DRES, Hh = world.h * DRES;
  const [cv, g] = canvas(W, Hh);
  const out = new Uint8Array(W * Hh * 4);
  const wrap = world.wrapX;
  // Map point -> canvas pixel (canvas y = map y, so the rows match the texture's rows).
  const P = (mx, my) => [mx * DRES, my * DRES];
  const copies = (f) => {
    f(0);
    if (wrap) {
      f(-W);
      f(W);
    }
  };
  const [cv2, g2] = canvas(W, Hh);
  // Each channel is drawn sharp, then blurred once as a whole (a filter per stroke would cost a
  // blur of the whole canvas per stroke).
  const channel = (k, draw, blur = 0) => {
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.filter = "none";
    g.globalCompositeOperation = "source-over";
    g.globalAlpha = 1;
    g.fillStyle = "#000";
    g.fillRect(0, 0, W, Hh);
    draw();
    let src = g;
    if (blur > 0) {
      g2.filter = "none";
      g2.fillStyle = "#000";
      g2.fillRect(0, 0, W, Hh);
      g2.filter = `blur(${blur}px)`;
      g2.drawImage(cv, 0, 0);
      g2.filter = "none";
      src = g2;
    }
    const px = src.getImageData(0, 0, W, Hh).data;
    for (let i = 0; i < W * Hh; i++) out[i * 4 + k] = px[i * 4];
  };

  // r: roads.
  channel(0, () => {
    g.strokeStyle = "#fff";
    g.lineCap = "round";
    g.lineJoin = "round";
    g.lineWidth = DRES * 0.11;
    for (let y = 0; y < world.h; y++) {
      for (let x = 0; x < world.w; x++) {
        const i = y * world.w + x;
        if (map.vis[i] === 0 || map.route[i] < 0) continue;
        let linked = false;
        for (const [dx, dy] of DIRS8) {
          const j = world.index(x + dx, y + dy);
          if (j < 0 || map.route[j] < 0 || map.vis[j] === 0) continue;
          linked = true;
          // To the midpoint, bowed sideways by a hash of the (unordered) pair.
          const a = Math.min(i, j), b = Math.max(i, j);
          const bow = (hashf(a, b) - 0.5) * 0.16;
          const [cx, cy] = [x + 0.5, y + 0.5];
          const mx = cx + dx * 0.5, my = cy + dy * 0.5;
          const l = Math.hypot(dx, dy);
          const kx = cx + dx * 0.25 - (dy / l) * bow * (i < j ? 1 : -1);
          const ky = cy + dy * 0.25 + (dx / l) * bow * (i < j ? 1 : -1);
          copies((o) => {
            g.beginPath();
            g.moveTo(P(cx, cy)[0] + o, P(cx, cy)[1]);
            g.quadraticCurveTo(P(kx, ky)[0] + o, P(kx, ky)[1], P(mx, my)[0] + o, P(mx, my)[1]);
            g.stroke();
          });
        }
        if (!linked) {
          copies((o) => {
            g.beginPath();
            g.arc(P(x + 0.5, y + 0.5)[0] + o, P(x + 0.5, y + 0.5)[1], DRES * 0.1, 0, Math.PI * 2);
            g.fillStyle = "#fff";
            g.fill();
          });
        }
      }
    }
  }, DRES * 0.03);

  // g: fields (farms and the gardens of villages), as ragged patchwork.
  channel(1, () => {
    g.fillStyle = "#fff";
    for (let y = 0; y < world.h; y++) {
      for (let x = 0; x < world.w; x++) {
        const i = y * world.w + x;
        if (map.vis[i] === 0 || map.improvement[i] < 0 || map.city[i] >= 0) continue;
        const t = tname("ImprovementType", map.improvement[i]) ?? "";
        const field = FIELD.test(t);
        const village = VILLAGE.test(t);
        if (!field && !village) continue;
        const n = field ? 5 : 3;
        for (let k = 0; k < n; k++) {
          const lx = (hashf(i, k, 1) - 0.5) * (field ? 0.55 : 0.6);
          const ly = (hashf(i, k, 2) - 0.5) * (field ? 0.55 : 0.6);
          const sw = (field ? 0.32 : 0.16) * (0.7 + 0.5 * hashf(i, k, 3));
          const sh = (field ? 0.26 : 0.14) * (0.7 + 0.5 * hashf(i, k, 4));
          const rot = (hashf(i, k, 5) - 0.5) * 0.5;
          copies((o) => {
            g.save();
            g.translate(P(x + 0.5 + lx, y + 0.5 + ly)[0] + o, P(x + 0.5 + lx, y + 0.5 + ly)[1]);
            g.rotate(rot);
            g.fillRect((-sw / 2) * DRES, (-sh / 2) * DRES, sw * DRES, sh * DRES);
            g.restore();
          });
        }
      }
    }
  });

  // b: wet banks along the rivers, and marsh.
  channel(2, () => {
    g.strokeStyle = "#fff";
    g.lineCap = "round";
    g.lineJoin = "round";
    g.lineWidth = DRES * 0.42;
    g.globalAlpha = 0.7;
    for (const line of world.rivers) {
      copies((o) => {
        g.beginPath();
        line.forEach(([mx, my], k) => (k ? g.lineTo(mx * DRES + o, my * DRES) : g.moveTo(mx * DRES + o, my * DRES)));
        g.stroke();
      });
    }
    g.globalAlpha = 1;
  }, DRES * 0.12);

  // a: settlement ground.
  channel(3, () => {
    g.fillStyle = "#fff";
    for (const c of cities) {
      const r = DRES * (0.3 + Math.min(0.14, (c.city.size ?? 1) * 0.015));
      copies((o) => {
        g.beginPath();
        g.ellipse((c.city.x + 0.5) * DRES + o, (c.city.y + 0.5) * DRES, r, r * 0.92, 0, 0, Math.PI * 2);
        g.fill();
      });
    }
    for (let y = 0; y < world.h; y++) {
      for (let x = 0; x < world.w; x++) {
        const i = y * world.w + x;
        if (map.vis[i] === 0 || map.improvement[i] < 0 || map.city[i] >= 0) continue;
        const t = tname("ImprovementType", map.improvement[i]) ?? "";
        if (!VILLAGE.test(t) && !/FORT|TOWER|MINE|QUARRY|LUMBERMILL/.test(t)) continue;
        copies((o) => {
          g.beginPath();
          g.arc((x + 0.5) * DRES + o, (y + 0.48) * DRES, DRES * 0.13, 0, Math.PI * 2);
          g.fill();
        });
      }
    }
  }, DRES * 0.06);
  void cv;
  return { data: out, w: W, h: Hh };
}

function rgba(hex, a) {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  return m ? `rgba(${parseInt(m[1], 16)},${parseInt(m[2], 16)},${parseInt(m[3], 16)},${a})` : `rgba(255,255,255,${a})`;
}

/** The light-pool texture's pixels (RGBA, LRES px per plot). `strength` scales the pools. */
export function rasterLight(world, map, cities, strength = 1) {
  const W = world.w * LRES, Hh = world.h * LRES;
  const [, g] = canvas(W, Hh);
  g.fillStyle = "#000";
  g.fillRect(0, 0, W, Hh);
  g.globalCompositeOperation = "lighter";
  const pool = (mx, my, r, hex, a) => {
    for (const o of world.wrapX ? [0, -W, W] : [0]) {
      const cx = mx * LRES + o, cy = my * LRES;
      const grd = g.createRadialGradient(cx, cy, 0, cx, cy, r * LRES);
      grd.addColorStop(0, rgba(hex, a * strength));
      grd.addColorStop(0.45, rgba(hex, a * strength * 0.4));
      grd.addColorStop(1, rgba(hex, 0));
      g.fillStyle = grd;
      g.fillRect(cx - r * LRES, cy - r * LRES, r * 2 * LRES, r * 2 * LRES);
    }
  };
  for (const c of cities) pool(c.city.x + 0.5, c.city.y + 0.5, 0.9 + Math.min(0.6, (c.city.size ?? 1) * 0.05), "#ffaa55", 0.16);
  for (let y = 0; y < world.h; y++) {
    for (let x = 0; x < world.w; x++) {
      const i = y * world.w + x;
      if (world.kind[i] === KIND.VOID) continue;
      if (isHell(world.ground[i])) pool(x + 0.5, y + 0.5, 0.7, "#ff3a18", 0.05);
      const f = world.feat[i];
      if (f === FEAT.FLAMES || f === FEAT.VOLCANO) pool(x + 0.5, y + 0.5, 0.8, "#ff6a20", 0.16);
      if (f === FEAT.ANCIENT) pool(x + 0.5, y + 0.5, 0.6, "#9affc0", 0.06);
      const b = map.bonus[i] >= 0 ? tname("BonusType", map.bonus[i]) ?? "" : "";
      const im = map.improvement[i] >= 0 ? tname("ImprovementType", map.improvement[i]) ?? "" : "";
      // A built mana node glows where it stands (by the plot's centre); raw mana at its marker.
      const node = /MANA_?(\w*)/.exec(im)?.[1];
      const school = node ?? /MANA_?(\w*)/.exec(b)?.[1];
      if (node !== undefined) pool(x + 0.5, y + 0.52, 0.6, MANA[school] ?? MANA[""], 0.22);
      else if (school !== undefined) pool(x + 0.24, y + 0.3, 0.55, MANA[school] ?? MANA[""], 0.2);
    }
  }
  const px = g.getImageData(0, 0, W, Hh).data;
  return { data: new Uint8Array(px.buffer.slice(0)), w: W, h: Hh };
}
