// Dressing the 3D world from the game state (docs/R3D.md "Placement"): which models stand where,
// from the same `map` and `view` the other renderers draw. Trees fill wooded plots, rocks the hills
// and peaks, structures the improved plots, settlements the cities, figures the unit stacks.
//
// Everything is a pure function of the state and the plot index (seeded per plot), so the same
// state always gives the same world, and only what the player knows is placed: fogged plots show
// what was seen there (the map reply's remembered improvements and cities), unrevealed plots show
// nothing, and units come from the view (only those in sight).

import { S, tname, me, isBarbarian } from "../state.js";
import * as T from "../tiles.js";
import { headUnit, stackState } from "../stacks.js";
import { rng, hash32, hashf } from "./noise.js";
import { hexLinear, clamp } from "./math.js";
import { KIND, FEAT, GROUND, SEA, DRAFT } from "./world.js";
import { CATALOG } from "./assets/catalog.js";
import { hasRecipe, buildAsset } from "./assets/registry.js";

/** Where on a plot things stand (plot-local, x east / z south, plot half-size 0.5). */
const UNIT_SPOT = [0, 0.04];
// The garrison stands at the city's front corner, not the gate centre, so it no longer covers the
// settlement's landmark and houses (F10). The badge still carries the stack count.
const CITY_UNIT_SPOT = [-0.27, 0.32];
const VOLCANO_UNIT_SPOT = [-0.14, 0.38];
const STRUCTURE_SPOT = [0.02, -0.12];
const BONUS_SPOT = [-0.26, 0.2];

let nextId = 1;
const ids = new Map();
/** A stable object id in 0..1 for outlines (one per key). */
function idOf(key) {
  let v = ids.get(key);
  if (v === undefined) {
    v = ((hash32(nextId++ * 2654435761) % 9973) + 1) / 9975;
    ids.set(key, v);
  }
  return v;
}

function teamColour(owner) {
  return T.playerColor(owner, isBarbarian(owner));
}

function hexRgb(hex) {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  return m ? [parseInt(m[1], 16) / 255, parseInt(m[2], 16) / 255, parseInt(m[3], 16) / 255] : [0.6, 0.6, 0.6];
}

/** An instance record (scene.js packs these into instance buffers). */
function inst(asset, x, y, z, o = {}) {
  return {
    asset,
    variant: o.variant ?? 0,
    param: o.param ?? "",
    x,
    y,
    z,
    yaw: o.yaw ?? 0,
    sx: o.sx ?? o.scale ?? 1,
    sy: o.sy ?? o.scale ?? 1,
    sz: o.sz ?? o.scale ?? 1,
    phase: o.phase ?? 0,
    tint: o.tint ?? [1, 1, 1],
    id: o.id ?? 0.0001,
    team: o.team ?? [1, 1, 1],
    protect: o.protect ?? 0,
    plot: o.plot ?? -1,
  };
}

const pick = (name, fallback) => (name && hasRecipe(name) ? name : fallback);

/** How far a figure's feet (or paws, or coils) reach from its centre, in plots: about half its
 *  modelled width and depth. Wings and tails reach further but stand in the air. */
function footOf(asset, variant, param, scale) {
  const m = buildAsset(asset, variant, param);
  const ext = Math.max(-m.lo[0], m.hi[0], -m.lo[2], m.hi[2]);
  return Math.min(0.42, ext * 0.5 * scale);
}

/** The height a figure with feet reaching `foot` from (mx, my) stands at: the highest ground under
 *  its feet, so that a big creature on a mountainside stands on the slope instead of in it. */
export function standHeight(world, mx, my, foot = 0) {
  let h = world.heightAt(mx, my);
  if (foot < 0.03) return h;
  for (let k = 0; k < 8; k++) {
    const a = (k / 8) * Math.PI * 2;
    h = Math.max(h, world.heightAt(mx + Math.cos(a) * foot, my + Math.sin(a) * foot));
  }
  return h;
}

/** What the static dressing reads from the view: the cities and where units stand (by plot). */
export function staticEnv(world) {
  const cityAt = new Map();
  for (const c of S.cities.values()) cityAt.set(c.y * world.w + c.x, c);
  const unitAt = new Set();
  for (const units of S.unitsAt.values()) unitAt.add(units[0].y * world.w + units[0].x);
  return { cityAt, unitAt };
}

/** The static dressing of the map: trees, plants, rocks, features, structures, resources and
 *  settlements, and the particle emitters. `world`: world.js; `plots`: only these plot indices
 *  (all when null). Each plot's dressing depends only on the state of the plot and its
 *  neighbours, so a change redresses the plots around it (map3d.js). Returns `{list, emitters,
 *  cities, byPlot}` (byPlot: plot index -> {list, emitters, city}). */
export function dressStatic(world, map, plots = null, env = staticEnv(world)) {
  const out = [];
  const emitters = [];
  const cities = [];
  const byPlot = new Map();
  const w = world.w;
  const { cityAt, unitAt } = env;
  const H = (mx, my) => world.heightAt(mx, my);
  const visit = plots ?? Array.from({ length: w * world.h }, (_, k) => k);
  let base = 0;

  for (const i of visit) {
    const x = i % w, y = (i - x) / w;
    const o0 = out.length, e0 = emitters.length, c0 = cities.length;
    base = o0;
    dressPlot();
    byPlot.set(i, { list: out.slice(o0), emitters: emitters.slice(e0), city: cities.length > c0 ? cities[c0] : null });

    function dressPlot() {
      const kind = world.kind[i];
      if (kind === KIND.VOID) return;
      const feat = world.feat[i];
      const ground = world.ground[i];
      const city = cityAt.get(i);
      const imp = map.improvement[i] >= 0 ? tname("ImprovementType", map.improvement[i]) : null;
      const bonus = map.bonus[i] >= 0 ? tname("BonusType", map.bonus[i]) : null;
      const held = !!city || unitAt.has(i) || !!imp;
      const r = rng(i * 31 + 7);
      // The plot owner's civilization and architectural family (a cottage in elven land is elven).
      const ownerId = map.owner[i] >= 0 ? map.owner[i] : -1;
      const ownerP = ownerId >= 0 ? S.players.get(ownerId) : null;
      const civ = ownerP ? tname("CivilizationType", ownerP.civ) : null;
      const family = civ ? CATALOG.family?.(civ, null) ?? "human" : "human";
      const team = ownerId >= 0 ? hexRgb(teamColour(ownerId)) : [0.62, 0.58, 0.5];
      const ctx = { r, x, y, i, kind, feat, ground, world, hills: kind === KIND.HILLS, peak: kind === KIND.PEAK, held, city: !!city, owner: ownerId, civ, family, unit: unitAt.has(i), bonus, improvement: imp, coast: shoreOf(world, x, y), river: map.river[i] > 0 };
      const at = (lx, lz) => [x + 0.5 + lx, y + 0.5 - lz]; // plot-local (x east, z south) -> map
      // Nothing grows in a river's channel.
      const nearRiver = (lx, lz) => world.riverCells.size > 0 && world.riverDist(x + 0.5 + lx, y + 0.5 - lz) < 0.2;
      ctx.nearRiver = nearRiver;
      // Something stands on the plot behind this one (north): its trees in front of it keep low.
      const north = world.index(x, y + 1);
      ctx.northHeld = north >= 0 && (unitAt.has(north) || cityAt.has(north));
      // A city behind this plot (north, or north-east / north-west): the trees here stand between
      // it and the camera, and a stand of great trees would hide it whole.
      const cityN = (dx) => {
        const j = world.index(x + dx, y + 1);
        return j >= 0 && cityAt.has(j);
      };
      ctx.cityBehind = { n: cityN(0), e: cityN(1), w: cityN(-1) };
      // A city on any neighbouring plot (wide crowns keep clear of its buildings).
      ctx.cityNear = false;
      for (let dy = -1; dy <= 1 && !ctx.cityNear; dy++) for (let dx = -1; dx <= 1; dx++) if ((dx || dy) && cityAt.has(world.index(x + dx, y + dy))) ctx.cityNear = true;
      const place = (asset, lx, lz, o = {}) => {
        const [mx, my] = at(lx, lz);
        // A floating thing rides at the sea's level, a hull a little under it (its draft).
        const yy = o.float ? Math.max(SEA - (o.draft ?? 0), H(mx, my)) : H(mx, my);
        out.push(inst(asset, mx, yy + (o.lift ?? 0), -my, { plot: i, id: o.id ?? idOf(`s${i}:${out.length - base}`), ...o }));
      };

      // Wooded plots: a stand of trees, thinned toward open neighbours, with a glade where a unit,
      // city or structure holds the plot.
      if (feat === FEAT.FOREST || feat === FEAT.JUNGLE || feat === FEAT.ANCIENT || feat === FEAT.BURNT || feat === FEAT.YOUNG) {
        if (!city) placeTrees(world, ctx, place);
      } else if (kind === KIND.LAND || kind === KIND.HILLS) {
        // Open ground: a few loose plants.
        const flora = CATALOG.flora?.(ctx) ?? [{ asset: "basic.tree", scale: 0.45, count: r() < 0.25 ? 1 : 0 }];
        for (const f of flora) {
          for (let k = 0; k < (f.count ?? 1); k++) {
            const lx = r.range(-0.42, 0.42), lz = r.range(-0.42, 0.42);
            if (held && Math.hypot(lx, lz) < 0.3) continue;
            if (city) continue;
            if (nearRiver(lx, lz)) continue;
            place(pick(f.asset, "basic.tree"), lx, lz, { variant: f.variant ?? r.int(0, 7), param: f.param, scale: (f.scale ?? 1) * r.range(0.8, 1.15), yaw: r.range(0, 6.28), phase: r(), tint: f.tint });
          }
        }
      }
      // Rocks on hills, peaks and rough ground.
      const rocks = CATALOG.rocks?.(ctx) ?? (kind === KIND.HILLS || kind === KIND.PEAK ? [{ asset: "basic.rock", scale: 1, count: kind === KIND.PEAK ? 3 : 1 }] : []);
      for (const rk of rocks) {
        for (let k = 0; k < (rk.count ?? 1); k++) {
          let lx = r.range(-0.4, 0.4), lz = r.range(-0.4, 0.4);
          // prefer "ridge": the highest of a few spots, so crags crown the ridge line instead of
          // dotting the flanks.
          if (rk.prefer === "ridge") {
            let best = H(...at(lx, lz));
            for (let t = 0; t < 5; t++) {
              const cx = r.range(-0.4, 0.4), cz = r.range(-0.4, 0.4);
              const hh = H(...at(cx, cz));
              if (hh > best) [best, lx, lz] = [hh, cx, cz];
            }
          }
          if (held && Math.hypot(lx, lz) < 0.32) continue;
          place(pick(rk.asset, "basic.rock"), lx, lz, { variant: rk.variant ?? r.int(0, 7), param: rk.param, scale: (rk.scale ?? 1) * r.range(0.75, 1.3), yaw: r.range(0, 6.28), lift: -0.01, tint: rk.tint });
        }
      }
      // One-off features (oasis, volcano, flood reeds, ice floes, flames).
      const feats = CATALOG.feature?.(feat, ctx) ?? [];
      for (const f of feats) {
        if (city && !f.always) continue;
        place(f.asset, f.dx ?? 0, f.dz ?? 0, { variant: f.variant ?? r.int(0, 7), param: f.param, scale: f.scale ?? 1, yaw: f.yaw ?? r.range(0, 6.28), float: f.float, draft: f.draft, tint: f.tint });
      }
      // The improvement's structure (not under a city: the settlement replaces it).
      if (imp && !city) {
        // Settlement-like improvements (cottages to towns, forts, towers) belong to the
        // architecture; the rest (farms, mines, lairs, mana nodes, ...) to the props.
        const got = CATALOG.settlement?.(imp, ctx) ?? CATALOG.improvement?.(imp, ctx) ?? null;
        const parts = Array.isArray(got) ? got : got ? [got] : [];
        const spot = unitAt.has(i) ? STRUCTURE_SPOT : [0, -0.02];
        const iid = idOf(`imp${i}`);
        for (const im of parts) {
          const px = im.dx ?? spot[0], pz = im.dz ?? spot[1];
          const yaw = im.yaw ?? r.range(-0.4, 0.4), sc = im.scale ?? 1;
          place(im.asset, px, pz, { variant: im.variant ?? r.int(0, 7), param: im.param ?? family, scale: sc, sx: im.sx, sy: im.sy, sz: im.sz, yaw, float: im.float, draft: im.draft, id: im.id ?? iid, team, tint: im.tint });
          for (const e of im.emit ?? []) emitters.push(partEmitter(e, x, y, px, pz, yaw, sc, im.float, H));
        }
      }
      // The resource's marker.
      if (bonus) {
        const b = CATALOG.bonus?.(bonus, ctx) ?? null;
        if (b) {
          const water = world.isWater(i);
          place(b.asset, b.dx ?? BONUS_SPOT[0], b.dz ?? BONUS_SPOT[1], { variant: b.variant ?? r.int(0, 7), param: b.param, scale: b.scale ?? 1, yaw: r.range(-0.6, 0.6), float: water, id: idOf(`bonus${i}`), tint: b.tint });
          for (const e of b.emit ?? []) emitters.push({ ...e, x: x + 0.5 + BONUS_SPOT[0], y: Math.max(SEA, H(x + 0.5, y + 0.5)) + (e.dy ?? 0.1), z: -(y + 0.5 - BONUS_SPOT[1]) });
        }
      }
      // Particles of the ground itself (hell's embers, ancient forests' motes).
      for (const e of CATALOG.emitters?.("plot", ctx) ?? []) emitters.push({ ...e, x: x + 0.5 + (e.dx ?? 0), y: H(x + 0.5, y + 0.5) + (e.dy ?? 0.05), z: -(y + 0.5 - (e.dz ?? 0)) });
      // Bridges where a road crosses a river edge to a routed neighbour (south and east edges;
      // each crossing belongs to one plot). The road runs straight across (decals.js), and the
      // bridge stands where it passes nearest the river's (smoothed, meandering) line, its deck at
      // the higher bank top (about 0.3 either side of the line, past water at most 0.44 wide): the
      // bridges span 0.68, their footings on the bank tops.
      if (map.route[i] >= 0 && map.river[i] > 0) {
        const bridge = (nx, ny, mx, my, yaw) => {
          const j = world.index(nx, ny);
          if (j < 0 || map.route[j] < 0 || map.vis[j] === 0) return;
          const ux = nx - x, uy = ny - y; // along the road, map units
          let best = Infinity, cx = mx, cy = my;
          for (let k = -8; k <= 8; k++) {
            const px = mx + ux * k * 0.03, py = my + uy * k * 0.03;
            const d = world.riverDist(px, py);
            if (d < best) [best, cx, cy] = [d, px, py];
          }
          const bank = Math.max(H(cx + ux * 0.32, cy + uy * 0.32), H(cx - ux * 0.32, cy - uy * 0.32), SEA + 0.01);
          const b = CATALOG.bridge?.({ ...ctx, crossing: j, alongX: yaw !== 0 }) ?? null;
          const asset = typeof b === "string" ? b : b?.asset;
          out.push(inst(pick(asset, "basic.bridge"), cx, bank - 0.006, -cy, { yaw, variant: b?.variant ?? r.int(0, 1), plot: i, id: idOf(`bridge${i}:${yaw}`) }));
        };
        if (map.river[i] & 1) bridge(x, y - 1, x + 0.5, y, 0);
        if (map.river[i] & 2) bridge(x + 1, y, x + 1, y + 0.5, Math.PI / 2);
      }
      // A settlement.
      if (city) {
        const owner = S.players.get(city.owner);
        const civ = owner ? tname("CivilizationType", owner.civ) : null;
        const family = CATALOG.family?.(civ, city) ?? "human";
        const team = hexRgb(teamColour(city.owner));
        const cr = rng(hash32(city.id * 977 + 13));
        const cctx = { r: cr, size: city.size ?? 1, capital: !!city.capital, walls: !!city.walls, family, civ, seed: city.id, height: (dx, dz) => H(x + 0.5 + dx, y + 0.5 - dz), world, x, y, coast: ctx.coast, river: ctx.river };
        const parts = CATALOG.city?.(cctx) ?? basicCity(cctx);
        for (const e of parts.emit ?? []) emitters.push({ ...e, x: x + 0.5 + (e.dx ?? 0), y: H(x + 0.5 + (e.dx ?? 0), y + 0.5 - (e.dz ?? 0)) + (e.dy ?? 0.2), z: -(y + 0.5 - (e.dz ?? 0)) });
        const cid = idOf(`city${city.id}`);
        for (const p of parts) {
          const [mx, my] = at(p.x, p.z);
          out.push(inst(pick(p.asset, "basic.house"), mx, H(mx, my) + (p.lift ?? -0.005), -my, { variant: p.variant ?? 0, param: p.param ?? family, yaw: p.yaw ?? 0, scale: p.scale ?? 1, sx: p.sx, sy: p.sy, sz: p.sz, team, plot: i, id: p.id ?? cid, tint: p.tint }));
        }
        for (const e of CATALOG.emitters?.("city", cctx) ?? []) emitters.push({ ...e, x: x + 0.5 + (e.dx ?? 0), y: H(x + 0.5, y + 0.5) + (e.dy ?? 0.2), z: -(y + 0.5 - (e.dz ?? 0)) });
        cities.push({ city, x: x + 0.5, y: H(x + 0.5, y + 0.5), z: -(y + 0.5) });
      }
    }
  }
  return { list: out, emitters, cities, byPlot };
}

/** Where the water is around plot (x, y): the plot-local directions ([dx, dz], x east, z south)
 *  of its water neighbours (sea, coast or lake), the four edges first. Empty inland. */
function shoreOf(world, x, y) {
  const out = [];
  for (const [dx, dy] of [[0, -1], [1, 0], [0, 1], [-1, 0], [1, -1], [1, 1], [-1, 1], [-1, -1]]) {
    const j = world.index(x + dx, y + dy);
    if (j >= 0 && world.isWater(j)) out.push([dx, -dy]);
  }
  return out;
}

/** An emitter carried by an improvement's part (`e.dx/dy/dz` in the part's own frame), placed in
 *  the world: turned with the part's yaw (as the prop shader turns it) and scaled with it. */
function partEmitter(e, x, y, px, pz, yaw, sc, float, H) {
  const ex = (e.dx ?? 0) * sc, ez = (e.dz ?? 0) * sc;
  const c = Math.cos(yaw), s = Math.sin(yaw);
  const wx = px + c * ex + s * ez, wz = pz - s * ex + c * ez;
  const mx = x + 0.5 + wx, my = y + 0.5 - wz;
  const ground = float ? Math.max(SEA, H(mx, my)) : H(mx, my);
  return { ...e, x: mx, y: ground + (e.dy ?? 0.1) * sc, z: -my };
}

/** A wooded plot's trees: a jittered grid of slots, each a tree of the plot's species mix, thinned
 *  toward open neighbours (soft forest edges) and cleared around the centre when the plot is held. */
function placeTrees(world, ctx, place) {
  const { r, x, y, feat } = ctx;
  const mix = CATALOG.forest?.(feat, ctx) ?? [{ asset: feat === FEAT.FOREST && (ctx.ground === GROUND.TUNDRA || ctx.ground === GROUND.SNOW || ctx.hills) ? "basic.pine" : "basic.tree", scale: feat === FEAT.YOUNG ? 0.6 : 1 }];
  const total = mix.reduce((a, m) => a + (m.weight ?? 1), 0);
  const choose = () => {
    let t = r() * total;
    for (const m of mix) if ((t -= m.weight ?? 1) <= 0) return m;
    return mix[mix.length - 1];
  };
  const wooded = (dx, dy) => {
    const j = world.index(x + dx, y + dy);
    if (j < 0) return false;
    const f = world.feat[j];
    return f === FEAT.FOREST || f === FEAT.JUNGLE || f === FEAT.ANCIENT || f === FEAT.BURNT || f === FEAT.YOUNG;
  };
  const open = { e: !wooded(1, 0), w: !wooded(-1, 0), n: !wooded(0, 1), s: !wooded(0, -1) };
  const grid = mix[0].grid ?? (feat === FEAT.ANCIENT ? 2 : 4);
  const cell = 1 / grid;
  for (let gy = 0; gy < grid; gy++) {
    for (let gx = 0; gx < grid; gx++) {
      const lx = -0.5 + (gx + 0.5) * cell + r.range(-0.42, 0.42) * cell;
      const lz = -0.5 + (gy + 0.5) * cell + r.range(-0.42, 0.42) * cell;
      // Forest edges thin out toward open ground.
      let keep = 1;
      if (open.e) keep *= clamp((0.5 - lx) / 0.35, 0.25, 1);
      if (open.w) keep *= clamp((lx + 0.5) / 0.35, 0.25, 1);
      if (open.s) keep *= clamp((0.5 - lz) / 0.35, 0.25, 1);
      if (open.n) keep *= clamp((lz + 0.5) / 0.35, 0.25, 1);
      if (r() > keep) continue;
      // A glade where something holds the plot (and in front of it, toward the camera).
      if (ctx.held && (Math.hypot(lx, lz) < 0.26 || (lz > 0 && Math.abs(lx) < 0.2))) continue;
      if (ctx.nearRiver(lx, lz)) continue;
      const m = choose();
      let s = (m.scale ?? 1) * r.range(0.8, 1.2);
      // Adjacent to a city, keep the trees shorter so the town (and its own great tree) shows over
      // the canopy instead of being buried (F2: Evermore in the ancient wood).
      if (ctx.cityNear) s *= 0.75;
      // The back row of this plot stands just in front of whatever holds the plot behind it.
      if (ctx.northHeld && lz < -0.15 && Math.abs(lx) < 0.3) s *= 0.6;
      // In front of a city the wood opens: clear on the half of the plot toward it, low beyond
      // (a tree here hides what stands up to about its own height behind it, and a city's houses
      // are a sixth of a plot tall).
      const cb = ctx.cityBehind;
      if (cb && (cb.n || cb.e || cb.w)) {
        const toward = (cb.n ? 1 : 0) * clamp(0.5 - lz, 0, 1) + (cb.e ? clamp(lx + 0.1, 0, 1) * clamp(0.3 - lz, 0, 1) : 0) + (cb.w ? clamp(0.1 - lx, 0, 1) * clamp(0.3 - lz, 0, 1) : 0);
        if (toward > 0.5) continue;
        s *= 1 - toward * 0.9;
      }
      place(m.asset, lx, lz, { variant: m.variant ?? r.int(0, 15), param: m.param, scale: s, yaw: r.range(0, 6.28), phase: r(), tint: m.tint, lift: -0.008 });
    }
  }
  // Undergrowth between the trunks.
  for (const u of mix.under ?? []) {
    for (let k = 0; k < (u.count ?? 2); k++) {
      const lx = r.range(-0.45, 0.45), lz = r.range(-0.45, 0.45);
      if (ctx.held && Math.hypot(lx, lz) < 0.26) continue;
      if (ctx.nearRiver(lx, lz)) continue;
      place(u.asset, lx, lz, { variant: u.variant ?? r.int(0, 7), param: u.param, scale: (u.scale ?? 1) * r.range(0.7, 1.2), yaw: r.range(0, 6.28), phase: r() });
    }
  }
}

/** The fallback settlement: houses in a ring around a small square, more with size. */
function basicCity(c) {
  const out = [];
  const n = Math.min(14, 3 + c.size * 2);
  for (let k = 0; k < n; k++) {
    const a = (k / n) * Math.PI * 2 + c.r.range(-0.2, 0.2);
    const d = 0.18 + (k % 2) * 0.12 + c.r.range(0, 0.06);
    const x = Math.cos(a) * d, z = Math.sin(a) * d;
    if (z > 0.15 && Math.abs(x) < 0.15) continue; // the gate's square
    out.push({ asset: "basic.house", variant: k, x, z, yaw: -a + Math.PI / 2, scale: 1 });
  }
  return out;
}

/** The unit stacks in sight: one figure for the stack's top unit at the plot centre (at a city's
 *  gate on a city plot), with up to two of its company behind it. */
export function dressUnits(world, map) {
  const out = [];
  const stacks = [];
  const w = world.w;
  const mine = me()?.player;
  const sel = S.sel?.units ?? [];
  // The stack's face is its best defender, as the 2D art shows (F4): topUnit returned units[0] for
  // a foreign stack, so a worker hid the armed unit behind it. headUnit picks the strongest.
  const strength = (u) => S.data?.tables?.UnitType?.[u.type]?.combat ?? 0;
  // The same ctx the art view builds, so stackState also gives the activity mark and hero for the
  // player's own stacks (F19). combat here is a proxy string (fights when its strength > 0); the
  // hero pip needs the art catalog and is left for later.
  const sctx = {
    me: mine,
    selected: sel,
    unitInfo: (t) => { const s = strength({ type: t }); return { hero: false, combat: s > 0 ? "COMBAT" : "NONE", strength: s }; },
    activity: (a) => tname("ActivityType", a),
    automated: (g) => { const grp = S.groups?.get?.(g); return !!grp && grp.automated !== null && grp.automated !== undefined; },
  };
  for (const units of S.unitsAt.values()) {
    const u0 = units[0];
    const i = u0.y * w + u0.x;
    const st = stackState(units, sctx);
    const top = st?.head ?? units[0];
    const onCity = map.city[i] >= 0;
    // On a volcano the cone fills the plot: the stack stands at its foot, toward the camera.
    const spot = onCity ? CITY_UNIT_SPOT : world.feat[i] === FEAT.VOLCANO ? VOLCANO_UNIT_SPOT : UNIT_SPOT;
    const team = hexRgb(teamColour(top.owner));
    // Company behind the head, strongest first (the badge count is unchanged).
    const order = [top, ...units.filter((u) => u !== top).sort((a, b) => strength(b) - strength(a))].slice(0, 3);
    const slots = [[0, 0], [-0.17, -0.12], [0.17, -0.12]];
    let anchor = null;
    order.forEach((u, k) => {
      const name = tname("UnitType", u.type);
      const owner = S.players.get(u.owner);
      const civ = owner ? tname("CivilizationType", owner.civ) : null;
      const info = S.data?.tables?.UnitType?.[u.type] ?? null;
      const domain = info ? tname("DomainType", info.domain) : null;
      const combat = info ? tname("UnitCombatType", info.unit_combat) : null;
      const look = CATALOG.unit?.(name, { info, civ, unit: u, domain, combat, family: CATALOG.family?.(civ, null) ?? "human", r: rng(u.id), name }) ?? { asset: "basic.unit", param: "" };
      const lx = spot[0] + slots[k][0];
      const lz = spot[1] + slots[k][1];
      const mx = u0.x + 0.5 + lx;
      const my = u0.y + 0.5 - lz;
      const s = (look.scale ?? 1) * (k === 0 ? 1 : onCity ? 0.6 : 0.78);
      const asset = pick(look.asset, "basic.unit");
      const foot = look.float || look.fly ? 0 : footOf(asset, look.variant ?? 0, look.param ?? "", s);
      const ground = standHeight(world, mx, my, foot);
      // Every floating unit is a hull or a swimmer: it sits at the draft. A land unit on a sea plot
      // is cargo: it stands at the waterline (on the deck), not on the sea floor.
      const yy = look.float ? Math.max(SEA - DRAFT, ground) : look.fly ? Math.max(SEA, ground) + look.fly : Math.max(SEA, ground);
      const it = inst(asset, mx, yy, -my, { param: look.param ?? "", variant: look.variant ?? 0, yaw: look.yaw ?? (k === 0 ? 0.25 : 0.25 + (k === 1 ? 0.3 : -0.3)), scale: s, phase: hashf(u.id, 3), team, protect: 1, id: idOf(`u${u.id}`), plot: i });
      it.uid = u.id;
      it.ground = !look.float && !look.fly;
      it.foot = foot;
      out.push(it);
      if (k === 0) anchor = { x: mx, y: yy, z: -my, height: (look.height ?? 0.5) * s };
    });
    stacks.push({ units, top, x: u0.x, y: u0.y, k: i, anchor, picked: sel.includes(top.id), team: teamColour(top.owner), spot, mark: st?.state?.mark ?? null, hero: st?.state?.hero ?? false });
  }
  return { list: out, stacks };
}

export { hexRgb, teamColour };
export { hexLinear };
