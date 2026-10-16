// citykit.mjs: the city sprite kit (docs/ART.md 9): a pixel grid, materials, building primitives
// and the compositor that citystyles.mjs uses to build every city/<style>_<tier> sprite.
//
// Pieces are drawn back to front, each with its own 1 px void outline (4.2), so a building in front
// is inked against the one behind. Moonlight from the upper left: the left column of a wall and the
// left slope of a roof carry the moon rim (stone `6`, a step above any ground, so the silhouette
// separates from dark land at 1x), the right side is in shade. Walls are lighter than roofs, so a
// roof reads at 1x as a dark cap with a lit edge. Windows are amber `l` (the renderer recolours
// them to the style's light, 7.3), mostly 1x2, with one glow `L` core on the main hall, and a lit
// house's door is a 1x2 amber doorway; the stone
// beside and under a window is pre-lit through the style's halo table (ember `e`/rust `x` for
// firelight), so a window reads as a small hearth, not a single pixel.

// ------------------------------------------------------------------------------------- grid

class G {
  constructor(w, h) { this.w = w; this.h = h; this.d = Array.from({ length: h }, () => Array(w).fill('.')); }
  /** A hand-drawn stamp: rows of keys, '.' or ' ' transparent; a leading '|' protects spaces. */
  static of(text) {
    const rows = text.split('\n').filter((r) => r.trim().length).map((r) => r.trim().replace(/^\|/, '').replace(/\|$/, ''));
    const g = new G(Math.max(...rows.map((r) => r.length)), rows.length);
    rows.forEach((r, y) => [...r].forEach((c, x) => { g.d[y][x] = c === ' ' ? '.' : c; }));
    return g;
  }
  get(x, y) { return x >= 0 && y >= 0 && x < this.w && y < this.h ? this.d[y][x] : '.'; }
  set(x, y, c) { if (x >= 0 && y >= 0 && x < this.w && y < this.h) this.d[y][x] = c; }
  op(x, y) { return this.get(x, y) !== '.'; }
  clone() { const g = new G(this.w, this.h); g.d = this.d.map((r) => r.slice()); return g; }
  map(fn) { const g = this.clone(); for (let y = 0; y < this.h; y++) for (let x = 0; x < this.w; x++) g.d[y][x] = fn(this.d[y][x], x, y); return g; }
  remap(t) { return t ? this.map((c) => (c in t ? t[c] : c)) : this; }
  mirror() { return this.map((_, x, y) => this.d[y][this.w - 1 - x]); }
  paint(src, x, y) {
    for (let j = 0; j < src.h; j++) for (let i = 0; i < src.w; i++) { const c = src.d[j][i]; if (c !== '.') this.set(x + i, y + j, c); }
    return this;
  }
  /** A copy grown by 1 px all round, with a void outline (4-neighbour) around opaque pixels. */
  outlined() {
    const g = new G(this.w + 2, this.h + 2);
    g.paint(this, 1, 1);
    const add = [];
    for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) {
      if (g.d[y][x] === '.' && (g.op(x - 1, y) || g.op(x + 1, y) || g.op(x, y - 1) || g.op(x, y + 1))) add.push([x, y]);
    }
    for (const [x, y] of add) g.d[y][x] = '0';
    return g;
  }
  bbox() {
    let x0 = this.w, y0 = this.h, x1 = -1, y1 = -1;
    for (let y = 0; y < this.h; y++) for (let x = 0; x < this.w; x++) if (this.d[y][x] !== '.') { x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y); }
    return { x0, y0, x1, y1 };
  }
  crop(x0, y0, w, h) { const g = new G(w, h); for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) g.d[y][x] = this.get(x0 + x, y0 + y); return g; }
  colours() { const s = new Set(); for (const r of this.d) for (const c of r) if (!'.0#%*@'.includes(c)) s.add(c); return s; }
  rows() { return this.d.map((r) => r.join('')); }
}
const P = (text) => G.of(text);

// ------------------------------------------------------------------------------- materials
// Walls: e (lit edge), b (body), s (shade side); roofs: r (rim on the lit slope), l (lit half),
// d (dark half), t (tip); door: an unlit doorway. The roof rim r is the moon rim (ART.md 4.2).

const M = {
  // daylight towns (the revamp): warm cream and stone walls under red, gold or straw roofs
  stone: { e: '8', b: '7', s: 'G', r: 'R', l: 'C', d: 'I', t: 'K', door: 'h' },
  dark: { e: '6', b: '5', s: '4', r: 'C', l: 'I', d: 'c', t: '6', door: '0' },
  faded: { e: '7', b: 'G', s: 'D', r: 'H', l: 'D', d: 'B', t: 'H', door: '4' },
  rime: { e: 'F', b: 'f', s: 'g', r: '8', l: 'F', d: 'g', t: '8', door: '1' },
  sand: { e: '8', b: '7', s: 'Q', r: 'K', l: 'y', d: 'j', t: 'K', door: 'h' },
  wood: { e: 'j', b: 'i', s: 'r', r: 'Q', l: 'j', d: 'i', t: 'Q', door: 'h' },
  hide: { e: '8', b: '7', s: 'j', r: '8', l: '7', d: 'j', t: 'i', door: 'h' },
  pelt: { e: 'G', b: 'D', s: 'B', r: 'H', l: 'G', d: 'D', t: 'i', door: 'h' },
  thatch: { e: 'Y', b: 'j', s: 'i', r: 'K', l: 'y', d: 'j', t: 'y', door: 'h' },
  pale: { e: '8', b: '7', s: '6', r: '8', l: 'F', d: 'f', t: 'K', door: '5' },
  thorn: { e: 'I', b: 'c', s: 'c', r: 'R', l: 'C', d: 'c', t: 'R', door: '0' },
  rock: { e: '6', b: '5', s: '4', r: '7', l: '5', d: '4', t: '7', door: 'h' },
};

// ------------------------------------------------------------------------------ primitives

/** A wall w x h: the left column lit, the right column in shade, the rest body. */
function wall(w, h, m, o = {}) {
  const g = new G(w, h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) g.d[y][x] = x === 0 && w > 1 ? m.e : x === w - 1 && w > 2 ? m.s : m.b;
  if (o.eave) for (let x = 1; x < w - 1; x++) g.d[0][x] = m.s;
  return g;
}

/** Row widths of a tapering roof from the top: `pitch` rows per 1 px step in on each side. */
function taper(w, pitch, top) {
  const rows = [];
  for (let wd = top ?? (w % 2 ? 1 : 2); wd <= w; wd += 2) for (let k = 0; k < pitch; k++) rows.push(wd);
  return rows;
}

/** A pointed roof (spire, gable end, tent): rim on the lit slope, left half lit, right half dark. */
function peak(w, pitch, m, o = {}) {
  const rows = taper(w, pitch, o.top);
  if (o.cut) rows.splice(0, o.cut);
  const g = new G(w, rows.length);
  const c = (w - 1) / 2;
  rows.forEach((wd, y) => {
    const a = (w - wd) / 2;
    for (let x = a; x < a + wd; x++) g.d[y][x] = x === a ? m.r : x <= c ? m.l : m.d;
  });
  if (o.tip !== false) g.d[0][Math.floor(c)] = o.tip || m.t;
  return g;
}

/** A long (hipped) roof seen from the side: a trapezoid h rows tall, lit ridge, shaded eave row. */
function hip(w, h, m) {
  const g = new G(w, h);
  for (let y = 0; y < h; y++) {
    const a = h - 1 - y;
    for (let x = a; x < w - a; x++) g.d[y][x] = y === 0 || x === a ? m.r : x === w - 1 - a ? m.d : y === h - 1 ? m.d : m.l;
  }
  return g;
}

/** A half-ellipse dome w wide and h tall, lit at the upper left. */
function dome(w, h, m) {
  const g = new G(w, h);
  const cx = (w - 1) / 2, rx = w / 2;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const u = (x - cx) / rx, v = (h - 0.5 - y) / h;
    if (u * u + v * v <= 1.05) g.d[y][x] = u > 0.25 ? m.d : m.l;
  }
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    if (g.d[y][x] === '.' || x > cx + 0.5) continue;
    if (g.get(x - 1, y) === '.' || g.get(x, y - 1) === '.') g.d[y][x] = m.r;
  }
  return g;
}

/** Union of discs [[cx, cy, r]], lit from the upper left: rim, lit, body, shade (canopies, mounds). */
function blobs(list, keys, o = {}) {
  const W = Math.ceil(Math.max(...list.map(([x, , r]) => x + r))) + 1, H = Math.ceil(Math.max(...list.map(([, y, r]) => y + r))) + 1;
  const g = new G(W, H);
  const inside = (x, y) => list.some(([cx, cy, r]) => (x - cx) ** 2 + (y - cy) ** 2 <= r * r);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    if (!inside(x, y)) continue;
    // nearest disc decides the light
    let best = null, bd = Infinity;
    for (const [cx, cy, r] of list) { const d = Math.hypot(x - cx, y - cy) / r; if (d < bd) { bd = d; best = [cx, cy, r]; } }
    const [cx, cy, r] = best;
    const nx = (x - cx) / r, ny = (y - cy) / r;
    const lightness = -nx * 0.6 - ny * 0.8;
    g.d[y][x] = lightness > 0.35 ? keys.lit : lightness < -0.25 ? keys.sh : keys.body;
  }
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    if (g.d[y][x] === '.') continue;
    if ((!g.op(x, y - 1) || !g.op(x - 1, y)) && g.d[y][x] === keys.lit) g.d[y][x] = keys.rim;
  }
  if (o.flatBottom) { const b = g.bbox(); return g.crop(b.x0, b.y0, b.x1 - b.x0 + 1, b.y1 - b.y0 + 1); }
  const b = g.bbox();
  return g.crop(b.x0, b.y0, b.x1 - b.x0 + 1, b.y1 - b.y0 + 1);
}

/**
 * A rock mound w wide, h tall at its crown: a rounded mesa (plus optional bumps [[x, h]]) lit from
 * the upper left: rim along the lit top edge, lit left flank, body, shade at the right with strata.
 */
function mound(w, h, keys = { rim: '5', lit: '4', body: '3', sh: '2' }, bumps = []) {
  const hs = [];
  for (let x = 0; x < w; x++) {
    const u = Math.abs((2 * x + 1) / w - 1);
    let hx = h * Math.pow(Math.max(0, 1 - Math.pow(u, 2.2)), 0.55);
    for (const [bx, bh, br = 4] of bumps) hx = Math.max(hx, bh * Math.max(0, 1 - Math.abs(x - bx) / br));
    hs.push(Math.max(1, Math.round(hx)));
  }
  const H = Math.max(...hs);
  const g = new G(w, H);
  for (let x = 0; x < w; x++) for (let y = H - hs[x]; y < H; y++) {
    const f = x / (w - 1);
    let k = f < 0.38 ? keys.lit : f > 0.72 ? keys.sh : keys.body;
    if (f > 0.5 && (y - (H - hs[x])) % 4 === 2 && x % 3 !== 0) k = keys.sh;
    g.d[y][x] = k;
  }
  for (let x = 0; x < w; x++) {
    const top = H - hs[x];
    if (x / (w - 1) < 0.6) { g.d[top][x] = keys.rim; if (x > 0 && hs[x] > hs[x - 1] + 1) for (let y = top; y < H - hs[x - 1]; y++) g.d[y][x] = keys.rim; }
  }
  return g;
}

/**
 * A crag w wide: a ridge through control points [[x, height], ...] (linear between them), lit on
 * the slopes that face the upper left (rim on top, lit below it), shade on the others, body under.
 */
function crag(w, pts, keys = { rim: '5', lit: '4', body: '3', sh: '2' }) {
  const hs = [];
  for (let x = 0; x < w; x++) {
    let k = 0;
    while (k < pts.length - 2 && x > pts[k + 1][0]) k++;
    const [x0, h0] = pts[k], [x1, h1] = pts[k + 1];
    hs.push(Math.max(1, Math.round(h0 + ((h1 - h0) * (x - x0)) / Math.max(1, x1 - x0))));
  }
  const H = Math.max(...hs);
  const g = new G(w, H);
  for (let x = 0; x < w; x++) {
    const up = (hs[x + 1] ?? hs[x]) - (hs[x - 1] ?? hs[x]);
    const litSlope = up > 0 || (up === 0 && x < w / 2);
    for (let y = H - hs[x]; y < H; y++) {
      const d = y - (H - hs[x]);
      let k = litSlope ? (d === 0 ? keys.rim : d < 3 ? keys.lit : keys.body) : d === 0 ? keys.body : keys.sh;
      if (!litSlope && d > 1 && (d + x) % 5 === 0) k = keys.body;
      g.d[y][x] = k;
    }
  }
  return g;
}

/** Stack parts vertically, centred: [top, ..., bottom]. */
function stack(...parts) {
  parts = parts.filter(Boolean);
  const W = Math.max(...parts.map((p) => p.w));
  const g = new G(W, parts.reduce((a, p) => a + p.h, 0));
  let y = 0;
  for (const p of parts) { g.paint(p, Math.floor((W - p.w) / 2), y); y += p.h; }
  return g;
}

/**
 * Put marks on a part: [[x, y, key, tall], ...] (windows, doors, details), in the part's coordinates.
 * tall 2 makes a 1x2 window (key on top, amber under it). A plain window (no key) is a 1x2 lancet by
 * default (second round: a lit window must read at 1x as a light, not a speck); a keyed mark is 1 px.
 */
function mark(g, list) {
  const o = g.clone();
  for (const [x, y, k, tall = k ? 1 : 2] of list) for (let j = 0; j < tall; j++) o.set(x, y + j, j ? 'l' : k || 'l');
  return o;
}

/** Paint parts side by side on a common ground line: [[part, x], ...] (no outlines: one mass). */
function side(items) {
  const W = Math.max(...items.map(([p, x]) => x + p.w)), H = Math.max(...items.map(([p]) => p.h));
  const g = new G(W, H);
  for (const [p, x] of items) g.paint(p, x, H - p.h);
  return g;
}

/**
 * A building: a wall with a roof. roof: gable | steep | spire | dome | flat | crenel | none;
 * eave: roof overhang per side; win: [[x, y, key]] in wall coordinates; door: x of a door.
 */
function house({ w, h, m = M.stone, roof = 'gable', eave = 1, win = [], door = null, lit = false, pitch, tip, domeH, rm }) {
  const R = rm || m;
  let body = wall(w, h, m, { eave: ['gable', 'steep', 'spire'].includes(roof) && w >= 4 });
  body = mark(body, win);
  if (door !== null) body = mark(body, lit ? [[door, h - 2, 'l'], [door, h - 1, 'l']] : [[door, h - 1, m.door], [door, h - 2, m.door]]);
  let top = null;
  const rw = w + 2 * eave;
  if (roof === 'gable') top = peak(rw, pitch || 1, R, { tip });
  else if (roof === 'steep') top = peak(rw, pitch || 2, R, { tip });
  else if (roof === 'spire') top = peak(rw, pitch || 3, R, { tip });
  else if (roof === 'dome') top = dome(rw, domeH || Math.ceil(rw / 2), R);
  else if (roof === 'flat') { top = new G(rw, 1); for (let x = 0; x < rw; x++) top.d[0][x] = x === 0 ? R.r : x === rw - 1 ? R.d : R.l; }
  else if (roof === 'crenel') {
    top = new G(w, 2);
    for (let x = 0; x < w; x++) { top.d[1][x] = x === 0 ? m.e : m.b; if (x % 2 === 0) top.d[0][x] = x === 0 ? m.e : m.b; }
  }
  return stack(top, body);
}

// ------------------------------------------------------------------------------ composing

/**
 * Compose pieces back to front: [part, x, lift, {bare}] with x the part's left edge and lift its
 * rows above the ground line. Each part gets its own outline unless bare.
 */
function compose(items, W = 40, H = 48) {
  const c = new G(W, H);
  const ground = H - 2;
  for (const it of items) {
    if (!it) continue;
    const [part, x, lift = 0, o = {}] = it;
    const p = o.bare ? part : part.outlined();
    const k = o.bare ? 0 : 1;
    c.paint(p, x + 2 - k, ground - lift - (part.h - 1) - k);
  }
  return c;
}

// Halo tables: what the stone beside and under a lit window becomes (the window's 1 px ring, 4.4).
// FIRE warms toward the ember ramp (dark stone to ember e, mid stone to rust x, lit stone to flame
// E), so a lit window reads as a small hearth in the wall; BLOOD and ARC do the same in crimson and
// lilac; COLD (Illians) frosts the stone instead.
const TABLES = {
  FIRE: { 1: 'e', 2: 'e', 3: 'e', 4: 'x', 5: 'x', 6: 'E', u: 'e', Q: 'x', y: 'E', Y: 'E', k: 'e', t: 'x', T: 'x', v: 'e', p: 'x', x: 'E', '#': '#', '%': '%' },
  BLOOD: { 1: 'c', 2: 'c', 3: 'c', 4: 'c', 5: 'C', 6: 'C', u: 'c' },
  ARC: { 1: 'v', 2: 'v', 3: 'p', 4: 'p', 5: 'p', 6: 'P', k: 'v', t: 'p', T: 'P', v: 'p', p: 'P', u: 'v', Q: 'p' },
  COLD: { 2: 'g', 3: 'g', 4: 'f', 5: 'f' },
  // the barbarians' rough camp: every dark material to ember, the bone and stone tops to flame
  EMBER: { 2: 'e', 3: 'e', 4: 'e', 5: 'e', u: 'e', Q: 'e', 6: 'E', 7: 'E' },
};

/** Pre-light the stone beside and under a window through `table` (its 1 px ring). */
function halo(g, table, keys = 'lL') {
  if (!table) return g;
  const out = g.clone();
  const T = (c) => (table[c] && !'0.'.includes(c) ? table[c] : c);
  for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) {
    if (!keys.includes(g.d[y][x])) continue;
    for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1]]) {
      const c = g.get(x + dx, y + dy);
      if (c === '.' || c === '0' || keys.includes(c)) continue;
      out.set(x + dx, y + dy, T(c));
    }
  }
  return out;
}

/** Count the lit windows of a grid: 4-connected groups of l and L pixels. */
function windows(g) {
  const seen = new Set();
  let n = 0;
  for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) {
    if (!'lL'.includes(g.d[y][x]) || seen.has(y * g.w + x)) continue;
    n++;
    const todo = [[x, y]];
    while (todo.length) {
      const [i, j] = todo.pop();
      if (seen.has(j * g.w + i) || !'lL'.includes(g.get(i, j))) continue;
      seen.add(j * g.w + i);
      todo.push([i + 1, j], [i - 1, j], [i, j + 1], [i, j - 1]);
    }
  }
  return n;
}

/** Trim to the opaque bounding box, keeping `room` empty rows above for the capital finial. */
function trim(g, room = 2) {
  const b = g.bbox();
  const y0 = Math.max(0, b.y0 - room);
  return g.crop(b.x0, y0, b.x1 - b.x0 + 1, b.y1 - y0 + 1);
}

/** The finial point: the topmost outline pixel (over the tallest spire). */
function finialOf(g) {
  for (let y = 0; y < g.h; y++) {
    const xs = [];
    for (let x = 0; x < g.w; x++) if (g.d[y][x] !== '.') xs.push(x);
    if (xs.length) return [xs[Math.floor((xs.length - 1) / 2)], y];
  }
  return [0, 0];
}


export { G, P, M, wall, taper, peak, hip, dome, blobs, mound, crag, stack, mark, side, house, compose, TABLES, halo, windows, trim, finialOf };
