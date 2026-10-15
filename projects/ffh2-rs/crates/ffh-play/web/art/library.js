// library.js: loads and merges every sprite set (palette, set list, per-set manifests, sources,
// types.json) into one library. Pure ES module: `read` is an async (relativePath) => text function,
// so the same code runs over fetch() in the browser and over the file system in node.
import { parsePalette, parseSetList, parseManifest, parseSprites, splitName } from './spritefmt.js';
import { Mapper } from './mapping.js';

/** Split a bundle (tools/bundle.mjs) into {path: text}. Files start with a line `@@file <path>`. */
export function splitBundle(text) {
  const files = {};
  let cur = null;
  for (const line of text.split('\n')) {
    const m = /^@@file (\S+)\s*$/.exec(line);
    if (m) { cur = m[1]; files[cur] = []; continue; }
    if (cur) files[cur].push(line);
  }
  const out = {};
  for (const [k, v] of Object.entries(files)) out[k] = v.join('\n');
  return out;
}

export async function loadLibrary(read) {
  const errors = [];
  const warnings = [];
  const palette = parsePalette(await read('palette.txt'), 'palette.txt');
  errors.push(...palette.errors);
  const sets = parseSetList(await read('sets.txt'));
  const manifests = await Promise.all(sets.map(async (set) => {
    const file = `${set}/manifest.txt`;
    const man = parseManifest(await read(file), file);
    const texts = await Promise.all(man.sources.map((s) => read(`${set}/${s}`).then((t) => [s, t])));
    return { set, man, texts };
  }));

  const sprites = new Map();
  const rules = [];
  const files = [];
  for (const { set, man, texts } of manifests) {
    errors.push(...man.errors);
    if (man.set && man.set !== set) errors.push(`${set}/manifest.txt: 'set ${man.set}' does not match its directory '${set}'`);
    rules.push(...man.rules.map((r) => ({ ...r, set })));
    for (const [src, text] of texts) {
      const file = `${set}/${src}`;
      const r = parseSprites(text, file, palette.keyIndex);
      errors.push(...r.errors);
      warnings.push(...r.warnings);
      files.push({ set, file, count: r.sprites.length });
      for (const s of r.sprites) {
        s.set = set;
        if (sprites.has(s.name)) {
          const o = sprites.get(s.name);
          errors.push(`${file}:${s.line}: sprite ${s.name} already defined at ${o.file}:${o.line}`);
        } else sprites.set(s.name, s);
      }
    }
  }

  // groups: base name -> {base, variants: [sprite names], frames: [sprite names]}
  const groups = new Map();
  const group = (base) => {
    if (!groups.has(base)) groups.set(base, { base, variants: [], frames: [] });
    return groups.get(base);
  };
  for (const name of [...sprites.keys()].sort()) {
    const { base, variant, frame } = splitName(name);
    if (variant) group(base).variants.push(name);
    else if (frame !== null) group(base).frames.push(name);
    else group(name).single = name;
  }
  for (const g of groups.values()) {
    g.frames.sort((a, b) => splitName(a).frame - splitName(b).frame);
    if (g.single && (g.variants.length || g.frames.length)) {
      errors.push(`sprite ${g.base} exists both alone and with variants/frames`);
    }
  }

  // sprite metadata `: types` -> exact mapping
  const metaTypes = new Map();
  for (const s of sprites.values()) {
    for (const t of s.meta.types) {
      const base = splitName(s.name).base;
      if (metaTypes.has(t) && metaTypes.get(t) !== base) errors.push(`${s.file}:${s.line}: type ${t} already claimed by ${metaTypes.get(t)}`);
      metaTypes.set(t, base);
    }
  }

  let types = {};
  try {
    types = JSON.parse(await read('types.json')).kinds;
  } catch (e) {
    errors.push(`types.json: ${e.message}`);
  }
  const has = (n) => n === 'none' || groups.has(n);
  const mapper = new Mapper(types, rules, has, metaTypes);
  return { palette, sets, sprites, groups, rules, files, mapper, types, errors, warnings };
}
