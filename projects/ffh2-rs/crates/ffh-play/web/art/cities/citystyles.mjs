#!/usr/bin/env node
// citystyles.mjs: the city styles (docs/ART.md 9.2) built with citykit.mjs; running it rewrites
// cities.spr, which is what the toolkit loads.
//
//   node cities/citystyles.mjs        (from web/art) rewrite cities/cities.spr, print sizes, colour
//                                     counts and lit windows per sprite
//
// Edit the styles here, not the generated file. A style is make(tier) -> a composed grid; the layouts
// follow the tiers of 9.1: footprints (outline included) I about 26x17, II 30x23, III 30x30,
// IV 33x33, V 34x34, with about 3 / 5 / 7 / 9 / 12 lit windows. Tiers I and II have outskirts: a
// low house (hut, pod, tent, shack...) behind each end of the town, so a small town already spreads
// across most of its tile (the art director's light review: bigger, better-lit cities). compose() takes [part, x, lift]
// items, back to front; x is the part's left edge, lift its rows above the ground line.
//
// Placement: the anchor sits LIFT rows below the sprite, so the city's ground line (its bottom
// outline row) lands on tile row 27 and the plaque (tile rows 29-35) never covers a door or the
// lowest windows; tile row 28 shows the lit ground in front of the city. The tallest masses keep to
// the middle columns: the tile's top-left belongs to the city banner, its top-right to a garrison's
// standard.
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';
import { G, P, M, wall, peak, hip, dome, blobs, crag, stack, mark, side, house, compose, TABLES, halo, windows, trim, finialOf } from './citykit.mjs';

const LIFT = 3;

// ================================================================================== cloister
// Gothic stone, steep dark roofs with a moonlit west edge, a chapel spire (Bannor, Elohim, Grigori,
// Kuriotates), with variants for the Amurites (a mage tower), Calabim (dark stone) and Sidar (faded).
function cloister(t, o = {}) {
  const m = o.m || M.stone;
  const Hs = (a) => house({ m, ...a });
  const spire = (w, h, win = [], pitch = 3) => house({ m, w, h, roof: 'spire', pitch, eave: 1, win });
  const crystal = P(`
    ..P..
    .PPp.
    PPPpp
    .Ppp.
    ..p..`);
  // the bell tower (w 5, its spire 12 rows), or the Amurites' slender mage tower crowned by a lilac
  // crystal, as tall
  const tower = (h, win = []) => (o.mage
    ? stack(crystal, wall(5, 1, { e: m.r, b: m.l, s: m.d }), house({ m, w: 3, h: h + 6, roof: 'none', win: win.map(([x, y, k, tl]) => [Math.min(1, x - 1), y, k, tl]) }))
    : spire(5, h, win));
  // the abbey front: a steep gable with a rose window (one L core), two lancets and a lit portal
  const front = (w, h) => {
    const c = w >> 1;
    return Hs({ w, h, roof: 'steep', win: [[c, 3, 'L'], [c - 1, 3], [c + 1, 3], [c, 2], [c, 4], [1, h - 4, 'l', 2], [w - 2, h - 4, 'l', 2]], door: c, lit: true });
  };
  if (t === 1) {
    return compose([
      [Hs({ w: 3, h: 2, roof: 'gable', win: [[1, 0]] }), 0],
      [Hs({ w: 3, h: 3, roof: 'steep' }), 21],
      [o.mage ? tower(2) : spire(3, 6), 14],
      [Hs({ w: 6, h: 5, roof: 'steep', win: [[1, 1, 'L', 2]], door: 4, lit: true }), 8],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[2, 1]] }), 4],
      [Hs({ w: 3, h: 3, roof: 'gable', win: [[1, 1]] }), 17],
    ]);
  }
  if (t === 2) {
    return compose([
      [Hs({ w: 3, h: 2, roof: 'gable', win: [[1, 0]] }), 0],
      [Hs({ w: 3, h: 3, roof: 'gable', win: [[1, 1]] }), 24],
      [o.mage ? tower(5, [[2, 4, 'l', 2]]) : spire(4, 12, [[1, 4, 'l', 2]]), 16],
      [Hs({ w: 8, h: 6, roof: 'steep', win: [[2, 2, 'L', 2], [5, 2, 'l', 2]], door: 4, lit: true }), 7],
      [Hs({ w: 4, h: 3, roof: 'gable' }), 3],
      [Hs({ w: 4, h: 4, roof: 'steep', win: [[2, 1]] }), 20],
    ]);
  }
  if (t === 3) {
    return compose([
      [tower(16, [[2, 4, 'l', 2], [2, 10]]), 13],
      [front(7, 9), 6],
      [Hs({ w: 4, h: 4, roof: 'gable', win: [[1, 1]] }), 0],
      [Hs({ w: 5, h: 5, roof: 'steep', win: [[2, 2, 'l', 2]] }), 19],
      [Hs({ w: 3, h: 3, roof: 'gable' }), 24],
    ]);
  }
  if (t === 4) {
    return compose([
      [tower(19, [[2, 4, 'l', 2], [2, 10], [2, 15]]), 15],
      [Hs({ w: 6, h: 6, roof: 'steep', win: [[3, 2, 'l', 2]] }), 22],
      [front(9, 10), 6],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[2, 1]] }), 0],
    ]);
  }
  return compose([
    [o.mage ? tower(20, [[2, 5, 'l', 2], [2, 12]]) : spire(3, 23, [[1, 5, 'l', 2], [1, 12]]), o.mage ? 13 : 14],
    [spire(4, 13, [[1, 3, 'l', 2], [2, 8], [1, 11]]), 8],
    [spire(4, 13, [[2, 3, 'l', 2], [1, 8], [2, 11]]), 19],
    [front(9, 11), 11],
    [Hs({ w: 5, h: 4, roof: 'gable', win: [[1, 1], [3, 2]] }), 0],
    [Hs({ w: 4, h: 5, roof: 'steep', win: [[1, 1], [2, 3]] }), 25],
  ]);
}

// ================================================================================== sun-domes
// Malakim: flat roofs, low domes, sand stone, a tall minaret with a lit gallery.
function sundomes(t) {
  const m = M.sand;
  const block = (w, h, win = []) => house({ m, w, h, roof: 'flat', eave: 0, win });
  const domed = (w, h, dw, dh, win = [], door = null) => stack(dome(dw, dh, m), house({ m, w, h, roof: 'flat', eave: (dw - w) >> 1, win, door, lit: true }));
  const finial = P('.Y.\n.6.');
  // the minaret: a cone, a gallery ledge, a 3 px shaft; windows in shaft coordinates
  const mn = (h, win = []) => stack(finial, peak(3, 2, m), wall(5, 1, { e: m.r, b: m.l, s: m.d }), mark(wall(3, h, m), win));
  if (t === 1) {
    return compose([
      [block(3, 2, [[1, 0]]), 0],
      [block(4, 3, [[2, 1]]), 21],
      [mn(8, [[1, 2]]), 15],
      [domed(6, 4, 8, 4, [[2, 1, 'L']]), 7],
      [block(4, 3), 3],
      [block(3, 2), 19],
    ]);
  }
  if (t === 2) {
    return compose([
      [block(3, 2, [[1, 0]]), 0],
      [block(3, 3, [[1, 1]]), 25],
      [mn(12, [[1, 3, 'l', 2]]), 17],
      [domed(8, 5, 9, 5, [[2, 1, 'L', 2]], 5), 7],
      [block(4, 3, [[1, 1]]), 3],
      [block(5, 3, [[3, 1]]), 21],
    ]);
  }
  if (t === 3) {
    return compose([
      [mn(19, [[1, 3, 'l', 2], [1, 10]]), 16],
      [block(5, 6, [[2, 2]]), 1],
      [domed(10, 6, 11, 6, [[2, 2, 'l', 2], [7, 2, 'L', 2]], 5), 6],
      [block(4, 3), 0],
      [domed(5, 4, 5, 3, [[2, 1]]), 20],
      [block(4, 2), 24],
    ]);
  }
  if (t === 4) {
    return compose([
      [mn(23, [[1, 3, 'l', 2], [1, 10], [1, 16]]), 18],
      [mn(6, [[1, 2]]), 2],
      [domed(12, 7, 13, 7, [[2, 2, 'l', 2], [6, 2, 'L', 2], [9, 2, 'l', 2]], 4), 6],
      [block(5, 4, [[1, 1]]), 0],
      [block(5, 4, [[2, 1, 'l', 2]]), 21],
      [domed(4, 2, 4, 2), 26],
    ]);
  }
  return compose([
    [mn(24, [[1, 3, 'l', 2], [1, 10], [1, 16]]), 20],
    [mn(7, [[1, 2, 'l', 2]]), 3],
    [block(4, 9, [[1, 2], [1, 5]]), 16],
    [domed(14, 8, 15, 8, [[2, 2, 'l', 2], [5, 2, 'l', 2], [8, 2, 'L', 2], [11, 2, 'l', 2]], 7), 6],
    [block(5, 4, [[1, 1], [3, 1]]), 0],
    [domed(5, 3, 5, 3, [[2, 1]]), 22],
    [block(3, 3, [[1, 1]]), 27],
  ]);
}

// ================================================================================== elven
// Ljosalfar (and Svartalfar): great trees with lit hollows and platforms, pods at their feet.
function elven(t, o = {}) {
  const C = o.dark ? { rim: 'P', lit: 'p', body: 'v', sh: 'v' } : { rim: 'T', lit: 't', body: 'k', sh: 'k' };
  const trunkM = o.dark ? { e: '4', b: '2', s: '1' } : { e: 'Q', b: 'u', s: '2' };
  const plank = o.dark ? '4' : 'x';
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
      [pod(4), 0],
      [pod(5), 19],
      [tree(13, 11, 3, 6, [[1, 1, 'L', 2]]), 6],
      [pod(5), 3],
      [pod(4, false), 17],
    ]);
  }
  if (t === 2) {
    return compose([
      [pod(4), 0],
      [pod(4), 25],
      [tree(15, 13, 3, 9, [[1, 1, 'L', 2], [1, 5]], [4]), 8],
      [pod(5), 3],
      [pod(5), 21],
      [pod(4, false), 16],
    ]);
  }
  if (t === 3) {
    return compose([
      [tree(11, 9, 3, 5, [[1, 2]]), 0],
      [tree(11, 9, 3, 7, [[1, 2, 'l', 2]], [4]), 19],
      [tree(18, 16, 4, 12, [[1, 1, 'L', 2], [2, 6], [1, 9]], [4]), 6],
      [pod(5), 1],
      [pod(6), 22],
    ]);
  }
  if (t === 4) {
    return compose([
      [tree(11, 9, 3, 5, [[1, 2]]), 0],
      [tree(11, 9, 3, 7, [[1, 2, 'l', 2]], [4]), 20],
      [tree(19, 17, 5, 14, [[2, 1, 'L', 2], [1, 6], [3, 6], [2, 10, 'l', 2]], [4, 9]), 7],
      [pod(5), 0],
      [pod(6), 23],
      [pod(4), 9],
    ]);
  }
  return compose([
    [tree(11, 9, 3, 5, [[1, 2]]), 0],
    [tree(11, 9, 3, 8, [[1, 2, 'l', 2], [1, 6]], [5]), 20],
    [tree(20, 18, 5, 15, [[2, 1, 'L', 2], [1, 6], [3, 6], [2, 11, 'l', 2], [0, 13], [4, 13]], [4, 9]), 7],
    [pod(5), 1],
    [pod(6), 24],
    [pod(4), 9],
    [pod(4), 19],
  ]);
}

// ================================================================================== dwarven
// Khazad (and Luchuirp): a hold built out of the mountain. Its silhouette is masonry, never a peak:
// flat ledges and upright walls (a stepped gatehouse, crenellated towers and ramparts) in warm grey
// stone with a bronze band, chimneys with ember tops, rock shoulders with ledged tops at its flanks.
// The Luchuirp add a gear wheel on a tower and a golem forge.
function dwarven(t, o = {}) {
  const S = { e: 'H', b: 'G', s: 'D', r: 'K', l: 'y', d: 'y', t: 'K', door: 'B' };
  const ROCK = { rim: 'G', lit: 'D', body: 'B', sh: 'B' };
  const L = o.luchuirp;
  // masonry w x h, lit left column, shaded right one, a bronze band on row `band`
  const block = (w, h, band = -1) => {
    const g = wall(w, h, S);
    if (band >= 0) for (let x = 0; x < w; x++) g.set(x, band, x === 0 ? 'K' : 'y');
    return g;
  };
  // battlements: merlons on the even columns (odd widths keep both ends up)
  const crenel = (w) => {
    const g = new G(w, 2);
    for (let x = 0; x < w; x++) {
      const k = x === 0 ? S.e : x === w - 1 ? S.s : S.b;
      g.set(x, 1, k);
      if (x % 2 === 0) g.set(x, 0, k);
    }
    return g;
  };
  // a square tower with battlements; win [[x, y]] are 1x2 lit windows in the masonry
  const tower = (w, h, win = [], band = 1) => mark(stack(crenel(w), block(w, h, band)), win.map(([x, y]) => [x, y + 2]));
  // a rampart: a battlemented wall line
  const rampart = (w, h, win = []) => tower(w, h, win, -1);
  // the gatehouse: a lit gate under a bronze lintel, crowned by two 2-row steps
  const gatehouse = (w, h) => {
    const body = block(w, h, 1);
    const c = w >> 1;
    for (let x = c - 1; x <= c + 1; x++) body.set(x, h - 4, 'K');
    for (let y = h - 3; y < h; y++) for (let x = c - 1; x <= c + 1; x++) body.set(x, y, x === c && y > h - 3 ? 'L' : 'l');
    return stack(block(w - 8, 2), block(w - 4, 2), body);
  };
  const chimney = (h) => { const g = new G(2, h + 1); g.set(0, 0, 'E'); g.set(1, 0, 'e'); for (let y = 1; y <= h; y++) { g.set(0, y, S.b); g.set(1, y, S.s); } return g; };
  const hut = (w, h, win = []) => house({ w, h, m: S, roof: 'flat', eave: 1, win });
  const gear = P(`
    .H.H.
    HHGGD
    .G.D.
    HGDBB
    .D.B.`);
  const forge = P(`
    HHD
    HED
    lEl`);
  // the mountain the hold is cut into: rock shoulders behind its flanks, plateau-topped (ledges, not
  // a peak), left one rising toward the hold, right one mirrored
  const shoulder = (w, h, right = false) => {
    const g = crag(w, [[0, Math.max(2, h - 6)], [Math.floor(w * 0.4), h - 1], [w - 3, h], [w - 1, h - 2]], ROCK);
    return right ? g.mirror() : g;
  };
  if (t === 1) {
    return compose([
      [hut(3, 2, [[1, 0]]), 0],
      [hut(3, 2), 21],
      [chimney(3), 17, 9],
      [tower(7, 12, [[3, 2], [3, 6]]), 9],
      [rampart(15, 3, [[2, 0], [12, 0]]), 5],
      [gatehouse(9, 5), 8],
    ]);
  }
  if (t === 2) {
    return compose([
      [shoulder(9, 12), 1, 2],
      [shoulder(9, 11, true), 20, 2],
      [hut(3, 2, [[1, 0]]), 0],
      [hut(4, 3, [[1, 1]]), 25],
      [chimney(4), 18, 12],
      [tower(5, 13, L ? [] : [[2, 2]]), 7],
      L ? [gear, 7, 9, { bare: true }] : null,
      [tower(5, 11, [[2, 2]]), 17],
      [rampart(19, 3, [[13, 0]]), 5],
      L ? [forge, 19, 0, { bare: true }] : null,
      [gatehouse(11, 7), 9],
    ]);
  }
  if (t === 3) {
    return compose([
      [shoulder(11, 17), 0, 2],
      [shoulder(11, 15, true), 18, 2],
      [chimney(5), 5, 15],
      [chimney(4), 22, 13],
      [tower(5, 18, [[2, 2], [2, 7]]), 3],
      [tower(5, 15, L ? [[2, 7]] : [[2, 2], [2, 7]]), 21],
      L ? [gear, 21, 15, { bare: true }] : null,
      [tower(7, 16, [[3, 3]], 2), 11, 6],
      [rampart(25, 4, [[2, 1], [21, 1]]), 2],
      L ? [forge, 5, 0, { bare: true }] : null,
      [gatehouse(13, 8), 8],
    ]);
  }
  if (t === 4) {
    return compose([
      [shoulder(12, 21), 0, 3],
      [shoulder(12, 19, true), 20, 3],
      [chimney(6), 6, 19],
      [chimney(5), 25, 17],
      [tower(7, 19, [[3, 2], [1, 8], [5, 8]]), 2],
      [tower(5, 17, L ? [[2, 7]] : [[2, 2], [2, 7]]), 24],
      L ? [gear, 24, 19, { bare: true }] : null,
      [stack(block(5, 2), tower(9, 20, [[4, 3], [2, 9], [6, 9]], 2)), 12, 5],
      [rampart(29, 5, [[2, 1], [9, 1], [25, 1]]), 1],
      L ? [forge, 4, 0, { bare: true }] : null,
      [gatehouse(13, 9), 9],
    ]);
  }
  return compose([
    [shoulder(12, 23), 0, 3],
    [shoulder(12, 21, true), 20, 3],
    [chimney(6), 7, 21],
    [chimney(6), 24, 19],
    [tower(7, 21, [[3, 2], [1, 8], [5, 8]]), 1],
    [tower(7, 19, L ? [[3, 9]] : [[3, 2], [1, 8], [5, 8]]), 24],
    L ? [gear, 25, 21, { bare: true }] : null,
    [stack(block(5, 2), block(9, 2), tower(13, 19, [[3, 3], [9, 3], [6, 9], [3, 12], [9, 12]], 2)), 9, 6],
    [rampart(29, 5, [[2, 1], [8, 1], [20, 1], [26, 1]]), 1],
    L ? [forge, 3, 0, { bare: true }] : null,
    [gatehouse(15, 10), 8],
  ]);
}

// ================================================================================== tribal
// Clan of Embers, Doviello, Hippus (and the barbarians): hide tents with lit door flaps around a
// great tent, a longhouse, palisade wings and a central bonfire, which carries its own FIRE pool
// (radius 8, `: light`), so even a small camp can be found at 1x.
function tribal(t, o = {}) {
  const m = o.barb ? M.pelt : M.hide;
  const wood = o.barb ? { e: '4', b: '3', s: '2' } : { e: 'Y', b: 'u', s: '2' };
  const tent = (w, lit = false, pitch = 2) => {
    const cone = peak(w, pitch, m, { tip: false });
    const g = new G(cone.w, cone.h + 1);
    g.paint(cone, 0, 1);
    const c = (w - 1) >> 1;
    g.set(c - 1, 0, 'u'); g.set(c + 1, 0, 'u'); g.set(c, 1, 'u');
    g.set(c, g.h - 1, lit ? 'l' : m.door); g.set(c, g.h - 2, lit ? 'l' : m.door);
    if (w >= 9) { g.set(c + 1, g.h - 1, m.door); g.set(c, g.h - 3, m.door); }
    if (w >= 15) { g.set(c - 1, g.h - 1, m.door); g.set(c + 1, g.h - 2, m.door); }
    return g;
  };
  const longhouse = (w, h, win = []) => stack(hip(w + 2, 5, m), mark(wall(w, h, wood), win));
  // the bonfire: a glow core over amber, flame and ember, logs at its foot
  const fire = P(`
    ..l..
    .lLl.
    .ElE.
    uEeEu`);
  // a palisade wing: stakes with gaps (the gaps become ink); the barbarians top some with skulls
  const palisade = (w) => {
    const g = new G(w, 5);
    for (let x = 0; x < w; x += 2) {
      const h = 4 + (x % 4 === 0 ? 1 : 0);
      for (let y = 5 - h; y < 5; y++) g.set(x, y, y === 5 - h ? (o.barb ? (x % 4 === 0 ? '7' : '6') : 'Y') : 'u');
    }
    return g;
  };
  const totem = (h) => {
    const head = o.barb ? P('7.7\n272') : P('Y.Y\nYyY');
    const g = new G(3, h + 2);
    g.paint(head, 0, 0);
    for (let y = 2; y < h + 2; y++) { g.set(1, y, 'u'); if (y % 4 === 3) { g.set(0, y, o.barb ? '6' : 'y'); g.set(2, y, o.barb ? '3' : 'u'); } }
    return g;
  };
  const lookout = P(`
    6344
    .lu.
    .uuu
    .u.u
    .uuu
    .u.u
    .uuu
    .u.u
    .u.u
    u..u`);
  const fireAt = (x) => [fire, x, 0, { bare: true }];
  if (t === 1) return compose([[tent(7, true), 0], [palisade(5), 21], [tent(11, true), 4], [tent(9, true), 14], fireAt(12)]);
  if (t === 2) return compose([[tent(7, true), 0], [palisade(5), 3], [palisade(5), 25], [tent(13, true), 9], [tent(9, true), 3], [tent(9, true), 20], fireAt(7)]);
  if (t === 3) {
    return compose([[palisade(5), 0], [palisade(5), 26], [totem(9), 27], [tent(15, true), 3], [longhouse(9, 4, [[2, 2], [6, 2]]), 17], [tent(9, true), 0], fireAt(16)]);
  }
  if (t === 4) {
    return compose([[palisade(5), 0], [palisade(5), 26], [o.barb ? lookout : totem(13), 28, 3], [tent(17, true), 3], [longhouse(11, 5, [[2, 2], [5, 2, 'l', 2], [8, 2]]), 18], [tent(9, true), 0], [tent(7, true), 24], fireAt(16)]);
  }
  return compose([[palisade(5), 0], [palisade(5), 27], [o.barb ? lookout : totem(15), 28, 6], [tent(19, true), 2], [longhouse(11, 6, [[2, 2], [5, 2, 'l', 2], [8, 2]]), 19], [tent(11, true), 0],
    [tent(9, true), 23], [tent(7, true), 13], fireAt(18)]);
}


// ================================================================================== harbour
// Lanun: gabled stilt houses on the shore at the left, a pier, moored ships (a square sail on its
// yard, the owner's pennant, a lit stern lantern) at the right, a lighthouse in the larger towns.
function harbour(t) {
  const m = M.wood;
  const stilts = (w, h) => { const g = new G(w, h); for (let y = 0; y < h; y++) for (let x = 0; x < w; x += 2) g.set(x, y, y === 0 ? 'Q' : 'u'); return g; };
  const shack = (w, h, win = [], roof = 'gable') => stack(house({ m, w, h, roof, eave: 1, win, rm: M.stone }), stilts(w + 2, 2));
  // a moored ship: pennant, mast, a square sail on its yard, bow and stern posts, a lit stern lantern
  const ship = P(`
    ......#......
    ......#%.....
    ......u......
    ...uuuuuuu...
    ...7777665...
    ...7777665...
    ....77665....
    ......u......
    ......u.....L
    u.....u....ul
    QQQQQQQQQQQQu
    .2uuuuuuuu2..
    ..22222222...`);
  // the big ship: two masts, a lit stern window
  const bigship = P(`
    ......#..........
    ......#%.........
    ......u.....#....
    ...uuuuuuu..#%...
    ...7777665..u....
    ...7777665uuuuu..
    ....77665.77665..
    ......u...77665..
    ......u....766...
    ......u.....u...L
    u.....u.....u..ul
    QQQQQQQQQQQQQQQQu
    .2uuuuuuuuuuuluu.
    ..22222222222222.`);
  const pier = (w) => { const g = new G(w, 2); for (let x = 0; x < w; x++) { g.set(x, 0, 'Q'); if (x % 3 === 0) g.set(x, 1, 'u'); } return g; };
  const LS = { e: '5', b: '2', s: '1' };
  const lighthouse = (h) => stack(P(`
    ..6..
    .652.
    .lLl.
    65551`), house({ m: LS, w: 3, h, roof: 'none', win: [[1, 3], [1, 8]] }), wall(5, 1, LS));
  if (t === 1) return compose([[shack(3, 2, [[1, 0]]), 0], [shack(5, 4, [[1, 1, 'L', 2]]), 4], [shack(3, 3, [[1, 1]]), 10], [ship, 13, 0]]);
  if (t === 2) return compose([[shack(3, 3, [[1, 1]]), 0], [shack(4, 3, [[1, 1]]), 3], [shack(5, 6, [[1, 1, 'L', 2], [3, 3]], 'steep'), 8], [pier(6), 15, 0, { bare: true }], [ship, 16, 0]]);
  if (t === 3) {
    return compose([[shack(4, 4, [[2, 1]]), 0], [shack(5, 9, [[1, 1, 'L', 2], [3, 4, 'l', 2]], 'steep'), 6], [shack(3, 3, [[1, 1]]), 12], [pier(6), 12, 0, { bare: true }], [bigship, 15, 0]]);
  }
  if (t === 4) {
    return compose([[shack(4, 4, [[2, 1], [1, 2]]), 0], [lighthouse(17), 8], [shack(5, 7, [[1, 1, 'L', 2], [3, 4, 'l', 2]], 'steep'), 11], [bigship, 15, 0]]);
  }
  return compose([[ship, 18, 6], [shack(4, 4, [[2, 1], [1, 2]]), 0], [lighthouse(21), 8], [shack(5, 8, [[1, 1, 'L', 2], [3, 3], [1, 5, 'l', 2]], 'steep'), 11],
    [bigship, 15, 0]]);
}


// ================================================================================== carnival
// Balseraphs: striped pavilions in the owner's colours (# field, % shade; the shade side darker),
// a big top with a two-stage roof in the larger towns, pennants and lantern strings.
function carnival(t) {
  // stripe a shape: lit half alternates # and %, the shade half % and 2
  const stripe = (g) => {
    const c = (g.w - 1) / 2;
    return g.map((k, x) => (k === '.' ? k : x <= c ? (x % 2 ? '%' : '#') : x % 2 ? '2' : '%'));
  };
  const hem = (w) => { const g = new G(w, 1); for (let x = 0; x < w; x++) if (x % 2 === 0) g.set(x, 0, x <= (w - 1) / 2 ? '#' : '%'); return g; };
  const flag = P('#%\n#.\nu.\nu.');
  // a pavilion: a low striped cone over striped walls, a lit door flap
  const pavilion = (w, h, lit = true) => {
    let body = stripe(wall(w - 2, h, M.wood));
    if (lit) body = mark(body, [[(w - 3) >> 1, h - 2, 'l', 2]]);
    return stack(flag, stripe(peak(w, 1, M.stone, { tip: false, top: 1 })), hem(w), body);
  };
  // the big top: a steep upper cone over a wide lower roof
  const bigtop = (w, h, win = []) => {
    const body = mark(stripe(wall(w - 2, h, M.wood)), win);
    const upper = stripe(peak(7, 2, M.stone, { tip: false }));
    const lower = stripe(peak(w, 1, M.stone, { tip: false, top: 7 }));
    return stack(flag, upper, lower, hem(w), body);
  };
  // a lantern string: a sagging cord between two poles, a lantern every 4 px
  const string = (w, sag = 2) => {
    const g = new G(w, sag + 2);
    for (let x = 0; x < w; x++) {
      const y = Math.round(sag * Math.sin((Math.PI * x) / (w - 1)));
      g.set(x, y, '2');
      if (x % 4 === 2) g.set(x, y + 1, 'l');
    }
    return g;
  };
  if (t === 1) return compose([[pavilion(7, 3), 0], [pavilion(7, 3, false), 19], [pavilion(11, 4), 4], [pavilion(9, 3), 14]]);
  if (t === 2) return compose([[pavilion(7, 3), 0], [pavilion(11, 5), 3], [pavilion(11, 4), 16], [pavilion(7, 3, false), 11], [string(11, 2), 9, 13, { bare: true }]]);
  if (t === 3) {
    return compose([[bigtop(15, 5, [[3, 1], [9, 1, 'L', 2]]), 7], [pavilion(9, 3), 0], [pavilion(9, 4), 20], [string(11, 2), 2, 16, { bare: true }], [string(11, 2), 17, 16, { bare: true }]]);
  }
  if (t === 4) {
    return compose([[bigtop(17, 6, [[3, 2], [7, 1, 'L', 2], [12, 2]]), 7], [pavilion(9, 3), 0], [pavilion(9, 4), 22], [pavilion(7, 3), 17],
      [string(12, 3), 2, 17, { bare: true }], [string(12, 3), 18, 17, { bare: true }]]);
  }
  return compose([[bigtop(19, 7, [[3, 2], [8, 3, 'L', 2], [14, 2]]), 7], [pavilion(9, 3), 0], [pavilion(9, 5), 23], [pavilion(7, 3), 19], [pavilion(7, 3), 4],
    [string(13, 3), 2, 19, { bare: true }], [string(13, 3), 18, 19, { bare: true }]]);
}

// ================================================================================== infernal
// Infernal and Sheaim: black jagged spires with a moonlit west edge, crimson cracks at the base,
// scarlet windows; their ground pool is BLOOD.
function infernal(t) {
  // a thorn: tapering, curving to the right toward its hooked tip
  const thorn = (h, w, lean = 2, win = []) => {
    const g = new G(w + Math.abs(lean) + 2, h);
    const off = lean < 0 ? -lean : 0;
    for (let y = 0; y < h; y++) {
      const f = y / (h - 1);
      const half = Math.max(0, Math.round(((w - 1) / 2) * Math.pow(f, 0.8)));
      const cx = off + Math.round((w - 1) / 2 + lean * Math.pow(1 - f, 2));
      for (let x = cx - half; x <= cx + half; x++) g.set(x, y, x === cx - half ? (y < 2 ? '6' : '5') : x <= cx ? '2' : '1');
    }
    const o2 = mark(g, win.map(([x, y, k, tl]) => [x + off, y, k, tl]));
    const b = o2.bbox();
    return o2.crop(b.x0, 0, b.x1 - b.x0 + 1, h);
  };
  // the base: a jagged black ridge (a zigzag of spurs) with the lower windows in it
  const base = (w, h, win = []) => {
    const pts = [];
    for (let x = 0; x < w; x += 2) pts.push([x, h + ((x * 7) % 5 === 0 ? 2 : (x * 3) % 4 === 1 ? 1 : 0)]);
    pts.push([w - 1, h]);
    const g = crag(w, pts, { rim: '5', lit: '2', body: '2', sh: '1' });
    // crimson cracks glowing in the rock
    for (let x = 3; x < w - 2; x += 6) { g.set(x, g.h - 2, 'C'); g.set(x, g.h - 3, 'c'); g.set(x + 1, g.h - 1, 'c'); }
    return mark(g, win.map(([x, y, k, tl]) => [x, g.h - h + y, k, tl]));
  };
  if (t === 1) {
    return compose([
      [thorn(6, 3, -1), 0],
      [thorn(6, 3, 1), 21],
      [thorn(15, 5, 2, [[2, 8, 'l', 2]]), 9],
      [thorn(9, 3, -1), 5],
      [thorn(8, 3, 1), 16],
      [base(21, 3, [[4, 1], [16, 1]]), 2],
    ]);
  }
  if (t === 2) {
    return compose([
      [thorn(7, 3, -1), 0],
      [thorn(8, 3, 2), 24],
      [thorn(21, 5, 2, [[2, 8, 'l', 2], [2, 14]]), 12],
      [thorn(13, 3, -1, [[1, 8]]), 6],
      [thorn(11, 3, 2), 19],
      [thorn(7, 3, 1), 3],
      [base(26, 3, [[5, 1], [14, 1], [21, 1]]), 2],
    ]);
  }
  if (t === 3) {
    return compose([
      [thorn(28, 5, 3, [[2, 9, 'l', 2], [2, 16], [2, 22]]), 12],
      [thorn(14, 5, -2, [[2, 7, 'l', 2]]), 4],
      [thorn(14, 3, 2, [[1, 9]]), 19],
      [thorn(8, 3, 1), 24],
      [thorn(7, 3, -1), 0],
      [base(25, 4, [[4, 2], [11, 1], [20, 2]]), 2],
    ]);
  }
  if (t === 4) {
    return compose([
      [thorn(31, 7, 3, [[3, 10, 'l', 2], [3, 16], [3, 22, 'L']]), 12],
      [thorn(15, 5, -2, [[2, 7, 'l', 2], [2, 11]]), 4],
      [thorn(17, 5, 2, [[2, 9, 'l', 2], [2, 13]]), 21],
      [thorn(10, 3, -1), 0],
      [thorn(9, 3, 2), 27],
      [base(29, 4, [[5, 2], [13, 1], [17, 2], [21, 2]]), 2],
    ]);
  }
  return compose([
    [thorn(32, 7, 3, [[3, 10, 'l', 2], [3, 16], [3, 21, 'L'], [3, 26]]), 12],
    [thorn(15, 5, -3, [[2, 6, 'l', 2], [2, 10], [2, 13]]), 4],
    [thorn(21, 5, 3, [[2, 9, 'l', 2], [2, 14], [2, 17]]), 21],
    [thorn(11, 3, -1, [[1, 7]]), 0],
    [thorn(11, 3, 2, [[1, 7]]), 28],
    [base(30, 5, [[5, 2], [9, 2], [13, 1], [18, 2], [22, 2]]), 2],
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
      [needle(2, 4, [[0, 1]]), 0],
      [house({ m, w: 3, h: 2, roof: 'dome', eave: 1, win: [[1, 0]] }), 19],
      [needle(3, 9, [[1, 3, 'l', 2]]), 12],
      [orb(3, 4, [[1, 1]]), 6],
      [needle(2, 5, [[0, 2]]), 18],
      [house({ m, w: 4, h: 2, roof: 'dome', eave: 1 }), 3],
    ]);
  }
  if (t === 2) {
    return compose([
      [needle(2, 6, [[0, 2]]), 0],
      [house({ m, w: 4, h: 2, roof: 'dome', eave: 1, win: [[2, 0]] }), 22],
      [needle(3, 14, [[1, 4, 'l', 2], [1, 10]]), 14],
      [orb(3, 6, [[1, 2, 'l', 2]]), 6],
      [needle(2, 8, [[0, 3]]), 21],
      [arch(5), 9, 6, { bare: true }],
      [house({ m, w: 4, h: 2, roof: 'dome', eave: 1 }), 3],
    ]);
  }
  if (t === 3) {
    return compose([
      [needle(3, 21, [[1, 4, 'l', 2], [1, 10], [1, 16]]), 13],
      [orb(3, 10, [[1, 3, 'l', 2]]), 5],
      [needle(2, 12, [[0, 4, 'l', 2]]), 20],
      [orb(3, 4, [[1, 1]]), 0],
      [arch(6), 8, 9, { bare: true }],
      [house({ m, w: 5, h: 3, roof: 'dome', eave: 1, win: [[2, 1]] }), 22],
    ]);
  }
  if (t === 4) {
    return compose([
      [needle(3, 23, [[1, 5, 'l', 2], [1, 11], [1, 17]]), 14],
      [orb(5, 8, [[2, 2, 'l', 2], [2, 6]]), 4],
      [needle(2, 15, [[0, 4, 'l', 2], [0, 10]]), 22],
      [needle(2, 6, [[0, 3]]), 0],
      [arch(5), 10, 11, { bare: true }],
      [arch(5), 18, 9, { bare: true }],
      [house({ m, w: 5, h: 3, roof: 'dome', eave: 1, win: [[2, 1]] }), 24],
    ]);
  }
  return compose([
    [needle(3, 24, [[1, 5, 'l', 2], [1, 11], [1, 17], [1, 21]]), 15],
    [orb(5, 8, [[2, 2, 'l', 2], [2, 6]]), 4],
    [needle(3, 18, [[1, 5, 'l', 2], [1, 11]]), 23],
    [needle(2, 6, [[0, 3]]), 0],
    [needle(2, 9, [[0, 3]]), 29],
    [arch(5), 10, 12, { bare: true }],
    [arch(5), 19, 10, { bare: true }],
    [house({ m, w: 5, h: 3, roof: 'dome', eave: 1, win: [[2, 1]] }), 18],
  ]);
}

// ================================================================================== frost
// Illians: rime-capped stone, cold rime windows, an ice spire.
function frost(t) {
  const m = M.rime;
  const Hs = (a) => house({ m, roof: 'steep', ...a });
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
      [Hs({ w: 3, h: 2, roof: 'gable', win: [[1, 0]] }), 0],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[2, 1]] }), 18],
      [Hs({ w: 3, h: 6, roof: 'spire', eave: 1 }), 15],
      [Hs({ w: 6, h: 5, win: [[1, 2, 'l', 2], [4, 2]] }), 8],
      [Hs({ w: 4, h: 3, roof: 'gable' }), 3],
    ]);
  }
  if (t === 2) {
    return compose([
      [Hs({ w: 3, h: 2, roof: 'gable', win: [[1, 0]] }), 0],
      [Hs({ w: 3, h: 3, roof: 'gable', win: [[1, 1]] }), 24],
      [Hs({ w: 3, h: 11, roof: 'spire', eave: 1, win: [[1, 3, 'l', 2]] }), 16],
      [Hs({ w: 8, h: 6, win: [[2, 2, 'l', 2], [5, 2, 'l', 2]] }), 7],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[2, 1]] }), 3],
      [Hs({ w: 4, h: 4, roof: 'gable' }), 21],
    ]);
  }
  if (t === 3) {
    return compose([
      [crystal(27), 11],
      [Hs({ w: 6, h: 6, win: [[1, 2, 'l', 2], [4, 2, 'l', 2]] }), 2],
      [Hs({ w: 6, h: 6, win: [[1, 1, 'l', 2], [4, 3]] }), 18],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[2, 1]] }), 11],
      [Hs({ w: 3, h: 3, roof: 'gable', win: [[1, 1]] }), 0],
      [Hs({ w: 3, h: 3, roof: 'gable', win: [[1, 1]] }), 24],
    ]);
  }
  if (t === 4) {
    return compose([
      [crystal(30), 12],
      [Hs({ w: 3, h: 4, roof: 'spire', eave: 1, win: [[1, 1, 'l', 2]] }), 4],
      [Hs({ w: 7, h: 7, win: [[1, 2, 'l', 2], [5, 2, 'l', 2], [3, 4]] }), 6],
      [Hs({ w: 6, h: 6, win: [[1, 2, 'l', 2], [4, 3]] }), 20],
      [Hs({ w: 4, h: 3, roof: 'gable', win: [[1, 1]] }), 0],
      [Hs({ w: 4, h: 3, win: [[2, 1]] }), 15],
      [Hs({ w: 3, h: 3, roof: 'gable' }), 27],
    ]);
  }
  return compose([
    [crystal(32), 13],
    [Hs({ w: 3, h: 4, roof: 'spire', eave: 1, win: [[1, 1, 'l', 2]] }), 4],
    [Hs({ w: 3, h: 11, roof: 'spire', eave: 1, win: [[1, 4], [1, 8]] }), 24],
    [Hs({ w: 8, h: 7, win: [[1, 2, 'l', 2], [6, 2, 'l', 2], [3, 4]] }), 7],
    [Hs({ w: 6, h: 6, win: [[1, 2, 'l', 2], [4, 3]] }), 19],
    [Hs({ w: 4, h: 3, roof: 'gable', win: [[1, 1]] }), 0],
    [Hs({ w: 4, h: 3, win: [[2, 1]] }), 15],
    [Hs({ w: 3, h: 3, roof: 'gable', win: [[1, 1]] }), 27],
  ]);
}


// ================================================================================== the list
// table: the halo table that pre-lights the stone around windows (null: none); light: a ground
// light of the sprite itself ([table, radius], placed under its glow core), e.g. the tribal bonfire.

/** Daylight towns (the cheerful revamp): windows are not pre-lit through the halo tables, and the
 * hand-drawn stamps' dark night-look neutrals are lifted two steps (doors, gates, crags, frames) except in
 * the styles meant to be dark or faded (lift: false). */
const DAYLIGHT = true;
const DAY_LIFT = { 1: '3', 2: '4', 3: '5', 4: '6', 5: 'H' };

const STYLES = [
  { name: 'cloister', note: 'gothic stone: steep dark roofs with a moonlit edge, a chapel spire, a nave, a cathedral front from tier IV', make: (t) => cloister(t), table: 'FIRE' },
  { name: 'amurite', note: 'cloister with a mage tower crowned by a lilac crystal; lilac windows', make: (t) => cloister(t, { mage: true, m: { ...M.stone, t: '5' } }), table: 'ARC' },
  { name: 'calabim', note: 'cloister in dark stone', make: (t) => cloister(t, { m: M.dark }), table: 'FIRE', lift: false },
  {
    name: 'sidar', note: 'faded cloister in ash; no lit windows above tier II', make: (t) => cloister(t, { m: M.faded }), table: null, lift: false,
    post: (g, t) => (t >= 3 ? g.remap({ l: '3', L: '3' }) : g),
  },
  { name: 'sundomes', note: 'sand stone, flat roofs, low domes and a minaret', make: sundomes, table: 'FIRE' },
  { name: 'elven', note: 'great trees with lit hollows, platforms and pods', make: (t) => elven(t), table: 'FIRE', post: (g) => g.remap({ k: 't', t: 'T', T: 'o' }) },
  { name: 'svartalfar', note: 'elven trees in dusk and bruise, lilac hollows', make: (t) => elven(t, { dark: true }), table: 'ARC' },
  { name: 'dwarven', note: 'a hold built out of the mountain: a stepped gatehouse, battlemented towers and ramparts in warm stone with a bronze band, ember chimneys, rock shoulders', make: (t) => dwarven(t), table: null, lift: false },
  { name: 'luchuirp', note: 'the dwarven hold with a gear wheel and a golem forge', make: (t) => dwarven(t, { luchuirp: true }), table: null, lift: false },
  { name: 'tribal', note: 'hide tents with lit doors, a longhouse, a palisade and a bonfire (FIRE pool, radius 8)', make: (t) => tribal(t), table: 'FIRE', light: ['FIRE', 8] },
  { name: 'barbarian', note: 'dark pelt tents, a skull-topped palisade, a lookout, a bonfire (FIRE pool, radius 8)', make: (t) => tribal(t, { barb: true }), table: 'EMBER', light: ['FIRE', 8] },
  { name: 'harbour', note: 'stilt houses, moored ships with square sails and stern lanterns, a pier, a lighthouse from tier IV', make: harbour, table: 'FIRE', post: (g) => g.remap({ y: 'Q', x: 'e' }) },
  { name: 'carnival', note: 'striped pavilions in the owner colours (# %), a big top, pennants and lantern strings', make: carnival, table: 'FIRE' },
  { name: 'infernal', note: 'black thorn spires with a moonlit edge, crimson cracks, scarlet windows', make: infernal, table: 'BLOOD', lift: false },
  { name: 'celestial', note: 'pale slender towers, needle spires and domes, vellum light', make: celestial, table: null },
  { name: 'frost', note: 'rime-capped stone and an ice spire, rime windows', make: frost, table: 'COLD' },
];

// ---------------------------------------------------------------------------------- output

const HEADER = `; cities (docs/ART.md 9): city/<style>_<tier>, tiers I (pop 1-3) to V (20+).
; GENERATED by citystyles.mjs (node cities/citystyles.mjs from web/art): edit the styles there.
; Dark masses lit from the upper left (moonlit roof edge, shade at the right, void outline), lit
; windows in amber l (recoloured to the style's light by the renderer), the stone around a window
; pre-lit through the style's halo table. The anchor is ${LIFT} rows below the sprite: the city's
; bottom row lands on tile row ${30 - LIFT}, clear of the plaque. \`: finial\` is the capital's finial
; point; \`: light\` a ground light of the sprite itself (the tribal bonfire).
`;

/** The glow core nearest the bottom centre (the bonfire), as the source of a sprite light. */
function lightSource(g) {
  let best = null;
  for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) {
    if (g.d[y][x] !== 'L') continue;
    const d = Math.abs(x - g.w / 2) + 2 * (g.h - y);
    if (!best || d < best[2]) best = [x, y, d];
  }
  return best;
}

export function build() {
  const out = [];
  for (const st of STYLES) {
    for (let t = 1; t <= 5; t++) {
      let g = st.make(t);
      // daylight: no firelight halo around the windows (night only)
      if (!DAYLIGHT) g = halo(g, TABLES[st.table]);
      if (st.post) g = st.post(g, t);
      if (DAYLIGHT && st.lift !== false) g = g.remap(DAY_LIFT);
      g = trim(g, 0);
      const [fx, fy] = finialOf(g);
      const meta = [`finial ${fx},${fy}`];
      if (st.light) {
        const src = lightSource(g);
        if (src) meta.push(`light ${src[0]},${Math.min(g.h - 1, src[1] + 3)} ${st.light[0]} ${st.light[1]}`);
      }
      if (t === 1 && st.note) meta.push(`note ${st.note}`);
      out.push({ name: `city/${st.name}_${t}`, g, ax: Math.floor(g.w / 2), ay: g.h - 1 + LIFT, meta, lit: windows(g) });
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
    const top = 30 - s.ay; // tile row of the sprite's top
    console.log(`${s.name.padEnd(24)} ${`${s.g.w}x${s.g.h}`.padEnd(6)} top ${String(top).padStart(3)}  lit ${String(s.lit).padStart(2)}  ${String(c.length).padStart(2)} ${c}${c.length > 10 || s.g.w > 34 || s.g.h > 34 ? '   OVER' : ''}`);
  }
}
