// The layout of a worked plot's fields (docs/R3D.md "The world", "Placement"): which parts of the
// plot are fields of what crop, where the farmstead stands, where its fences run and its stooks
// stand. A pure function of the plot (index, improvement and resource names), shared by the field
// decal (decals.js), which paints the soil and the crop rows into the ground, and the props
// (assets/props.js), which stand on them -- so the barn stands in its yard, not in the wheat.
//
// Plot-local coordinates as the props use them: x east, z south (toward the camera), the plot
// spanning -0.5..0.5. The decal encodes a field's crop by its value (CROP_LEVEL) and the terrain
// shader reads it back: green rows on tilled soil (vegetable beds, running east-west), bare tilled
// soil, golden grain (rows running north-south). Rows are world-anchored, ROWS a plot, so the beds
// the props raise sit on the rows the ground paints.

import { hashf } from "./noise.js";

export const CROP = { GREEN: 1, SOIL: 2, GRAIN: 3 };
/** The field channel's value per crop (the shader cuts at the midpoints; a golden field's edge
 *  passes through the soil level, so grain is rimmed by a furrow of bare earth). */
export const CROP_LEVEL = { [CROP.GREEN]: 0.33, [CROP.SOIL]: 0.67, [CROP.GRAIN]: 1 };
/** Crop rows per plot: vegetable beds (along x, by map y) and grain (along y, by map x). Keep in
 *  step with the terrain shader's field read-out. */
export const ROWS = { green: 14, grain: 18 };

/** How far either side of a road's centre line the fields keep off it (plots): the track (~0.07)
 *  and a strip of verge. */
export const ROAD_CLEAR = 0.12;

const GRAIN = /WHEAT|CORN|RICE|BARLEY|OATS|RYE/;
const DIRS8 = [[1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1]];

/** The roads out of plot (x, y), as decals.js draws them: [dx, dy] (map directions, y north) to
 *  each routed, revealed neighbour, leaving out a diagonal step when a routed plot beside both
 *  already joins the two (it would close a triangle at every corner of the network). Empty when
 *  the plot has no road. */
export function roadLinks(world, map, x, y) {
  const routed = (x, y) => {
    const j = world.index(x, y);
    return j >= 0 && map.route[j] >= 0 && map.vis[j] !== 0;
  };
  if (!routed(x, y)) return [];
  const out = [];
  for (const [dx, dy] of DIRS8) {
    if (!routed(x + dx, y + dy)) continue;
    if (dx && dy && (routed(x + dx, y) || routed(x, y + dy))) continue;
    out.push([dx, dy]);
  }
  return out;
}

/** Whether plot-local point (lx, lz) lies within `pad` of the road `links` (roadLinks) of its plot:
 *  fields leave a strip either side of a track, and nothing of a farm stands on it. A routed plot
 *  without links has its road as a spot at the centre. */
export function onRoad(links, routed, lx, lz, pad) {
  if (routed && !links.length) return Math.hypot(lx, lz) < pad;
  for (const [dx, dy] of links) {
    const ex = dx * 0.5, ez = -dy * 0.5;
    const t = Math.max(0, Math.min(1, (lx * ex + lz * ez) / (ex * ex + ez * ez)));
    if (Math.hypot(lx - ex * t, lz - ez * t) < pad) return true;
  }
  return false;
}

/** A rectangle [x0, z0, x1, z1] with a crop. */
const rect = (x0, z0, x1, z1, crop) => ({ x0, z0, x1, z1, crop });

/** The fields of a worked plot, or null when the improvement tills none. Returns
 *  `{fields: [{x0, z0, x1, z1, crop}], yard: {x, z, side} | null, fences: [[[x, z], [x, z]]],
 *  stooks: [[x, z]], grain}`. `type` and `bonus` are type names (IMPROVEMENT_*, BONUS_*). */
export function fieldLayout(i, type, bonus = null) {
  const t = String(type ?? "").replace("IMPROVEMENT_", "");
  const h = (k) => hashf(i, k, 0x6f1e);
  const grain = GRAIN.test(bonus ?? "");
  if (t === "FARM") return farm(h, grain);
  if (t === "PLANTATION") {
    // One broad green field (the plantation's rows stand on it) and a strip of bare soil.
    const s = h(0) < 0.5 ? -1 : 1;
    return { fields: [rect(-0.42, -0.06, 0.42, 0.42, CROP.GREEN), s < 0 ? rect(-0.42, -0.42, -0.02, -0.1, CROP.SOIL) : rect(0.02, -0.42, 0.42, -0.1, CROP.SOIL)], yard: { x: -0.22 * s, z: -0.24, side: -s }, fences: [], stooks: [], grain: false };
  }
  if (/WINERY|VINEYARD/.test(t)) return { fields: [rect(-0.38, -0.06, 0.4, 0.38, CROP.SOIL)], yard: null, fences: [], stooks: [], grain: false };
  if (t === "LAND_WORKED") return { fields: [rect(-0.4, -0.38, -0.02, 0.38, CROP.SOIL), rect(0.03, -0.38, 0.4, 0.38, h(1) < 0.5 ? CROP.GREEN : CROP.SOIL)], yard: null, fences: [], stooks: [], grain: false };
  return null;
}

/** Whether plot-local point (lx, lz) of a plot (index `i`, improvement and resource names) lies on
 *  one of its fields, or within `pad` of one: loose plants keep off the crops. */
export function inField(i, type, bonus, lx, lz, pad = 0) {
  const lay = fieldLayout(i, type, bonus);
  return !!lay && lay.fields.some((f) => lx > f.x0 - pad && lx < f.x1 + pad && lz > f.z0 - pad && lz < f.z1 + pad);
}

/** A farm: two fields across the front of the plot, the farmstead's yard in a back corner with a
 *  third field beside it; fences along the front and one side; stooks on the grain. */
function farm(h, grain) {
  const side = h(0) < 0.5 ? -1 : 1; // the farmstead's side
  const X0 = -0.44, X1 = 0.44, Z0 = -0.44, Z1 = 0.42, gap = 0.035;
  const zb = -0.1 + (h(1) - 0.5) * 0.08; // front fields / back strip
  const xs = (h(2) - 0.5) * 0.3; // the front fields' split
  const xy = side * (0.02 + h(3) * 0.08); // the back strip's split: the yard on `side`
  let crops;
  if (grain) crops = [CROP.GRAIN, h(4) < 0.35 ? CROP.SOIL : CROP.GRAIN, h(5) < 0.5 ? CROP.GRAIN : CROP.SOIL];
  else {
    // Vegetable beds beside bare tilled soil, one of each at the front at least.
    const a = h(4) < 0.5;
    crops = [a ? CROP.GREEN : CROP.SOIL, a ? CROP.SOIL : CROP.GREEN, h(5) < 0.6 ? CROP.GREEN : CROP.SOIL];
    if (h(6) < 0.3) crops[0] = crops[1] = CROP.GREEN;
  }
  const fields = [
    rect(X0, zb + gap / 2, xs - gap / 2, Z1, crops[0]),
    rect(xs + gap / 2, zb + gap / 2, X1, Z1, crops[1]),
    side > 0 ? rect(X0, Z0, xy - gap / 2, zb - gap / 2, crops[2]) : rect(xy + gap / 2, Z0, X1, zb - gap / 2, crops[2]),
  ];
  // Fences: along the front (most farms) and down the side away from the yard (some), each a few
  // runs of rails with a gate's gap.
  const fences = [];
  const fz = Z1 + 0.025;
  if (h(7) < 0.75) {
    const gate = X0 + 0.15 + h(8) * (X1 - X0 - 0.3);
    if (gate - X0 > 0.12) fences.push([[X0, fz], [gate - 0.05, fz]]);
    if (X1 - gate > 0.12) fences.push([[gate + 0.05, fz], [X1, fz]]);
  }
  if (h(9) < 0.5) {
    const fx = -side * (X1 + 0.025);
    fences.push([[fx, zb], [fx, fz]]);
  }
  // Stooks (sheaves stood together to dry): over a grain farm's reaped field, or at the side of its
  // standing grain when none is reaped yet.
  const stooks = [];
  if (grain) {
    const reaped = fields.find((f) => f.crop === CROP.SOIL);
    if (reaped) {
      for (const [u, v] of [[0.25, 0.3], [0.7, 0.25], [0.45, 0.72], [0.8, 0.75]]) {
        if (h(10 + stooks.length) < 0.25) continue;
        stooks.push([reaped.x0 + (reaped.x1 - reaped.x0) * (u + (h(20 + stooks.length) - 0.5) * 0.12), reaped.z0 + (reaped.z1 - reaped.z0) * (v + (h(30 + stooks.length) - 0.5) * 0.12)]);
      }
    } else {
      const f = fields[0];
      stooks.push([f.x0 + 0.035, f.z1 - 0.05], [f.x0 + 0.035, f.z1 - 0.15]);
    }
  }
  return { fields, yard: { x: side * 0.27, z: -0.27, side }, fences, stooks, grain };
}
