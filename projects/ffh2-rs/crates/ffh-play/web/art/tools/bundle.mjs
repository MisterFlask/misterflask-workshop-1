#!/usr/bin/env node
// bundle.mjs: concatenate every art source (palette, set list, types.json, manifests, sprites)
// into one text file, for a server that embeds assets (include_str!) or to save requests.
//
//   node tools/bundle.mjs > art.bundle.txt        then: loadArt({ bundle: await (await fetch(url)).text() })
//
// Each file starts with a line `@@file <path>`. The bundle is a build product: do not commit it.
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ART_DIR } from './gen-types.mjs';
import { parseSetList, parseManifest } from '../spritefmt.js';

export function makeBundle(dir = ART_DIR) {
  const files = ['palette.txt', 'sets.txt', 'types.json'];
  for (const set of parseSetList(readFileSync(join(dir, 'sets.txt'), 'utf8'))) {
    const man = `${set}/manifest.txt`;
    files.push(man);
    for (const s of parseManifest(readFileSync(join(dir, man), 'utf8'), man).sources) files.push(`${set}/${s}`);
  }
  const out = [];
  for (const f of files) {
    const text = readFileSync(join(dir, f), 'utf8');
    if (/^@@file /m.test(text)) throw new Error(`bundle: ${f} contains a line starting with '@@file '`);
    out.push(`@@file ${f}`, text.replace(/\n$/, ''));
  }
  return out.join('\n') + '\n';
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.stdout.write(makeBundle());
