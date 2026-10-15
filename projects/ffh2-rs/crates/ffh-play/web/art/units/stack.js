// units/stack.js: the unit stack presentation, designed to read at the survey zoom (1x, 32 px a tile).
//
// The overlay of a stack (docs/ART.md 10.1, 7.4) is gathered in one place, so the unit sprite keeps
// the middle of the tile:
//
//   standard   top-left: a gonfalon on a crossbar and pole (ui/stack_banner) in the owner's field
//              and division, with the civilization's sigil in the charge colour. The tile's left
//              7 columns never hold unit pixels (the 24x24 box starts at x = 6), so nothing hides.
//   moves      the standard is the moves marker: bright with a vellum finial while the stack can
//              move, passed through FOG (dimmed) with an ash finial when its moves are spent.
//              A hero in the stack gilds the finial (ochre with a glow pixel).
//   count      a soot chip in the tally font hung on the pole under the standard (2 or more units).
//   health     a 12x2 bar on the bottom edge with a void frame, drawn when the head unit is hurt
//              (or selected): bone above 50%, ochre from 25%, crimson below 25%; the rest iron.
//   state      top-right: the 8x8 state mark (ui/mark_<state>: fortify, sleep, sentry, heal...).
//
// No standard for animals, equipment on the ground and hidden nationality (privateers, assassins):
// their own units then show moves as a 2x2 pip at the end of the health bar's row.
//
// Use: `installStackOverlay(art)` once after loadArt(); art.drawUnit then draws this overlay
// instead of the seed pennant/strip (layer 'sprite' is unchanged; 'overlay' and 'all' use this).
// Or call `drawStackOverlay(art, ctx, x, y, unitType, owner, state)` directly after
// drawUnit(..., { layer: 'sprite' }). Position: the canvas point of the tile's top-left corner.

const BANNER = 'ui/stack_banner';

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

/** The stack count: a soot chip with vellum tally digits and a void frame, top-left at tile (i, j). */
function countChip(art, ctx, x, y, i, j, n, dim) {
  const s = art.scale;
  const text = n > 99 ? '99' : String(n);
  const w = art.measureText(text, 'tally') + 2;
  rect(art, ctx, x, y, i - 1, j - 1, w + 2, 9, '0');
  rect(art, ctx, x, y, i, j, w, 7, '1');
  art.drawText(ctx, x + (i + 1) * s, y + (j + 5) * s, text, { font: 'tally', color: dim ? '6' : '8' });
  return w;
}

function hpColour(hp) {
  return hp > 50 ? '7' : hp >= 25 ? 'y' : 'C';
}

export function drawStackOverlay(art, ctx, x, y, unitType, owner, state = {}) {
  const s = art.scale;
  const type = art.typeName('unit', unitType);
  const entry = type ? art.mapper.entry('unit', type) : null;
  const animal = entry?.combat === 'ANIMAL' || !!owner?.animal;
  const item = !!entry?.equipment;
  const count = state.count | 0;
  const known = state.moves !== undefined && state.moves !== null;
  const spent = known && !state.moves;
  const banner = owner && !animal && !item && !owner.hidden && !state.noPennant && art.hasSprite(BANNER);

  if (banner) {
    const table = spent ? 'FOG' : undefined;
    art.drawSprite(ctx, x, y, art.pick(BANNER).name, { owner, flag: true, rim: 1, table });
    if (owner.sigil && art.hasSprite(owner.sigil)) {
      art.drawSprite(ctx, x + 3 * s, y + 4 * s, art.pick(owner.sigil).name, { owner, table });
    }
    if (state.hero) px(art, ctx, x, y, [[1, 1, 'y'], [1, 0, 'L']]);
    else px(art, ctx, x, y, [[1, 1, spent ? '4' : known ? '8' : '7']]);
    if (count > 1) countChip(art, ctx, x, y, 3, 16, count, spent);
  } else if (count > 1) {
    countChip(art, ctx, x, y, 2, 2, count, spent);
  }

  if (state.mark && !(count > 1 && !banner)) {
    const m = art.hasSprite(`ui/mark_${state.mark}`) ? art.pick(`ui/mark_${state.mark}`) : null;
    if (m) art.drawSprite(ctx, x + 23 * s, y + 1 * s, m.name);
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
