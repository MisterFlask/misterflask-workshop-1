#!/usr/bin/env node
// sprtool.mjs: authoring aids for sprite text.
//
//   node tools/sprtool.mjs pad     <draft.spr>   pad short rows (and missing rows) with '.'
//   node tools/sprtool.mjs outline <draft.spr>   pad, then add the 1 px void outline (docs/ART.md 4.2)
//                                                 to every sprite whose header line ends with ' +outline'
//   node tools/sprtool.mjs mirror  <draft.spr>   flip every sprite left-right (anchor mirrored)
//   node tools/sprtool.mjs stats   <file.spr>    colours used per sprite (the guide's limits: unit 8,
//                                                 city 10, feature 6, icon 4, outline and slots excluded)
//
// Drafts may leave rows short: write the leading dots that place a shape, skip the trailing ones.
// Output goes to stdout; the result is ordinary sprite text.
import { readFileSync } from 'node:fs';

const [cmd, file] = process.argv.slice(2);
if (!cmd || !file) { console.error('usage: node tools/sprtool.mjs pad|outline|mirror|stats <file.spr>'); process.exit(2); }
const lines = readFileSync(file, 'utf8').split('\n').map((l) => l.replace(/\r$/, ''));

const out = [];
let i = 0;
const sprites = [];
while (i < lines.length) {
  const t = lines[i].trim();
  const m = /^=\s*(\S+)\s+(\d+)x(\d+)\s+(-?\d+),(-?\d+)(\s+\+outline)?\s*$/.exec(t);
  if (!m) { out.push({ text: lines[i] }); i++; continue; }
  const spr = { name: m[1], w: +m[2], h: +m[3], ax: +m[4], ay: +m[5], outline: !!m[6], meta: [], rows: [] };
  i++;
  while (i < lines.length && lines[i].trimStart().startsWith(':')) spr.meta.push(lines[i++].trim());
  while (i < lines.length && spr.rows.length < spr.h) {
    const row = lines[i];
    if (row.trim().startsWith('=') || row.trim().startsWith(';')) break;
    if (row.trim() === '' && spr.rows.length === 0) { i++; continue; }
    if (row.trim() === '' ) break;
    if (row.length > spr.w) { console.error(`${file}:${i + 1}: ${spr.name} row longer than ${spr.w}`); process.exit(1); }
    spr.rows.push(row.padEnd(spr.w, '.'));
    i++;
  }
  while (spr.rows.length < spr.h) spr.rows.push('.'.repeat(spr.w));
  out.push({ spr });
  sprites.push(spr);
}

function outline(spr) {
  const g = spr.rows.map((r) => [...r]);
  const op = (x, y) => x >= 0 && y >= 0 && x < spr.w && y < spr.h && g[y][x] !== '.' && g[y][x] !== '0';
  const add = [];
  for (let y = 0; y < spr.h; y++) for (let x = 0; x < spr.w; x++) {
    if (g[y][x] !== '.') continue;
    if (op(x - 1, y) || op(x + 1, y) || op(x, y - 1) || op(x, y + 1)) add.push([x, y]);
  }
  for (const [x, y] of add) g[y][x] = '0';
  spr.rows = g.map((r) => r.join(''));
}

if (cmd === 'stats') {
  for (const s of sprites) {
    const used = new Set(s.rows.join('').replace(/[.0#%*@]/g, ''));
    console.log(`${s.name.padEnd(32)} ${String(used.size).padStart(2)} colours: ${[...used].sort().join('')}`);
  }
  process.exit(0);
}
for (const s of sprites) {
  if (cmd === 'outline' && s.outline) outline(s);
  if (cmd === 'mirror') { s.rows = s.rows.map((r) => [...r].reverse().join('')); s.ax = s.w - 1 - s.ax; }
}
const text = out.map((o) => (o.spr ? [`= ${o.spr.name} ${o.spr.w}x${o.spr.h} ${o.spr.ax},${o.spr.ay}`, ...o.spr.meta, ...o.spr.rows].join('\n') : o.text)).join('\n');
process.stdout.write(text.endsWith('\n') ? text : text + '\n');
