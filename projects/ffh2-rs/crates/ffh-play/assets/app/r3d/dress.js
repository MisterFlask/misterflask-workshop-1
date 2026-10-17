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
import { rng, hash32, hashf } from "./noise.js";
import { hexLinear, clamp } from "./math.js";
import { KIND, FEAT, GROUND, SEA } from "./world.js";
import { CATALOG } from "./assets/catalog.js";
import { hasRecipe } from "./assets/registry.js";

/** Where on a plot things stand (plot-local, x east / z south, plot half-size 0.5). */
const UNIT_SPOT = [0, 0.04];
const CITY_UNIT_SPOT = [0.0, 0.3];
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

/** The static dressing of the map: trees, plants, rocks, features, structures, resources and
 *  settlements, and the particle emitters. `world`: world.js; returns `{list, emitters, cities}`. */
export function dressStatic(world, map) {
  const out = [];
  const emitters = [];
  const cities = [];
  const w = world.w;
  const cityAt = new Map();
  for (const c of S.cities.values()) cityAt.set(c.y * w + c.x, c);
  const unitAt = new Set();
  for (const units of S.unitsAt.values()) unitAt.add(units[0].y * w + units[0].x);
  const H = (mx, my) => world.heightAt(mx, my);

  for (let y = 0; y < world.h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      const kind = world.kind[i];
      if (kind === KIND.VOID) continue;
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
      const ctx = { r, x, y, i, kind, feat, ground, world, hills: kind === KIND.HILLS, peak: kind === KIND.PEAK, held, city: !!city, owner: ownerId, civ, family, unit: unitAt.has(i), bonus, improvement: imp };
      const at = (lx, lz) => [x + 0.5 + lx, y + 0.5 - lz]; // plot-local (x east, z south) -> map
      const place = (asset, lx, lz, o = {}) => {
        const [mx, my] = at(lx, lz);
        const yy = o.float ? Math.max(SEA, H(mx, my)) : H(mx, my);
        out.push(inst(asset, mx, yy + (o.lift ?? 0), -my, { plot: i, id: o.id ?? idOf(`s${i}:${out.length}`), ...o }));
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
            place(pick(f.asset, "basic.tree"), lx, lz, { variant: r.int(0, 7), scale: (f.scale ?? 1) * r.range(0.8, 1.15), yaw: r.range(0, 6.28), phase: r(), tint: f.tint });
          }
        }
      }
      // Rocks on hills, peaks and rough ground.
      const rocks = CATALOG.rocks?.(ctx) ?? (kind === KIND.HILLS || kind === KIND.PEAK ? [{ asset: "basic.rock", scale: 1, count: kind === KIND.PEAK ? 3 : 1 }] : []);
      for (const rk of rocks) {
        for (let k = 0; k < (rk.count ?? 1); k++) {
          const lx = r.range(-0.4, 0.4), lz = r.range(-0.4, 0.4);
          if (held && Math.hypot(lx, lz) < 0.32) continue;
          place(pick(rk.asset, "basic.rock"), lx, lz, { variant: r.int(0, 7), scale: (rk.scale ?? 1) * r.range(0.75, 1.3), yaw: r.range(0, 6.28), lift: -0.01, tint: rk.tint });
        }
      }
      // One-off features (oasis, volcano, flood reeds, ice floes, flames).
      const feats = CATALOG.feature?.(feat, ctx) ?? [];
      for (const f of feats) {
        if (city && !f.always) continue;
        place(f.asset, f.dx ?? 0, f.dz ?? 0, { variant: f.variant ?? r.int(0, 7), param: f.param, scale: f.scale ?? 1, yaw: f.yaw ?? r.range(0, 6.28), float: f.float, tint: f.tint });
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
          place(im.asset, im.dx ?? spot[0], im.dz ?? spot[1], { variant: im.variant ?? r.int(0, 7), param: im.param ?? family, scale: im.scale ?? 1, yaw: im.yaw ?? r.range(-0.4, 0.4), float: im.float, id: im.id ?? iid, team, tint: im.tint });
          for (const e of im.emit ?? []) emitters.push({ ...e, x: x + 0.5 + (im.dx ?? spot[0]) + (e.dx ?? 0), y: H(x + 0.5, y + 0.5) + (e.dy ?? 0.1), z: -(y + 0.5 - (im.dz ?? spot[1]) - (e.dz ?? 0)) });
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
      // A settlement.
      if (city) {
        const owner = S.players.get(city.owner);
        const civ = owner ? tname("CivilizationType", owner.civ) : null;
        const family = CATALOG.family?.(civ, city) ?? "human";
        const team = hexRgb(teamColour(city.owner));
        const cr = rng(hash32(city.id * 977 + 13));
        const cctx = { r: cr, size: city.size ?? 1, capital: !!city.capital, walls: !!city.walls, family, civ, seed: city.id, height: (dx, dz) => H(x + 0.5 + dx, y + 0.5 - dz), world, x, y };
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
  return { list: out, emitters, cities };
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
      const m = choose();
      const s = (m.scale ?? 1) * r.range(0.8, 1.2);
      place(m.asset, lx, lz, { variant: m.variant ?? r.int(0, 15), param: m.param, scale: s, yaw: r.range(0, 6.28), phase: r(), tint: m.tint, lift: -0.008 });
    }
  }
  // Undergrowth between the trunks.
  for (const u of mix.under ?? []) {
    for (let k = 0; k < (u.count ?? 2); k++) {
      const lx = r.range(-0.45, 0.45), lz = r.range(-0.45, 0.45);
      if (ctx.held && Math.hypot(lx, lz) < 0.26) continue;
      place(u.asset, lx, lz, { variant: r.int(0, 7), scale: (u.scale ?? 1) * r.range(0.7, 1.2), yaw: r.range(0, 6.28), phase: r() });
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
  for (const units of S.unitsAt.values()) {
    const u0 = units[0];
    const i = u0.y * w + u0.x;
    const top = T.topUnit(units, mine, sel);
    const onCity = map.city[i] >= 0;
    const spot = onCity ? CITY_UNIT_SPOT : UNIT_SPOT;
    const team = hexRgb(teamColour(top.owner));
    const order = [top, ...units.filter((u) => u !== top)].slice(0, 3);
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
      const ground = world.heightAt(mx, my);
      const yy = look.float ? Math.max(SEA + 0.005, ground) : look.fly ? Math.max(SEA, ground) + look.fly : ground;
      const s = (look.scale ?? 1) * (k === 0 ? 1 : 0.78);
      out.push(inst(pick(look.asset, "basic.unit"), mx, yy, -my, { param: look.param ?? "", variant: look.variant ?? 0, yaw: look.yaw ?? (k === 0 ? 0.25 : 0.25 + (k === 1 ? 0.3 : -0.3)), scale: s, phase: hashf(u.id, 3), team, protect: 1, id: idOf(`u${u.id}`), plot: i }));
      if (k === 0) anchor = { x: mx, y: yy, z: -my, height: (look.height ?? 0.5) * s };
    });
    stacks.push({ units, top, x: u0.x, y: u0.y, k: i, anchor, picked: sel.includes(top.id), team: teamColour(top.owner), spot });
  }
  return { list: out, stacks };
}

export { hexRgb, teamColour };
export { hexLinear };
