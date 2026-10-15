// What the art renderer draws, computed without the DOM or the art (tested by
// assets/test/stacks.test.mjs): the zoom steps, the terrain cache's chunks in the art's row order
// (row 0 = north), each tile's stack (head unit and its state: docs/ART.md 10.1 and
// web/art/units/stack.js), the cities as the art draws them and their signatures (a changed city
// redraws the terrain cache around it), and the facing of units (east, or west after a move west).

/** The terrain cache's chunks: CHUNK x CHUNK plots, in rows from the north. Map widths are
 *  multiples of 4, so chunks are whole across the east-west seam; a chunk of the art takes
 *  about 25-40 ms to draw in Chromium, about a frame and a half. */
export const CHUNK = 4;

/** The zoom steps of the art renderer, in CSS px per tile: two chart steps (flat colours, no
 *  sprites: docs/ART.md 3.2) and the art scales 1x-4x (survey, default, close, study), each a
 *  whole number of device pixels per art pixel at device-pixel ratio `dpr`. */
export function artZooms(dpr = 1) {
  const out = [8, 16];
  for (let k = 1; k <= 4; k++) {
    const ts = (32 * Math.max(1, Math.round(k * dpr))) / dpr;
    if (!out.some((v) => Math.abs(v - ts) < 0.01)) out.push(ts);
  }
  return out;
}

/** The default zoom of the art renderer: 2x (docs/ART.md 3.2). */
export function defaultArtZoom(dpr = 1) {
  return artZooms(dpr)[3];
}

/** The art's device pixels per art pixel at tile size `ts` (CSS px), or 0 at a chart step. */
export function artScale(ts, dpr = 1) {
  const s = Math.round((ts * dpr) / 32);
  return ts >= 31 ? Math.max(1, s) : 0;
}

/** The row (art order, 0 = north) of map row `y` (Civ4 order, 0 = south). */
export function rowOf(map, y) {
  return map.h - 1 - y;
}

/** The chunk keys (`"cx,cr"`, in rows from the north) whose drawing depends on the tiles
 *  `tiles` (indices `y * w + x`): each tile's own chunk and those of its eight neighbours, since a
 *  tile's ground, rivers, woods and fog edge depend on its neighbours and uprights rise into the
 *  tile to the north. */
export function rowChunksOfTiles(map, tiles) {
  const out = new Set();
  for (const i of tiles) {
    const x = i % map.w;
    const y = Math.floor(i / map.w);
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        let nx = x + dx;
        const ny = y + dy;
        if (ny < 0 || ny >= map.h) continue;
        if (nx < 0 || nx >= map.w) {
          if (!map.wrap_x) continue;
          nx = (nx + map.w) % map.w;
        }
        out.add(`${Math.floor(nx / CHUNK)},${Math.floor(rowOf(map, ny) / CHUNK)}`);
      }
    }
  }
  return out;
}

/** The tiles that differ between two maps of the same size (`layers`: the arrays compared), or
 *  "all" when there is no earlier map or its size differs. */
export function changedTiles(prev, next, layers) {
  if (!prev || prev.w !== next.w || prev.h !== next.h || prev.wrap_x !== next.wrap_x) return "all";
  const out = [];
  const n = next.w * next.h;
  for (let i = 0; i < n; i++) if (layers.some((k) => prev[k]?.[i] !== next[k]?.[i])) out.push(i);
  return out;
}

/** A player's key for heraldry: the slot and its civilization. */
export function ownerKey(p) {
  return `${p.id}:${p.civ ?? "?"}`;
}

/** Heraldry (docs/ART.md 7.2) for players in the order they became known: each civilization's
 *  tincture plain, or the next free division of it; barbarians are not assigned. `players`:
 *  [{id, civ (type name), barbarian}]; `memo`: a Map(ownerKey -> spec) kept for the game, so an
 *  assignment never changes; `tinctureOf(civ)` names a civilization's tincture. Returns the specs
 *  [{id, civ, division, barbarian}] that `ownerStyleOf` turns into the art's owner styles (the
 *  same on the page and in the art worker). */
export function assignHeraldry(players, memo, tinctureOf) {
  const DIVISIONS = ["plain", "pale", "fess", "bend", "quarterly"];
  const taken = new Set([...memo.values()].filter((s) => !s.barbarian).map((s) => `${tinctureOf(s.civ)}/${s.division}`));
  const out = [];
  for (const p of players) {
    const key = ownerKey(p);
    let spec = memo.get(key);
    if (!spec) {
      if (p.barbarian || p.civ === "CIVILIZATION_BARBARIAN") spec = { id: p.id, civ: "CIVILIZATION_BARBARIAN", division: "plain", barbarian: true };
      else {
        const t = tinctureOf(p.civ);
        const division = DIVISIONS.find((d) => !taken.has(`${t}/${d}`)) ?? "plain";
        taken.add(`${t}/${division}`);
        spec = { id: p.id, civ: p.civ ?? null, division, barbarian: false };
      }
      memo.set(key, spec);
    }
    out.push(spec);
  }
  return out;
}

/** The art's owner style of a heraldry spec (`assignHeraldry`); a player the client does not
 *  know (no spec, or no civilization) flies the lozenge on Argent. */
export function ownerStyleOf(art, spec) {
  if (spec?.barbarian) return art.ownerStyle("CIVILIZATION_BARBARIAN", { id: spec.id, barbarian: true });
  if (!spec?.civ) return art.ownerStyle("CIVILIZATION_UNKNOWN", { id: `unknown-${spec?.id}` });
  return art.ownerStyle(spec.civ, { id: spec.id, division: spec.division ?? "plain" });
}

/** Owner styles by player id from heraldry specs, made when first asked for (unknown players:
 *  the lozenge). */
export class OwnerStyles {
  constructor(art, specs) {
    this.art = art;
    this.specs = new Map(specs.map((s) => [s.id, s]));
    this.styles = new Map();
  }

  get(id) {
    if (id === null || id === undefined || id < 0) return undefined;
    let st = this.styles.get(id);
    if (!st) {
      st = ownerStyleOf(this.art, this.specs.get(id) ?? { id });
      this.styles.set(id, st);
    }
    return st;
  }
}

/** Civ4's activities as the art's state marks (ui/mark_<state>). Fortified units sleep in C++
 *  (MISSION_FORTIFY sets ACTIVITY_SLEEP), so a sleeping unit that can fight shows the shield. */
export function markOf(activity, { canFight = false, automated = false, promote = false } = {}) {
  if (promote) return "promote";
  if (automated) return "auto";
  switch (activity) {
    case "ACTIVITY_SLEEP":
      return canFight ? "fortify" : "sleep";
    case "ACTIVITY_SENTRY":
    case "ACTIVITY_SENTRY_NAVAL_UNITS":
    case "ACTIVITY_SENTRY_LAND_UNITS":
      return "sentry";
    case "ACTIVITY_HEAL":
      return "heal";
    default:
      return null;
  }
}

/** The head of a stack (the unit drawn): the selection's first, else the viewer's own unit that
 *  can move, else its first, else the strongest foreign one (the best defender stands in front:
 *  docs/ART.md 10.1). `strength(unit)` rates a foreign unit. */
export function headUnit(units, me, selected = [], strength = () => 0) {
  const sel = units.find((u) => selected.includes(u.id));
  if (sel) return sel;
  const own = units.filter((u) => u.owner === me);
  if (own.length) return own.find((u) => (u.moves ?? 0) > 0) ?? own[0];
  let best = units[0] ?? null;
  for (const u of units) if (strength(u) > strength(best)) best = u;
  return best;
}

/**
 * The stack on one tile as the art draws it: `{head, state}` with state `{own, moves, selected,
 * hp, mark, hero, count, facing}` (web/art/units/stack.js). `units`: the view's units on the tile;
 * `ctx`: { me, selected (unit ids), activity(i) -> name, unitInfo(type) -> {hero, combat},
 * automated(group) -> bool, facing(id) -> 'left'|'right' }.
 */
export function stackState(units, ctx) {
  const info = (u) => ctx.unitInfo?.(u.type) ?? {};
  const head = headUnit(units, ctx.me, ctx.selected ?? [], (u) => info(u).strength ?? 0);
  if (!head) return null;
  const own = head.owner === ctx.me;
  const state = {
    own,
    selected: (ctx.selected ?? []).includes(head.id),
    hp: head.max_hp > 0 ? Math.round((100 * head.hp) / head.max_hp) : 100,
    count: units.length,
    hero: units.some((u) => !!info(u).hero),
    facing: ctx.facing?.(head.id) ?? "right",
  };
  if (own) {
    state.moves = (head.moves ?? 0) > 0;
    const promote = units.some((u) => u.owner === ctx.me && u.promo_ready);
    const a = head.activity === null || head.activity === undefined ? null : ctx.activity?.(head.activity);
    const fights = (info(head).combat ?? "NONE") !== "NONE" && (info(head).strength ?? 1) > 0;
    state.mark = markOf(a, { canFight: fights, automated: !!ctx.automated?.(head.group), promote });
  }
  return { head, state };
}

/** A city's size tier (docs/ART.md 9.1). */
export function tierOf(size) {
  return size <= 3 ? 1 : size <= 7 ? 2 : size <= 12 ? 3 : size <= 19 ? 4 : 5;
}

/** What the terrain cache draws of a city (its sprite and pool): owner, tier, capital, lit, walls.
 *  When it changes the chunks around the city are redrawn. */
export function citySignature(c) {
  return `${c.owner}|${tierOf(c.size || 1)}|${c.capital ? 1 : 0}|${c.visible ? 1 : 0}|${c.disorder ? 1 : 0}|${c.walls ? 1 : 0}`;
}

/** Facing (docs/ART.md 10.1): units face east and are mirrored after a move with a westward
 *  component. `prev`: Map(id -> {x, facing}) from the last view; returns the new Map for `units`
 *  (wrap-aware on a map `w` wide that wraps when `wrap`). */
export function updateFacing(prev, units, w, wrap) {
  const out = new Map();
  for (const u of units) {
    const p = prev.get(u.id);
    let facing = p?.facing ?? "right";
    if (p && p.x !== u.x) {
      let dx = u.x - p.x;
      if (wrap && w > 0) {
        if (dx > w / 2) dx -= w;
        if (dx < -w / 2) dx += w;
      }
      facing = dx < 0 ? "left" : "right";
    }
    out.set(u.id, { x: u.x, facing });
  }
  return out;
}

/** The units that changed tile since the last view, for the move slide (artmap.js): `prev` is
 *  Map(id -> {x, y}) from the last view; returns `{ positions: Map(id -> {x, y}) for `units`,
 *  moved: [{id, dx, dy}] }` where dx/dy are the wrap-aware net displacement (east, north) of each
 *  unit the client already knew and that moved. A new or vanished unit is never "moved". */
export function unitMoves(prev, units, w, wrap) {
  const positions = new Map();
  const moved = [];
  for (const u of units) {
    positions.set(u.id, { x: u.x, y: u.y });
    const p = prev.get(u.id);
    if (!p || (p.x === u.x && p.y === u.y)) continue;
    let dx = u.x - p.x;
    if (wrap && w > 0) {
      if (dx > w / 2) dx -= w;
      if (dx < -w / 2) dx += w;
    }
    moved.push({ id: u.id, dx, dy: u.y - p.y });
  }
  return { positions, moved };
}
