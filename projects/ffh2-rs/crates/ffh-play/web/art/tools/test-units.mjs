#!/usr/bin/env node
// test-units.mjs: unit tests of the parsers and the mapping logic on synthetic data (they do not
// depend on the real art, so they stay valid as sprites are added). Run by tools/check.sh.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parsePalette, parseSprites, parseManifest, splitName, TRANSPARENT, SLOT_FIELD, SLOT_ACCENT, NUM_COLORS } from '../spritefmt.js';
import { Mapper, conventionName } from '../mapping.js';
import { splitBundle } from '../library.js';

const KEYS = '012345678dnNgfFktTqmoOuQxyYvpPeElLcCRsSWabhrijwzABDGHIJK';
const PALETTE = `palette
${Array.from({ length: NUM_COLORS }, (_, i) => `${i} ${KEYS[i]} c${i} #${i.toString(16).padStart(6, '0')} f`).join('\n')}
remap
key FOG
${[...KEYS].map((k) => `${k} 0`).join('\n')}
tinctures
Argent 7 6 1 7 6
`;

test('palette parses', () => {
  const p = parsePalette(PALETTE);
  assert.deepEqual(p.errors, []);
  assert.equal(KEYS.length, NUM_COLORS);
  assert.equal(p.colors.length, NUM_COLORS);
  assert.equal(p.keyIndex.get('W'), 39);
  assert.equal(p.remaps.FOG[39], 0);
  assert.equal(p.tinctures.Argent.field, p.keyIndex.get('7'));
});

test('sprites parse with metadata, slots and errors', () => {
  const { keyIndex } = parsePalette(PALETTE);
  const ok = parseSprites(`; c\n= unit/x 3x2 1,1\n: types UNIT_X UNIT_Y\n: light 1,0 FIRE 5\n: flat\n.#@\n70.\n`, 'a.spr', keyIndex);
  assert.deepEqual(ok.errors, []);
  const s = ok.sprites[0];
  assert.equal(s.name, 'unit/x');
  assert.deepEqual(s.meta.types, ['UNIT_X', 'UNIT_Y']);
  assert.deepEqual(s.meta.lights, [{ x: 1, y: 0, table: 'FIRE', radius: 5 }]);
  assert.equal(s.meta.flat, true);
  assert.deepEqual([...s.data], [TRANSPARENT, SLOT_FIELD, SLOT_ACCENT, keyIndex.get('7'), 0, TRANSPARENT]);
  const bad = parseSprites(`= a 3x2 0,0\nZZZ\n..\n= b 2x2 0,0\n..\n`, 'b.spr', keyIndex);
  assert.ok(bad.errors.some((e) => e.includes("'Z' is not a palette key")));
  assert.ok(bad.errors.some((e) => e.includes('has 2 chars, expected 3')));
  assert.ok(bad.errors.some((e) => e.includes('b has 1 rows, expected 2')));
});

test('names split into variants and frames', () => {
  assert.deepEqual(splitName('feature/conifer_b'), { base: 'feature/conifer', variant: 'b', frame: null });
  assert.deepEqual(splitName('feature/flames_f2'), { base: 'feature/flames', variant: null, frame: 2 });
  assert.deepEqual(splitName('font/vigil/a_sc'), { base: 'font/vigil/a_sc', variant: null, frame: null });
  assert.equal(conventionName('unit', 'UNIT_WAR_CHARIOT'), 'unit/war_chariot');
  assert.equal(conventionName('unit', 'EQUIPMENT_ATHAME'), 'unit/equipment_athame');
  assert.equal(conventionName('civ', 'CIVILIZATION_BANNOR'), 'sigil/bannor');
});

test('manifest rules parse', () => {
  const m = parseManifest('set u\nsource a.spr\nmap unit combat=MELEE|ARCHER tier=4 -> unit/_m remap=34 prio=2\nmap unit UNIT_A|UNIT_B -> none\nmap unit domain!=SEA -> unit/_l\nbogus\n', 'm.txt');
  assert.equal(m.set, 'u');
  assert.deepEqual(m.sources, ['a.spr']);
  assert.equal(m.rules.length, 3);
  assert.deepEqual(m.rules[0].conds, [{ attr: 'combat', values: ['MELEE', 'ARCHER'] }, { attr: 'tier', values: ['4'] }]);
  assert.deepEqual(m.rules[0].opts, { remap: '34', prio: '2' });
  assert.deepEqual(m.rules[1].conds, [{ attr: 'type', values: ['UNIT_A', 'UNIT_B'] }]);
  assert.equal(m.rules[2].conds[0].attr, 'domain!');
  assert.equal(m.errors.length, 1);
});

test('mapping resolution order and fallbacks', () => {
  const types = {
    unit: [
      { type: 'UNIT_AXEMAN', combat: 'MELEE', tier: 2, race: ['NONE'], domain: 'LAND', base: 'NONE' },
      { type: 'UNIT_UU', combat: 'MELEE', tier: 2, race: ['NONE'], domain: 'LAND', base: 'UNIT_AXEMAN' },
      { type: 'UNIT_BONES', combat: 'MELEE', tier: 1, race: ['UNDEAD'], domain: 'LAND', base: 'NONE' },
      { type: 'UNIT_CHAMP', combat: 'MELEE', tier: 4, race: ['NONE'], domain: 'LAND', base: 'NONE' },
      { type: 'UNIT_BOAT', combat: 'NONE', tier: 1, race: ['NONE'], domain: 'SEA', base: 'NONE' },
      { type: 'UNIT_ODD', combat: 'NONE', tier: 1, race: ['NONE'], domain: 'LAND', base: 'NONE' },
      { type: 'UNIT_CLAIMED', combat: 'MELEE', tier: 1, race: ['NONE'], domain: 'LAND', base: 'NONE' },
    ],
    bonus: [{ type: 'BONUS_WHEAT', cat: 'food' }, { type: 'BONUS_GOLD', cat: 'luxury' }],
  };
  const rule = (kind, conds, sprite, opts = {}) => ({ kind, conds, sprite, opts });
  const c = (attr, ...values) => ({ attr, values });
  const rules = [
    rule('unit', [c('race', 'UNDEAD')], 'unit/_undead', { prio: '15' }),
    rule('unit', [c('combat', 'MELEE')], 'unit/_melee'),
    rule('unit', [c('combat', 'MELEE'), c('tier', '4')], 'unit/_melee', { remap: '34' }),
    rule('unit', [c('domain!', 'LAND')], 'unit/_ship'),
    rule('unit', [], 'unit/_civilian'),
    rule('bonus', [c('cat', 'food')], 'res/_food', { rim: 'o' }),
    rule('bonus', [c('cat', 'luxury')], 'res/_luxury', { rim: 'y' }),
  ];
  const sprites = new Set(['unit/_undead', 'unit/_melee', 'unit/_ship', 'unit/_civilian', 'unit/axeman', 'unit/claimed_by_meta', 'res/_food', 'res/_luxury', 'res/wheat']);
  const m = new Mapper(types, rules, (n) => n === 'none' || sprites.has(n), new Map([['UNIT_CLAIMED', 'unit/claimed_by_meta']]));
  const r = (k, t) => { const x = m.resolve(k, t); return x && [x.sprite, x.via, x.opts.remap || x.opts.rim || '']; };
  assert.deepEqual(r('unit', 'UNIT_AXEMAN'), ['unit/axeman', 'name', '']);
  assert.deepEqual(r('unit', 'UNIT_UU'), ['unit/axeman', 'base', '']);
  assert.deepEqual(r('unit', 'UNIT_BONES'), ['unit/_undead', 'category', '']);
  assert.deepEqual(r('unit', 'UNIT_CHAMP'), ['unit/_melee', 'category', '34']);
  assert.deepEqual(r('unit', 'UNIT_BOAT'), ['unit/_ship', 'category', '']);
  assert.deepEqual(r('unit', 'UNIT_ODD'), ['unit/_civilian', 'category', '']);
  assert.deepEqual(r('unit', 'UNIT_CLAIMED'), ['unit/claimed_by_meta', 'meta', '']);
  // a bespoke resource keeps the category's rim (inherited option), never its remap
  assert.deepEqual(r('bonus', 'BONUS_WHEAT'), ['res/wheat', 'name', 'o']);
  assert.deepEqual(r('bonus', 'BONUS_GOLD'), ['res/_luxury', 'category', 'y']);
  const cov = m.coverage(['unit', 'bonus']);
  assert.equal(cov.unit.missing.length, 0);
  assert.equal(cov.unit.category, 4);
});

test('bundles split into files', () => {
  assert.deepEqual(splitBundle('@@file a.txt\nx\ny\n@@file b/c.spr\nz'), { 'a.txt': 'x\ny', 'b/c.spr': 'z' });
});
