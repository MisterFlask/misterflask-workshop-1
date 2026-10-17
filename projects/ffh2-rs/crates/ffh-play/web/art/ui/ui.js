// ui/ui.js: helpers for the ui sprite set (docs/ART.md 11). Vanilla ES module over art.js.
//
//   import { loadArt } from '../art.js';
//   import * as UI from './ui/ui.js';
//   const art = await loadArt();
//   UI.installCSS(art, { u: 2 });                 // .ui-frame-panel, .ui-button, .ui-cursor-move ...
//   UI.drawNine(art, ctx, x, y, w, h, 'ui/frame_panel');
//   UI.drawPath(art, ctx, x0, y0, steps, { attack: true, odds: 73 });
//
// Canvas functions take canvas positions (pixels) and sizes in art pixels (times art.scale), like
// art.js. DOM helpers return canvases sized in CSS pixels for a UI scale `u` (docs/ART.md 3.2) and a
// device-pixel ratio `dpr`, so one art pixel is always a whole number of device pixels.
//
// Frames are nine-slice sprites: `: tags nine=L,T,R,B` gives the corner insets; the strip between two
// corners repeats along the edge (every edge in this set is uniform, so repeating equals stretching).

const BAR_FILL = { food: 'ui/bar_food', production: 'ui/bar_production', research: 'ui/bar_research', culture: 'ui/bar_culture',
  gp: 'ui/bar_gp', xp: 'ui/bar_xp', hp: 'ui/bar_hp', hp_low: 'ui/bar_hp_low', mana: 'ui/bar_mana', doom: 'ui/bar_doom' };
const YIELDS = ['food', 'production', 'commerce'];
const COMMERCES = ['gold', 'research', 'culture', 'espionage'];

// ------------------------------------------------------------------------------------ names

/**
 * Sprite name of an interface icon. kind: 'yield' | 'commerce' (index or name), 'civicoption'
 * (CIVICOPTION_X), 'alignment' (ALIGNMENT_X), 'mana' (BONUS_MANA_X or the sphere name),
 * 'armageddon' (the counter, 0-100), or 'icon' (a bare name: happy, healthy, turn...). size 16 or 8.
 * Game types with a mapping kind (religion, promotion, unitcombat, specialist, building, civ) go
 * through art.iconFor instead.
 */
export function iconName(kind, key, size = 16) {
  const ns = size === 8 ? 'icon8/' : 'icon16/';
  const low = (v) => String(v).toLowerCase();
  switch (kind) {
    case 'yield': return ns + (typeof key === 'number' ? YIELDS[key] : low(key).replace(/^yield_/, ''));
    case 'commerce': return ns + (typeof key === 'number' ? COMMERCES[key] : low(key).replace(/^commerce_/, ''));
    case 'civicoption': return ns + 'civic_' + low(key).replace(/^civicoption_/, '');
    case 'alignment': return ns + 'alignment_' + low(key).replace(/^alignment_/, '');
    case 'mana': return ns + 'sphere_' + low(key).replace(/^bonus_mana_|^sphere_/, '');
    case 'armageddon': {
      const n = Number(key) || 0;
      return ns + 'armageddon_' + (n >= 100 ? 'doom' : n >= 75 ? 'high' : n >= 50 ? 'mid' : 'low');
    }
    default: return ns + low(key);
  }
}

/**
 * The icon of a game type through the mapping (art.resolve), at 16 or 8 px: { name, remap }, or null
 * when the type has none. size 8 takes the icon8/ twin of an icon16/ sprite when the set has one
 * (religions, building classes; a temple's 8 px twin is its faith's glyph), else keeps the 16 px icon.
 */
export function iconFor(art, kind, type, size = 16) {
  if (!type) return null;
  const r = art.resolve(kind, type);
  if (!r || r.sprite === 'none') return null;
  let name = r.sprite;
  if (size === 8 && name.startsWith('icon16/')) {
    const small = 'icon8/' + name.slice(7).replace(/^bclass_temple_/, 'religion_');
    if (art.hasSprite(small)) name = small;
  }
  return { name, remap: r.opts.remap };
}

// ------------------------------------------------------------------------------------ nine-slice

/** Nine-slice insets of a frame sprite: { spr, L, T, R, B, fill } or null. */
export function nineOf(art, name) {
  const spr = art.sprite(name);
  if (!spr) return null;
  let L = 0, T = 0, R = 0, B = 0;
  for (const t of spr.meta.tags) {
    const m = /^nine=(\d+),(\d+),(\d+),(\d+)$/.exec(t);
    if (m) [L, T, R, B] = m.slice(1).map(Number);
  }
  const fill = spr.data[T * spr.w + L] !== 255;
  return { spr, L, T, R, B, fill };
}

/**
 * Draw a nine-slice frame with its top-left at canvas (x, y), w x h art px. v: sprite variant
 * options for art.entry (remap ...), plus fill: false to leave the centre out.
 */
export function drawNine(art, ctx, x, y, w, h, name, v = {}) {
  const n = nineOf(art, name);
  if (!n) return;
  const e = art.entry(name, v);
  const s = art.scale;
  ctx.imageSmoothingEnabled = false;
  const L = Math.min(n.L, Math.floor(w / 2)), R = Math.min(n.R, w - L), T = Math.min(n.T, Math.floor(h / 2)), B = Math.min(n.B, h - T);
  const sw = e.w, sh = e.h;
  const mw = sw - n.L - n.R, mh = sh - n.T - n.B;
  const cw = w - L - R, ch = h - T - B;
  const blit = (sx, sy, sW, sH, dx, dy, dW, dH) => {
    if (sW > 0 && sH > 0 && dW > 0 && dH > 0) ctx.drawImage(e.canvas, e.sx + sx * s, e.sy + sy * s, sW * s, sH * s, x + dx * s, y + dy * s, dW * s, dH * s);
  };
  // repeat a source strip over a destination rectangle (a 1 px strip is stretched in one call)
  const tile = (sx, sy, sW, sH, dx, dy, dW, dH) => {
    if (sW <= 0 || sH <= 0 || dW <= 0 || dH <= 0) return;
    const stepX = sW === 1 ? dW : sW, stepY = sH === 1 ? dH : sH;
    for (let j = 0; j < dH; j += stepY) for (let i = 0; i < dW; i += stepX) {
      const w1 = Math.min(stepX, dW - i), h1 = Math.min(stepY, dH - j);
      blit(sx, sy, sW === 1 ? 1 : w1, sH === 1 ? 1 : h1, dx + i, dy + j, w1, h1);
    }
  };
  blit(0, 0, L, T, 0, 0, L, T);
  blit(sw - R, 0, R, T, w - R, 0, R, T);
  blit(0, sh - B, L, B, 0, h - B, L, B);
  blit(sw - R, sh - B, R, B, w - R, h - B, R, B);
  tile(n.L, 0, mw, T, L, 0, cw, T);
  tile(n.L, sh - B, mw, B, L, h - B, cw, B);
  tile(0, n.T, L, mh, 0, T, L, ch);
  tile(sw - R, n.T, R, mh, w - R, T, R, ch);
  if (v.fill !== false && n.fill) tile(n.L, n.T, mw, mh, L, T, cw, ch);
}

function makeCanvas(w, h) {
  const c = document.createElement('canvas');
  c.width = Math.max(1, w); c.height = Math.max(1, h);
  return c;
}

/** Run fn with art.scale temporarily set to k. */
function withScale(art, k, fn) {
  const old = art.scale;
  art.scale = k;
  try { return fn(); } finally { art.scale = old; }
}

/** A canvas holding a nine-slice frame of w x h art px at art.scale. */
export function nineCanvas(art, name, w, h, v = {}) {
  const c = makeCanvas(w * art.scale, h * art.scale);
  drawNine(art, c.getContext('2d'), 0, 0, w, h, name, v);
  return c;
}

/**
 * CSS declarations that draw a frame sprite as a border-image at UI scale u (CSS px per art px)
 * and device-pixel ratio dpr: 'border-style: solid; border-width: ...; border-image: ...'.
 * The image is pre-scaled to u*dpr, so slices map 1:1 to device pixels and nothing is resampled.
 */
export function nineCSS(art, name, { u = 2, dpr = 1, remap } = {}) {
  const n = nineOf(art, name);
  if (!n) return '';
  const k = Math.max(1, Math.round(u * dpr));
  const url = art.icon(name, { scale: k, remap }).toDataURL();
  const px = (v) => `${v * u}px`;
  return `border-style: solid; border-width: ${px(n.T)} ${px(n.R)} ${px(n.B)} ${px(n.L)}; ` +
    `border-image: url(${url}) ${n.T * k} ${n.R * k} ${n.B * k} ${n.L * k}${n.fill ? ' fill' : ''} / ` +
    `${px(n.T)} ${px(n.R)} ${px(n.B)} ${px(n.L)} repeat;`;
}

// ------------------------------------------------------------------------------------ DOM helpers

/** A <canvas> showing a sprite (icon) at UI scale u, sized in CSS px. opts: { u, dpr, remap, title } */
export function iconEl(art, name, { u = 2, dpr = 1, remap, title, cls } = {}) {
  const k = Math.max(1, Math.round(u * dpr));
  const src = art.icon(name, { scale: k, remap });
  const spr = art.pick(name);
  const c = makeCanvas(src ? src.width : k, src ? src.height : k);
  if (src) c.getContext('2d').drawImage(src, 0, 0);
  c.style.width = `${(spr ? spr.w : 1) * u}px`;
  c.style.height = `${(spr ? spr.h : 1) * u}px`;
  c.className = 'ui-px' + (cls ? ' ' + cls : '');
  if (title) c.title = title;
  return c;
}

/**
 * A <canvas> with text in a bitmap font (Vigil or Tally), sized in CSS px. The canvas is as tall as
 * the font's capitals (7, or 5 for Tally) plus pad above and below. opts: { font, color (palette key),
 * shadow, smallCaps, u, dpr, pad, title }
 */
export function textEl(art, text, { font = 'vigil', color = '7', shadow = false, smallCaps = false, u = 2, dpr = 1, pad = 0, cls } = {}) {
  const label = text;
  text = foldText(text);
  const k = Math.max(1, Math.round(u * dpr));
  const tw = art.measureText(text, font, smallCaps) + (shadow ? 1 : 0);
  const th = (font === 'tally' ? 5 : 7) + (shadow ? 1 : 0) + pad * 2;
  const c = makeCanvas(Math.max(1, tw) * k, th * k);
  withScale(art, k, () => art.drawText(c.getContext('2d'), 0, (pad + (font === 'tally' ? 4 : 6)) * k, text, { font, color, shadow, smallCaps }));
  c.style.width = `${Math.max(1, tw) * u}px`;
  c.style.height = `${th * u}px`;
  c.className = 'ui-px ui-text' + (cls ? ' ' + cls : '');
  c.setAttribute('aria-label', label);
  return c;
}

/** The body face (Vigil Book, fonts.spr): lower case, x-height 5, on Vigil's 7-row cell. */
export const BOOK = 'vigil/book';

/**
 * Text folded to the glyphs the bitmap fonts have: accents dropped (Ljósálfar -> Ljosalfar), curly
 * quotes straightened, an ellipsis as three periods, a non-breaking space as a space.
 */
export function foldText(text) {
  return String(text).normalize('NFD').replace(/[\u0300-\u036f]/g, '')
    .replace(/[\u2018\u2019\u02bc]/g, "'").replace(/[\u201c\u201d]/g, '"').replace(/\u2026/g, '...').replace(/\u00a0/g, ' ');
}

const maskCache = new WeakMap(); // art -> Map(key -> data URL)

/**
 * Bitmap text as a DOM span whose colour is CSS `color` (currentColor), so it follows :hover,
 * :disabled, .sel and colour classes like ordinary text, while every glyph pixel stays on the art
 * grid: the text is drawn once at u*dpr and used as the span's mask. The real text sits inside
 * for screen readers and find-in-page. opts: { font ('vigil/book' default, 'vigil', 'tally'),
 * smallCaps, u, dpr, cls, title }. Height: 7 art px (5 for Tally).
 */
export function textSpan(art, text, { font = BOOK, smallCaps = false, u = 2, dpr = 1, cls, title } = {}) {
  const t = foldText(text);
  const k = Math.max(1, Math.round(u * dpr));
  const th = font === 'tally' ? 5 : 7;
  const tw = Math.max(1, art.measureText(t, font, smallCaps));
  if (!maskCache.has(art)) maskCache.set(art, new Map());
  const cache = maskCache.get(art);
  const key = `${font}|${smallCaps ? 1 : 0}|${k}|${t}`;
  let url = cache.get(key);
  if (!url) {
    const c = makeCanvas(tw * k, th * k);
    withScale(art, k, () => art.drawText(c.getContext('2d'), 0, (th - 1) * k, t, { font, color: '8', smallCaps }));
    url = c.toDataURL();
    cache.set(key, url);
  }
  const w = tw * u, h = th * u;
  const span = document.createElement('span');
  span.className = 'ui-ptext' + (cls ? ' ' + cls : '');
  const m = `url(${url}) 0 0 / ${w}px ${h}px no-repeat`;
  span.style.cssText = `width: ${w}px; height: ${h}px; -webkit-mask: ${m}; mask: ${m};`;
  if (title) span.title = title;
  const sr = document.createElement('span');
  sr.className = 'ui-sr';
  sr.textContent = text;
  span.append(sr);
  return span;
}

/**
 * Greedy word wrap for the bitmap fonts: lines of at most `width` art px (a longer word gets a line
 * of its own). Returns the lines as strings.
 */
export function wrapText(art, text, width, { font = BOOK, smallCaps = false } = {}) {
  const lines = [];
  for (const para of foldText(text).split('\n')) {
    let line = '';
    for (const word of para.split(/\s+/).filter(Boolean)) {
      const next = line ? `${line} ${word}` : word;
      if (line && art.measureText(next, font, smallCaps) > width) { lines.push(line); line = word; } else line = next;
    }
    lines.push(line);
  }
  return lines;
}

/**
 * A block of short bitmap text wrapped to `width` art px: a div of textSpans, one per line, `gap`
 * art px apart (default 3: a 10 px line pitch). For tooltips, option effects and notes; long prose
 * stays in the system serif.
 */
export function textBlock(art, text, width, { font = BOOK, smallCaps = false, u = 2, dpr = 1, gap = 3, cls } = {}) {
  const div = document.createElement('div');
  div.className = 'ui-ptext-block' + (cls ? ' ' + cls : '');
  div.style.cssText = `display: flex; flex-direction: column; align-items: flex-start; gap: ${gap * u}px;`;
  for (const line of wrapText(art, text, width, { font, smallCaps })) div.append(line ? textSpan(art, line, { font, smallCaps, u, dpr }) : document.createElement('br'));
  return div;
}

/** A canvas element with a nine-slice frame, sized in CSS px. */
export function nineEl(art, name, w, h, { u = 2, dpr = 1 } = {}) {
  const k = Math.max(1, Math.round(u * dpr));
  const c = withScale(art, k, () => nineCanvas(art, name, w, h));
  c.style.width = `${w * u}px`;
  c.style.height = `${h * u}px`;
  c.className = 'ui-px';
  return c;
}

/** CSS custom properties for the palette colours: --c-void ... --c-gold (docs/ART.md 11.2). */
export function paletteCSS(art) {
  return ':root {\n' + art.palette.colors.map((c) => `  --c-${c.name}: ${c.hex};`).join('\n') + '\n}\n';
}

/** CSS for a pointer cursor sprite (anchor = hotspot), at UI scale u. */
export function cursorCSS(art, name, { u = 2 } = {}) {
  const spr = art.pick(name);
  if (!spr) return 'auto';
  const url = art.icon(name, { scale: u }).toDataURL();
  return `url(${url}) ${spr.ax * u} ${spr.ay * u}, auto`;
}

/**
 * The kit as a stylesheet: palette properties; .ui-frame-<x> for every ui/frame_* sprite;
 * .ui-button (normal, :hover, :active, :disabled, .primary, .danger, :focus-visible);
 * .ui-cursor-<x> for every ui/cursor* sprite; .ui-px (pixelated canvases).
 */
export function cssText(art, { u = 2, dpr = 1 } = {}) {
  const o = { u, dpr };
  const out = [paletteCSS(art)];
  out.push(`.ui-px { image-rendering: pixelated; image-rendering: crisp-edges; display: inline-block; vertical-align: middle; }`);
  out.push(`.ui-ptext { display: inline-block; vertical-align: middle; flex: none; position: relative; background-color: currentColor; }`);
  out.push(`.ui-sr { position: absolute; left: 0; top: 0; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }`);
  for (const name of [...art.lib.sprites.keys()].filter((n) => n.startsWith('ui/frame_')).sort()) {
    out.push(`.ui-${name.slice(3).replace(/_/g, '-')} { ${nineCSS(art, name, o)} }`);
  }
  const btn = (n) => (art.sprite(n) ? nineCSS(art, n, o) : '');
  out.push(`.ui-button { ${btn('ui/button')} background: none; color: var(--c-bone); font: inherit; cursor: pointer; padding: 0 ${3 * u}px; min-height: ${11 * u}px; position: relative; display: inline-flex; align-items: center; justify-content: center; gap: ${2 * u}px; }`);
  out.push(`.ui-button:hover { ${btn('ui/button_hover')} color: var(--c-vellum); }`);
  out.push(`.ui-button:active { ${btn('ui/button_pressed')} color: var(--c-vellum); padding-top: ${u}px; }`);
  out.push(`.ui-button:disabled, .ui-button.disabled { ${btn('ui/button_disabled')} color: var(--c-ash); cursor: default; padding-top: 0; }`);
  out.push(`.ui-button.primary { ${btn('ui/button_primary')} color: var(--c-vellum); }`);
  out.push(`.ui-button.primary:hover { ${btn('ui/button_primary_hover')} }`);
  out.push(`.ui-button.danger { ${btn('ui/button_danger')} color: var(--c-bone); }`);
  out.push(`.ui-button.danger:hover { ${btn('ui/button_danger_hover')} }`);
  if (art.sprite('ui/focus')) {
    out.push(`.ui-button:focus { outline: none; }`);
    out.push(`.ui-button:focus-visible::after { content: ''; position: absolute; inset: ${-2 * u}px; pointer-events: none; ${nineCSS(art, 'ui/focus', o)} }`);
  }
  for (const name of [...art.lib.sprites.keys()].filter((n) => /^ui\/cursor/.test(n)).sort()) {
    const cls = name === 'ui/cursor' ? 'ui-cursor' : `ui-${name.slice(3).replace(/_/g, '-')}`;
    out.push(`.${cls} { cursor: ${cursorCSS(art, name, { u })}; }`);
  }
  return out.join('\n');
}

/** Install cssText into the document (replacing a previous install). Returns the <style>. */
export function installCSS(art, opts = {}) {
  let st = document.getElementById('ui-kit-css');
  if (!st) { st = document.createElement('style'); st.id = 'ui-kit-css'; document.head.append(st); }
  st.textContent = cssText(art, opts);
  return st;
}

// ------------------------------------------------------------------------------------ controls

const BUTTON = {
  normal: { normal: ['ui/button', '7'], hover: ['ui/button_hover', '8'], pressed: ['ui/button_pressed', '8'], disabled: ['ui/button_disabled', '5'] },
  primary: { normal: ['ui/button_primary', '8'], hover: ['ui/button_primary_hover', '8'], pressed: ['ui/button_pressed', '8'], disabled: ['ui/button_disabled', '5'] },
  danger: { normal: ['ui/button_danger', '7'], hover: ['ui/button_danger_hover', '8'], pressed: ['ui/button_pressed', '8'], disabled: ['ui/button_disabled', '5'] },
};

/**
 * A button on a canvas: frame by kind ('normal' | 'primary' | 'danger') and state ('normal' |
 * 'hover' | 'pressed' | 'disabled'), a label centred (Vigil small capitals, or font: BOOK), an
 * optional 8x8 or 16x16 icon at the left. (x, y) canvas position, w x h art px (h 11 for text, 13
 * with a 16 px icon...).
 */
export function drawButton(art, ctx, x, y, w, h, label, { kind = 'normal', state = 'normal', icon = null, focus = false, smallCaps = true, font = 'vigil' } = {}) {
  const s = art.scale;
  const [frame, color] = (BUTTON[kind] || BUTTON.normal)[state] || BUTTON.normal.normal;
  drawNine(art, ctx, x, y, w, h, frame);
  const push = state === 'pressed' ? 1 : 0;
  const ic = icon ? art.pick(icon) : null;
  if (label) label = foldText(label);
  const tw = label ? art.measureText(label, font, smallCaps) : 0;
  const total = tw + (ic ? ic.w + (label ? 2 : 0) : 0);
  let cx = Math.round((w - total) / 2) + push;
  if (ic) {
    art.drawSprite(ctx, x + (cx + ic.ax) * s, y + (Math.round((h - ic.h) / 2) + push + ic.ay) * s, ic.name,
      state === 'disabled' ? { remap: disabledRemap(art, ic) } : {});
    cx += ic.w + 2;
  }
  if (label) art.drawText(ctx, x + cx * s, y + (Math.round((h - 7) / 2) + 6 + push) * s, label, { font, color, smallCaps });
  if (focus) drawNine(art, ctx, x - 2 * s, y - 2 * s, w + 4, h + 4, 'ui/focus');
}

/** Remap that greys a sprite out (every colour to iron/ash by value), for disabled controls. */
export function disabledRemap(art, spr) {
  const used = new Set();
  for (const v of spr.data) if (v < art.palette.colors.length && v !== 0) used.add(v);
  return [...used].map((i) => {
    const L = art.palette.colors[i];
    const lum = 0.3 * L.rgb[0] + 0.59 * L.rgb[1] + 0.11 * L.rgb[2];
    return L.key + (lum > 120 ? '5' : lum > 50 ? '4' : '3');
  }).join(',');
}

/**
 * A progress bar: void-outlined track w x 5 art px, filled to frac (0-1) with the kind's colours
 * (food production research culture gp xp hp hp_low mana doom); `next` (0-1) adds a 50% checker
 * up to the value expected next turn.
 */
export function drawBar(art, ctx, x, y, w, frac, kind = 'research', { next = null, h = 5 } = {}) {
  const s = art.scale;
  drawNine(art, ctx, x, y, w, h, 'ui/bar');
  const fill = art.pick(BAR_FILL[kind] || BAR_FILL.research);
  if (!fill) return;
  const inner = w - 2;
  const f = Math.max(0, Math.min(inner, Math.round(inner * Math.max(0, Math.min(1, frac)))));
  const e = art.entry(fill.name);
  for (let j = 0; j < h - 2; j++) {
    const sy = Math.min(fill.h - 1, Math.round((j * (fill.h - 1)) / Math.max(1, h - 3)));
    if (f > 0) ctx.drawImage(e.canvas, e.sx, e.sy + sy * s, s, s, x + s, y + (1 + j) * s, f * s, s);
    if (next != null && next > frac) {
      const n = Math.min(inner, Math.round(inner * Math.min(1, next)));
      for (let i = f; i < n; i++) if (((i + j) & 1) === 0) ctx.drawImage(e.canvas, e.sx, e.sy + sy * s, s, s, x + (1 + i) * s, y + (1 + j) * s, s, s);
    }
  }
}

/** A commerce slider (11.4): a stone track with tick dots every 10%, the bone handle at pct (0-100). */
export function drawSlider(art, ctx, x, y, w, pct) {
  const s = art.scale;
  ctx.fillStyle = art.css('6');
  ctx.fillRect(x, y + 4 * s, w * s, s);
  ctx.fillStyle = art.css('5');
  for (let i = 0; i <= 10; i++) ctx.fillRect(x + Math.round(((w - 1) * i) / 10) * s, y + 6 * s, s, s);
  const h = art.pick('ui/slider_handle');
  if (h) art.drawSprite(ctx, x + Math.round(((w - 1) * pct) / 100) * s, y + 4 * s, h.name);
}

/** A horizontal rule with a centre ornament (the board's '—◆—'): iron line, ui/orn sprite. */
export function drawRule(art, ctx, x, y, w, { orn = 'ui/orn', color = '3' } = {}) {
  const s = art.scale;
  ctx.fillStyle = art.css(color);
  ctx.fillRect(x, y, w * s, s);
  const o = art.pick(orn);
  if (o) art.drawSprite(ctx, x + Math.floor(w / 2) * s, y, o.name);
}

/**
 * The illuminated initial of a modal (11.3): the letter in Vigil at 2x, ochre with an amber
 * highlight pixel and a void shadow, on the dusk square of ui/frame_initial. size in art px.
 */
export function drawInitial(art, ctx, x, y, letter, { size = 22 } = {}) {
  const s = art.scale;
  drawNine(art, ctx, x, y, size, size, 'ui/frame_initial');
  const g = art._glyph('vigil', letter.toUpperCase(), false);
  if (!g) return;
  const gx = Math.round((size - g.w * 2) / 2), gy = Math.round((size - g.h * 2) / 2);
  withScale(art, s * 2, () => {
    art.drawSprite(ctx, x + (gx + 1) * s + g.ax * 2 * s, y + (gy + 1) * s + g.ay * 2 * s, g.name, { remap: '80' });
    art.drawSprite(ctx, x + gx * s + g.ax * 2 * s, y + gy * s + g.ay * 2 * s, g.name, { remap: '8y' });
  });
  // the highlight: the glyph's first lit pixel from the top-left, in amber
  for (let j = 0; j < g.h; j++) for (let i = 0; i < g.w; i++) {
    if (g.data[j * g.w + i] !== 255) {
      ctx.fillStyle = art.css('l');
      ctx.fillRect(x + (gx + i * 2) * s, y + (gy + j * 2) * s, s, s);
      return;
    }
  }
}

// ------------------------------------------------------------------------------------ map overlays

/** Tile marks (11.7): kind 'hover' | 'select' | 'attack' | 'route' | 'target'; (x, y) the tile's top-left. */
export function drawTileMark(art, ctx, x, y, kind = 'hover', { mx = 0, my = 0 } = {}) {
  if (kind === 'target') drawRange(art, ctx, x, y, { mx, my, ring: false });
  // the 34x34 marks are anchored at (1,1): drawn at the tile's top-left, they stand 1 px outside
  // the tile on every side
  const spr = art.pick(`ui/tile_${kind}`);
  if (spr) art.drawSprite(ctx, x, y, spr.name);
}

/**
 * Range and area previews (11.7): the tile through a 25% grid of the ARC remap, and the dotted
 * lilac ring. (mx, my): the tile's map coordinates, so the grid stays in phase across tiles.
 */
export function drawRange(art, ctx, x, y, { mx = 0, my = 0, ring = true } = {}) {
  const T = art.tables.ARC;
  art.remapRegion(ctx, x, y, 32, 32, (i, j, c) => (((mx * 32 + i) & 1) === 0 && ((my * 32 + j) & 1) === 0 ? T[c] : c));
  const spr = ring ? art.pick('ui/tile_target') : null;
  if (spr) art.drawSprite(ctx, x, y, spr.name);
}

/**
 * The yield layer (PLAY_DESIGN "yield digits"): each non-zero yield of a tile as its 8x8 icon and a
 * Tally number with a void shadow, stacked in the tile's left column. yields: [food, production,
 * commerce]; (x, y) the tile's top-left.
 */
export function drawTileYields(art, ctx, x, y, yields) {
  const s = art.scale;
  let row = 0;
  YIELDS.forEach((name, i) => {
    const v = yields[i] | 0;
    if (!v) return;
    const yy = y + (2 + row * 9) * s;
    const spr = art.pick(`icon8/${name}`);
    if (spr) art.drawSprite(ctx, x + (1 + spr.ax) * s, yy + spr.ay * s, spr.name);
    art.drawText(ctx, x + 9 * s, yy + 6 * s, String(v), { font: 'tally', color: '8', shadow: true });
    row++;
  });
}

/** Where a path leg starts and ends inside a tile (the road anchor, docs/ART.md 5.6). */
export const PATH_ANCHOR = [16, 18];

/**
 * Path preview (11.7). (x0, y0): canvas position of tile (0, 0); steps: [{x, y, turn}] in tile
 * coordinates from the unit's tile (first) to the destination (last); `turn` is the turn on which
 * the unit reaches the step (1 = this turn). opts: { kind: 'move' | 'route' | 'attack',
 * unreachable, odds (attack, 0-100), time (ms, for the marching dots) }.
 */
export function drawPath(art, ctx, x0, y0, steps, { kind = 'move', unreachable = false, odds = null, time = art.time || 0 } = {}) {
  const s = art.scale;
  if (!steps || steps.length < 2) return;
  const [ax, ay] = PATH_ANCHOR;
  const P = (st) => [st.x * 32 + ax, st.y * 32 + ay];
  const last = steps.length - 1;
  const phase = Math.floor(time / 150) % 3;
  const marks = [];
  for (let i = 1; i <= last; i++) {
    const turn = steps[i].turn || 1;
    if (i < last && (steps[i + 1].turn || 1) !== turn) marks.push(P(steps[i]));
  }
  const end = P(steps[last]);
  const near = (x, y) => marks.some(([mx, my]) => Math.abs(mx - x) <= 4 && Math.abs(my - y) <= 4) || Math.abs(end[0] - x) <= 5 && Math.abs(end[1] - y) <= 5;
  for (let i = 1; i <= last; i++) {
    const [x1, y1] = P(steps[i - 1]), [x2, y2] = P(steps[i]);
    const turn = steps[i].turn || 1;
    const dot = kind === 'route' ? 'ui/path_dot_route' : kind === 'attack' && i === last ? 'ui/path_dot_attack' : turn > 1 ? 'ui/path_dot_far' : 'ui/path_dot';
    const n = Math.max(Math.abs(x2 - x1), Math.abs(y2 - y1));
    for (let k = (3 - phase) % 3; k < n; k += 3) {
      const px = Math.round(x1 + ((x2 - x1) * k) / n), py = Math.round(y1 + ((y2 - y1) * k) / n);
      if (i === 1 && k < 6) continue; // leave the unit's own tile clear
      if (near(px, py)) continue;
      art.drawSprite(ctx, x0 + px * s, y0 + py * s, dot);
    }
  }
  steps.slice(1, last).forEach((st, k) => {
    const i = k + 1;
    if ((steps[i + 1].turn || 1) !== (st.turn || 1)) roundel(art, ctx, x0, y0, P(st), String(st.turn || 1));
  });
  const fin = steps[last];
  if (kind === 'attack') {
    drawTileMark(art, ctx, x0 + fin.x * 32 * s, y0 + fin.y * 32 * s, 'attack');
    art.drawSprite(ctx, x0 + (fin.x * 32 + 16) * s, y0 + (fin.y * 32 + 15) * s, 'ui/path_attack');
    if (odds != null) drawOdds(art, ctx, x0 + (fin.x * 32 + 16) * s, y0 + (fin.y * 32 - 5) * s, odds);
  } else if (unreachable) {
    art.drawSprite(ctx, x0 + end[0] * s, y0 + end[1] * s, 'ui/path_unreachable');
  } else if ((fin.turn || 1) > 1) {
    roundel(art, ctx, x0, y0, end, String(fin.turn));
  } else {
    art.drawSprite(ctx, x0 + end[0] * s, y0 + end[1] * s, 'ui/path_end');
  }
}

function roundel(art, ctx, x0, y0, [x, y], text) {
  const s = art.scale;
  art.drawSprite(ctx, x0 + x * s, y0 + y * s, 'ui/path_turn');
  const tw = art.measureText(text, 'tally');
  art.drawText(ctx, x0 + (x - Math.floor(tw / 2)) * s, y0 + (y + 2) * s, text, { font: 'tally', color: '8' });
}

/**
 * Combat-odds chip (11.7): the odds in Tally on a soot chip centred at canvas (cx, cy); vellum
 * digits at 50% or more, scarlet on a crimson rim below. pct: a number (0-100, one decimal kept
 * under 10 and over 90) or a string.
 */
export function drawOdds(art, ctx, cx, cy, pct) {
  const s = art.scale;
  const v = typeof pct === 'number' ? pct : parseFloat(pct);
  let text = typeof pct === 'string' ? pct : (v < 10 || v > 90) && Math.round(v * 10) % 10 ? `${v.toFixed(1)}%` : `${Math.round(v)}%`;
  const good = !(v < 50);
  const tw = art.measureText(text, 'tally');
  const w = tw + 6, h = 9;
  const x = cx - Math.floor(w / 2) * s, y = cy - Math.floor(h / 2) * s;
  drawNine(art, ctx, x, y, w, h, good ? 'ui/odds' : 'ui/odds_bad');
  art.drawText(ctx, x + 3 * s, y + 6 * s, text, { font: 'tally', color: good ? '8' : 'R' });
  return w;
}

// ------------------------------------------------------------------------------------ promotions

/**
 * A promotion seal (11.5) with its level as vellum pips inside the lower rim (levels 1-5).
 * (x, y): canvas position of the seal's top-left. Returns false if the type has no icon.
 */
export function drawPromotion(art, ctx, x, y, type, { pips = true } = {}) {
  const r = art.resolve('promotion', type);
  if (!r || r.sprite === 'none') return false;
  const spr = art.pick(r.sprite);
  art.drawSprite(ctx, x + spr.ax * art.scale, y + spr.ay * art.scale, spr.name, { remap: r.opts.remap });
  const level = pips ? Number(art.mapper.entry('promotion', art.typeName('promotion', type))?.level) || 0 : 0;
  if (level > 0) drawPips(art, ctx, x, y, level, spr.w);
  return true;
}

/** Level pips inside the lower rim of a seal of width w: n vellum dots 2 px apart, centred (row 13). */
export function drawPips(art, ctx, x, y, n, w = 16) {
  const s = art.scale;
  const x1 = Math.floor((w - (2 * n - 1)) / 2);
  ctx.fillStyle = art.css("8");
  for (let i = 0; i < n; i++) ctx.fillRect(x + (x1 + 2 * i) * s, y + 13 * s, s, s);
}

/** Promotion seal as a DOM canvas (seal plus pips) at UI scale u. */
export function promotionEl(art, type, { u = 2, dpr = 1, title } = {}) {
  const k = Math.max(1, Math.round(u * dpr));
  const c = makeCanvas(16 * k, 16 * k);
  withScale(art, k, () => drawPromotion(art, c.getContext('2d'), 0, 0, type));
  c.style.width = `${16 * u}px`; c.style.height = `${16 * u}px`;
  c.className = 'ui-px';
  c.title = title || type;
  return c;
}
