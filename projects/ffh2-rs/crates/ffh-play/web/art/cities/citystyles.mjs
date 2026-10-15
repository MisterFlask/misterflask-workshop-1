#!/usr/bin/env node
// citystyles.mjs: the city styles (docs/ART.md 9.2) built with citykit.mjs; running it rewrites
// cities.spr, which is what the toolkit loads.
//
//   node cities/citystyles.mjs        (from web/art) rewrite cities/cities.spr, print colour counts
//
// Edit the styles here, not the generated file. A style is make(tier) -> a composed grid; the layouts
// follow the tiers of 9.1 (I 1 lit window ... V 8), footprints I ~16 wide to V ~32 (outline
// included), at most 34x34 with the base on tile row 30. compose() takes [part, x, lift] items, back
// to front; x is the part's left edge, lift its rows above the ground line.
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';
import { G, P, M, wall, peak, hip, dome, blobs, mound, stack, mark, side, house, compose, TABLES, halo, trim, finialOf } from './citykit.mjs';

// ================================================================================== cloister
// Gothic stone, steep dark roofs, a chapel spire (Bannor, Elohim, Grigori, Kuriotates), with
// variants for the Amurites (a mage tower), Calabim (dark stone) and Sidar (faded ash).
function cloister(t, o = {}) {
  const m = o.m || M.stone;
  const Hs = (a) => house({ m, ...a });
  const spire = (w, h, win = [], pitch = 3) => house({ m, w, h, roof: 'spire', pitch, eave: 1, win });
  const crystal = P(`
    .P.
    PP8
    Ppp
    .p.`);
  // the landmark: the chapel spire, or the Amurites' slender mage tower with a lilac crystal
  const land = (h, win) => (o.mage
    ? stack(crystal, wall(3, 1, { e: m.r, b: m.l, s: m.d }), house({ m, w: 3, h, roof: 'none', win }))
    : spire(3, h, win));
  // a cathedral front: a gable with a rose window (one L core) and a lit portal
  const front = (w, h) => Hs({ w, h, roof: 'steep', win: [[w >> 1, 3, 'L'], [(w >> 1) - 1, 3], [(w >> 1) + 1, 3], [w >> 1, 2], [w >> 1, 4]], door: w >> 1, lit: true });
  if (t === 1) {
    return compose([
      [land(6), 10],
      [Hs({ w: 5, h: 4, roof: 'steep' }), 5],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[1, 1]] }), 0],
    ]);
  }
  if (t === 2) {
    return compose([
      [land(9, [[1, 3]]), 12],
      [Hs({ w: 7, h: 5, roof: 'steep', win: [[3, 2]] }), 4],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[1, 1]] }), 0],
      [Hs({ w: 3, h: 3, roof: 'steep' }), 16],
    ]);
  }
  if (t === 3) {
    return compose([
      [Hs({ w: 3, h: 7, roof: 'spire', eave: 1 }), 1],
      [land(12, [[1, 3]]), 15],
      [Hs({ w: 8, h: 6, roof: 'steep', win: [[2, 2], [5, 2]] }), 5],
      [Hs({ w: 4, h: 4, roof: 'gable', win: [[1, 1]] }), 0],
      [Hs({ w: 4, h: 3, roof: 'steep' }), 12],
      [Hs({ w: 5, h: 3, roof: 'gable', win: [[3, 1]] }), 18],
    ]);
  }
  if (t === 4) {
    return compose([
      [land(15, [[1, 4], [1, 9]]), 18],
      [front(7, 9), 6],
      [spire(3, 10), 3],
      [spire(3, 10, [[1, 3]]), 13],
      [Hs({ w: 3, h: 4, roof: 'gable', win: [[1, 1]] }), 0],
      [Hs({ w: 5, h: 4, roof: 'steep', win: [[3, 1]] }), 22],
    ]);
  }
  return compose([
    [Hs({ w: 9, h: 9, roof: 'steep' }), 13],
    [land(19, [[1, 4], [1, 9], [1, 14]]), 21],
    [front(7, 10), 6],
    [spire(3, 11), 3],
    [spire(3, 11, [[1, 3]]), 13],
    [Hs({ w: 3, h: 5, roof: 'steep', win: [[1, 2]] }), 0],
    [Hs({ w: 4, h: 4, roof: 'gable' }), 18],
    [Hs({ w: 5, h: 4, roof: 'steep', win: [[3, 1]] }), 25],
  ]);
}

// ================================================================================== sun-domes
// Malakim: flat roofs, low domes, sand stone, a tall minaret.
function sundomes(t) {
  const m = M.sand;
  const block = (w, h, win = []) => house({ m, w, h, roof: 'flat', eave: 0, win });
  const domed = (w, h, dw, dh, win = [], door = null) => stack(dome(dw, dh, m), house({ m, w, h, roof: 'flat', eave: (dw - w) >> 1, win, door, lit: true }));
  const finial = P('Y\n6');
  const minaret = (h, win = []) => stack(finial, peak(3, 2, m), wall(4, 1, { e: m.r, b: m.l, s: m.d }), wall(2, h, m).remap({ [m.s]: m.b }), wall(2, 0, m)).map((c, x, y) => c);
  const mn = (h, win = []) => mark(minaret(h), win.map(([x, y]) => [x + 1, y + 6]));
  if (t === 1) {
    return compose([
      [mn(5), 10],
      [domed(5, 3, 5, 3), 4],
      [block(4, 3, [[1, 1]]), 0],
    ]);
  }
  if (t === 2) {
    return compose([
      [mn(9, [[0, 2]]), 13],
      [domed(7, 4, 7, 4, [], 3), 5],
      [block(4, 3, [[1, 1]]), 0],
      [block(4, 2), 16],
    ]);
  }
  if (t === 3) {
    return compose([
      [mn(13, [[0, 3]]), 17],
      [block(4, 5), 2],
      [domed(9, 5, 9, 5, [[2, 1], [6, 1]], 4), 6],
      [block(4, 3, [[1, 1]]), 0],
      [domed(4, 3, 4, 2), 19],
    ]);
  }
  if (t === 4) {
    return compose([
      [mn(16, [[0, 3], [0, 8]]), 21],
      [mn(10), 2],
      [domed(11, 6, 11, 6, [[2, 2], [8, 2]], 5), 7],
      [block(5, 4, [[1, 1]]), 0],
      [block(4, 3, [[2, 1]]), 19],
      [domed(4, 2, 4, 2), 24],
    ]);
  }
  return compose([
    [mn(19, [[0, 3], [0, 8]]), 25],
    [mn(13, [[0, 3]]), 4],
    [block(4, 8), 18],
    [domed(13, 7, 13, 7, [[2, 2], [10, 2], [6, 1, 'L']], 6), 8],
    [block(5, 4, [[1, 1]]), 0],
    [domed(5, 3, 5, 3, [[2, 1]]), 21],
    [block(5, 3), 27],
  ]);
}

// ================================================================================== elven
// Ljosalfar (and Svartalfar): great trees with lit hollows and platforms.
function elven(t, o = {}) {
  const C = o.dark ? { rim: 'P', lit: 'p', body: 'v', sh: 'v' } : { rim: 'T', lit: 't', body: 'k', sh: 'k' };
  const trunkM = o.dark ? { e: '4', b: '2', s: '1' } : { e: 'Q', b: 'u', s: '2' };
  const plank = o.dark ? '4' : 'y';
  // a tree: canopy lobes over a trunk with hollows (lit windows) and a platform
  const tree = (cw, ch, tw, th, win = [], plat = []) => {
    const r = cw / 2;
    const lobes = [[r, r * 0.9, r * 0.82], [r * 0.55, r * 1.2, r * 0.6], [r * 1.45, r * 1.15, r * 0.58]];
    if (ch > cw * 0.8) lobes.push([r, r * 0.35, r * 0.6]);
    const can = blobs(lobes, C);
    let trunk = wall(tw, th, trunkM);
    trunk = mark(trunk, win);
    const g = new G(Math.max(can.w, tw + 4), can.h + th - 2);
    const tx = Math.floor((g.w - tw) / 2);
    g.paint(trunk, tx, can.h - 2);
    // roots
    g.set(tx - 1, g.h - 1, trunkM.e); g.set(tx + tw, g.h - 1, trunkM.s);
    g.paint(can, Math.floor((g.w - can.w) / 2), 0);
    for (const y of plat) for (let x = tx - 2; x < tx + tw + 2; x++) g.set(x, can.h - 2 + y, plank);
    return g;
  };
  const pod = (w, win = true) => stack(dome(w, Math.ceil(w / 2), { r: C.rim, l: C.lit, d: C.body }), mark(wall(w, 2, trunkM), win ? [[w >> 1, 0]] : []));
  if (t === 1) {
    return compose([
      [tree(11, 9, 3, 5, [[1, 2]]), 2],
      [pod(4, false), 0],
    ]);
  }
  if (t === 2) {
    return compose([
      [tree(13, 11, 3, 7, [[1, 2], [1, 5]], [3]), 4],
      [pod(4), 0],
      [pod(4, false), 16],
    ]);
  }
  if (t === 3) {
    return compose([
      [tree(11, 9, 3, 6, [[1, 2]]), 13],
      [tree(15, 13, 3, 9, [[1, 2], [1, 6]], [4]), 2],
      [pod(4), 0],
      [pod(5), 18],
    ]);
  }
  if (t === 4) {
    return compose([
      [tree(13, 11, 3, 8, [[1, 3]], [5]), 15],
      [tree(17, 15, 5, 11, [[2, 2], [1, 6], [3, 6]], [4, 8]), 2],
      [pod(4), 0],
      [pod(5), 21],
      [pod(4, false), 12],
    ]);
  }
  return compose([
    [tree(11, 9, 3, 7, [[1, 2]]), 0],
    [tree(11, 10, 3, 9, [[1, 3]], [5]), 20],
    [tree(19, 17, 5, 14, [[2, 2], [1, 6], [3, 6], [2, 10, 'L']], [4, 9]), 7],
    [pod(4), 2],
    [pod(5), 24],
    [pod(4, false), 16],
  ]);
}

// ================================================================================== dwarven
// Khazad (and Luchuirp): halls cut into a rock mound: carved doorways, a great gate, chimneys with
// ember tops; the Luchuirp add a gear wheel and a golem forge.
function dwarven(t, o = {}) {
  const door = P(`
    555
    512
    5l2`);
  const shut = P(`
    552
    512
    512`);
  const gate = P(`
    .555.
    55552
    50002
    50l02
    5lLl2`);
  const chimney = (h) => { const g = new G(2, h + 1); g.set(0, 0, 'E'); g.set(1, 0, 'e'); for (let y = 1; y <= h; y++) { g.set(0, y, '4'); g.set(1, y, '2'); } return g; };
  const gear = P(`
    .5.5.
    55442
    .4.3.
    54322
    .3.2.`);
  const forge = P(`
    552
    5E2
    lEl`);
  const L = o.luchuirp;
  // the mound with its carvings ([stamp, x, rows above the ground]) and chimneys ([x, height])
  const hall = (w, h, carv, chim, bumps = []) => {
    const g = mound(w, h, undefined, bumps);
    for (const [st, x, yb] of carv) g.paint(st, x, g.h - yb - st.h);
    const top = (x) => { for (let y = 0; y < g.h; y++) if (g.op(x, y)) return g.h - y; return 0; };
    return compose([[g, 0], ...chim.map(([x, ch]) => [chimney(ch), x, Math.max(0, Math.min(top(x), top(x + 1)) - 2)])]);
  };
  if (t === 1) return hall(14, 7, [[door, 5, 0]], [[9, 3]]);
  if (t === 2) return hall(18, 9, [[door, 3, 0], [L ? forge : door, 12, 0], ...(L ? [[gear, 7, 3]] : [])], [[5, 3], [12, 3]]);
  if (t === 3) {
    return hall(22, 11, [[gate, 8, 0], [door, 2, 0], [door, 17, 0], L ? [gear, 3, 4] : [shut, 4, 5]], [[6, 3], [16, 4]], [[11, 13, 5]]);
  }
  if (t === 4) {
    return hall(26, 13, [[gate, 10, 0], [door, 3, 0], [L ? forge : door, 20, 0], [door, 6, 5], L ? [gear, 17, 6] : [shut, 18, 6], [mark(shut, [[1, 1, 'l']]), 11, 8]],
      [[4, 3], [19, 4], [22, 3]], [[12, 17, 6]]);
  }
  return hall(30, 15, [[gate, 12, 0], [door, 3, 0], [L ? forge : door, 24, 0], [door, 7, 6], [door, 20, 6], [mark(shut, [[1, 1, 'L']]), 13, 9], L ? [gear, 17, 11] : [shut, 9, 12]],
    [[4, 3], [24, 4], [21, 5]], [[10, 20, 6], [19, 22, 6]]);
}

// ================================================================================== tribal
// Clan of Embers, Doviello, Hippus (and the barbarians): hide tents, a longhouse, palisade wings
// and a central bonfire.
function tribal(t, o = {}) {
  const m = o.barb ? M.pelt : M.hide;
  const tent = (w, lit = false) => {
    const cone = peak(w, 2, m, { tip: false });
    const g = new G(cone.w, cone.h + 1);
    g.paint(cone, 0, 1);
    const c = (w - 1) >> 1;
    g.set(c - 1, 0, 'u'); g.set(c + 1, 0, 'u'); g.set(c, 1, 'u');
    g.set(c, g.h - 1, lit ? 'l' : m.door); g.set(c, g.h - 2, m.door);
    if (w >= 9) g.set(c + 1, g.h - 1, m.door);
    return g;
  };
  const longhouse = (w, h, win = []) => stack(hip(w + 2, 4, m), mark(wall(w, h, M.wood), win));
  const fire = P(`
    .l.
    lLE
    eEe`);
  // a palisade wing: stakes with gaps (the gaps become ink); the barbarians top some with skulls
  const palisade = (w) => {
    const g = new G(w, 4);
    for (let x = 0; x < w; x += 2) {
      const h = 3 + (x % 4 === 0 ? 1 : 0);
      for (let y = 4 - h; y < 4; y++) g.set(x, y, y === 4 - h ? (o.barb && x % 4 === 0 ? '7' : 'Q') : 'u');
    }
    return g;
  };
  const totem = o.barb ? P(`
    7.7
    272
    .u.
    .u.
    .u.
    .u.`) : P(`
    y.y
    yQy
    .u.
    .Q.
    .u.
    .u.`);
  const lookout = P(`
    5355
    .u.u
    .uuu
    .u.u
    .uuu
    .u.u
    u..u`);
  const fireAt = (x) => [fire, x, 1, { bare: true }];
  if (t === 1) return compose([[tent(7, true), 3], [tent(5), 9], fireAt(1), [palisade(5), 0], [palisade(5), 11]]);
  if (t === 2) return compose([[tent(7, true), 1], [longhouse(7, 3, [[3, 1]]), 8], [tent(5, true), 16], fireAt(7), [palisade(5), 0], [palisade(5), 17]]);
  if (t === 3) {
    return compose([[totem, 10, 2], [longhouse(9, 4, [[2, 1], [6, 1]]), 11], [tent(9, true), 2], [tent(5), 20], fireAt(9), [palisade(7), 0], [palisade(7), 19]]);
  }
  if (t === 4) {
    return compose([[o.barb ? lookout : totem, 1, 3], [totem, 24, 3], [longhouse(11, 5, [[2, 2], [8, 2]]), 12], [tent(9, true), 5], [tent(7, true), 21], [tent(5), 0], fireAt(13), [palisade(7), 0], [palisade(7), 23]]);
  }
  return compose([[o.barb ? lookout : totem, 27, 4], [totem, 2, 4], [longhouse(13, 6, [[2, 2], [6, 2], [10, 2]]), 10], [tent(11, true), 1], [tent(9, true), 23], [tent(7), 16], [tent(5, true), 0], fireAt(14), [palisade(9), 0], [palisade(7), 25]]);
}

// ================================================================================== harbour
// Lanun: stilt houses and piers, masts with furled sails, a lighthouse in the larger towns.
function harbour(t) {
  const m = M.wood;
  const stilts = (w, h) => { const g = new G(w, h); for (let y = 0; y < h; y++) for (let x = 0; x < w; x += 2) g.set(x, y, y === 0 ? 'Q' : 'u'); return g; };
  const shack = (w, h, s, win = []) => stack(house({ m, w, h, roof: 'steep', eave: 1, win }), stilts(w + 2, s));
  const mast = (h, yard = 5) => {
    const g = new G(yard, h + 3);
    const c = yard >> 1;
    g.set(c, 0, '#'); g.set(c + 1, 0, '#'); g.set(c, 1, '#');
    for (let y = 1; y < h + 3; y++) if (g.get(c, y) === '.') g.set(c, y, 'u');
    for (let x = 0; x < yard; x++) { g.set(x, 4, x === 0 ? 'Q' : 'u'); g.set(x, 5, x === 0 ? '7' : x === yard - 1 ? '5' : '6'); }
    return g;
  };
  const hull = (w) => {
    const g = new G(w, 3);
    for (let x = 0; x < w; x++) { g.set(x, 0, x === 0 ? 'Q' : 'y'); g.set(x, 1, x === 0 ? 'Q' : 'u'); if (x > 0 && x < w - 1) g.set(x, 2, '2'); }
    g.set(w - 1, 0, 'u');
    return g;
  };
  const pier = (w) => { const g = new G(w, 2); for (let x = 0; x < w; x++) { g.set(x, 0, x === 0 ? 'y' : 'Q'); if (x % 3 === 0) g.set(x, 1, 'u'); } return g; };
  const lighthouse = (h) => stack(P('.L.\nlLl').remap({}), house({ m: M.stone, w: 3, h: 1, roof: 'none' }), house({ m: M.stone, w: 3, h, roof: 'none', win: [[1, 2]] }));
  const lh = (h) => stack(peak(5, 1, M.stone), P('0lL0\n'.replace(/\n$/, '')).map((c) => c), house({ m: M.stone, w: 3, h, roof: 'none', win: [[1, 3]] }));
  if (t === 1) {
    return compose([
      [mast(9, 5), 11, 1],
      [hull(7), 9, 0],
      [shack(5, 3, 2, [[2, 1]]), 0],
    ]);
  }
  if (t === 2) {
    return compose([
      [mast(11, 5), 15, 1],
      [hull(8), 13, 0],
      [shack(5, 4, 2, [[2, 1]]), 0],
      [shack(4, 3, 2, [[1, 1]]), 7],
      [pier(5), 12, 0, { bare: true }],
    ]);
  }
  if (t === 3) {
    return compose([
      [mast(14, 7), 16, 1],
      [mast(10, 5), 20, 1],
      [hull(10), 15, 0],
      [shack(6, 5, 2, [[1, 1], [4, 1]]), 0],
      [shack(4, 3, 2, [[1, 1]]), 8],
    ]);
  }
  const big = t === 5;
  return compose([
    [lighthouse(big ? 16 : 13), 0, 0],
    [mast(big ? 17 : 14, 7), big ? 22 : 19, 1],
    [mast(big ? 12 : 10, 5), big ? 27 : 24, 1],
    [hull(big ? 12 : 10), big ? 20 : 18, 0],
    [shack(6, 6, 2, [[1, 1], [4, 1], [1, 4]]), 4],
    [shack(5, 4, 2, [[2, 1]]), 12],
    ...(big ? [[shack(4, 3, 2, [[1, 1]]), 8, 0]] : []),
  ]);
}

// ================================================================================== carnival
// Balseraphs: peaked tents in the owner's stripes (# field, % shade), a lantern string.
function carnival(t) {
  const tent = (w, h, lit = true) => {
    const cone = peak(w, 2, M.stone, { tip: false });
    const c = (w - 1) / 2;
    const roof = cone.map((k, x) => (k === '.' ? k : x <= c ? (x % 2 ? '%' : '#') : x % 2 ? '2' : '%'));
    // scalloped hem
    const hem = new G(w, 1);
    for (let x = 0; x < w; x++) if (x % 2 === 0) hem.set(x, 0, x <= c ? '#' : '%');
    let body = wall(w - 2, h, M.wood);
    if (lit) body = mark(body, [[(w - 2) >> 1, h - 1, 'l'], [(w - 2) >> 1, h - 2, 'u']]);
    const flag = P('#%\n#.\n0.').map((k) => (k === '0' ? 'u' : k));
    return stack(flag, roof, hem, body);
  };
  const wagon = (w) => stack(dome(w, 2, { r: '#', l: '#', d: '%' }), mark(wall(w, 2, M.wood), [[1, 0]]), P('u'.repeat(1)).map((k) => k));
  // lantern string: a sagging cord between two points, a lantern every 3 px
  const string = (w, sag = 2) => {
    const g = new G(w, sag + 2);
    for (let x = 0; x < w; x++) {
      const y = Math.round(sag * Math.sin((Math.PI * x) / (w - 1)));
      g.set(x, y, '2');
      if (x % 3 === 1) g.set(x, y + 1, 'l');
    }
    return g;
  };
  if (t === 1) {
    return compose([
      [tent(9, 3), 0],
      [tent(5, 2, false), 9],
    ]);
  }
  if (t === 2) {
    return compose([
      [string(9, 2), 4, 9, { bare: true }],
      [tent(9, 3), 0],
      [tent(7, 3), 11],
      [tent(5, 2, false), 7],
    ]);
  }
  if (t === 3) {
    return compose([
      [string(9, 2), 5, 11, { bare: true }],
      [tent(13, 4), 4],
      [tent(7, 3), 0],
      [tent(7, 3), 16],
      [tent(5, 2, false), 13],
    ]);
  }
  if (t === 4) {
    return compose([
      [string(9, 2), 3, 13, { bare: true }],
      [string(9, 2), 14, 13, { bare: true }],
      [tent(15, 5), 6],
      [tent(7, 3), 0],
      [tent(9, 4), 19],
      [tent(5, 2, false), 13],
    ]);
  }
  return compose([
    [string(11, 3), 2, 16, { bare: true }],
    [string(11, 3), 17, 16, { bare: true }],
    [tent(17, 7), 8],
    [tent(9, 4), 0],
    [tent(9, 5), 23],
    [tent(5, 2, false), 18],
    [tent(5, 2), 6],
  ]);
}

// ================================================================================== infernal
// Infernal and Sheaim: black jagged spires, crimson cracks at the base, scarlet windows.
function infernal(t) {
  // a thorn: tapering, curving to the right toward its hooked tip
  const thorn = (h, w, lean = 2, win = []) => {
    const g = new G(w + lean + 2, h);
    for (let y = 0; y < h; y++) {
      const f = y / (h - 1);
      const half = Math.max(0, Math.round(((w - 1) / 2) * Math.pow(f, 0.8)));
      const cx = Math.round((w - 1) / 2 + lean * Math.pow(1 - f, 2));
      for (let x = cx - half; x <= cx + half; x++) g.set(x, y, x === cx - half ? (y === 0 ? '5' : '4') : x <= cx ? '2' : '1');
    }
    let o2 = mark(g, win);
    const b = o2.bbox();
    return o2.crop(b.x0, 0, b.x1 - b.x0 + 1, h);
  };
  const base = (w, h, win = []) => mark(wall(w, h, M.thorn).map((k, x, y) => (y === 0 && x % 3 === 1 ? '.' : k)), win);
  const crack = (w) => { const g = new G(w, 1); for (let x = 0; x < w; x++) g.set(x, 0, x % 5 === 1 ? 'C' : x % 5 === 2 ? 'c' : '.'); return g; };
  if (t === 1) {
    return compose([
      [thorn(13, 5, 2, [[2, 9]]), 5],
      [thorn(8, 3, -1), 1],
      [thorn(7, 3, 1), 11],
      [base(12, 2), 2],
    ]);
  }
  if (t === 2) {
    return compose([
      [thorn(17, 5, 2, [[2, 8], [2, 13]]), 7],
      [thorn(11, 3, -1), 2],
      [thorn(9, 3, 2), 14],
      [thorn(6, 3, 1), 0],
      [base(16, 3, [[11, 1]]), 2],
    ]);
  }
  if (t === 3) {
    return compose([
      [thorn(21, 5, 3, [[2, 9], [2, 15]]), 10],
      [thorn(14, 5, -2, [[2, 9]]), 2],
      [thorn(11, 3, 2), 17],
      [thorn(7, 3, 1), 21],
      [base(20, 3, [[4, 1], [15, 1]]), 2],
    ]);
  }
  if (t === 4) {
    return compose([
      [thorn(25, 7, 3, [[3, 10], [3, 16], [3, 21, 'L']]), 10],
      [thorn(17, 5, -2, [[2, 10]]), 3],
      [thorn(13, 5, 2, [[2, 9]]), 19],
      [thorn(9, 3, -1), 0],
      [thorn(8, 3, 2), 25],
      [base(24, 4, [[5, 2], [12, 1], [19, 2]]), 2],
    ]);
  }
  return compose([
    [thorn(29, 7, 3, [[3, 11], [3, 18], [3, 24, 'L']]), 12],
    [thorn(21, 5, -3, [[2, 11], [2, 16]]), 4],
    [thorn(17, 5, 3, [[2, 10]]), 22],
    [thorn(11, 3, -1), 0],
    [thorn(10, 3, 2), 28],
    [base(28, 5, [[5, 2], [13, 1], [21, 2]]), 2],
  ]);
}

// ================================================================================== celestial
// Mercurians: pale slender towers with needle spires and domes, vellum light.
function celestial(t) {
  const m = M.pale;
  const needle = (w, h, win = []) => house({ m, w, h, roof: 'spire', pitch: 4, eave: 0, win });
  const orb = (w, h, win = []) => stack(P('8'), dome(w + 2, (w + 3) >> 1, m), house({ m, w, h, roof: 'none', win }));
  const arch = (w) => { const g = new G(w, 2); for (let x = 0; x < w; x++) { g.set(x, 0, x === 0 ? '8' : '7'); if (x === 0 || x === w - 1) g.set(x, 1, '6'); } return g; };
  if (t === 1) {
    return compose([
      [needle(3, 9, [[1, 3]]), 7],
      [orb(3, 3), 2],
    ]);
  }
  if (t === 2) {
    return compose([
      [needle(3, 13, [[1, 4]]), 9],
      [orb(3, 5, [[1, 2]]), 2],
      [needle(2, 6), 15],
      [arch(4), 5, 5, { bare: true }],
    ]);
  }
  if (t === 3) {
    return compose([
      [needle(3, 17, [[1, 4], [1, 9]]), 11],
      [orb(3, 8, [[1, 3]]), 3],
      [needle(2, 9), 18],
      [orb(3, 4), 0],
      [arch(5), 6, 7, { bare: true }],
      [house({ m, w: 4, h: 3, roof: 'dome', eave: 1, win: [[1, 1]] }), 15],
    ]);
  }
  if (t === 4) {
    return compose([
      [needle(3, 21, [[1, 5], [1, 11]]), 13],
      [orb(5, 10, [[2, 3], [2, 6]]), 3],
      [needle(2, 13, [[0, 4]]), 21],
      [needle(2, 8), 0],
      [arch(4), 9, 9, { bare: true }],
      [arch(4), 17, 7, { bare: true }],
      [house({ m, w: 5, h: 3, roof: 'dome', eave: 1, win: [[2, 1]] }), 23],
    ]);
  }
  return compose([
    [needle(3, 23, [[1, 5], [1, 11], [1, 17]]), 15],
    [orb(5, 13, [[2, 3], [2, 8]]), 4],
    [needle(3, 16, [[1, 5]]), 24],
    [needle(2, 10), 0],
    [needle(2, 9), 30],
    [arch(5), 10, 11, { bare: true }],
    [arch(5), 19, 9, { bare: true }],
    [house({ m, w: 5, h: 3, roof: 'dome', eave: 1 }), 18],
  ]);
}

// ================================================================================== frost
// Illians: rime-capped stone, cold rime windows, an ice spire.
function frost(t) {
  const m = M.rime;
  const Hs = (a) => {
    const g = house({ m, roof: 'steep', ...a });
    // icicles under the eaves
    return g;
  };
  const shard = (h, w, win = []) => {
    const g = new G(w, h);
    const c = (w - 1) / 2;
    for (let y = 0; y < h; y++) {
      const half = Math.min(c, Math.floor((y / h) * (w + 2) / 2));
      for (let x = Math.ceil(c - half); x <= Math.floor(c + half); x++) g.set(x, y, x < c ? (x === Math.ceil(c - half) ? 'F' : 'f') : x === Math.floor(c) && w % 2 ? 'F' : 'g');
    }
    g.set(Math.floor(c), 0, '8');
    return mark(g, win);
  };
  const crystal = (h) => side([[shard(h - 4, 3), 0], [shard(h, 5), 2], [shard(h - 6, 3), 6]]);
  if (t === 1) {
    return compose([
      [Hs({ w: 5, h: 4, win: [[2, 2]] }), 6],
      [Hs({ w: 4, h: 3, roof: 'gable' }), 0],
    ]);
  }
  if (t === 2) {
    return compose([
      [Hs({ w: 3, h: 8, roof: 'spire', eave: 1, win: [[1, 3]] }), 12],
      [Hs({ w: 7, h: 5, win: [[2, 2], [5, 2]] }), 4],
      [Hs({ w: 4, h: 3, roof: 'gable' }), 0],
    ]);
  }
  if (t === 3) {
    return compose([
      [crystal(16), 9],
      [Hs({ w: 6, h: 5, win: [[2, 2]] }), 2],
      [Hs({ w: 6, h: 4, win: [[3, 1]] }), 15],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[1, 1]] }), 0],
      [Hs({ w: 4, h: 3 }), 10],
    ]);
  }
  if (t === 4) {
    return compose([
      [crystal(21), 11],
      [Hs({ w: 3, h: 9, roof: 'spire', eave: 1, win: [[1, 3]] }), 3],
      [Hs({ w: 7, h: 6, win: [[2, 2], [5, 2]] }), 6],
      [Hs({ w: 6, h: 5, win: [[2, 2]] }), 18],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[1, 1]] }), 0],
      [Hs({ w: 4, h: 3 }), 14],
      [Hs({ w: 3, h: 3, roof: 'gable' }), 25],
    ]);
  }
  return compose([
    [crystal(27), 13],
    [Hs({ w: 3, h: 12, roof: 'spire', eave: 1, win: [[1, 3], [1, 8]] }), 4],
    [Hs({ w: 3, h: 9, roof: 'spire', eave: 1 }), 25],
    [Hs({ w: 8, h: 7, win: [[2, 2], [5, 2], [2, 5]] }), 7],
    [Hs({ w: 6, h: 5, win: [[2, 2]] }), 19],
    [Hs({ w: 4, h: 3, roof: 'gable', win: [[1, 1]] }), 0],
    [Hs({ w: 4, h: 3 }), 15],
    [Hs({ w: 3, h: 3, roof: 'gable', win: [[1, 1]] }), 26],
  ]);
}

// ================================================================================== the list
// table: the pool table that pre-lights the stone around windows (null: none).

const STYLES = [
  { name: 'cloister', note: 'gothic stone: steep dark roofs, a chapel spire, a cathedral front from tier IV', make: (t) => cloister(t), table: 'FIRE' },
  { name: 'amurite', note: 'cloister with a mage tower crowned by a lilac crystal; lilac windows', make: (t) => cloister(t, { mage: true }), table: 'ARC' },
  { name: 'calabim', note: 'cloister in dark stone', make: (t) => cloister(t, { m: M.dark }), table: 'FIRE' },
  {
    name: 'sidar', note: 'faded cloister in ash; no lit windows above tier II', make: (t) => cloister(t, { m: M.faded }), table: null,
    post: (g, t) => (t >= 3 ? g.remap({ l: '3', L: '3' }) : g),
  },
  { name: 'sundomes', note: 'sand stone, flat roofs, low domes and a minaret', make: sundomes, table: 'FIRE' },
  { name: 'elven', note: 'great trees with lit hollows, platforms and pods', make: (t) => elven(t), table: 'FIRE' },
  { name: 'svartalfar', note: 'elven trees in dusk and bruise, lilac hollows', make: (t) => elven(t, { dark: true }), table: 'ARC' },
  { name: 'dwarven', note: 'halls cut into a rock mound: carved doors, a gate, ember chimneys', make: (t) => dwarven(t), table: 'FIRE' },
  { name: 'luchuirp', note: 'dwarven halls with a gear wheel and a golem forge', make: (t) => dwarven(t, { luchuirp: true }), table: 'FIRE' },
  { name: 'tribal', note: 'hide tents, a longhouse, a palisade and a bonfire', make: (t) => tribal(t), table: 'FIRE' },
  { name: 'barbarian', note: 'dark pelt tents, a skull-topped palisade, a lookout', make: (t) => tribal(t, { barb: true }), table: 'FIRE' },
  { name: 'harbour', note: 'stilt houses, a hull at the pier, masts, a lighthouse from tier IV', make: harbour, table: 'FIRE' },
  { name: 'carnival', note: 'peaked tents in the owner stripes (# %), lantern strings', make: carnival, table: 'FIRE' },
  { name: 'infernal', note: 'black thorn spires, crimson cracks, scarlet windows', make: infernal, table: 'BLOOD', post: (g) => g },
  { name: 'celestial', note: 'pale slender towers, needle spires and domes, vellum light', make: celestial, table: null },
  { name: 'frost', note: 'rime-capped stone and an ice spire, rime windows', make: frost, table: 'COLD' },
];

// ---------------------------------------------------------------------------------- output

const HEADER = `; cities (docs/ART.md 9): city/<style>_<tier>, tiers I (pop 1-3) to V (20+).
; GENERATED by citystyles.mjs (node cities/citystyles.mjs from web/art): edit the styles there.
; Dark masses lit from the upper left (lit edge, shade at the right, void outline), windows in amber
; l (recoloured to the style's light by the renderer), the stone beside a window pre-lit through the
; style's pool table. Base on the bottom row (tile row 30). \`: finial\` is the capital's finial point.
`;

export function build() {
  const out = [];
  for (const st of STYLES) {
    for (let t = 1; t <= 5; t++) {
      let g = st.make(t);
      g = halo(g, TABLES[st.table]);
      if (st.post) g = st.post(g, t);
      g = trim(g, 0);
      const [fx, fy] = finialOf(g);
      out.push({ name: `city/${st.name}_${t}`, g, ax: Math.floor(g.w / 2), ay: g.h - 1, meta: [`finial ${fx},${fy}`, ...(t === 1 && st.note ? [`note ${st.note}`] : [])] });
    }
  }
  return out;
}

function spr(s) { return [`= ${s.name} ${s.g.w}x${s.g.h} ${s.ax},${s.ay}`, ...s.meta.map((m) => `: ${m}`), ...s.g.rows()].join('\n'); }

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  const list = build();
  const file = join(dirname(fileURLToPath(import.meta.url)), 'cities.spr');
  writeFileSync(file, `${HEADER}\n${list.map(spr).join('\n\n')}\n`);
  for (const s of list) {
    const c = [...s.g.colours()].sort().join('');
    console.log(`${s.name.padEnd(24)} ${`${s.g.w}x${s.g.h}`.padEnd(6)} ${String(c.length).padStart(2)} ${c}${c.length > 10 || s.g.w > 34 || s.g.h > 34 ? '   OVER' : ''}`);
  }
}
