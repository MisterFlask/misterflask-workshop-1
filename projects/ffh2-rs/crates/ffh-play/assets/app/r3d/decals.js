// World-anchored paint layers of the 3D map (docs/R3D.md "Terrain"), rasterised on the CPU with a
// 2D canvas whenever the map changes and sampled by the terrain shader:
//
//   decal texture (DRES px per plot): r roads (a distance ramp), g fields (the crop by value),
//                                     b wet ground (river banks), a trodden settlement ground
//   light texture (LRES px per plot): rgb pools of local light (towns, lava, mana) that
//                                     the terrain and the props are lit by
//
// Roads run from each routed plot's centre to its routed neighbours, bowed a little so a network
// reads as tracks rather than a grid; a diagonal step is left out where the two plots already
// meet through a routed plot beside both (no triangle at a junction). The road channel holds a
// ramp, 1 on a track's centre line falling to 0 at ROAD_RAMP plots from it (the union of the
// network), so the terrain shader cuts a crisp edge, wheel ruts and a worn verge from it at any
// zoom. Fields follow fields.js. Map wrap: anything near the east/west seam is drawn twice.

import { tname } from "../state.js";
import { FEAT, KIND, isHell, RIVER_HALF, RIVER_WANDER, RIVER_BANK } from "./world.js";
import { hashf } from "./noise.js";
import { MANA } from "./assets/palette.js";
import { fieldLayout, roadLinks, ROAD_CLEAR, CROP, CROP_LEVEL } from "./fields.js";

export const DRES = 16;
export const LRES = 4;
/** How far from a road's centre line its ramp reaches (plots); the terrain shader's road read-out
 *  divides by the same. */
export const ROAD_RAMP = 0.3;
const ROAD_STEPS = 12;

function canvas(w, h) {
  const c = typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(w, h) : Object.assign(document.createElement("canvas"), { width: w, height: h });
  return [c, c.getContext("2d", { willReadFrequently: true })];
}

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

  // The road tracks out of a plot (fields.js roadLinks): from its centre to each link's midpoint,
  // bowed sideways by a hash of the (unordered) pair -- straight over a river, onto its bridge
  // (dress.js) -- as [x0, y0, kx, ky, x1, y1] (a quadratic); a lone routed plot's road is a spot,
  // [x, y].
  const riv = (k, bit) => map.river[k] > 0 && (map.river[k] & bit) !== 0;
  const tracks = (x, y) => {
    const i = y * world.w + x;
    if (map.vis[i] === 0 || map.route[i] < 0) return [];
    const links = roadLinks(world, map, x, y);
    if (!links.length) return [[x + 0.5, y + 0.5]];
    return links.map(([dx, dy]) => {
      const j = world.index(x + dx, y + dy);
      const a = Math.min(i, j), b = Math.max(i, j);
      const bridged = (dy < 0 && !dx && riv(i, 1)) || (dy > 0 && !dx && riv(j, 1)) || (dx > 0 && !dy && riv(i, 2)) || (dx < 0 && !dy && riv(j, 2));
      const bow = bridged ? 0 : (hashf(a, b) - 0.5) * 0.16;
      const [cx, cy] = [x + 0.5, y + 0.5];
      const l = Math.hypot(dx, dy);
      const kx = cx + dx * 0.25 - (dy / l) * bow * (i < j ? 1 : -1);
      const ky = cy + dy * 0.25 + (dx / l) * bow * (i < j ? 1 : -1);
      return [cx, cy, kx, ky, cx + dx * 0.5, cy + dy * 0.5];
    });
  };
  const trace = (p, o) => {
    g.beginPath();
    if (p.length === 2) g.arc(p[0] * DRES + o, p[1] * DRES, g.lineWidth * 0.3, 0, Math.PI * 2);
    else {
      g.moveTo(p[0] * DRES + o, p[1] * DRES);
      g.quadraticCurveTo(p[2] * DRES + o, p[3] * DRES, p[4] * DRES + o, p[5] * DRES);
    }
  };

  // r: roads, as a ramp: each track stroked ROAD_STEPS times, wide and faint to narrow and bright,
  // kept by "lighten" (the brightest wins), so a pixel holds 1 - (its distance to the nearest
  // track) / ROAD_RAMP.
  // Every track of a step goes into one path (one stroke, one fill: the same pixels as a stroke per
  // track, all being one colour and width under "lighten"), and a wrapping map's copies across the
  // seam only for the tracks that reach within a ramp of it -- this channel is repainted on every
  // map change.
  channel(0, () => {
    const lines = [], spots = [];
    for (let y = 0; y < world.h; y++) {
      for (let x = 0; x < world.w; x++) {
        for (const p of tracks(x, y)) {
          const xs = p.length === 2 ? [p[0]] : [p[0], p[2], p[4]];
          const offs = [0];
          if (wrap && Math.max(...xs) + ROAD_RAMP > world.w) offs.push(-W);
          if (wrap && Math.min(...xs) - ROAD_RAMP < 0) offs.push(W);
          for (const o of offs) (p.length === 2 ? spots : lines).push([p, o]);
        }
      }
    }
    g.globalCompositeOperation = "lighten";
    g.lineCap = "round";
    g.lineJoin = "round";
    for (let k = 1; k <= ROAD_STEPS; k++) {
      const v = Math.round((255 * k) / ROAD_STEPS);
      g.strokeStyle = g.fillStyle = `rgb(${v},${v},${v})`;
      g.lineWidth = ROAD_RAMP * (1 - (k - 1) / ROAD_STEPS) * DRES * 2;
      g.beginPath();
      for (const [p, o] of lines) {
        g.moveTo(p[0] * DRES + o, p[1] * DRES);
        g.quadraticCurveTo(p[2] * DRES + o, p[3] * DRES, p[4] * DRES + o, p[5] * DRES);
      }
      g.stroke();
      g.beginPath();
      for (const [p, o] of spots) {
        const r = g.lineWidth * 0.3;
        g.moveTo(p[0] * DRES + o + r, p[1] * DRES);
        g.arc(p[0] * DRES + o, p[1] * DRES, r, 0, Math.PI * 2);
      }
      g.fill();
    }
  }, DRES * 0.025);

  // g: fields (farms, plantations, vineyards, worked land; fields.js) and the gardens of
  // villages, each crop by its value (CROP_LEVEL).
  channel(1, () => {
    const level = (crop) => {
      const v = Math.round(255 * CROP_LEVEL[crop]);
      return `rgb(${v},${v},${v})`;
    };
    const tilled = [];
    for (let y = 0; y < world.h; y++) {
      for (let x = 0; x < world.w; x++) {
        const i = y * world.w + x;
        if (map.vis[i] === 0 || map.improvement[i] < 0 || map.city[i] >= 0) continue;
        const t = tname("ImprovementType", map.improvement[i]) ?? "";
        const b = map.bonus[i] >= 0 ? tname("BonusType", map.bonus[i]) ?? "" : "";
        const lay = fieldLayout(i, t, b);
        if (lay || VILLAGE.test(t)) tilled.push([x, y]);
        if (lay) {
          // Plot-local (x east, z south) rectangles, their corners nudged a little off square.
          for (const [k, f] of lay.fields.entries()) {
            const jx = (n) => (hashf(i, k, n) - 0.5) * 0.03;
            const pts = [[f.x0 + jx(1), f.z0 + jx(2)], [f.x1 + jx(3), f.z0 + jx(4)], [f.x1 + jx(5), f.z1 + jx(6)], [f.x0 + jx(7), f.z1 + jx(8)]];
            g.fillStyle = level(f.crop);
            copies((o) => {
              g.beginPath();
              pts.forEach(([lx, lz], n) => (n ? g.lineTo : g.moveTo).call(g, (x + 0.5 + lx) * DRES + o, (y + 0.5 - lz) * DRES));
              g.closePath();
              g.fill();
            });
          }
          continue;
        }
        if (!VILLAGE.test(t)) continue;
        // A village's gardens: a few small beds and plots of dug earth around it.
        for (let k = 0; k < 3; k++) {
          const lx = (hashf(i, k, 1) - 0.5) * 0.6;
          const ly = (hashf(i, k, 2) - 0.5) * 0.6;
          const sw = 0.16 * (0.7 + 0.5 * hashf(i, k, 3));
          const sh = 0.14 * (0.7 + 0.5 * hashf(i, k, 4));
          const rot = (hashf(i, k, 5) - 0.5) * 0.3;
          g.fillStyle = level(hashf(i, k, 6) < 0.6 ? CROP.GREEN : CROP.SOIL);
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
    // A road through a farmed plot runs between its fields, a strip of grass either side
    // (the farm's props keep off it too: fields.js onRoad).
    g.strokeStyle = g.fillStyle = "#000";
    g.lineCap = "round";
    g.lineWidth = ROAD_CLEAR * 2 * DRES;
    for (const [x, y] of tilled) {
      for (const p of tracks(x, y)) {
        copies((o) => {
          trace(p, o);
          if (p.length === 2) g.fill();
          else g.stroke();
        });
      }
    }
  });

  // b: wet banks along the rivers: full under the water and up the cut bank (the terrain shader
  // lays wet earth there), fading out over the bank tops (lush grass), in nested strokes.
  channel(2, () => {
    g.strokeStyle = "#fff";
    g.lineCap = "round";
    g.lineJoin = "round";
    for (const [half, a] of [[RIVER_BANK + 0.14, 0.3], [RIVER_BANK + 0.07, 0.35], [RIVER_BANK, 0.4], [RIVER_HALF + RIVER_WANDER, 1]]) {
      g.lineWidth = DRES * half * 2;
      g.globalAlpha = a;
      for (const line of world.rivers) {
        copies((o) => {
          g.beginPath();
          line.forEach(([mx, my], k) => (k ? g.lineTo(mx * DRES + o, my * DRES) : g.moveTo(mx * DRES + o, my * DRES)));
          g.stroke();
        });
      }
    }
    g.globalAlpha = 1;
  }, DRES * 0.06);

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
        // A farmstead's yard (fields.js), trodden bare round the barn and the ricks.
        const yard = /FARM|PLANTATION/.test(t) ? fieldLayout(i, t, null)?.yard : null;
        if (yard) {
          const s = yard.side;
          copies((o) => {
            g.beginPath();
            g.ellipse((x + 0.5 + yard.x - s * 0.06) * DRES + o, (y + 0.5 - yard.z) * DRES, DRES * 0.2, DRES * 0.11, 0, 0, Math.PI * 2);
            g.fill();
          });
          continue;
        }
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
