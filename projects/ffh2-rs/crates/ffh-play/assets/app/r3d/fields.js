// The layout of a worked plot's fields (docs/R3D.md "The world", "Placement"): which parts of the
// plot are fields of what crop, where the farmstead stands, where its fences run and its stooks
// stand, a pasture's paddock and the ground trodden bare. A pure function of the plot (index,
// improvement and resource names, and the rivers round it: riversAround), shared by the field decal
// (decals.js), which paints the soil, the crop rows and the trodden ground, and the props
// (assets/props.js), which stand on them -- so the barn stands in its yard, not in the wheat, and
// nothing of a farm stands in the river.
//
// Plot-local coordinates as the props use them: x east, z south (toward the camera), the plot
// spanning -0.5..0.5. The decal encodes a field's crop by its value (CROP_LEVEL) and the terrain
// shader reads it back: green rows on tilled soil (vegetable beds, running east-west), bare tilled
// soil, golden grain (rows running north-south), and a paddock's grazed turf (short, paler grass,
// trodden bare in places: the worn ground). Rows are world-anchored, ROWS a plot, so the beds
// the props raise sit on the rows the ground paints. A river along an edge takes the plot's ground
// up to its bank top (RIVER_BANK from the edge), so a layout is laid out over the whole plot and
// then squeezed off that edge (everything keeps its proportions), and cut back from a corner a
// river only touches.

import { hashf } from "./noise.js";
import { clamp } from "./math.js";
import { RIVER_BANK } from "./world.js";

export const CROP = { GREEN: 1, SOIL: 2, GRAIN: 3, TURF: 4 };
/** The field channel's value per crop (the shader cuts at the midpoints; a golden field's edge
 *  passes through the soil level, so grain is rimmed by a furrow of bare earth). Turf, the lowest,
 *  is where the field channel fades in fully (the shader's fieldW). */
export const CROP_LEVEL = { [CROP.TURF]: 0.2, [CROP.GREEN]: 0.33, [CROP.SOIL]: 0.67, [CROP.GRAIN]: 1 };
/** Crop rows per plot: vegetable beds (along x, by map y) and grain (along y, by map x). Keep in
 *  step with the terrain shader's field read-out. */
export const ROWS = { green: 14, grain: 18 };

/** How far either side of a road's centre line the fields keep off it (plots): the track (~0.07)
 *  and a strip of verge. */
export const ROAD_CLEAR = 0.12;

/** A plot's rivers as fieldLayout takes them (riversAround): RIV.N, E, S, W for a river along that
 *  edge (north is -z), RIV.NE, SE, SW, NW for one that only touches that corner (it ends or turns
 *  there, and its water reaches round the corner into the plot). */
export const RIV = { N: 1, E: 2, S: 4, W: 8, NE: 16, SE: 32, SW: 64, NW: 128 };

/** How far a layout reaches from the plot's centre over open ground (a side fence stands at
 *  0.465), and toward a river along an edge: its bank top. */
const REACH = 0.47;
const DRY = 0.5 - RIVER_BANK;

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

/** The rivers round plot (x, y) as RIV bits: a pure function of the map's river bits on the plots
 *  the player knows (as world.js traces them). A plot's own bits are its south (1) and east (2)
 *  edges; its north and west edges are its neighbours'. A corner counts when a river leaves it
 *  away from the plot and neither of the plot's own edges there carries one. */
export function riversAround(world, map, x, y) {
  const on = (px, py, bit) => {
    const j = world.index(px, py);
    return j >= 0 && map.vis[j] !== 0 && map.river[j] > 0 && (map.river[j] & bit) !== 0;
  };
  let m = (on(x, y + 1, 1) ? RIV.N : 0) | (on(x, y, 2) ? RIV.E : 0) | (on(x, y, 1) ? RIV.S : 0) | (on(x - 1, y, 2) ? RIV.W : 0);
  if (!(m & (RIV.N | RIV.E)) && (on(x + 1, y + 1, 1) || on(x, y + 1, 2))) m |= RIV.NE;
  if (!(m & (RIV.S | RIV.E)) && (on(x + 1, y, 1) || on(x, y - 1, 2))) m |= RIV.SE;
  if (!(m & (RIV.S | RIV.W)) && (on(x - 1, y, 1) || on(x - 1, y - 1, 2))) m |= RIV.SW;
  if (!(m & (RIV.N | RIV.W)) && (on(x - 1, y + 1, 1) || on(x - 1, y + 1, 2))) m |= RIV.NW;
  return m;
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
/** A layout with what it leaves out empty. */
const layout = (o) => ({ fields: [], yard: null, fences: [], stooks: [], grain: false, paddock: null, worn: [], ...o });
/** A farmstead's yard, trodden bare round the barn and the ricks (toward the plot's centre). */
const yardWorn = (yard) => [[yard.x - yard.side * 0.06, yard.z, 0.2, 0.11, 1]];

// The last layout asked for: the dressing asks for one plot's many times over (every plant it
// scatters there, then the props), the decal twice.
let memo = { key: "", lay: null };

/** The fields of a worked plot, or null when the improvement tills and fences none. Returns
 *  `{fields: [{x0, z0, x1, z1, crop}], yard: {x, z, side} | null, fences: [[[x, z], [x, z]]],
 *  stooks: [[x, z]], grain, paddock: {x, z, rx, rz, sx, sz, variant, turf} | null,
 *  worn: [[x, z, rx, rz, a]]}` (paddock: an ellipse, turf when grazed; worn: ellipses of ground
 *  trodden bare, a its strength). `type` and `bonus` are type names (IMPROVEMENT_*, BONUS_*),
 *  `rivers` the plot's riversAround. The same arguments give the same object (shared: not to be
 *  changed). */
export function fieldLayout(i, type, bonus = null, rivers = 0) {
  const key = `${i}|${type ?? ""}|${bonus ?? ""}|${rivers}`;
  if (key !== memo.key) memo = { key, lay: layOut(i, String(type ?? "").replace("IMPROVEMENT_", ""), bonus ?? "", rivers) };
  return memo.lay;
}

function layOut(i, t, bonus, rivers) {
  const h = (k) => hashf(i, k, 0x6f1e);
  if (t === "PASTURE" || t === "MUSHROOMS") return paddock(h, t === "MUSHROOMS", rivers);
  let lay = null;
  if (t === "FARM") lay = farm(h, GRAIN.test(bonus), rivers);
  else if (t === "PLANTATION") {
    // One broad green field (the plantation's rows stand on it), a strip of bare soil and the
    // drying shed's yard at the back.
    const side = yardSide(rivers, h(0) < 0.5 ? 1 : -1), s = -side;
    const yard = { x: -0.22 * s, z: -0.24, side };
    lay = layout({ fields: [rect(-0.42, -0.06, 0.42, 0.42, CROP.GREEN), s < 0 ? rect(-0.42, -0.42, -0.02, -0.1, CROP.SOIL) : rect(0.02, -0.42, 0.42, -0.1, CROP.SOIL)], yard, worn: yardWorn(yard) });
  } else if (/WINERY|VINEYARD/.test(t)) {
    // The vines' field, the press behind it.
    const side = yardSide(rivers, h(0) < 0.5 ? -1 : 1);
    lay = layout({ fields: [rect(-0.38, -0.06, 0.4, 0.38, CROP.SOIL)], yard: { x: 0.19 * side, z: -0.23, side } });
  } else if (t === "LAND_WORKED") lay = layout({ fields: [rect(-0.4, -0.38, -0.02, 0.38, CROP.SOIL), rect(0.03, -0.38, 0.4, 0.38, h(1) < 0.5 ? CROP.GREEN : CROP.SOIL)] });
  return lay && rivers ? clear(lay, rivers) : lay;
}

/** Whether plot-local point (lx, lz) of a plot (fieldLayout's arguments) lies on one of its fields
 *  or in its paddock, or within `pad` of one: loose plants and the river's bank dressing keep off
 *  the crops and out of the paddock. */
export function inField(i, type, bonus, rivers, lx, lz, pad = 0) {
  const lay = fieldLayout(i, type, bonus, rivers);
  if (!lay) return false;
  const p = lay.paddock;
  if (p && ((lx - p.x) / (p.rx + pad)) ** 2 + ((lz - p.z) / (p.rz + pad)) ** 2 < 1) return true;
  return lay.fields.some((f) => lx > f.x0 - pad && lx < f.x1 + pad && lz > f.z0 - pad && lz < f.z1 + pad);
}

/** Which side (x: 1 east, -1 west) a yard at the back of the plot takes: away from a river along a
 *  side edge or round a back corner, else `dflt` (the plot's own pick). */
function yardSide(rivers, dflt) {
  const e = (rivers & (RIV.E | RIV.NE)) !== 0, w = (rivers & (RIV.W | RIV.NW)) !== 0;
  return e && !w ? -1 : w && !e ? 1 : dflt;
}

/** A layout laid out over the whole plot, kept to the ground its rivers leave: squeezed off each
 *  river edge to the bank top (an affine map per axis, the identity without a river), then cut
 *  back from a corner a river touches -- a field to the larger of what is left beside or behind
 *  the corner, a fence run broken off, a stook dropped. */
function clear(lay, rivers) {
  const axis = (lo, hi) => (v) => lo + ((v + REACH) / (2 * REACH)) * (hi - lo);
  const X = axis(rivers & RIV.W ? -DRY : -REACH, rivers & RIV.E ? DRY : REACH);
  const Z = axis(rivers & RIV.N ? -DRY : -REACH, rivers & RIV.S ? DRY : REACH);
  // The corners a river touches, as [sx, sz]: the square past DRY on both axes toward them.
  const wet = [[RIV.NE, 1, -1], [RIV.SE, 1, 1], [RIV.SW, -1, 1], [RIV.NW, -1, -1]].filter(([b]) => rivers & b).map(([, sx, sz]) => [sx, sz]);
  let fields = lay.fields.map((f) => rect(X(f.x0), Z(f.z0), X(f.x1), Z(f.z1), f.crop));
  for (const [sx, sz] of wet) fields = fields.map((f) => cutCorner(f, sx, sz)).filter(Boolean);
  const fences = [];
  for (const [a, b] of lay.fences) fences.push(...outside([X(a[0]), Z(a[1])], [X(b[0]), Z(b[1])], wet));
  const stooks = lay.stooks.map(([x, z]) => [X(x), Z(z)]).filter(([x, z]) => !wet.some(([sx, sz]) => x * sx > DRY - 0.03 && z * sz > DRY - 0.03));
  const yard = lay.yard && { ...lay.yard, x: X(lay.yard.x), z: Z(lay.yard.z) };
  const worn = lay.worn.map(([x, z, rx, rz, a]) => [X(x), Z(z), rx, rz, a]);
  return { ...lay, fields, fences, stooks, yard, worn };
}

/** Field `f` clear of the corner square toward [sx, sz]: as it is when it stays out of it, else
 *  the larger of its parts beside and behind the square (null when that is a sliver). */
function cutCorner(f, sx, sz) {
  const inX = sx > 0 ? f.x1 > DRY : f.x0 < -DRY, inZ = sz > 0 ? f.z1 > DRY : f.z0 < -DRY;
  if (!inX || !inZ) return f;
  const a = sx > 0 ? { ...f, x1: DRY } : { ...f, x0: -DRY };
  const b = sz > 0 ? { ...f, z1: DRY } : { ...f, z0: -DRY };
  const area = (r) => Math.max(0, r.x1 - r.x0) * Math.max(0, r.z1 - r.z0);
  const best = area(a) >= area(b) ? a : b;
  return Math.min(best.x1 - best.x0, best.z1 - best.z0) < 0.06 ? null : best;
}

/** The parts of fence run a-b outside the `wet` corner squares (fieldLayout's fences: straight
 *  runs), leaving out stubs too short for a panel. */
function outside(a, b, wet) {
  let spans = [[0, 1]];
  const dx = b[0] - a[0], dz = b[1] - a[1];
  for (const [sx, sz] of wet) {
    // Where along the run it is past DRY toward the corner on both axes.
    let t0 = 0, t1 = 1;
    for (const [p, d, s] of [[a[0], dx, sx], [a[1], dz, sz]]) {
      if (Math.abs(d) < 1e-9) {
        if (p * s <= DRY) t1 = -1;
      } else {
        const t = (DRY - p * s) / (d * s);
        if (d * s > 0) t0 = Math.max(t0, t);
        else t1 = Math.min(t1, t);
      }
    }
    if (t1 <= t0) continue;
    spans = spans.flatMap(([u0, u1]) => [[u0, Math.min(u1, t0)], [Math.max(u0, t1), u1]].filter(([v0, v1]) => v1 > v0));
  }
  const L = Math.hypot(dx, dz);
  return spans.filter(([u0, u1]) => (u1 - u0) * L > 0.08).map(([u0, u1]) => [[a[0] + dx * u0, a[1] + dz * u0], [a[0] + dx * u1, a[1] + dz * u1]]);
}

/** A farm: two fields across the front of the plot, the farmstead's yard in a back corner with a
 *  third field beside it; fences along the front and one side; stooks on the grain. */
function farm(h, grain, rivers) {
  const side = yardSide(rivers, h(0) < 0.5 ? -1 : 1); // the farmstead's side
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
  const yard = { x: side * 0.27, z: -0.27, side };
  return layout({ fields, yard, fences, stooks, grain, worn: yardWorn(yard) });
}

/** A paddock: a pasture's grazed turf, inside the ring of rails the paddock prop raises round it
 *  (props.js imp.paddock: the gate at the front, the shelter and trough at the back), trodden bare
 *  at the gate, round the trough and where the herd stands about; or, unfenced, the patch of
 *  trodden earth mushrooms spring from. It keeps its size where it fits, moved off a river edge to
 *  the bank top, and shrinks only between two (`sx`, `sz`: the prop's scale). Its flanks keep
 *  about 0.3 from the plot's corners, clear of a river's water round one. */
function paddock(h, mush, rivers) {
  // The patch's radii, and how far the rails reach (they wobble 4% off the ellipse).
  const R = mush ? [0.21, 0.18] : [0.36, 0.3], reach = mush ? 1 : 1.04;
  const c0 = mush ? [0.04, -0.13] : [0, 0];
  const lo = [rivers & RIV.W ? -DRY : -REACH, rivers & RIV.N ? -DRY : -REACH];
  const hi = [rivers & RIV.E ? DRY : REACH, rivers & RIV.S ? DRY : REACH];
  const fit = (k) => {
    const r = Math.min(R[k] * reach, (hi[k] - lo[k]) / 2);
    return [clamp(c0[k], lo[k] + r, hi[k] - r), r / reach / R[k]];
  };
  const [x, sx] = fit(0), [z, sz] = fit(1);
  const rx = R[0] * sx, rz = R[1] * sz;
  if (mush) return layout({ paddock: { x, z, rx, rz, sx, sz, variant: 0, turf: false }, worn: [[x, z, rx, rz, 0.4], [x, z + 0.02, rx * 0.55, rz * 0.55, 0.6]] });
  const variant = h(1) < 0.5 ? 0 : 1;
  // Bare patches where the herd stands about, a track from the gate to the trough, and the ground
  // round the trough and the shelter (props.js buildPaddock) and in the gateway.
  const worn = [];
  for (let k = 0; k < 4; k++) {
    const a = h(10 + k) * Math.PI * 2, d = 0.3 + 0.45 * h(20 + k);
    worn.push([x + Math.cos(a) * rx * d, z + Math.sin(a) * rz * d, 0.05 + 0.04 * h(30 + k), 0.04 + 0.03 * h(40 + k), 0.8]);
  }
  const tx = x + (variant ? 0.08 : -0.08) * sx, tz = z - 0.19 * sz;
  for (let k = 1; k < 4; k++) worn.push([x + ((tx - x) * k) / 4, z + rz + ((tz - z - rz) * k) / 4, 0.04, 0.035, 0.85]);
  worn.push([tx, tz, 0.13, 0.075, 1], [x, z + rz, 0.08, 0.06, 1]);
  return layout({ paddock: { x, z, rx, rz, sx, sz, variant, turf: true }, worn });
}
