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
import { hexLinear, clamp, smoothstep } from "./math.js";
import { KIND, FEAT, GROUND, SEA, DRAFT, RIVER_HALF, RIVER_WANDER, RIVER_BANK } from "./world.js";
import { roadLinks, inField, riversAround } from "./fields.js";
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
      const nearRiver = (lx, lz) => world.riverCells.size > 0 && world.riverDist(x + 0.5 + lx, y + 0.5 - lz) < RIVER_HALF + RIVER_WANDER + 0.05;
      ctx.nearRiver = nearRiver;
      // The rivers round the plot, as its fields keep off them (fields.js fieldLayout).
      ctx.rivers = riversAround(world, map, x, y);
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
      // A great town spills over its plot (the catalog's cityReach, up to ~0.9): nothing grows or
      // lies within its reach, on its own plot or the neighbouring ones, else trees and rocks stand
      // inside its walls.
      const towns = city ? [[0, 0, (CATALOG.cityReach?.({ size: city.size ?? 1, walls: !!city.walls, capital: !!city.capital }) ?? 0.45) + 0.06]] : [];
      if (ctx.cityNear) {
        for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
          const c = (dx || dy) && cityAt.get(world.index(x + dx, y + dy));
          if (c) towns.push([dx, -dy, (CATALOG.cityReach?.({ size: c.size ?? 1, walls: !!c.walls, capital: !!c.capital }) ?? 0.45) + 0.06]);
        }
      }
      ctx.inTown = (lx, lz) => towns.some(([cx, cz, R]) => Math.hypot(lx - cx, lz - cz) < R);
      // Wooded neighbours (open ground next to a wood takes more trees and bushes: the wood's
      // edge spills over) and the road through the plot (plants keep off it).
      ctx.woods = 0;
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) if ((dx || dy) && WOODED.has(world.feat[world.index(x + dx, y + dy)])) ctx.woods++;
      const roads = roadsOf(world, map, x, y, i);
      ctx.roadDist = (lx, lz) => roadDist(roads, lx, lz);
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
      } else if ((kind === KIND.LAND || kind === KIND.HILLS) && !city) {
        // Open ground: plants scattered in clumps, thick in some places and bare in others.
        const flora = CATALOG.flora?.(ctx) ?? [{ asset: "basic.tree", scale: 0.45, count: r() < 0.25 ? 1 : 0, size: 2 }];
        scatterFlora(world, ctx, place, flora);
      }
      // Rocks on hills, peaks and rough ground (off the roads and out of the river).
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
          if (ctx.roadDist(lx, lz) < 0.12 || nearRiver(lx, lz) || ctx.inTown(lx, lz)) continue;
          if (imp && inField(i, imp, bonus, ctx.rivers, lx, lz, 0.03)) continue; // not in the crops
          place(pick(rk.asset, "basic.rock"), lx, lz, { variant: rk.variant ?? r.int(0, 7), param: rk.param, scale: (rk.scale ?? 1) * r.range(0.75, 1.3), yaw: r.range(0, 6.28), lift: -0.01, tint: rk.tint });
        }
      }
      placeBank(world, map, ctx, place);
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
          const bank = Math.max(H(cx + ux * RIVER_BANK, cy + uy * RIVER_BANK), H(cx - ux * RIVER_BANK, cy - uy * RIVER_BANK), SEA + 0.01);
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

/** A river's banks (docs/R3D.md §5.2): boulders along the water's edge in clusters, now and then
 *  one out in the stream (the water breaks white round it), reeds standing in the shallows at the
 *  edge and grass leaning over it from the bank tops. Each plot dresses the banks in its own
 *  square from a seed of its own (its other dressing draws as before); nothing at a road's
 *  crossing (a bridge spans it), in a city, round a held plot's centre, on a farm's fields or in a
 *  paddock (fields.js inField) or out on the sea. */
function placeBank(world, map, ctx, place) {
  const { x, y, i } = ctx;
  if (!world.riverCells.size || ctx.city || world.isWater(i)) return;
  // Rivers run along plot edges: a plot with none within reach of its square has no bank.
  if (world.riverDist(x + 0.5, y + 0.5) > 0.5 + RIVER_BANK) return;
  const known = (j) => j >= 0 && map.vis[j] !== 0;
  const routed = (j) => known(j) && map.route[j] >= 0;
  const edge = (px, py, bit, nx, ny) => {
    const j = world.index(px, py);
    return known(j) && map.river[j] > 0 && (map.river[j] & bit) !== 0 && routed(j) && routed(world.index(nx, ny));
  };
  // The crossings on this plot's four edges (a south or east edge belongs to its plot, dress.js
  // bridge): where the roads cross, the banks stay clear for the bridge.
  const cross = [];
  if (edge(x, y, 1, x, y - 1)) cross.push([x + 0.5, y]);
  if (edge(x, y + 1, 1, x, y)) cross.push([x + 0.5, y + 1]);
  if (edge(x, y, 2, x + 1, y)) cross.push([x + 1, y + 0.5]);
  if (edge(x - 1, y, 2, x, y)) cross.push([x, y + 0.5]);
  const r = rng(i * 131 + 17);
  const G = 8;
  for (let gy = 0; gy < G; gy++) {
    for (let gx = 0; gx < G; gx++) {
      // A fixed number of draws per slot, so the dressing of one slot never shifts another's.
      const lx = -0.5 + (gx + r()) / G, lz = -0.5 + (gy + r()) / G;
      const roll = r(), sz = r(), yaw = r.range(0, 6.28), variant = r.int(0, 7);
      const mx = x + 0.5 + lx, my = y + 0.5 - lz;
      const d = world.riverDist(mx, my);
      if (d > RIVER_BANK + 0.1) continue;
      if (cross.some(([cx, cy]) => Math.hypot(mx - cx, my - cy) < 0.42)) continue;
      if (ctx.held && Math.hypot(lx, lz) < 0.27) continue;
      if (ctx.inTown(lx, lz)) continue;
      if (ctx.improvement && inField(i, ctx.improvement, ctx.bonus, ctx.rivers, lx, lz, 0.03)) continue;
      if (world.heightAt(mx, my, false) < SEA) continue;
      // Clusters: rocks where the broad noise is high, reeds where it is low.
      const clump = clamp(0.5 + world.n2(mx, my, 1.3, 3.1, 7.7) * 1.2, 0, 1);
      const inWater = d < RIVER_HALF + RIVER_WANDER;
      if (d < RIVER_HALF - 0.09) {
        if (roll < 0.05 * clump) place(pick("rock.boulder", "basic.rock"), lx, lz, { variant, scale: 0.6 + sz * 0.35, yaw, lift: 0.03 });
      } else if (d < RIVER_HALF + 0.08) {
        if (roll < 0.75 * clump * clump) place(pick(variant < 3 ? "rock.boulder.moss" : "rock.boulder", "basic.rock"), lx, lz, { variant, scale: 0.5 + sz * 0.6, yaw, lift: inWater ? 0.018 : -0.004 });
        else if (d > RIVER_HALF - 0.05 && roll > 1 - 0.4 * (1 - clump)) place(pick("reeds", "basic.tree"), lx, lz, { variant, scale: 0.7 + sz * 0.5, yaw, phase: sz });
      } else if (d > RIVER_BANK - 0.06) {
        if (roll < 0.25 * clump * clump) place(pick("rock.boulder", "basic.rock"), lx, lz, { variant, scale: 0.35 + sz * 0.3, yaw, lift: -0.004 });
        else if (roll > 0.55) place(pick("plant.grass", "basic.tree"), lx, lz, { variant, scale: 1.0 + sz * 0.6, yaw, phase: sz });
      }
    }
  }
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

const WOODED = new Set([FEAT.FOREST, FEAT.JUNGLE, FEAT.ANCIENT, FEAT.BURNT, FEAT.YOUNG]);

/** The road through plot (x, y) as the decal layer paints it (decals.js: from the plot's centre to
 *  the edge midpoint toward each routed neighbour, bowed by a hash of the pair), as plot-local
 *  polylines (x east, z south); a routed plot with no routed neighbour is a dot at its centre. */
function roadsOf(world, map, x, y, i) {
  if (map.route[i] < 0) return [];
  const out = [];
  const riv = (k, bit) => map.river[k] > 0 && (map.river[k] & bit) !== 0;
  for (const [dx, dy] of roadLinks(world, map, x, y)) {
    const j = world.index(x + dx, y + dy);
    // As decals.js: straight over a river, onto its bridge.
    const bridged = (dy < 0 && !dx && riv(i, 1)) || (dy > 0 && !dx && riv(j, 1)) || (dx > 0 && !dy && riv(i, 2)) || (dx < 0 && !dy && riv(j, 2));
    const bow = bridged ? 0 : (hashf(Math.min(i, j), Math.max(i, j)) - 0.5) * 0.16 * (i < j ? 1 : -1);
    const l = Math.hypot(dx, dy);
    const kx = dx * 0.25 - (dy / l) * bow, ky = dy * 0.25 + (dx / l) * bow;
    const line = [];
    for (let k = 0; k <= 4; k++) {
      const t = k / 4;
      line.push([2 * (1 - t) * t * kx + t * t * dx * 0.5, -(2 * (1 - t) * t * ky + t * t * dy * 0.5)]);
    }
    out.push(line);
  }
  return out.length ? out : [[[0, 0]]];
}

/** The distance from plot-local (lx, lz) to the nearest of `roads` (roadsOf); Infinity off them. */
function roadDist(roads, lx, lz) {
  let best = Infinity;
  for (const line of roads) {
    if (line.length === 1) best = Math.min(best, Math.hypot(lx - line[0][0], lz - line[0][1]));
    for (let k = 0; k + 1 < line.length; k++) {
      const [ax, az] = line[k], [bx, bz] = line[k + 1];
      const ex = bx - ax, ez = bz - az;
      const t = clamp(((lx - ax) * ex + (lz - az) * ez) / (ex * ex + ez * ez || 1), 0, 1);
      best = Math.min(best, Math.hypot(lx - ax - ex * t, lz - az - ez * t));
    }
  }
  return best;
}

/** How far each size of plant keeps from things (flora `size`: 0 low clutter -- grass, flowers --
 *  1 bushes, 2 trees): its footprint, and its clearance from the unit's spot at the plot centre. */
const FOOT = [0.03, 0.055, 0.09];
const UNIT_CLEAR = [0.25, 0.32, 0.36];

/** Open ground's plants (the catalog's flora entries, catalog.js): each entry's `count` in clumps
 *  of `clump` ([min, max]) within `spread` of a centre, a clump kept or dropped by a slow density
 *  field anchored to the map (`dense`: how much it follows it), so meadows run thick in some places
 *  and bare in others across plot borders and trees gather in groves. Nothing stands on a road, in
 *  a river, on a beach or on a resource's marker; on an improved plot plants keep to its rim (its
 *  fields and buildings fill the rest); round a unit only low clutter, outside its ring; trees keep
 *  out of the way of what stands on the plot, behind it and in front of it. */
function scatterFlora(world, ctx, place, flora) {
  const { r, x, y } = ctx;
  const field = (lx, lz, f, o) => world.n2(x + 0.5 + lx, y + 0.5 - lz, f, o, o * 0.7);
  const meadow = (lx, lz) => smoothstep(-0.55, 0.45, field(lx, lz, 0.75, 3.1) + 0.5 * field(lx, lz, 2.1, 7.7));
  const grove = (lx, lz) => smoothstep(-0.3, 0.4, field(lx, lz, 0.4, 11.3) + 0.3 * field(lx, lz, 1.3, 5.9));
  const cityBehind = ctx.cityBehind.n || ctx.cityBehind.e || ctx.cityBehind.w;
  // The low clutter of a plot is one object to the ink (no outline between overlapping tufts).
  const low = idOf(`fl${ctx.i}`);
  const free = (lx, lz, size) => {
    if (Math.abs(lx) > 0.49 || Math.abs(lz) > 0.49) return false;
    const foot = FOOT[size];
    if (ctx.unit && Math.hypot(lx - UNIT_SPOT[0], lz - UNIT_SPOT[1]) < UNIT_CLEAR[size]) return false;
    if (ctx.improvement && Math.max(Math.abs(lx), Math.abs(lz)) < 0.38) return false;
    if (ctx.improvement && inField(ctx.i, ctx.improvement, ctx.bonus, ctx.rivers, lx, lz, foot)) return false;
    if (ctx.bonus && Math.hypot(lx - BONUS_SPOT[0], lz - BONUS_SPOT[1]) < 0.09 + foot) return false;
    if (ctx.roadDist(lx, lz) < 0.08 + foot) return false;
    if (ctx.inTown(lx, lz)) return false;
    if (world.riverCells.size > 0 && world.riverDist(x + 0.5 + lx, y + 0.5 - lz) < RIVER_HALF + RIVER_WANDER + 0.02 + foot) return false;
    if (ctx.coast.length && world.heightAt(x + 0.5 + lx, y + 0.5 - lz) < SEA + 0.02) return false;
    if (size === 2) {
      // A tree hides what stands up to about its height behind it: none in front of a unit or a
      // structure on this plot, in front of whatever holds the plot behind, or before a city.
      if ((ctx.unit || ctx.improvement) && lz > -0.12 && Math.abs(lx) < 0.32) return false;
      if (ctx.northHeld && lz < -0.15 && Math.abs(lx) < 0.32) return false;
      if (cityBehind && lz < 0.15) return false;
    }
    return true;
  };
  for (const f of flora) {
    const size = f.size ?? 0;
    const [c0, c1] = Array.isArray(f.clump) ? f.clump : [f.clump ?? 1, f.clump ?? 1];
    const spread = f.spread ?? 0.06, dense = f.dense ?? 0.7;
    const total = f.mix ? f.mix.reduce((a, m) => a + (m.weight ?? 1), 0) : 0;
    for (let left = f.count ?? 1; left > 0; ) {
      const n = Math.min(left, r.int(c0, c1));
      left -= n;
      const cx = r.range(-0.44, 0.44), cz = r.range(-0.44, 0.44);
      if (r() > 1 - dense + dense * (size === 2 ? grove(cx, cz) : meadow(cx, cz))) continue;
      for (let k = 0; k < n; k++) {
        // The first of a clump near its centre, the rest round it (trees a trunk's width apart).
        const a = r.range(0, Math.PI * 2), d = spread * (k ? 0.45 + 0.55 * Math.sqrt(r()) : 0.3 * r());
        const lx = cx + Math.cos(a) * d, lz = cz + Math.sin(a) * d;
        let m = f;
        if (f.mix) {
          let t = r() * total;
          for (const e of f.mix) if ((t -= e.weight ?? 1) <= 0) { m = e; break; }
        }
        const s = (m.scale ?? 1) * (f.mix ? f.scale ?? 1 : 1) * r.range(0.8, 1.15) * (size === 2 && ctx.cityNear ? 0.8 : 1);
        const yaw = r.range(0, 6.28), phase = r(), variant = m.variant ?? r.int(0, 7);
        if (!free(lx, lz, size)) continue;
        const o = { variant, param: m.param, scale: s, yaw, phase, tint: m.tint, lift: size === 2 ? -0.008 : 0 };
        if (size === 0) o.id = low;
        place(pick(m.asset, "basic.tree"), lx, lz, o);
      }
    }
  }
}

/** A wooded plot's trees: a jittered grid of slots, each a tree of the plot's species mix, thinned
 *  toward open neighbours (soft forest edges, where bushes and saplings stand instead), shorter
 *  trees darker (in the shade of the canopy), and cleared around the centre when the plot is held
 *  and off the road through it. */
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
  const edge = mix.edge ?? [];
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
      // A glade where something holds the plot (and in front of it, toward the camera).
      const glade = ctx.held && (Math.hypot(lx, lz) < 0.26 || (lz > 0 && Math.abs(lx) < 0.2));
      if (r() > keep) {
        // The wood's edge: bushes and saplings where the trees thin out.
        const e = edge.length && r() < 0.75 ? edge[r.int(0, edge.length - 1)] : null;
        if (e && !glade && !ctx.nearRiver(lx, lz) && !ctx.inTown(lx, lz) && ctx.roadDist(lx, lz) > 0.12) place(e.asset, lx, lz, { variant: e.variant ?? r.int(0, 7), param: e.param, scale: (e.scale ?? 1) * r.range(0.8, 1.2), yaw: r.range(0, 6.28), phase: r() });
        continue;
      }
      if (glade) continue;
      if (ctx.nearRiver(lx, lz) || ctx.inTown(lx, lz) || ctx.roadDist(lx, lz) < 0.1) continue;
      const m = choose();
      const v = r.range(0.75, 1.25);
      let s = (m.scale ?? 1) * v;
      // The shorter trees stand in the shade of the canopy: darker, the wood's interior.
      const dark = 0.78 + 0.22 * smoothstep(0.75, 1.15, v);
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
      const tint = (m.tint ?? [1, 1, 1]).map((c) => c * dark);
      place(m.asset, lx, lz, { variant: m.variant ?? r.int(0, 15), param: m.param, scale: s, yaw: r.range(0, 6.28), phase: r(), tint, lift: -0.008 });
    }
  }
  // Undergrowth between the trunks.
  for (const u of mix.under ?? []) {
    for (let k = 0; k < (u.count ?? 2); k++) {
      const lx = r.range(-0.45, 0.45), lz = r.range(-0.45, 0.45);
      if (ctx.held && Math.hypot(lx, lz) < 0.26) continue;
      if (ctx.nearRiver(lx, lz) || ctx.inTown(lx, lz) || ctx.roadDist(lx, lz) < 0.1) continue;
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
