// units/stack.js: the unit stack presentation, designed to read at the survey zoom (1x, 32 px a tile).
//
// The overlay of a stack (docs/ART.md 10.1, 7.4) is gathered in one place, so the unit sprite keeps
// the middle of the tile:
//
//   standard   top-left: a 7x12 gonfalon (ui/stack_banner: pole, crossbar and a 5x8 field) in the
//              owner's field and division, with the civilization's sigil in the charge colour. The
//              unit box starts at tile x = 6, so the standard sits beside the figure, not on it.
//   quiet      the standard is drawn in the tincture's darker partner (the shade becomes the field,
//              the charge takes whichever of charge and field reads best on it, vellum stepped down
//              to bone) for every stack except your own movable or selected one: foreign stacks and
//              your spent ones stay dark, so the map is not a field of bright flags.
//   moves      your own movable or selected stack shows the full tincture and a vellum finial pip:
//              the pip is the 'can move' signal. A spent stack is quiet and passed through FOG, with
//              an iron pip. Foreign stacks (moves unknown) show an ash knob. A hero in the stack
//              gilds the finial (ochre with a glow pixel above it).
//   count      a soot chip in the tally font hung under the pole (2 or more units): vellum digits
//              when active, bone when quiet, stone when spent.
//   health     a 12x2 bar on the bottom edge with a void frame, drawn when the head unit is hurt
//              (or selected): bone above 50%, ochre from 25%, crimson below 25%; the rest iron.
//   state      top-right: the 8x8 state mark (ui/mark_<state>: fortify, sleep, sentry, heal...).
//   in a city  state.inCity (or state.standard = 'right') mirrors the standard to the top-right,
//              since the city banner holds the top-left; the state mark then sits at the top centre.
//   lanterns   a head unit on water whose sprite carries a FIRE, RITE or ARC `: light` casts a
//              broken reflection on the water under the source (docs/ART.md 5.4): dashes of the
//              light's ring colour, then its dark partner, 2 px apart, narrowing downwards.
//
// No standard for animals, equipment on the ground and hidden nationality (privateers, assassins):
// their own units then show moves as a 2x2 pip at the end of the health bar's row.
//
// Geometry comes from the standard sprite's metadata (`: flag` places the sigil, `: finial` the
// pip; the count chip hangs under the sprite), so the sprite can be redrawn or moved to another set
// without touching this file.
//
// Use: loadArt() installs it (art.js PASSES); by hand, `installStackOverlay(art)` once after
// loadArt({ passes: false }); art.drawUnit then draws this overlay
// instead of the seed pennant/strip (layer 'sprite' is unchanged; 'overlay' and 'all' use this).
// Or call `drawStackOverlay(art, ctx, x, y, unitType, owner, state)` directly after
// drawUnit(..., { layer: 'sprite' }). Position: the canvas point of the tile's top-left corner.
// state: { moves (true/false for your own units, absent for foreign ones), selected, own, hero,
// count, hp, mark, inCity, standard ('left'|'right'), facing, noPennant, lit }.

const BANNER = 'ui/stack_banner';
const SIGIL = 5; // sigils on the standard are the 5x5 charges
const TRANSPARENT = 255, NUM_COLORS = 56; // as in spritefmt.js

/** Paint art pixels [x, y, key] relative to the tile corner (x, y). */
function px(art, ctx, x, y, list) {
  const s = art.scale;
  for (const [i, j, k] of list) {
    ctx.fillStyle = art.css(k);
    ctx.fillRect(x + i * s, y + j * s, s, s);
  }
}

function rect(art, ctx, x, y, i, j, w, h, key) {
  const s = art.scale;
  ctx.fillStyle = art.css(key);
  ctx.fillRect(x + i * s, y + j * s, w * s, h * s);
}

/** The stack count: a soot chip with tally digits and a void frame, top-left at tile (i, j). */
function countChip(art, ctx, x, y, i, j, text, color) {
  const s = art.scale;
  const w = art.measureText(text, 'tally') + 2;
  rect(art, ctx, x, y, i - 1, j - 1, w + 2, 9, '0');
  rect(art, ctx, x, y, i, j, w, 7, '1');
  art.drawText(ctx, x + (i + 1) * s, y + (j + 5) * s, text, { font: 'tally', color });
  return w;
}

function hpColour(hp) {
  return hp > 50 ? '7' : hp >= 25 ? 'y' : 'C';
}

/** CIE L* of a palette index (cached per art). */
function lightness(art, i) {
  if (!art._stackL) {
    art._stackL = art.palette.colors.map(({ rgb }) => {
      const f = (c) => { c /= 255; return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
      const Y = 0.2126 * f(rgb[0]) + 0.7152 * f(rgb[1]) + 0.0722 * f(rgb[2]);
      return Y > 0.008856 ? 116 * Math.cbrt(Y) - 16 : 903.3 * Y;
    });
  }
  return art._stackL[i];
}

/**
 * The quiet tincture: the shade is the field, FOG of the shade the fold, and the charge whichever of
 * the charge and the field stands further from the shade in lightness (vellum stepped down to bone).
 */
function quietTincture(art, t) {
  const L = (i) => lightness(art, i);
  const fog = art.tables.FOG;
  let charge = Math.abs(L(t.charge) - L(t.shade)) >= Math.abs(L(t.field) - L(t.shade)) ? t.charge : t.field;
  if (charge === art.K('8')) charge = art.K('7');
  return { ...t, field: t.shade, shade: fog[t.shade], charge };
}

/** An owner style whose tinctures are quiet (cached per owner style object). */
export function quietOwner(art, owner) {
  if (!owner) return owner;
  if (!art._stackQuiet) art._stackQuiet = new WeakMap();
  let q = art._stackQuiet.get(owner);
  if (!q) {
    q = {
      ...owner,
      tinctureName: `${owner.tinctureName}~quiet`,
      tincture: quietTincture(art, owner.tincture),
      second: owner.second ? quietTincture(art, owner.second) : owner.second,
    };
    art._stackQuiet.set(owner, q);
  }
  return q;
}

/** Is the field point (u, v) in [0,1)^2 on the second tincture of a division? (as art._slotFill) */
function onSecond(division, u, v) {
  if (division === 'pale') return u >= 0.5;
  if (division === 'fess') return v >= 0.5;
  if (division === 'bend') return v > u;
  if (division === 'quarterly') return (u >= 0.5) !== (v >= 0.5);
  return false;
}

/**
 * The sigil on the standard, centred on the field, upright even when the standard is mirrored.
 * On a divided field the charge is counterchanged: over the second tincture it takes that
 * tincture's charge (bone on Sable, soot on Argent), so it never sinks into the field.
 */
function drawSigil(art, ctx, x, y, sg, look, [fx, fy, fw, fh], table, right, at) {
  const s = art.scale;
  const T = table ? art.tables[table] : null;
  const sx = fx + Math.floor((fw - sg.w) / 2), sy = fy + Math.max(0, Math.floor((fh - SIGIL) / 2));
  const left = right ? at(sx + sg.w - 1) : at(sx);
  const divided = look.division && look.division !== 'plain' && look.second;
  for (let j = 0; j < sg.h; j++) for (let i = 0; i < sg.w; i++) {
    let c = sg.data[j * sg.w + i];
    if (c === TRANSPARENT) continue;
    if (c >= NUM_COLORS) {
      const col = right ? sx + sg.w - 1 - i : sx + i; // the standard's own column under this pixel
      const second = divided && onSecond(look.division, (col - fx + 0.5) / fw, (sy + j - fy + 0.5) / fh);
      c = second ? look.second.charge : look.tincture.charge;
    }
    if (T) c = T[c];
    ctx.fillStyle = art.palette.colors[c].hex;
    ctx.fillRect(x + (left + i) * s, y + (1 + sy + j) * s, s, s);
  }
}

/** Which look a stack's standard takes: 'active' (full tincture), 'quiet' or 'spent' (quiet, then FOG). */
export function standardMode(state = {}) {
  const known = state.moves !== undefined && state.moves !== null;
  if (state.selected) return 'active';
  if (known && state.own !== false) return state.moves ? 'active' : 'spent';
  return 'quiet';
}

// reflection colours by pool table: the ring colour, then its dark partner
const REFLECT = { FIRE: ['E', 'e'], RITE: ['S', 's'], ARC: ['P', 'p'] };
// the dashes under a source, top to bottom: [offset from the source column, width, colour 0|1]
const DASHES = [[-1, 3, 0], [0, 2, 0], [-1, 2, 1], [0, 1, 1]];
const WATER_MARKS = ['N', 'n', 'g', 'd'];
const waterlines = new WeakMap(); // sprite -> waterline row

/**
 * The sprite row of a ship's waterline: `: tags waterline=<row>` when the sprite names it (oars or
 * a keel dipping below the hull), else its lowest row holding anything but water marks.
 */
function waterline(art, spr) {
  if (waterlines.has(spr)) return waterlines.get(spr);
  const tag = spr.meta.tags.find((t) => t.startsWith('waterline='));
  if (tag) { waterlines.set(spr, Number(tag.slice(10))); return waterlines.get(spr); }
  const marks = new Set(WATER_MARKS.map((k) => art.K(k)));
  let row = spr.ay;
  for (let j = spr.h - 1; j >= 0; j--) {
    let hull = false;
    for (let i = 0; i < spr.w && !hull; i++) {
      const c = spr.data[j * spr.w + i];
      hull = c !== 255 && !marks.has(c);
    }
    if (hull) { row = j; break; }
  }
  waterlines.set(spr, row);
  return row;
}

/**
 * Lantern reflections on water (docs/ART.md 5.4) for the head unit drawn at tile (x, y): for each
 * FIRE, RITE or ARC `: light` of its sprite, 4 dashes under the source from the waterline down,
 * 2 px apart: two in the ring colour (3 px, then 2), two in its dark partner (2, then 1). Drawn in
 * the overlay pass, so no hull covers them, and never below the tile.
 */
export function drawReflections(art, ctx, x, y, unitType, state = {}) {
  if (state.lit === false) return;
  const type = art.typeName('unit', unitType);
  const r = type ? art.mapper.resolve('unit', type) : null;
  const entry = type ? art.mapper.entry('unit', type) : null;
  if (!r || r.sprite === 'none' || entry?.domain !== 'SEA') return;
  const spr = art.pick(r.sprite, 0);
  if (!spr || !spr.meta.lights.length) return;
  const huge = spr.w >= 32;
  const fx = huge ? 15 : 17, fy = huge ? 29 : 28; // the sprite's anchor in the tile (as art.drawUnit)
  const mirror = state.facing === 'left';
  const top = fy - spr.ay + waterline(art, spr) + 1; // first water row under the hull
  for (const l of spr.meta.lights) {
    const cols = REFLECT[l.table];
    if (!cols) continue;
    const cx = mirror ? fx + spr.ax - l.x : fx - spr.ax + l.x; // tile column of the source
    DASHES.forEach(([o, w, k], n) => {
      const j = top + 2 * n;
      if (j > 31) return;
      const i0 = mirror ? cx - o - (w - 1) : cx + o;
      rect(art, ctx, x, y, i0, j, w, 1, cols[k]);
    });
  }
}

export function drawStackOverlay(art, ctx, x, y, unitType, owner, state = {}) {
  const s = art.scale;
  const type = art.typeName('unit', unitType);
  const entry = type ? art.mapper.entry('unit', type) : null;
  const animal = entry?.combat === 'ANIMAL' || !!owner?.animal;
  const item = !!entry?.equipment;
  const count = state.count | 0;
  const known = state.moves !== undefined && state.moves !== null;
  const mode = standardMode(state);
  const spent = mode === 'spent';
  const banner = owner && !animal && !item && !owner.hidden && !state.noPennant && art.hasSprite(BANNER);

  drawReflections(art, ctx, x, y, unitType, state);

  // in a city the city's own banner holds the top-left: the standard moves to the top-right
  const right = !!(state.inCity || state.standard === 'right');
  if (banner) {
    const spr = art.pick(BANNER);
    const look = mode === 'active' ? owner : quietOwner(art, owner);
    const table = spent ? 'FOG' : undefined;
    // the sprite's (0,0) sits at tile (1,1), so its moon rim stays inside the tile
    const at = (i) => (right ? 31 - 1 - i : 1 + i); // tile column of sprite column i
    const v = { owner: look, flag: true, rim: 1, table };
    if (right) art.drawSprite(ctx, x + 30 * s, y + 1 * s, spr.name, { ...v, mirror: true });
    else art.drawSprite(ctx, x + 1 * s, y + 1 * s, spr.name, v);
    const [fx, fy, fw, fh] = spr.meta.flag || [1, 2, spr.w - 2, spr.h - 4];
    if (owner.sigil && art.hasSprite(owner.sigil)) drawSigil(art, ctx, x, y, art.pick(owner.sigil), look, [fx, fy, fw, fh], table, right, at);
    const [pi, pj] = spr.meta.finial || [0, 0];
    const pole = at(pi);
    if (state.hero) px(art, ctx, x, y, [[pole, 1 + pj, 'y'], [pole, pj, 'L']]);
    else px(art, ctx, x, y, [[pole, 1 + pj, known && state.own !== false ? (state.moves ? '8' : '3') : '5']]);
    if (count > 1) {
      const text = count > 99 ? '99' : String(count);
      const w = art.measureText(text, 'tally') + 2;
      const j = 1 + spr.h + 1;
      countChip(art, ctx, x, y, right ? 31 - w : 1, j, text, mode === 'active' ? '8' : spent ? '6' : '7');
    }
  } else if (count > 1) {
    countChip(art, ctx, x, y, 2, 2, count > 99 ? '99' : String(count), spent ? '6' : known ? '8' : '7');
  }

  if (state.mark && !(count > 1 && !banner)) {
    const m = art.hasSprite(`ui/mark_${state.mark}`) ? art.pick(`ui/mark_${state.mark}`) : null;
    if (m) art.drawSprite(ctx, x + (right && banner ? 12 : 23) * s, y + 1 * s, m.name);
  }

  const hp = state.hp;
  const hurt = hp !== undefined && hp !== null && (hp < 100 || state.selected);
  if (hurt) {
    const v = Math.max(0, Math.min(100, hp));
    const fill = v > 0 ? Math.max(1, Math.round((12 * v) / 100)) : 0;
    rect(art, ctx, x, y, 9, 29, 14, 3, '0');
    rect(art, ctx, x, y, 10, 30, 12, 2, '3');
    if (fill) rect(art, ctx, x, y, 10, 30, fill, 2, hpColour(v));
  }
  if (!banner && known && owner && !animal && !item) {
    rect(art, ctx, x, y, 23, 29, 4, 3, '0');
    rect(art, ctx, x, y, 24, 30, 2, 2, spent ? '3' : '8');
  }
}

/** Route art.drawUnit's overlay layer through drawStackOverlay. Idempotent; returns art. */
export function installStackOverlay(art) {
  if (art._stackOverlay) return art;
  const base = art.drawUnit.bind(art);
  art.drawUnit = (ctx, x, y, unitType, owner, state = {}) => {
    const layer = state.layer || 'all';
    if (layer !== 'overlay') base(ctx, x, y, unitType, owner, { ...state, layer: 'sprite' });
    if (layer !== 'sprite') drawStackOverlay(art, ctx, x, y, unitType, owner, state);
  };
  art._stackOverlay = true;
  return art;
}
