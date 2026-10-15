// compose.js: draws a whole map view with art.js in the order of docs/ART.md 3.1:
// ground, rivers, roads, flat improvements, light pools, uprights row by row (relief, features,
// improvements, cities), resource seals, borders, fog, vignette, units row by row, overlays.
//
//   const plots = makePlots(map);           // map: { width, height, wrapX, tiles: [...] }
//   renderMap(art, ctx, map, { plots, owners, cities, stacks, x0, y0 });
//
// Tiles are in screen orientation (row 0 = north), row-major. A tile is
// { terrain, plotType, feature, bonus, improvement, route, river, owner, vis, city } with type
// names or indices (art.setIndexNames). `fromApiMap` converts the ffh-play /api/map arrays
// (Civ4 orientation, y = 0 south) into this form.
import { TILE, tierOf } from './art.js';

/** Wrap tiles into plot objects with coordinates and a neighbour accessor. */
export function makePlots(map) {
  const { width: W, height: H, wrapX } = map;
  const plots = map.tiles.map((t, i) => ({ ...t, x: i % W, y: Math.floor(i / W) }));
  const at = (x, y) => {
    if (y < 0 || y >= H) return null;
    if (x < 0 || x >= W) { if (!wrapX) return null; x = ((x % W) + W) % W; }
    return plots[y * W + x];
  };
  for (const p of plots) p.at = (dx, dy) => at(p.x + dx, p.y + dy);
  plots.get = at;
  return plots;
}

/**
 * Convert /api/map (flat arrays indexed y*w+x, Civ4 y-up; docs/PLAY_DESIGN.md 4.4) into a
 * compose map. Type indices are kept; call art.setIndexNames(kind, names) from /api/data.
 */
export function fromApiMap(api, width, height, wrapX = false) {
  const tiles = [];
  for (let row = 0; row < height; row++) {
    const y = height - 1 - row;
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      const g = (k) => (api[k] ? api[k][i] : -1);
      const opt = (k) => (g(k) === -1 || g(k) === null ? null : g(k));
      tiles.push({
        vis: g('vis'), plotType: g('plot_type'), terrain: opt('terrain'), feature: opt('feature'), bonus: opt('bonus'),
        improvement: opt('improvement'), route: opt('route'), owner: opt('owner'), river: Math.max(0, g('river')),
        // a revealed city as '#<id>': the art only asks whether a city stands there, and city 0 is one
        city: opt('city') === null ? null : `#${g('city')}`,
      });
    }
  }
  return { width, height, wrapX, tiles };
}

/**
 * Render a view. opts:
 *   plots       from makePlots(map)
 *   x0, y0      canvas position of tile (0, 0); tx0, ty0, cols, rows: the tile range drawn
 *   owners      Map(ownerId -> art owner style)
 *   cities      [{ x, y, owner, size, name, capital, own, turns, disorder }]
 *   stacks      [{ x, y, unit, owner, count, state }]   one per tile: the head unit; state as
 *               units/stack.js reads it ({ own, moves, selected, hp, mark, hero, facing, ... });
 *               inCity defaults to whether a city of `cities` stands on the tile
 *   cursor      { x, y, selected }
 *   vignette    bool (default true); skip: Set of layers to leave out (rivers roads lights borders fog)
 *   timings     an object that receives milliseconds per phase
 *   only        'terrain': phases 1-9 (everything a terrain cache holds; no vignette, no units);
 *               'units': units, city overlays and the cursor only, over what is already drawn.
 *               A client keeping a terrain cache blits it, calls art.drawVignette on the view,
 *               then renderMap(..., { only: 'units' }).
 * Drawing is clipped to the view rectangle (sprites overhang into the tile to the north).
 * Fog and light pools read the canvas back: create its context with { willReadFrequently: true }.
 */
export function renderMap(art, ctx, map, opts = {}) {
  const s = art.scale;
  const plots = opts.plots || makePlots(map);
  const x0 = opts.x0 || 0, y0 = opts.y0 || 0;
  const tx0 = opts.tx0 || 0, ty0 = opts.ty0 || 0;
  const cols = opts.cols || map.width, rows = opts.rows || map.height;
  const owners = opts.owners || new Map();
  const skip = opts.skip || new Set();
  art.setWorld({ width: map.width, height: map.height, wrapX: map.wrapX });
  const P = (x, y) => plots.get(x, y);
  const px = (x) => x0 + (x - tx0) * TILE * s;
  const py = (y) => y0 + (y - ty0) * TILE * s;
  const each = (margin, fn) => {
    for (let y = ty0 - margin; y < ty0 + rows + margin; y++) for (let x = tx0 - margin; x < tx0 + cols + margin; x++) {
      const p = P(x, y);
      if (p) fn(p, px(x), py(y), x, y);
    }
  };
  const visible = (p) => p.vis === undefined || p.vis === 2;
  const cityAt = new Map((opts.cities || []).map((c) => [`${c.x},${c.y}`, c]));
  const T = opts.timings || {};
  let t0 = performance.now();
  const lap = (name) => { const t = performance.now(); T[name] = (T[name] || 0) + Math.round(t - t0); t0 = t; };
  ctx.save();
  ctx.beginPath();
  ctx.rect(x0, y0, cols * TILE * s, rows * TILE * s);
  ctx.clip();
  if (opts.only !== 'units') terrainLayers();
  if (opts.only !== 'terrain') unitLayers();
  ctx.restore();

  function terrainLayers() {
    // 1. ground
    ctx.fillStyle = art.css('0');
    ctx.fillRect(x0, y0, cols * TILE * s, rows * TILE * s);
    each(0, (p, x, y) => art.drawTerrain(ctx, x, y, p));
    lap('ground');
    // 2. rivers, 3. roads, 4. flat improvements
    if (!skip.has('rivers')) each(1, (p, x, y) => { if (p.vis) art.drawRiver(ctx, x, y, p); });
    if (!skip.has('roads')) each(0, (p, x, y) => { if (p.vis) art.drawRoad(ctx, x, y, p); });
    each(0, (p, x, y) => { if (p.vis && p.improvement != null && art.isFlatImprovement(p.improvement)) art.drawImprovement(ctx, x, y, p.improvement, p); });
    lap('lines');
    // 5. city light pools (lights only on visible tiles)
    if (!skip.has('lights')) {
      for (const c of opts.cities || []) {
        const p = P(c.x, c.y);
        const o = owners.get(c.owner);
        if (!p || !visible(p) || c.disorder || (o && o.pool === 'none')) continue;
        const tier = tierOf(c.size | 0 || 1);
        art.drawLight(ctx, px(c.x) + 16 * s, py(c.y) + 28 * s, o?.pool || 'FIRE', 12 + 3 * tier, { inner: true });
      }
    }
    lap('lights');
    // 6. uprights row by row (one row of margin above: sprites rise into the tile to the north)
    for (let y = ty0 - 1; y < ty0 + rows + 1; y++) {
      for (let x = tx0 - 1; x < tx0 + cols + 1; x++) {
        const p = P(x, y);
        if (!p || !p.vis) continue;
        const X = px(x), Y = py(y);
        art.drawRelief(ctx, X, Y, p);
        if (p.feature != null) art.drawFeature(ctx, X, Y, p);
        if (p.improvement != null && !art.isFlatImprovement(p.improvement)) art.drawImprovement(ctx, X, Y, p.improvement, p);
        const c = cityAt.get(`${x},${y}`);
        if (c) art.drawCity(ctx, X, Y, { ...c, visible: visible(p), layer: 'sprite', pool: false }, owners.get(c.owner));
      }
    }
    lap('uprights');
    // 7. resource seals
    each(0, (p, x, y) => { if (p.vis && p.bonus != null && !cityAt.has(`${p.x},${p.y}`)) art.drawResource(ctx, x, y, p.bonus); });
    // 8. borders
    if (!skip.has('borders')) each(0, (p, x, y) => { if (p.vis && p.owner != null) art.drawBorder(ctx, x, y, p, owners.get(p.owner)); });
    // 9. fog, 10. vignette
    lap('borders');
    if (!skip.has('fog')) each(0, (p, x, y) => art.drawFog(ctx, x, y, p));
    lap('fog');
    if (opts.vignette !== false && opts.only !== 'terrain') art.drawVignette(ctx, x0, y0, cols * TILE * s, rows * TILE * s);
    lap('vignette');
  }

  function unitLayers() {
    // 11. units row by row, then overlays. A stack on a city tile flies its standard at the
    // top-right (state.inCity, units/stack.js): the city banner holds the top-left.
    const stacks = (opts.stacks || []).slice().sort((a, b) => a.y - b.y || a.x - b.x);
    const stateOf = (st, layer) => ({ inCity: cityAt.has(`${st.x},${st.y}`), ...st.state, count: st.count, layer });
    for (const st of stacks) art.drawUnit(ctx, px(st.x), py(st.y), st.unit, owners.get(st.owner), stateOf(st, 'sprite'));
    for (const c of opts.cities || []) {
      const p = P(c.x, c.y);
      if (p && p.vis) art.drawCity(ctx, px(c.x), py(c.y), { ...c, visible: visible(p), layer: 'overlay' }, owners.get(c.owner));
    }
    for (const st of stacks) art.drawUnit(ctx, px(st.x), py(st.y), st.unit, owners.get(st.owner), stateOf(st, 'overlay'));
    if (opts.cursor) art.drawTileCursor(ctx, px(opts.cursor.x), py(opts.cursor.y), !!opts.cursor.selected);
    lap('units');
  }
}
